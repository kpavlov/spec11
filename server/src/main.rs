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
    #[cfg(debug_assertions)]
    pub http: reqwest::Client,
}

#[derive(Parser)]
#[command(name = "spec11", about = "Deterministic spec-driven-development evidence: evidence, not truth.")]
struct Cli {
    /// Repo root (default: nearest ancestor with a specs/ folder).
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
    Doctor,
    /// Serve the dashboard (`/`, `/api`) and the MCP server (`/mcp`) on 127.0.0.1.
    Serve {
        #[arg(long, default_value_t = 7411)]
        port: u16,
        /// Serve the UI and /api on this port instead; `--port` then carries only /mcp.
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

/// 403 unless the Host (and Origin, when sent) is one this server bound.
async fn guard(State(s): State<AppState>, headers: HeaderMap, req: axum::extract::Request, next: Next) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
    let origin_ok = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()).is_none_or(|o| o.strip_prefix("http://").is_some_and(|h| s.hosts.iter().any(|x| x == h)));
    if s.hosts.iter().any(|h| h == host) && origin_ok { next.run(req).await } else { StatusCode::FORBIDDEN.into_response() }
}

/// (main router, optional UI router). Without `ui_port` the main router serves everything; with it, `/mcp` only.
fn routers(state: AppState) -> (Router, Option<Router>) {
    let split = state.hosts.len() > 2;
    let ui = || Router::new().nest("/api", api::router()).route("/", axum::routing::get(frontend::handler)).route("/{*path}", axum::routing::get(frontend::handler));
    let mcp = || Router::new().nest_service("/mcp", mcp::service(state.clone(), &state.hosts));
    let guarded = |r: Router<AppState>| -> Router {
        r.layer(middleware::from_fn_with_state(state.clone(), guard)).layer(tower_http::compression::CompressionLayer::new()).with_state(state.clone())
    };
    if split { (guarded(mcp()), Some(guarded(ui()))) } else { (guarded(ui().merge(mcp())), None) }
}

fn app_state(root: Option<PathBuf>, ports: &[u16]) -> AppState {
    let hosts = ports.iter().flat_map(|p| [format!("127.0.0.1:{p}"), format!("localhost:{p}")]).collect();
    AppState { root, dashboard: format!("http://127.0.0.1:{}/", ports.last().unwrap()), mcp_port: ports[0], hosts: Arc::new(hosts), #[cfg(debug_assertions)] http: reqwest::Client::new() }
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
        let (main, ui) = routers(state);
        let run = |l, app: Router| async move { axum::serve(l, app).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await.map_err(|e| e.to_string()) };
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
        Cmd::Doctor => {
            let checks = doctor::run(cli.root.as_deref(), 7411);
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
        Cmd::Serve { .. } | Cmd::Install { .. } | Cmd::Uninstall { .. } | Cmd::Doctor => unreachable!(),
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
    use tower::ServiceExt;

    fn repo() -> PathBuf {
        let d = std::env::temp_dir().join(format!("spec11-srv-{}", std::process::id()));
        std::fs::create_dir_all(d.join("specs")).unwrap();
        std::fs::write(d.join("specs/01-a.md"), "- **AAA-01** MUST — one.\n- **AAA-02** MUST — two.\n").unwrap();
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

    #[tokio::test]
    async fn api_and_guard() {
        let (app, ui) = routers(app_state(Some(repo()), &[7411]));
        assert!(ui.is_none());
        let (st, body) = send(&app, post("/api/call/spec_report", "127.0.0.1:7411", "{}")).await;
        assert_eq!(st, StatusCode::OK, "{body}");
        assert!(body.contains("SDD STATUS") && body.contains("\"none\":2"), "{body}");
        let (st, body) = send(&app, post("/api/call/spec_trace", "localhost:7411", r#"{"id":"aaa-01"}"#)).await;
        assert!(st == StatusCode::OK && body.contains("AAA-01"), "{body}");
        let (_, body) = send(&app, post("/api/call/spec_file", "127.0.0.1:7411", "{}")).await;
        assert!(body.contains("01-a.md"), "{body}");
        let (st, body) = send(&app, post("/api/call/spec_file", "127.0.0.1:7411", r#"{"name":"01-a.md"}"#)).await;
        assert!(st == StatusCode::OK && body.contains("AAA-02"), "{body}");
        for bad in ["../Cargo.toml", "/etc/passwd", "01-a"] {
            let (st, _) = send(&app, post("/api/call/spec_file", "127.0.0.1:7411", &format!(r#"{{"name":"{bad}"}}"#))).await;
            assert_eq!(st, StatusCode::BAD_REQUEST, "{bad}");
        }
        assert_eq!(send(&app, post("/api/call/nope", "127.0.0.1:7411", "{}")).await.0, StatusCode::BAD_REQUEST);
        assert_eq!(send(&app, post("/api/call/spec_report", "evil.test", "{}")).await.0, StatusCode::FORBIDDEN);
        let mut r = post("/api/call/spec_report", "127.0.0.1:7411", "{}");
        r.headers_mut().insert("origin", "http://evil.test".parse().unwrap());
        assert_eq!(send(&app, r).await.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn install_tools_write_the_repo_and_report_status() {
        let repo = repo();
        let (app, _) = routers(app_state(Some(repo.clone()), &[7411]));
        let call = |n: &str, a: &str| post(&format!("/api/call/{n}"), "127.0.0.1:7411", a);
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
        let mut bad = rpc(5, "tools/list", "{}");
        bad.headers_mut().insert("host", "evil.test".parse().unwrap());
        assert_eq!(send(&app, bad).await.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn split_ports_separate_mcp_from_ui() {
        let (mcp_app, ui) = routers(app_state(Some(repo()), &[7411, 7412]));
        let ui = ui.unwrap();
        assert_eq!(send(&ui, post("/api/call/spec_report", "127.0.0.1:7412", "{}")).await.0, StatusCode::OK);
        assert_ne!(send(&ui, post("/mcp", "127.0.0.1:7412", "{}")).await.0, StatusCode::OK);
        let (st, _) = send(&mcp_app, post("/api/call/spec_report", "127.0.0.1:7411", "{}")).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    }

    /// Release builds only: the embedded UI, SPA fallback, 404 for a missing asset.
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
