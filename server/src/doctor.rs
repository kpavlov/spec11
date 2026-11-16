//! `spec11 doctor`: is this repo set up for spec11, and is CodeGraph usable here? `run` only looks and suggests. The one thing that writes is `index`, which runs only when the user asks (`doctor --fix`, the Setup tab button).
use serde::Serialize;
use spec11_core::{Analysis, codegraph::{Graph, GraphError}, scan::Project};
use std::path::Path;
use std::time::SystemTime;

#[derive(Serialize)]
pub struct Check {
    pub name: &'static str,
    /// `ok`, `warn` (works, but something is missing) or `fail` (spec11 cannot work here).
    pub level: &'static str,
    pub detail: String,
    pub hint: Option<String>,
}

fn c(name: &'static str, level: &'static str, detail: impl Into<String>, hint: Option<&str>) -> Check {
    Check { name, level, detail: detail.into(), hint: hint.map(str::to_owned) }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// `codegraph --version`, or None when the binary is not on PATH.
fn codegraph_version() -> Option<String> {
    let out = std::process::Command::new("codegraph").arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(if out.stdout.is_empty() { &out.stderr } else { &out.stdout }).trim().to_string();
    Some(text.lines().next().unwrap_or("").to_string())
}

/// Build or refresh the CodeGraph index by running the `codegraph` binary: `init` when there is no index yet, `sync` when there is.
/// Never called by `run` or on its own: indexing is the user's decision.
// (SETUP-07)
pub fn index(root: Option<&Path>, action: &str) -> Result<String, String> {
    let p = Project::open(root)?;
    let exists = p.root.join(&p.cfg.codegraph).exists();
    let args: [&str; 2] = match (action, exists) {
        ("init", false) => ["init", "-y"],
        ("sync", true) => ["sync", ""],
        ("init", true) => return Err("an index already exists; use sync to refresh it".into()),
        ("sync", false) => return Err("no index yet; use init first".into()),
        _ => return Err(format!("unknown action '{action}': use init or sync")),
    };
    codegraph_version().ok_or("the codegraph binary is not on PATH: install CodeGraph first")?;
    let out = std::process::Command::new("codegraph").args(args.iter().filter(|a| !a.is_empty())).arg(&p.root).current_dir(&p.root).output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&[out.stdout, out.stderr].concat()).trim().to_string();
    let tail = text.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    if out.status.success() { Ok(format!("codegraph {action} finished\n{tail}\n")) } else { Err(format!("codegraph {action} failed\n{tail}")) }
}

/// Which index action the checks call for, if any: `init` when missing, `sync` when stale.
pub fn suggested(checks: &[Check]) -> Option<&'static str> {
    let k = checks.iter().find(|k| k.name == "codegraph index")?;
    if checks.iter().any(|b| b.name == "codegraph binary" && b.level != "ok") { return None; }
    if k.detail.contains("not initialized") { Some("init") } else if k.detail.contains("older than") { Some("sync") } else { None }
}

// (SETUP-05, SETUP-06)
pub fn run(root: Option<&Path>, mcp_port: u16) -> Vec<Check> {
    let mut out = vec![];
    let p = match Project::open(root) {
        Ok(p) => p,
        Err(e) => return vec![c("repo", "fail", e, Some("run inside a repo with a specs/ folder, or pass --root; other layouts: spec11.json"))],
    };
    out.push(c("repo", "ok", p.root.display().to_string(), None));
    let cfg_file = p.root.join("spec11.json");
    out.push(c("config", "ok", if cfg_file.exists() { "spec11.json".to_string() } else { "no spec11.json: Swift defaults".to_string() }, None));

    match Analysis::load(Some(&p.root)) {
        Ok(a) => {
            let r = a.report();
            let (level, hint) = if r.requirements == 0 { ("warn", Some("no `- **ABC-12** MUST — text` bullets found in specs/*.md")) } else { ("ok", None) };
            out.push(c("specs", level, format!("{} requirements in {} spec files", r.requirements, r.spec_files), hint));
            if !r.spec_issues.is_empty() {
                out.push(c("spec issues", "warn", format!("{} (first: {})", r.spec_issues.len(), r.spec_issues[0]), Some("spec11 status --issues")));
            }
        }
        Err(e) => out.push(c("specs", "fail", e, None)),
    }

    for (name, dirs) in [("tests", &p.cfg.tests), ("sources", &p.cfg.src)] {
        let missing: Vec<&String> = dirs.iter().filter(|d| !p.root.join(d).is_dir()).collect();
        let n = p.files(dirs).len();
        let list = dirs.join(", ");
        if !missing.is_empty() {
            out.push(c(name, "warn", format!("not found: {}", missing.iter().map(|d| d.as_str()).collect::<Vec<_>>().join(", ")), Some("set the folders in spec11.json (\"tests\", \"src\", \"ext\")")));
        } else if n == 0 {
            out.push(c(name, "warn", format!("no .{} files in {list}", p.cfg.ext.join("/.")), Some("check \"ext\" in spec11.json")));
        } else {
            out.push(c(name, "ok", format!("{n} files in {list}"), None));
        }
    }

    let git = std::process::Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).current_dir(&p.root).output().is_ok_and(|o| o.status.success());
    out.push(if git { c("git", "ok", "work tree", None) } else { c("git", "warn", "not a git work tree", Some("`status --changed` needs git")) });

    // CodeGraph: optional, adds test-to-code links, overlap and uncovered code
    let bin = codegraph_version();
    out.push(match &bin {
        Some(v) => c("codegraph binary", "ok", if v.is_empty() { "on PATH".to_string() } else { v.clone() }, None),
        None => c("codegraph binary", "warn", "not on PATH", Some("optional: install CodeGraph for links, overlap and uncovered code; spec11 works without it")),
    });
    let db = p.root.join(&p.cfg.codegraph);
    out.push(match Graph::open(&db) {
        Ok(None) => {
            let hint = if bin.is_some() { "run `codegraph init` in the repo if you want it (indexing is your call; spec11 never does it)" } else { "needs the codegraph binary first" };
            c("codegraph index", "warn", format!("{} not found: not initialized", p.cfg.codegraph), Some(hint))
        }
        Ok(Some(_)) => {
            let newest = p.files(&p.cfg.src).into_iter().chain(p.files(&p.cfg.tests)).filter_map(|f| mtime(&p.root.join(f))).max();
            match (mtime(&db), newest) {
                (Some(d), Some(n)) if n > d => c("codegraph index", "warn", "initialized, but older than the newest source or test file", Some("run `codegraph sync`")),
                _ => c("codegraph index", "ok", format!("{} readable, schema supported", p.cfg.codegraph), None),
            }
        }
        Err(GraphError::SchemaDrift(m)) => c("codegraph index", "warn", format!("schema not supported by this spec11: {m}"), Some("update spec11 or CodeGraph; those checks show as \"not checked\"")),
        Err(e) => c("codegraph index", "warn", e.to_string(), Some("the index may be corrupt: `codegraph init` again")),
    });

    match crate::skill::status(Some(&p.root), mcp_port) {
        Ok(st) => {
            let todo = crate::skill::suggestions(&st);
            if todo.is_empty() {
                out.push(c("agent setup", "ok", "skill and MCP entry installed", None));
            } else {
                todo.iter().for_each(|t| out.push(c("agent setup", "warn", t.clone(), Some("spec11 install"))));
            }
        }
        Err(e) => out.push(c("agent setup", "warn", e, None)),
    }
    out
}

pub fn text(checks: &[Check]) -> String {
    let mut s = String::from("spec11 doctor\n");
    for k in checks {
        let tag = match k.level {
            "ok" => "ok  ",
            "warn" => "warn",
            _ => "FAIL",
        };
        s += &format!("  {tag}  {:<17} {}\n", k.name, k.detail);
        if let Some(h) = &k.hint {
            s += &format!("        {:<17} -> {h}\n", "");
        }
    }
    let (w, f) = (checks.iter().filter(|k| k.level == "warn").count(), checks.iter().filter(|k| k.level == "fail").count());
    s + &format!("{}\n", if f > 0 { format!("{f} failed, {w} warnings") } else if w > 0 { format!("works, {w} warnings") } else { "all good".to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level<'a>(cs: &'a [Check], name: &str) -> &'a str {
        cs.iter().find(|k| k.name == name).unwrap_or_else(|| panic!("no check {name}")).level
    }

    #[doc = "[SETUP-05]"]
    #[test]
    fn doctor_reports_each_part_of_the_setup() {
        let d = std::env::temp_dir().join(format!("spec11-doctor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("specs")).unwrap();
        std::fs::create_dir_all(d.join("RunSolo/RunSolo")).unwrap(); // sources exist, tests do not
        std::fs::write(d.join("specs/01-a.md"), "- **AAA-01** MUST — one.\n").unwrap();
        std::fs::write(d.join("RunSolo/RunSolo/A.swift"), "struct A {}\n").unwrap();
        let cs = run(Some(&d), 7411);
        assert_eq!(level(&cs, "repo"), "ok");
        assert_eq!(level(&cs, "specs"), "ok");
        assert_eq!(level(&cs, "sources"), "ok");
        assert_eq!(level(&cs, "tests"), "warn");
        assert_eq!(level(&cs, "codegraph index"), "warn"); // no .codegraph here
        assert!(cs.iter().any(|k| k.name == "codegraph binary"));
        let t = text(&cs);
        assert!(t.contains("not initialized") && t.contains("warnings"), "{t}");
    }

    #[doc = "[SETUP-07]"]
    #[test]
    fn indexing_runs_only_on_request_and_checks_state_first() {
        let d = std::env::temp_dir().join(format!("spec11-doctor-idx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("specs")).unwrap();
        assert!(index(Some(&d), "sync").unwrap_err().contains("use init first"));
        assert!(index(Some(&d), "bogus").unwrap_err().contains("unknown action"));
        std::fs::create_dir_all(d.join(".codegraph")).unwrap();
        std::fs::write(d.join(".codegraph/codegraph.db"), "").unwrap();
        assert!(index(Some(&d), "init").unwrap_err().contains("already exists"));
        assert!(!run(Some(&d), 7411).is_empty() && d.join(".codegraph/codegraph.db").metadata().unwrap().len() == 0, "run() must not touch the index");
    }

    #[doc = "[SETUP-06]"]
    #[test]
    fn no_specs_folder_is_a_failure() {
        let d = std::env::temp_dir().join(format!("spec11-doctor-bare-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let cs = run(Some(&d), 7411);
        assert_eq!((cs.len(), cs[0].level), (1, "fail"));
    }
}
