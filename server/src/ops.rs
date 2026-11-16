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
        Tool { name: "spec_dashboard", description: "URL of the spec11 web dashboard served by this process.", schema: obj(json!({})), ui: true },
    ]
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

/// Run one tool against the repo at `root` (or the nearest ancestor of the cwd with a specs folder). Blocking.
pub fn call(root: Option<&Path>, name: &str, args: &Value, dashboard: &str) -> Result<(String, Value), String> {
    if name == "spec_dashboard" {
        return Ok((format!("{dashboard}\n"), json!({"url": dashboard})));
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
