//! The tools, defined once: the CLI, `/api/call/{name}` and `/mcp` all run these over `spec11-core`.
//! Every tool returns plain text (what the Python scripts printed) and a JSON object (the same data, typed).
use serde_json::{Value, json};
use spec11_core::{Analysis, StatusOpts};
use std::path::{Path, PathBuf};

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
    /// Shown in an MCP Apps host as the dashboard.
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
        "built": {"type": "boolean", "description": "requirements marked later/deferred that already have tests or code"},
        "done": {"type": "boolean", "description": "check-marked requirements, each with its evidence verdict"},
        "unproven": {"type": "boolean", "description": "check-marked requirements with no test or no link to code"},
        "issues": {"type": "boolean", "description": "requirements with a spec issue"},
        "effort": {"type": "string", "enum": ["low", "medium", "high"], "description": "evidence per listed line"},
        "limit": {"type": "integer", "description": "max listed lines (default 200)"}
    });
    let path = json!({"path": {"type": "string", "description": "only files whose path contains this"}});
    vec![
        Tool { name: "spec_report", description: "Whole-project spec-driven status: coverage headline, problem tally and one section per problem kind. Deterministic. Evidence, not truth.", schema: obj(json!({"limit": {"type": "integer", "description": "lines per problem section (default 12)"}})), ui: true },
        Tool { name: "spec_status", description: "Per-spec-file summary, or one line per requirement when a list filter is given (missing, changed, built, done, unproven, issues, all). Deterministic. Evidence, not truth.", schema: obj(status_props), ui: true },
        Tool { name: "spec_trace", description: "One requirement in depth: spec text, tests tagged with it, source citations with their declaration, related code. Evidence, not truth.", schema: required(obj(json!({"id": {"type": "string", "description": "requirement ID, e.g. DOC-25"}})), "id"), ui: false },
        Tool { name: "spec_overlap", description: "Code covered by 2+ requirements (CodeGraph index): where two specs may conflict or complement each other. A candidate list, not a verdict.", schema: obj(merge(path.clone(), json!({"all": {"type": "boolean", "description": "also pairs from the same spec file"}}))), ui: false },
        Tool { name: "spec_uncovered", description: "Code no requirement covers (CodeGraph index): candidates for a missing requirement, not proof.", schema: obj(merge(path, json!({"fanin": {"type": "integer"}, "min": {"type": "integer"}}))), ui: false },
        Tool { name: "spec_file", description: "A spec file's markdown (read-only; name is a file in the specs folder, e.g. 04-documents.md). Without name: the list of spec files.", schema: obj(json!({"name": {"type": "string", "description": "spec file name, e.g. 04-documents.md"}})), ui: false },
        Tool { name: "spec_source", description: "A test or source file's text (read-only; path is one of the scanned test/source files, as in spec_trace output).", schema: obj(json!({"path": {"type": "string", "description": "repo-relative path, e.g. RunSolo/RunSolo/A.swift"}})), ui: false },
        Tool { name: "spec_doctor", description: "Repo setup checks: specs, tests and source folders, git, the codegraph binary and whether the CodeGraph index is initialized, readable and fresh, skill and MCP entry. Only looks; never indexes or installs.", schema: obj(json!({})), ui: false },
        Tool { name: "spec_codegraph", description: "WRITES FILES, SLOW: build (action=init, when there is no .codegraph) or refresh (action=sync) the CodeGraph index by running the codegraph binary. Indexing is the user's decision: call only when the user asked for it. Same as `spec11 doctor --fix`.", schema: required(obj(json!({"action": {"type": "string", "enum": ["init", "sync"]}})), "action"), ui: false },
        Tool { name: "spec_install_status", description: "Whether the spec11 skill and MCP entry are installed for this repo (.agents/skills, .mcp.json) or globally (~/.agents/skills), with what to run if not.", schema: obj(json!({})), ui: true },
        Tool { name: "spec_install", description: "WRITES FILES: install the spec11 skill into <repo>/.agents/skills (global=true: ~/.agents/skills) and add the spec11 entry to <repo>/.mcp.json, keeping other servers. Same as `spec11 install`.", schema: obj(json!({"global": {"type": "boolean", "description": "install the skill into ~/.agents/skills instead of this repo"}, "port": {"type": "integer", "description": "MCP port for the .mcp.json entry (default: the port this server runs on)"}})), ui: false },
        Tool { name: "spec_uninstall", description: "WRITES FILES: remove what spec_install added (the skill, and the spec11 entry in .mcp.json), keeping everything else. Same as `spec11 uninstall`.", schema: obj(json!({"global": {"type": "boolean"}})), ui: false },
        Tool { name: "spec_dashboard", description: "URL of the spec11 web dashboard served by this process.", schema: obj(json!({})), ui: true },
    ]
}

/// The install tools: they write the skill and `.mcp.json`, so they never run an analysis.
fn setup(root: Option<&Path>, name: &str, args: &Value, mcp_port: u16) -> Option<Result<(String, Value), String>> {
    use crate::skill;
    let global = args["global"].as_bool().unwrap_or(false);
    let changes = match name {
        "spec_install_status" => None,
        "spec_install" => Some(skill::apply(root, global, Some(num(args, "port").map_or(mcp_port, |p| p as u16)))),
        "spec_uninstall" => Some(skill::apply(root, global, None)),
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
    if name == "spec_dashboard" {
        return Ok((format!("{dashboard}\n"), json!({"url": dashboard})));
    }
    if name == "spec_doctor" {
        let checks = crate::doctor::run(root, mcp_port);
        return Ok((crate::doctor::text(&checks), json!({"checks": checks, "index_action": crate::doctor::suggested(&checks)})));
    }
    if name == "spec_codegraph" {
        let text = crate::doctor::index(root, txt(args, "action"))?;
        return Ok((text.clone(), json!({"output": text})));
    }
    if let Some(r) = setup(root, name, args, mcp_port) {
        return r;
    }
    let a = Analysis::load(root)?;
    let note = |a: &Analysis| a.graph_note.clone().unwrap_or_else(|| "no .codegraph index here; the user decides about indexing".into());
    let out = match name {
        "spec_report" => {
            let r = a.report();
            (r.text(&a, num(args, "limit").unwrap_or(12).max(0) as usize), json!(r))
        }
        "spec_status" => {
            let o = StatusOpts {
                spec: args.get("spec").and_then(Value::as_str).map(str::to_owned),
                prefix: args.get("prefix").and_then(Value::as_str).map(str::to_owned),
                all: args["all"].as_bool().unwrap_or(false),
                missing: args["missing"].as_bool().unwrap_or(false),
                changed: args.get("changed").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned),
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
        "spec_file" => {
            let dir = a.p.root.join(&a.p.cfg.specs);
            let mut names: Vec<String> = std::fs::read_dir(&dir).map_err(|e| e.to_string())?.filter_map(|e| e.ok()?.file_name().into_string().ok()).filter(|n| n.ends_with(".md")).collect();
            names.sort();
            let want = txt(args, "name");
            if want.is_empty() {
                (names.join("\n") + "\n", json!({"files": names}))
            } else if names.iter().any(|n| n == want) {
                // only names read from the folder listing: no path traversal
                let md = std::fs::read_to_string(dir.join(want)).map_err(|e| e.to_string())?;
                (md.clone(), json!({"name": want, "markdown": md}))
            } else {
                return Err(format!("{want}: not a spec file"));
            }
        }
        "spec_source" => {
            let want = txt(args, "path");
            // only paths the scanner itself lists: no traversal, nothing outside tests/src
            if !a.p.files(&a.p.cfg.tests).iter().chain(a.p.files(&a.p.cfg.src).iter()).any(|f| f == want) {
                return Err(format!("{want}: not a scanned test or source file"));
            }
            let text = std::fs::read_to_string(a.p.root.join(want)).map_err(|e| e.to_string())?;
            (text.clone(), json!({"path": want, "text": text}))
        }
        "spec_trace" => match a.trace(txt(args, "id")) {
            Some(t) => (t.text(), json!(t)),
            None => return Err(format!("{}: not in specs, tests or sources", txt(args, "id").to_uppercase())),
        },
        "spec_overlap" => {
            let rows = a.overlap(txt(args, "path"), args["all"].as_bool().unwrap_or(false)).map_err(|e| e.to_string())?.ok_or_else(|| note(&a))?;
            (a.overlap_text(&rows, 25), json!({"rows": rows}))
        }
        "spec_uncovered" => {
            let u = a.uncovered(txt(args, "path"), num(args, "fanin").unwrap_or(5).max(0) as usize, num(args, "min").unwrap_or(10)).map_err(|e| e.to_string())?.ok_or_else(|| note(&a))?;
            (u.text(), json!(u))
        }
        other => return Err(format!("unknown tool {other}")),
    };
    Ok(out)
}

pub fn evidence_note() -> &'static str {
    NOTE
}

/// What the dashboard shows in its header.
pub fn info(root: Option<&PathBuf>) -> Result<Value, String> {
    let p = spec11_core::scan::Project::open(root.map(|r| r.as_path()))?;
    Ok(json!({"root": p.root, "note": NOTE}))
}
