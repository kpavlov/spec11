mod api;
mod doctor;
mod frontend;
mod mcp;
mod ops;
mod skill;

use axum::{Router, extract::State, http::{HeaderMap, StatusCode, header}, middleware::{self, Next}, response::{IntoResponse, Response}};
use clap::{Args, Parser, Subcommand};
use spec11_core::{Analysis, StatusOpts};
use std::{path::PathBuf, process::ExitCode, sync::Arc};

#[derive(Clone)]
pub struct AppState {
    pub root: Option<PathBuf>,
    pub dashboard: String,
    /// Port of /mcp, for the .mcp.json entry the install tools write.
    pub mcp_port: u16,
    /// `host:port` values a request may carry (DNS-rebinding guard).
    pub hosts: Arc<Vec<String>>,
    /// Cancelled on Ctrl-C so open `subscriptions/listen` streams end and the graceful shutdown can finish.
    pub shutdown: tokio_util::sync::CancellationToken,
    #[cfg(debug_assertions)]
    pub http: reqwest::Client,
}

#[derive(Parser)]
#[command(name = "spec11", version, about = "Deterministic spec-driven-development evidence: evidence, not truth.")]
struct Cli {
    /// Repo root (default: nearest ancestor with a specs folder).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Without a command: `serve` for the repo in the current directory, and open the dashboard.
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Whole-project status: coverage, problems, where to look next.
    Report {
        /// Lines per problem section.
        #[arg(long, default_value_t = 12)]
        limit: usize,
    },
    /// Per-spec summary, or one line per requirement with a list flag.
    Status(StatusArgs),
    /// One requirement in depth: spec, tests, citations, related code.
    Trace {
        id: String,
        /// Also run `codegraph explore` for the related code.
        #[arg(long)]
        codegraph: bool,
    },
    /// Install the spec11 skill into <repo>/.agents/skills and add the spec11 MCP server to <repo>/.mcp.json.
    Install {
        /// Install the skill into ~/.agents/skills instead (the MCP config is per repo and is left alone).
        #[arg(long)]
        global: bool,
        /// Port of `spec11 serve` that the MCP entry points at.
        #[arg(long, default_value_t = 7411)]
        port: u16,
    },
    /// Remove what `install` added; other skills and MCP servers stay.
    Uninstall {
        #[arg(long)]
        global: bool,
    },
    /// Check the repo setup: specs, tests, sources, git, the codegraph binary and index, skill and MCP entry.
    Doctor {
        /// Also build or refresh the CodeGraph index when the checks call for it (runs `codegraph init` / `sync`). Without this flag doctor only reads.
        #[arg(long)]
        fix: bool,
    },
    /// Serve the dashboard (`/`, `/api/info`) and the MCP server (`/mcp`) on 127.0.0.1.
    Serve {
        #[arg(long, default_value_t = 7411)]
        port: u16,
        /// Serve the UI on this port instead (it also carries /mcp, which the UI uses); `--port` then carries only /mcp.
        #[arg(long)]
        ui_port: Option<u16>,
        /// Don't open the dashboard in the browser.
        #[arg(long)]
        no_open: bool,
    },
    /// Code covered by 2+ requirements (needs the CodeGraph index).
    Overlap {
        path: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Code no requirement covers (needs the CodeGraph index).
    Uncovered {
        path: Option<String>,
        #[arg(long, default_value_t = 5)]
        fanin: usize,
        #[arg(long = "min", default_value_t = 10)]
        min_lines: i64,
    },
}

#[derive(Args)]
struct StatusArgs {
    #[arg(long)]
    spec: Option<String>,
    #[arg(long)]
    prefix: Option<String>,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    missing: bool,
    #[arg(long, num_args = 0..=1, default_missing_value = "HEAD", value_name = "REF")]
    changed: Option<String>,
    #[arg(long)]
    built: bool,
    #[arg(long)]
    done: bool,
    #[arg(long)]
    unproven: bool,
    #[arg(long)]
    issues: bool,
    #[arg(long, default_value = "low", value_parser = ["low", "medium", "high"])]
    effort: String,
    #[arg(long, num_args = 0..=1, default_missing_value = "", value_name = "PATH")]
    overlap: Option<String>,
    #[arg(long, num_args = 0..=1, default_missing_value = "", value_name = "PATH")]
    uncovered: Option<String>,
}

fn out<T: serde::Serialize>(json: bool, v: &T, text: impl FnOnce() -> String) {
    if json { println!("{}", serde_json::to_string_pretty(v).unwrap()) } else { print!("{}", text()) }
}

// (API-02)
/// 403 unless the Host (and Origin, when sent) is one this server bound.
async fn guard(State(s): State<AppState>, headers: HeaderMap, req: axum::extract::Request, next: Next) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
    let origin_ok = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()).is_none_or(|o| o.strip_prefix("http://").is_some_and(|h| s.hosts.iter().any(|x| x == h)));
    if s.hosts.iter().any(|h| h == host) && origin_ok { next.run(req).await } else { StatusCode::FORBIDDEN.into_response() }
}

/// (main router, optional UI router). Without `ui_port` the main router serves everything; with it, `/mcp` only, and the UI router
/// serves the UI and `/mcp` too, since the dashboard talks to the server over MCP.
fn routers(state: AppState) -> (Router, Option<Router>) {
    let split = state.hosts.len() > 2;
    let ui = || Router::new().nest("/api", api::router()).route("/", axum::routing::get(frontend::handler)).route("/{*path}", axum::routing::get(frontend::handler));
    let mcp = || Router::new().nest_service("/mcp", mcp::service(state.clone(), &state.hosts));
    let guarded = |r: Router<AppState>| -> Router {
        r.layer(middleware::from_fn_with_state(state.clone(), guard)).layer(tower_http::compression::CompressionLayer::new()).with_state(state.clone())
    };
    if split { (guarded(mcp()), Some(guarded(ui().merge(mcp())))) } else { (guarded(ui().merge(mcp())), None) }
}

fn app_state(root: Option<PathBuf>, ports: &[u16]) -> AppState {
    let hosts = ports.iter().flat_map(|p| [format!("127.0.0.1:{p}"), format!("localhost:{p}")]).collect();
    AppState { root, dashboard: format!("http://127.0.0.1:{}/", ports.last().unwrap()), mcp_port: ports[0], hosts: Arc::new(hosts), shutdown: Default::default(), #[cfg(debug_assertions)] http: reqwest::Client::new() }
}

/// Best effort: a failure to launch a browser is not an error.
fn open_browser(url: &str) {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") { ("open", vec![url]) } else if cfg!(windows) { ("cmd", vec!["/c", "start", "", url]) } else { ("xdg-open", vec![url]) };
    let _ = std::process::Command::new(cmd).args(args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
}

fn serve(root: Option<PathBuf>, port: u16, ui_port: Option<u16>, no_open: bool) -> Result<u8, String> {
    // the workspace is fixed once, here: the repo that holds the current directory (or --root)
    let root = spec11_core::scan::Project::open(root.as_deref())?.root;
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let bind = |p: u16| async move { tokio::net::TcpListener::bind(("127.0.0.1", p)).await.map_err(|e| format!("127.0.0.1:{p}: {e}")) };
        let ports: Vec<u16> = std::iter::once(port).chain(ui_port).collect();
        let state = app_state(Some(root.clone()), &ports);
        eprintln!("spec11: {}\n  dashboard {}\n  mcp       http://127.0.0.1:{port}/mcp", root.display(), state.dashboard);
        match skill::status(Some(&root), port) {
            Ok(st) => skill::suggestions(&st).iter().for_each(|t| eprintln!("  setup     {t}  (or use Setup in the dashboard)")),
            Err(e) => eprintln!("  setup     could not check the install: {e}"),
        }
        let dashboard = state.dashboard.clone();
        let stop = state.shutdown.clone();
        let (main, ui) = routers(state);
        let run = |l, app: Router| {
            let stop = stop.clone();
            async move { axum::serve(l, app).with_graceful_shutdown(async move { let _ = tokio::signal::ctrl_c().await; stop.cancel(); }).await.map_err(|e| e.to_string()) }
        };
        let main_l = bind(port).await?;
        let ui_l = match ui_port { Some(up) => Some(bind(up).await?), None => None };
        if !no_open {
            open_browser(&dashboard); // after the bind, so the page is there when the browser asks
        }
        match (ui, ui_l) {
            (Some(ui), Some(ul)) => {
                let (a, b) = tokio::join!(run(main_l, main), run(ul, ui));
                a?;
                b?;
            }
            _ => run(main_l, main).await?,
        }
        Ok(0)
    })
}

/// `port` = install, `None` = uninstall.
fn install(root: Option<&std::path::Path>, global: bool, port: Option<u16>) -> Result<u8, String> {
    skill::apply(root, global, port)?.iter().for_each(|l| println!("{l}"));
    if port.is_some() && !global {
        println!("next   spec11   # starts the MCP server and dashboard the skill talks to");
    }
    Ok(0)
}

fn run(cli: Cli) -> Result<u8, String> {
    let cmd = cli.cmd.unwrap_or(Cmd::Serve { port: 7411, ui_port: None, no_open: false });
    match cmd {
        Cmd::Serve { port, ui_port, no_open } => return serve(cli.root, port, ui_port, no_open),
        Cmd::Doctor { fix } => {
            let mut checks = doctor::run(cli.root.as_deref(), 7411);
            if fix && let Some(action) = doctor::suggested(&checks) {
                eprint!("{}", doctor::index(cli.root.as_deref(), action)?);
                checks = doctor::run(cli.root.as_deref(), 7411);
            }
            out(cli.json, &checks, || doctor::text(&checks));
            return Ok(u8::from(checks.iter().any(|k| k.level == "fail")));
        }
        Cmd::Install { global, port } => return install(cli.root.as_deref(), global, Some(port)),
        Cmd::Uninstall { global } => return install(cli.root.as_deref(), global, None),
        _ => {}
    }
    let a = Analysis::load(cli.root.as_deref())?;
    let no_index = || a.graph_note.clone().unwrap_or_else(|| "spec11: no .codegraph index here; the user decides about indexing".into());
    match cmd {
        Cmd::Report { limit } => {
            let r = a.report();
            out(cli.json, &r, || r.text(&a, limit));
        }
        Cmd::Status(s) => {
            if let Some(p) = &s.overlap {
                let rows = a.overlap(p, false).map_err(|e| e.to_string())?.ok_or_else(no_index)?;
                out(cli.json, &rows, || a.overlap_text(&rows, 25));
            } else if let Some(p) = &s.uncovered {
                let u = a.uncovered(p, 5, 10).map_err(|e| e.to_string())?.ok_or_else(no_index)?;
                out(cli.json, &u, || u.text());
            } else {
                let o = StatusOpts { spec: s.spec, prefix: s.prefix, all: s.all, missing: s.missing, changed: s.changed, built: s.built, done: s.done, unproven: s.unproven, issues: s.issues, effort: s.effort };
                let st = a.status(&o)?;
                out(cli.json, &st, || st.text(&o));
            }
        }
        Cmd::Trace { id, codegraph } => match a.trace(&id) {
            Some(t) => {
                out(cli.json, &t, || t.text());
                if codegraph && !t.query.is_empty() {
                    let _ = std::process::Command::new("codegraph").args(["explore", &t.query]).current_dir(&a.p.root).status();
                }
            }
            None => {
                println!("{}: not in specs", id.to_uppercase());
                return Ok(1);
            }
        },
        Cmd::Serve { .. } | Cmd::Install { .. } | Cmd::Uninstall { .. } | Cmd::Doctor { .. } => unreachable!(),
        Cmd::Overlap { path, all } => {
            let rows = a.overlap(path.as_deref().unwrap_or(""), all).map_err(|e| e.to_string())?.ok_or_else(no_index)?;
            out(cli.json, &rows, || a.overlap_text(&rows, 25));
        }
        Cmd::Uncovered { path, fanin, min_lines } => {
            let u = a.uncovered(path.as_deref().unwrap_or(""), fanin, min_lines).map_err(|e| e.to_string())?.ok_or_else(no_index)?;
            out(cli.json, &u, || u.text());
        }
    }
    Ok(0)
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(c) => ExitCode::from(c),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use serde_json::json;
    use tower::ServiceExt;

    fn repo() -> PathBuf {
        let d = std::env::temp_dir().join(format!("spec11-srv-{}", std::process::id()));
        std::fs::create_dir_all(d.join("specs")).unwrap();
        std::fs::write(d.join("specs/01-a.md"), "- **AAA-01** MUST — one.\n- **AAA-02** MUST — two.\n").unwrap();
        std::fs::create_dir_all(d.join("RunSolo/RunSolo")).unwrap();
        std::fs::write(d.join("RunSolo/RunSolo/A.swift"), "struct A {}\n").unwrap();
        d
    }

    async fn send(app: &Router, req: Request<Body>) -> (StatusCode, String) {
        let r = app.clone().oneshot(req).await.unwrap();
        let st = r.status();
        (st, String::from_utf8_lossy(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).into_owned())
    }

    fn post(uri: &str, host: &str, body: &str) -> Request<Body> {
        Request::post(uri).header("host", host).header("content-type", "application/json").header("accept", "application/json, text/event-stream").body(Body::from(body.to_string())).unwrap()
    }

    /// A tool call over `/mcp`: what the CLI runs, and what the dashboard sends.
    fn tool(name: &str, args: &str, host: &str) -> Request<Body> {
        post("/mcp", host, &format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#))
    }

    #[doc = "[API-07]"]
    #[tokio::test]
    async fn mcp_2026_07_28_needs_no_handshake() {
        let (app, _) = routers(app_state(Some(repo()), &[7411]));
        let read = |uri: &str| {
            let meta = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"t","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}"#;
            let body = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{{"uri":"{uri}",{meta}}}}}"#);
            let mut r = post("/mcp", "127.0.0.1:7411", &body);
            r.headers_mut().insert("mcp-protocol-version", "2026-07-28".parse().unwrap());
            r.headers_mut().insert("mcp-method", "resources/read".parse().unwrap());
            r.headers_mut().insert("mcp-name", uri.parse().unwrap());
            r
        };
        let (st, body) = send(&app, read("spec11://tests")).await;
        assert!(st == StatusCode::OK && body.contains("total"), "{st} {body}");
        let mut d = post("/mcp", "127.0.0.1:7411", r#"{"jsonrpc":"2.0","id":2,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"t","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}}}"#);
        d.headers_mut().insert("mcp-protocol-version", "2026-07-28".parse().unwrap());
        d.headers_mut().insert("mcp-method", "server/discover".parse().unwrap());
        let (st, body) = send(&app, d).await;
        assert!(st == StatusCode::OK && body.contains("2026-07-28"), "{st} {body}");
    }

    #[doc = "[API-08]"]
    #[tokio::test]
    async fn subscription_notifies_when_the_repo_changes() {
        use http_body_util::BodyExt;
        use std::time::Duration;
        let d = std::env::temp_dir().join(format!("spec11-sub-{}", std::process::id()));
        std::fs::create_dir_all(d.join("specs")).unwrap();
        std::fs::write(d.join("specs/01-a.md"), "- **AAA-01** MUST — one.\n").unwrap();
        let state = app_state(Some(d.clone()), &[7411]);
        let stop = state.shutdown.clone();
        let (app, _) = routers(state);
        let meta = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"t","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}"#;
        let body = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"subscriptions/listen","params":{{{meta},"notifications":{{"resourceSubscriptions":["spec11://report","spec11://nope"]}}}}}}"#);
        let mut req = post("/mcp", "127.0.0.1:7411", &body);
        req.headers_mut().insert("mcp-protocol-version", "2026-07-28".parse().unwrap());
        req.headers_mut().insert("mcp-method", "subscriptions/listen".parse().unwrap());
        let mut stream = app.clone().oneshot(req).await.unwrap().into_body();
        // reads frames until `want` shows up in what has arrived so far
        let mut seen = String::new();
        let mut until = async |want: &str, seen: &mut String| {
            tokio::time::timeout(Duration::from_secs(8), async {
                while !seen.contains(want) {
                    match stream.frame().await {
                        Some(Ok(f)) => seen.push_str(&String::from_utf8_lossy(f.data_ref().map_or(&[][..], |b| &b[..]))),
                        _ => break,
                    }
                }
            })
            .await
            .unwrap();
            assert!(seen.contains(want), "{want} not in {seen}");
        };
        until("acknowledged", &mut seen).await;
        assert!(seen.contains("spec11://report") && !seen.contains("spec11://nope"), "{seen}");
        seen.clear();
        tokio::time::sleep(Duration::from_millis(500)).await; // the listener takes its first stamp after the ack
        std::fs::write(d.join("specs/01-a.md"), "- **AAA-01** MUST — one.\n- **AAA-02** MUST — two.\n").unwrap();
        until("notifications/resources/updated", &mut seen).await;
        assert!(seen.contains("spec11://report"), "{seen}");
        seen.clear();
        stop.cancel(); // shutdown ends the stream with the graceful result
        until("complete", &mut seen).await;
        // (API-08) the view is readable too, beside its tool
        let (_, body) = send(&app, post("/mcp", "127.0.0.1:7411", r#"{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"spec11://report"}}"#)).await;
        assert!(body.contains("text/plain") && body.contains("SDD STATUS"), "{body}");
    }



    #[doc = "[API-09]"]
    #[test]
    fn tools_declare_an_output_schema_whose_required_keys_their_results_have() {
        let d = repo();
        let args = |n: &str| match n {
            "spec_trace" => json!({"id": "AAA-01"}),
            "spec_source" => json!({"path": "RunSolo/RunSolo/A.swift"}),
            _ => json!({}),
        };
        let mut checked = 0;
        for t in ops::tools() {
            let schema = ops::output_schema(t.name).unwrap_or_else(|| panic!("{} has no output schema", t.name));
            assert_eq!(schema["type"], "object", "{}", t.name);
            // the writing tools are not run here, and overlap/uncovered need an index this fixture lacks
            if matches!(t.name, "spec_install" | "spec_uninstall" | "spec_codegraph" | "spec_overlap" | "spec_uncovered") {
                continue;
            }
            let (_, data) = ops::call(Some(&d), t.name, &args(t.name), "http://x/", 7411).unwrap();
            for k in schema["required"].as_array().unwrap() {
                assert!(data.get(k.as_str().unwrap()).is_some(), "{}: `{k}` missing from {data}", t.name);
            }
            checked += 1;
        }
        assert!(checked >= 6, "{checked}");
    }

    #[doc = "[API-10]"]
    #[tokio::test]
    async fn json_resources_name_the_schema_that_types_them() {
        let (app, _) = routers(app_state(Some(repo()), &[7411]));
        let rpc = |id: u32, method: &str, params: &str| post("/mcp", "127.0.0.1:7411", &format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#));
        let init = r#"{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"1"}}"#;
        send(&app, rpc(1, "initialize", init)).await;
        let (_, list) = send(&app, rpc(2, "resources/list", "{}")).await;
        for n in ["report", "status", "uncovered", "overlap", "modules", "tests"] {
            assert!(list.contains(&format!("spec11://schemas/{n}")), "{n}: {list}");
        }
        assert!(list.contains(r#""schema":"spec11://schemas/report""#) && list.contains("spec11://report.json"), "{list}");
        let (_, body) = send(&app, rpc(3, "resources/read", r#"{"uri":"spec11://report.json"}"#)).await;
        assert!(body.contains("application/json") && body.contains("requirements"), "{body}");
        let (_, body) = send(&app, rpc(4, "resources/read", r#"{"uri":"spec11://schemas/modules"}"#)).await;
        assert!(body.contains("application/schema+json") && body.contains("properties"), "{body}");
        let (_, tools) = send(&app, rpc(5, "tools/list", "{}")).await;
        assert!(tools.contains("outputSchema"), "{tools}");
    }

    #[doc = "[API-01, API-02, API-03]"]
    #[tokio::test]
    async fn api_and_guard() {
        let (app, ui) = routers(app_state(Some(repo()), &[7411]));
        assert!(ui.is_none());
        let (st, body) = send(&app, tool("spec_report", "{}", "127.0.0.1:7411")).await;
        assert_eq!(st, StatusCode::OK, "{body}");
        assert!(body.contains("SDD STATUS") && body.contains("\"none\":2"), "{body}");
        let (st, body) = send(&app, tool("spec_trace", r#"{"id":"aaa-01"}"#, "localhost:7411")).await;
        assert!(st == StatusCode::OK && body.contains("AAA-01"), "{body}");
        let (_, body) = send(&app, tool("spec_file", "{}", "127.0.0.1:7411")).await;
        assert!(body.contains("01-a.md"), "{body}");
        let (st, body) = send(&app, tool("spec_file", r#"{"name":"specs/01-a.md"}"#, "127.0.0.1:7411")).await;
        assert!(st == StatusCode::OK && body.contains("AAA-02"), "{body}");
        for bad in ["../Cargo.toml", "/etc/passwd", "01-a"] {
            let (_, body) = send(&app, tool("spec_file", &format!(r#"{{"name":"{bad}"}}"#), "127.0.0.1:7411")).await;
            assert!(body.contains(r#""isError":true"#), "{bad}: {body}");
        }
        let (st, body) = send(&app, tool("spec_source", r#"{"path":"RunSolo/RunSolo/A.swift"}"#, "127.0.0.1:7411")).await;
        assert!(st == StatusCode::OK && body.contains("struct A"), "{body}");
        for bad in ["../Cargo.toml", "/etc/passwd", "specs/01-a.md", "RunSolo/RunSolo/../../specs/01-a.md"] {
            let (_, body) = send(&app, tool("spec_source", &format!(r#"{{"path":"{bad}"}}"#), "127.0.0.1:7411")).await;
            assert!(body.contains(r#""isError":true"#), "{bad}: {body}");
        }
        assert!(send(&app, tool("nope", "{}", "127.0.0.1:7411")).await.1.contains(r#""isError":true"#));
        assert_eq!(send(&app, tool("spec_report", "{}", "evil.test")).await.0, StatusCode::FORBIDDEN);
        let mut r = tool("spec_report", "{}", "127.0.0.1:7411");
        r.headers_mut().insert("origin", "http://evil.test".parse().unwrap());
        assert_eq!(send(&app, r).await.0, StatusCode::FORBIDDEN);
    }

    #[doc = "[SETUP-01]"]
    #[tokio::test]
    async fn install_tools_write_the_repo_and_report_status() {
        let repo = repo();
        let (app, _) = routers(app_state(Some(repo.clone()), &[7411]));
        let call = |n: &str, a: &str| tool(n, a, "127.0.0.1:7411");
        let (_, body) = send(&app, call("spec_install_status", "{}")).await;
        assert!(body.contains(r#""skill":"missing""#) && body.contains("spec11 install"), "{body}");
        std::fs::write(repo.join(".mcp.json"), r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();
        let (st, body) = send(&app, call("spec_install", "{}")).await;
        assert_eq!(st, StatusCode::OK, "{body}");
        assert!(body.contains(r#""skill":"current""#) && body.contains(r#""mcp":"ok""#), "{body}");
        let mcp = std::fs::read_to_string(repo.join(".mcp.json")).unwrap();
        assert!(mcp.contains("\"other\"") && mcp.contains("127.0.0.1:7411/mcp"), "{mcp}");
        assert!(repo.join(".agents/skills/spec11/SKILL.md").exists());
        let (_, body) = send(&app, call("spec_uninstall", "{}")).await;
        assert!(body.contains(r#""skill":"missing""#), "{body}");
        assert!(!repo.join(".agents/skills/spec11/SKILL.md").exists());
        assert!(std::fs::read_to_string(repo.join(".mcp.json")).unwrap().contains("\"other\""));
    }

    #[doc = "[API-01, API-04, API-07]"]
    #[tokio::test]
    async fn mcp_tools_and_ui_resource() {
        let (app, _) = routers(app_state(Some(repo()), &[7411]));
        let rpc = |id: u32, method: &str, params: &str| post("/mcp", "127.0.0.1:7411", &format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#));
        let init = r#"{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"1"}}"#;
        let (st, body) = send(&app, rpc(1, "initialize", init)).await;
        assert_eq!(st, StatusCode::OK, "{body}");
        let (_, body) = send(&app, rpc(2, "tools/list", "{}")).await;
        assert!(body.contains("spec_report") && body.contains("ui://spec11/dashboard"), "{body}");
        let (_, body) = send(&app, rpc(3, "tools/call", r#"{"name":"spec_status","arguments":{"missing":true}}"#)).await;
        assert!(body.contains("AAA-02"), "{body}");
        let (_, body) = send(&app, rpc(4, "resources/read", r#"{"uri":"ui://spec11/dashboard"}"#)).await;
        assert!(body.contains("text/html;profile=mcp-app"), "{body}");
        // (API-07)
        let (_, body) = send(&app, rpc(6, "resources/list", "{}")).await;
        assert!(body.contains("spec11://modules") && body.contains("spec11://tests"), "{body}");
        let (_, body) = send(&app, rpc(7, "resources/read", r#"{"uri":"spec11://modules"}"#)).await;
        assert!(body.contains("application/json") && body.contains("modules"), "{body}");
        let (_, body) = send(&app, rpc(8, "resources/read", r#"{"uri":"spec11://tests"}"#)).await;
        assert!(body.contains("not run") && body.contains("total"), "{body}");
        let (_, body) = send(&app, rpc(9, "resources/read", r#"{"uri":"spec11://nope"}"#)).await;
        assert!(body.contains("unknown resource"), "{body}");
        let mut bad = rpc(5, "tools/list", "{}");
        bad.headers_mut().insert("host", "evil.test".parse().unwrap());
        assert_eq!(send(&app, bad).await.0, StatusCode::FORBIDDEN);
    }

    #[doc = "[API-05]"]
    #[tokio::test]
    async fn split_ports_separate_mcp_from_ui() {
        let (mcp_app, ui) = routers(app_state(Some(repo()), &[7411, 7412]));
        let ui = ui.unwrap();
        // the UI port serves the info route and MCP (the dashboard talks MCP); the main port serves MCP only
        let get = |host: &str| Request::get("/api/info").header("host", host).body(Body::empty()).unwrap();
        assert_eq!(send(&ui, get("127.0.0.1:7412")).await.0, StatusCode::OK);
        let (st, body) = send(&ui, tool("spec_report", "{}", "127.0.0.1:7412")).await;
        assert!(st == StatusCode::OK && body.contains("SDD STATUS"), "{body}");
        assert_eq!(send(&mcp_app, get("127.0.0.1:7411")).await.0, StatusCode::NOT_FOUND);
        assert!(send(&mcp_app, tool("spec_report", "{}", "127.0.0.1:7411")).await.1.contains("SDD STATUS"));
    }

    /// Release builds only: the embedded UI, SPA fallback, 404 for a missing asset.
    #[doc = "[API-06]"]
    #[cfg(not(debug_assertions))]
    #[tokio::test]
    async fn embedded_ui() {
        let (app, _) = routers(app_state(Some(repo()), &[7411]));
        let get = |uri: &str| Request::get(uri).header("host", "127.0.0.1:7411").body(Body::empty()).unwrap();
        let (st, body) = send(&app, get("/")).await;
        assert!(st == StatusCode::OK && body.contains("<div id=\"app\">"), "{st} {body:.200}");
        assert_eq!(send(&app, get("/requirements/AAA-01")).await.1, body); // deep link -> index.html
        assert_eq!(send(&app, get("/assets/nope.js")).await.0, StatusCode::NOT_FOUND);
    }
}
