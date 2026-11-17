//! The tools, defined once: the CLI and `/mcp` (the dashboard included) both run these over `spec11-core`.
//! Every tool returns plain text (what the Python scripts printed) and a JSON object (the same data, typed).
use serde_json::{Value, json};
use spec11_core::{Analysis, ChangeOpts, StatusOpts};
use std::path::{Path, PathBuf};

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
    /// Opens the dashboard in an MCP Apps host (only `dashboard` does: one app, the page comes from its `view` argument).
    pub ui: bool,
}

const NOTE: &str = "Evidence, not truth: a citation or tag does not prove the requirement is implemented or that its test passes.";

pub fn tools() -> Vec<Tool> {
    let status_props = json!({
        "spec": {"type": "string", "description": "only spec files whose name contains this, e.g. 04"},
        "prefix": {"type": "string", "description": "only IDs with this prefix, e.g. DOC"},
        "all": {"type": "boolean", "description": "list every requirement"},
        "missing": {"type": "boolean", "description": "due requirements with no test"},
        "changed": {"type": "string", "description": "requirements touched by the working tree vs this git ref (e.g. HEAD)"},
        "staged": {"type": "boolean", "description": "with changed (HEAD if not given): only what is staged"},
        "include_untracked": {"type": "boolean", "description": "with changed: count untracked files (default true)"},
        "ignore_whitespace": {"type": "boolean", "description": "with changed: leave out changes that only touch whitespace"},
        "built": {"type": "boolean", "description": "requirements marked later/deferred that already have tests or code"},
        "done": {"type": "boolean", "description": "check-marked requirements, each with its evidence verdict"},
        "unproven": {"type": "boolean", "description": "check-marked requirements with no test or no link to code"},
        "issues": {"type": "boolean", "description": "requirements with a spec issue"},
        "effort": {"type": "string", "enum": ["low", "medium", "high"], "description": "evidence per listed line"},
        "limit": {"type": "integer", "description": "max listed lines (default 200)"}
    });
    let change_props = json!({
        "ref": {"type": "string", "description": "git ref the working tree is compared with (default HEAD), e.g. main or origin/main"},
        "commit": {"type": "string", "description": "explain this one commit against its first parent (sha or ref; ignores ref, staged and include_untracked)"},
        "staged": {"type": "boolean", "description": "only what is staged"},
        "include_untracked": {"type": "boolean", "description": "count untracked files (default true)"},
        "ignore_whitespace": {"type": "boolean", "description": "leave out changes that only touch whitespace"},
        "show_ignored": {"type": "boolean", "description": "also files .aiignore, .spec11ignore and .agents hide (default false)"},
        "show_all": {"type": "boolean", "description": "also files spec11 does not scan: kind other (default false)"}
    });
    let path = json!({"path": {"type": "string", "description": "only files whose path contains this"}});
    vec![
        Tool { name: "report", description: "Whole-project spec-driven status: coverage headline, problem tally and one section per problem kind. Deterministic. Evidence, not truth.", schema: obj(json!({"limit": {"type": "integer", "description": "lines per problem section (default 12)"}})), ui: false },
        Tool { name: "status", description: "Per-spec-file summary, or one line per requirement when a list filter is given (missing, changed, built, done, unproven, issues, all). Deterministic. Evidence, not truth.", schema: obj(status_props), ui: false },
        Tool { name: "changes", description: "What changed against a git ref (HEAD by default) or in one commit: every changed file (added, modified, deleted, renamed; staged, unstaged, untracked or committed; spec, test, src, and other with show_all) and the requirements the changed lines touch: a removed bullet, an edited test or a cited function, with what happened to it. Evidence of edits, not that anything is done.", schema: obj(change_props.clone()), ui: false },
        Tool { name: "diff", description: "Unified diff of one changed file against a ref or in one commit (a rename shows both paths; an untracked file is all additions), and under each hunk the requirements it touches. Takes the same arguments as `changes`, and only files it lists. Shows what was edited, not whether it is right.", schema: required(obj(merge(change_props.clone(), json!({"path": {"type": "string", "description": "repo-relative path of a changed file, as in `changes`"}}))), "path"), ui: false },
        Tool { name: "history", description: "Commits on a branch (HEAD by default) that are not on the base branch (main or master by default), newest first, and how many files are uncommitted. Explain one with `changes commit=<sha>`. Needs git.", schema: obj(json!({"base": {"type": "string", "description": "branch the commits are not on (default: main, master, origin/main or origin/master)"}, "head": {"type": "string", "description": "branch to list (default HEAD)"}, "limit": {"type": "integer", "description": "max commits (default 50)"}})), ui: false },
        Tool { name: "trace", description: "One requirement in depth: spec text, tests tagged with it, source citations with their declaration, related code. Evidence, not truth.", schema: required(obj(json!({"id": {"type": "string", "description": "requirement ID, e.g. DOC-25"}})), "id"), ui: false },
        Tool { name: "overlap", description: "Code covered by 2+ requirements (CodeGraph index): where two specs may conflict or complement each other. A candidate list, not a verdict.", schema: obj(merge(path.clone(), json!({"all": {"type": "boolean", "description": "also pairs from the same spec file"}}))), ui: false },
        Tool { name: "uncovered", description: "Code no requirement covers (CodeGraph index): candidates for a missing requirement, not proof.", schema: obj(merge(path, json!({"fanin": {"type": "integer"}, "min": {"type": "integer"}}))), ui: false },
        Tool { name: "file", description: "A spec file's markdown (read-only; name is a file in the specs folder, e.g. 04-documents.md). Without name: the list of spec files.", schema: obj(json!({"name": {"type": "string", "description": "spec file name, e.g. 04-documents.md"}})), ui: false },
        Tool { name: "source", description: "A test or source file's text (read-only; path is one of the scanned test/source files, as in trace output).", schema: obj(json!({"path": {"type": "string", "description": "repo-relative path, e.g. src/main.rs"}})), ui: false },
        Tool { name: "doctor", description: "Repo setup checks: specs, tests and source folders, git, the codegraph binary and whether the CodeGraph index is initialized, readable and fresh, skill and MCP entry. Only looks; never indexes or installs.", schema: obj(json!({})), ui: false },
        Tool { name: "codegraph", description: "WRITES FILES, SLOW: build (action=init, when there is no .codegraph) or refresh (action=sync) the CodeGraph index by running the codegraph binary. Indexing is the user's decision: call only when the user asked for it. Same as `spec11 doctor --fix`.", schema: required(obj(json!({"action": {"type": "string", "enum": ["init", "sync"]}})), "action"), ui: false },
        Tool { name: "install_status", description: "Whether the spec11 skill and MCP entry are installed for this repo (.agents/skills, .mcp.json) or globally (~/.agents/skills), with what to run if not.", schema: obj(json!({})), ui: false },
        Tool { name: "install", description: "WRITES FILES: install the spec11 skill into <repo>/.agents/skills (global=true: ~/.agents/skills) and add the spec11 entry to <repo>/.mcp.json, keeping other servers. Same as `spec11 install`.", schema: obj(json!({"global": {"type": "boolean", "description": "install the skill into ~/.agents/skills instead of this repo"}, "port": {"type": "integer", "description": "MCP port for the .mcp.json entry (default: the port this server runs on)"}})), ui: false },
        Tool { name: "uninstall", description: "WRITES FILES: remove what install added (the skill, and the spec11 entry in .mcp.json), keeping everything else. Same as `spec11 uninstall`.", schema: obj(json!({"global": {"type": "boolean"}})), ui: false },
        Tool { name: "module", description: "WRITES FILES: set a module hidden (true) or shown (false) by adding or removing its folder line in <repo>/.spec11ignore (gitignore syntax), keeping every other line. To hide, path is a module listed by spec11://modules; to show, one of its `hidden`. Only when the user asked for it.", schema: merge(obj(json!({"path": {"type": "string", "description": "module path, e.g. server"}, "hidden": {"type": "boolean", "description": "true: leave the module out of every scan; false: list it again"}})), json!({"required": ["path", "hidden"]})), ui: false },
        Tool { name: "dashboard", description: "URL of the spec11 web dashboard served by this process; in an MCP Apps host this tool opens the dashboard. view (optional) picks the page: a tab, or a tab with its selection and filters, e.g. modules, changes, requirements/DOC-25, requirements?changed=1, tests?module=core&state=untagged, specs?file=04-documents.md, setup.", schema: obj(json!({"view": {"type": "string", "description": "tab or route to open, e.g. requirements/DOC-25?state=none (default: the dashboard)"}})), ui: true },
    ]
}

fn change_opts(args: &Value) -> ChangeOpts {
    let on = |k: &str| args[k].as_bool().unwrap_or(false);
    ChangeOpts {
        reference: txt(args, "ref").into(), commit: txt(args, "commit").into(), staged: on("staged"), no_untracked: args["include_untracked"].as_bool() == Some(false),
        ignore_whitespace: on("ignore_whitespace"), show_ignored: on("show_ignored"), show_all: on("show_all"),
    }
}

/// The install tools: they write the skill and `.mcp.json`, so they never run an analysis.
fn setup(root: Option<&Path>, name: &str, args: &Value, mcp_port: u16) -> Option<Result<(String, Value), String>> {
    use crate::skill;
    let global = args["global"].as_bool().unwrap_or(false);
    let changes = match name {
        "install_status" => None,
        "install" => Some(skill::apply(root, global, Some(num(args, "port").map_or(mcp_port, |p| p as u16)))),
        "uninstall" => Some(skill::apply(root, global, None)),
        _ => return None,
    };
    Some((|| {
        let changed = changes.transpose()?;
        let st = skill::status(root, mcp_port)?;
        let mut text = changed.unwrap_or_default().join("\n");
        let todo = skill::suggestions(&st);
        text += &(if todo.is_empty() { "\ninstalled: the agent can use spec11 here".to_string() } else { format!("\n{}", todo.join("\n")) });
        Ok((text.trim_start().to_string() + "\n", json!({"status": st, "suggestions": todo})))
    })())
}

fn obj(props: Value) -> Value {
    json!({"type": "object", "properties": props})
}

fn required(mut schema: Value, field: &str) -> Value {
    schema["required"] = json!([field]);
    schema
}

fn merge(mut a: Value, b: Value) -> Value {
    a.as_object_mut().unwrap().extend(b.as_object().unwrap().clone());
    a
}

fn num(args: &Value, k: &str) -> Option<i64> {
    args.get(k).and_then(Value::as_i64)
}

fn txt<'a>(args: &'a Value, k: &str) -> &'a str {
    args.get(k).and_then(Value::as_str).unwrap_or("")
}

// (API-01)
/// Run one tool against the repo at `root` (or the nearest ancestor of the cwd with a specs folder). Blocking.
pub fn call(root: Option<&Path>, name: &str, args: &Value, dashboard: &str, mcp_port: u16) -> Result<(String, Value), String> {
    if name == "dashboard" {
        let url = format!("{dashboard}{}", args["view"].as_str().unwrap_or("").trim_start_matches('/'));
        return Ok((format!("{url}\n"), json!({"url": url})));
    }
    if name == "doctor" {
        let checks = crate::doctor::run(root, mcp_port);
        return Ok((crate::doctor::text(&checks), json!({"checks": checks, "index_action": crate::doctor::suggested(&checks)})));
    }
    if name == "codegraph" {
        let text = crate::doctor::index(root, txt(args, "action"))?;
        return Ok((text.clone(), json!({"output": text})));
    }
    if let Some(r) = setup(root, name, args, mcp_port) {
        return r;
    }
    let a = Analysis::load(root)?;
    let note = |a: &Analysis| a.graph_note.clone().unwrap_or_else(|| "no .codegraph index here; the user decides about indexing".into());
    let out = match name {
        "report" => {
            let r = a.report();
            (r.text(&a, num(args, "limit").unwrap_or(12).max(0) as usize), json!(r))
        }
        "status" => {
            let o = StatusOpts {
                spec: args.get("spec").and_then(Value::as_str).map(str::to_owned),
                prefix: args.get("prefix").and_then(Value::as_str).map(str::to_owned),
                all: args["all"].as_bool().unwrap_or(false),
                missing: args["missing"].as_bool().unwrap_or(false),
                changed: args.get("changed").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned),
                staged: args["staged"].as_bool().unwrap_or(false),
                no_untracked: args["include_untracked"].as_bool() == Some(false),
                ignore_whitespace: args["ignore_whitespace"].as_bool().unwrap_or(false),
                built: args["built"].as_bool().unwrap_or(false),
                done: args["done"].as_bool().unwrap_or(false),
                unproven: args["unproven"].as_bool().unwrap_or(false),
                issues: args["issues"].as_bool().unwrap_or(false),
                effort: txt(args, "effort").to_string(),
            };
            let mut st = a.status(&o)?;
            let limit = num(args, "limit").unwrap_or(200).max(0) as usize;
            let more = st.rows.len().saturating_sub(limit);
            st.rows.truncate(limit);
            let mut text = st.text(&o);
            if more > 0 {
                text = format!("(first {limit} of {} listed; narrow with spec/prefix or raise limit)\n{text}", limit + more);
            }
            (text, json!(st))
        }
        "changes" => {
            let c = a.changes(&change_opts(args))?;
            (c.text(), json!(c))
        }
        "diff" => {
            let d = a.file_diff(txt(args, "path"), &change_opts(args))?;
            (d.text(), json!(d))
        }
        "history" => {
            let h = a.p.history(txt(args, "base"), txt(args, "head"), num(args, "limit").unwrap_or(50).clamp(1, 500) as usize)?;
            (h.text(), json!(h))
        }
        "file" => {
            let names = a.p.spec_files();
            let want = txt(args, "name");
            if want.is_empty() {
                (names.join("\n") + "\n", json!({"files": names}))
            } else if names.iter().any(|n| n == want) {
                // only names read from the folder listing: no path traversal
                let md = std::fs::read_to_string(a.p.root.join(want)).map_err(|e| e.to_string())?;
                (md.clone(), json!({"name": want, "markdown": md}))
            } else {
                return Err(format!("{want}: not a spec file"));
            }
        }
        "source" => {
            let want = txt(args, "path");
            // only paths the scanner itself lists: no traversal, nothing outside tests/src
            if !a.p.test_files().iter().chain(a.p.src_files().iter()).any(|f| f == want) {
                return Err(format!("{want}: not a scanned test or source file"));
            }
            let text = std::fs::read_to_string(a.p.root.join(want)).map_err(|e| e.to_string())?;
            (text.clone(), json!({"path": want, "text": text}))
        }
        "trace" => match a.trace(txt(args, "id")) {
            Some(t) => (t.text(), json!(t)),
            None => return Err(format!("{}: not in specs, tests or sources", txt(args, "id").to_uppercase())),
        },
        "overlap" => {
            let rows = a.overlap(txt(args, "path"), args["all"].as_bool().unwrap_or(false)).map_err(|e| e.to_string())?.ok_or_else(|| note(&a))?;
            (a.overlap_text(&rows, 25), json!({"rows": rows}))
        }
        "uncovered" => {
            let u = a.uncovered(txt(args, "path"), num(args, "fanin").unwrap_or(5).max(0) as usize, num(args, "min").unwrap_or(10)).map_err(|e| e.to_string())?.ok_or_else(|| note(&a))?;
            (u.text(), json!(u))
        }
        // (API-11, API-12)
        "module" => {
            let want = txt(args, "path").trim_matches('/');
            let line = format!("/{want}/");
            let file = a.p.root.join(".spec11ignore");
            let mut text = std::fs::read_to_string(&file).unwrap_or_default();
            let hidden = spec11_core::detect::Ignore::hidden_modules(&a.p.root);
            let msg = if args["hidden"].as_bool().ok_or("hidden: true or false")? {
                // only a module the scanner lists, never the root or a glob: the line is that folder and nothing else
                if want == "." || want.contains(['*', '?', '[', '\\', '\n']) || !a.modules_view().modules.iter().any(|m| m.path == want) {
                    return Err(format!("{want}: not a module that can be hidden"));
                }
                if !hidden.iter().any(|m| m == want) {
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                    text += &format!("{line}\n");
                }
                format!("{line} is in .spec11ignore\n")
            } else {
                if !hidden.iter().any(|m| m == want) {
                    return Err(format!("{want}: not a hidden module"));
                }
                text = text.lines().filter(|l| l.trim() != line).map(|l| format!("{l}\n")).collect();
                format!("{line} is not in .spec11ignore\n")
            };
            std::fs::write(&file, text).map_err(|e| format!(".spec11ignore: {e}"))?;
            (msg, json!({"path": want, "hidden": args["hidden"]}))
        }
        other => return Err(format!("unknown tool {other}")),
    };
    Ok(out)
}

/// Read-only JSON MCP resources: (uri, name, description, JSON text of the analysis). Listed and read through this table only.
pub const RESOURCES: [(&str, &str, &str, fn(&Analysis) -> serde_json::Result<String>); 2] = [
    ("spec11://modules", "spec11 modules", "Modules of the repo (nearest build manifest, else the folder above src): files, languages, tests, CodeGraph symbols and the modules each depends on. Evidence, not truth.", |a| serde_json::to_string_pretty(&a.modules_view())),
    ("spec11://tests", "spec11 tests", "Test cases detected in the test files, grouped by module and file, with the requirement IDs tagged on each. Detected, not run: it does not say a test passes.", |a| serde_json::to_string_pretty(&a.tests_view())),
];

/// The read-only tools that take no argument, also as plain-text MCP resources: (uri, name, tool). The text is what the CLI prints.
/// The description is the tool's. Tools with arguments (`trace`, status filters) and the writing ones stay tools only.
pub const VIEWS: [(&str, &str, &str); 5] = [
    ("spec11://changes", "spec11 changes", "changes"),
    ("spec11://report", "spec11 report", "report"),
    ("spec11://status", "spec11 status", "status"),
    ("spec11://uncovered", "spec11 uncovered", "uncovered"),
    ("spec11://overlap", "spec11 overlap", "overlap"),
];

/// The same tools as JSON resources, `<view uri>.json`: (uri, name, tool). The JSON is the tool's `structuredContent`, typed by
/// the schema at `spec11://schemas/<name>`. Without a CodeGraph index `uncovered` and `overlap` cannot be typed data: reading
/// them is an error that says why.
pub const DATA: [(&str, &str, &str); 5] = [
    ("spec11://changes.json", "spec11 changes (JSON)", "changes"),
    ("spec11://report.json", "spec11 report (JSON)", "report"),
    ("spec11://status.json", "spec11 status (JSON)", "status"),
    ("spec11://uncovered.json", "spec11 uncovered (JSON)", "uncovered"),
    ("spec11://overlap.json", "spec11 overlap (JSON)", "overlap"),
];

pub const SCHEMA_PREFIX: &str = "spec11://schemas/";
pub const JSON_MIME: &str = "application/json";
pub const SCHEMA_MIME: &str = "application/schema+json";
pub const TEXT_MIME: &str = "text/plain";

/// The description of a tool, for the resource that shows the same thing.
pub fn tool_description(name: &str) -> &'static str {
    tools().into_iter().find(|t| t.name == name).map_or("", |t| t.description)
}

/// Every resource that can change when the repo does (and so can be subscribed to). The schemas never change.
pub fn resource_uris() -> impl Iterator<Item = &'static str> {
    RESOURCES.iter().map(|r| r.0).chain(VIEWS.iter().map(|v| v.0)).chain(DATA.iter().map(|d| d.0))
}

/// The schema a JSON resource is typed by, as its `spec11://schemas/<name>` URI.
pub fn schema_uri(resource: &str) -> Option<String> {
    let name = resource.strip_prefix("spec11://")?;
    let name = name.strip_suffix(".json").unwrap_or(name);
    schema(name).map(|_| format!("{SCHEMA_PREFIX}{name}"))
}

/// Names that have a published schema (`spec11://schemas/<name>`): the JSON resources.
pub const SCHEMAS: [&str; 7] = ["report", "status", "changes", "uncovered", "overlap", "modules", "tests"];

/// The schema of a type. schemars' numeric `format` hints (`uint`, `int64`, ...) are not JSON Schema formats: validators warn on them.
fn of<T: schemars::JsonSchema>() -> Value {
    fn plain(v: &mut Value) {
        match v {
            Value::Object(m) => {
                if m.get("format").and_then(Value::as_str).is_some_and(|f| f.starts_with("uint") || f.starts_with("int") || f.starts_with("double") || f.starts_with("float")) {
                    m.remove("format");
                }
                // `"type": ["string", "null"]` is legal but less portable (Gemini/OpenAPI-style clients): use `anyOf` of single types.
                let nullable = match m.get("type").and_then(Value::as_array).map(Vec::as_slice) {
                    Some([t, n]) if t.is_string() && n == "null" => Some(t.clone()),
                    _ => None,
                };
                if let Some(t) = nullable {
                    let mut branch = std::mem::take(m);
                    for k in ["title", "description", "default"] {
                        if let Some(x) = branch.remove(k) {
                            m.insert(k.into(), x);
                        }
                    }
                    branch.insert("type".into(), t);
                    m.insert("anyOf".into(), json!([branch, {"type": "null"}]));
                }
                m.values_mut().for_each(plain);
            }
            Value::Array(a) => a.iter_mut().for_each(plain),
            _ => {}
        }
    }
    let mut v = serde_json::to_value(schemars::schema_for!(T)).unwrap_or_default();
    plain(&mut v);
    v
}

fn strs() -> Value {
    json!({"type": "array", "items": {"type": "string"}})
}

fn shape(props: Value, required: &[&str]) -> Value {
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object", "properties": props, "required": required})
}

/// JSON Schema of a result's structured content: `report`, `status`, `trace`, `overlap`, `uncovered`, `modules`, `tests`,
/// `file`, `source`, `doctor`, `codegraph`, `install`, `dashboard`. Typed from the core structs where there are any.
pub fn schema(name: &str) -> Option<Value> {
    use spec11_core::{Changes, FileDiff, History, Report, Status, Trace, coverage::{OverlapRow, Uncovered}, modules::{ModulesView, TestsView}};
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Overlap {
        rows: Vec<OverlapRow>,
    }
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Doctor {
        checks: Vec<crate::doctor::Check>,
        index_action: Option<String>,
    }
    Some(match name {
        "report" => of::<Report>(),
        "status" => of::<Status>(),
        "changes" => of::<Changes>(),
        "trace" => of::<Trace>(),
        "uncovered" => of::<Uncovered>(),
        "overlap" => of::<Overlap>(),
        "modules" => of::<ModulesView>(),
        "tests" => of::<TestsView>(),
        "doctor" => of::<Doctor>(),
        "file" => shape(json!({"files": strs(), "name": {"type": "string"}, "markdown": {"type": "string"}}), &[]),
        "diff" => of::<FileDiff>(),
        "history" => of::<History>(),
        "source" => shape(json!({"path": {"type": "string"}, "text": {"type": "string"}}), &["path", "text"]),
        "codegraph" => shape(json!({"output": {"type": "string"}}), &["output"]),
        "install" => shape(json!({"status": {"type": "object"}, "suggestions": strs()}), &["status", "suggestions"]),
        "module" => shape(json!({"path": {"type": "string"}, "hidden": {"type": "boolean"}}), &["path", "hidden"]),
        "dashboard" => shape(json!({"url": {"type": "string"}}), &["url"]),
        _ => return None,
    })
}

/// The `outputSchema` of a tool: its result's `structuredContent`.
pub fn output_schema(tool: &str) -> Option<Value> {
    schema(if tool.starts_with("install") || tool == "uninstall" { "install" } else { tool })
}

// (API-07, API-08)
/// (mime type, text) of a resource, `None` for an unknown URI.
pub fn read_resource(root: Option<&Path>, uri: &str) -> Result<Option<(&'static str, String)>, String> {
    if let Some((.., tool)) = VIEWS.iter().find(|v| v.0 == uri) {
        return match call(root, tool, &json!({}), "", 0) {
            Ok((text, _)) => Ok(Some((TEXT_MIME, text))),
            // "not checked" (no usable CodeGraph index) is content here, not an error: indexing is the user's decision
            Err(e) if matches!(*tool, "overlap" | "uncovered") && Analysis::load(root).is_ok_and(|a| a.graph.is_none()) => Ok(Some((TEXT_MIME, e))),
            Err(e) => Err(e),
        };
    }
    if let Some((.., tool)) = DATA.iter().find(|d| d.0 == uri) {
        let (_, data) = call(root, tool, &json!({}), "", 0)?;
        return serde_json::to_string_pretty(&data).map(|t| Some((JSON_MIME, t))).map_err(|e| e.to_string());
    }
    if let Some(name) = uri.strip_prefix(SCHEMA_PREFIX).filter(|n| SCHEMAS.contains(n)) {
        return Ok(schema(name).map(|s| (SCHEMA_MIME, s.to_string())));
    }
    let Some((.., render)) = RESOURCES.iter().find(|r| r.0 == uri) else { return Ok(None) };
    render(&Analysis::load(root)?).map(|t| Some((JSON_MIME, t))).map_err(|e| e.to_string())
}

// (API-08)
/// A fingerprint (path, mtime, size) of what the analysis reads: the repo's files, the spec files, `spec11.json` and the
/// CodeGraph index with its WAL. Equal stamps mean the resources read the same. 0 when the repo cannot be opened. Blocking.
// ponytail: one repo walk per poll and per subscriber; share one poller or use a file watcher if that ever costs.
pub fn stamp(root: Option<&Path>) -> u64 {
    use std::hash::{Hash, Hasher};
    let Ok(p) = spec11_core::scan::Project::open(root) else { return 0 };
    let db = if p.cfg.codegraph.is_empty() { ".codegraph/codegraph.db".to_string() } else { p.cfg.codegraph.clone() };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let extra = ["spec11.json".to_string(), ".git/index".to_string(), ".git/HEAD".to_string(), ".git/logs/HEAD".to_string(), format!("{db}-wal"), format!("{db}-shm"), db];
    for rel in p.files.iter().chain(&p.spec_files()).chain(&extra) {
        if let Ok(m) = std::fs::metadata(p.root.join(rel)) {
            (rel, m.len(), m.modified().ok()).hash(&mut h);
        }
    }
    h.finish()
}

pub fn evidence_note() -> &'static str {
    NOTE
}

/// What the dashboard shows in its header.
pub fn info(root: Option<&PathBuf>) -> Result<Value, String> {
    let p = spec11_core::scan::Project::open(root.map(|r| r.as_path()))?;
    Ok(json!({"root": p.root, "note": NOTE}))
}
