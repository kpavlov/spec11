//! `spec11 install` / `uninstall`: the skill ships inside the binary and is written to `<base>/skills/spec11/SKILL.md`.
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

const NAME: &str = "spec11";
const SKILL: &str = include_str!("../../skills/spec11/SKILL.md");

/// `<repo>/.agents/skills` or `~/.agents/skills`.
pub fn skills_dir(root: Option<&Path>, global: bool) -> Result<PathBuf, String> {
    if global {
        return Ok(home()?.join(".agents/skills"));
    }
    let base = repo_root(root)?;
    Ok(base.join(".agents/skills"))
}

pub fn install(dir: &Path) -> Result<PathBuf, String> {
    let d = dir.join(NAME);
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    let f = d.join("SKILL.md");
    std::fs::write(&f, SKILL).map_err(|e| format!("{}: {e}", f.display()))?;
    Ok(f)
}

/// Removes only the file we wrote, and the folder if that leaves it empty: anything else the user put there stays.
pub fn uninstall(dir: &Path) -> Result<Option<PathBuf>, String> {
    let d = dir.join(NAME);
    let f = d.join("SKILL.md");
    if !f.exists() {
        return Ok(None);
    }
    std::fs::remove_file(&f).map_err(|e| format!("{}: {e}", f.display()))?;
    let _ = std::fs::remove_dir(&d); // fails (and is kept) when it still holds other files
    Ok(Some(f))
}

pub enum McpChange {
    Created,
    Added,
    Updated,
    Unchanged,
    Removed,
    /// The file held nothing else, so it was deleted.
    Deleted,
    Absent,
}

fn read_config(f: &Path) -> Result<Option<Map<String, Value>>, String> {
    match std::fs::read_to_string(f) {
        Ok(t) if t.trim().is_empty() => Ok(Some(Map::new())),
        Ok(t) => match serde_json::from_str::<Value>(&t) {
            Ok(Value::Object(m)) => Ok(Some(m)),
            Ok(_) => Err(format!("{}: not a JSON object, left untouched", f.display())),
            Err(e) => Err(format!("{}: {e}; left untouched", f.display())),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", f.display())),
    }
}

fn write_config(f: &Path, m: Map<String, Value>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&Value::Object(m)).map_err(|e| e.to_string())? + "\n";
    std::fs::write(f, text).map_err(|e| format!("{}: {e}", f.display()))
}

// (SETUP-01, SETUP-02)
/// Adds `mcpServers.spec11` to `<base>/.mcp.json`, creating the file if needed. Other servers and keys are kept as they are.
pub fn add_mcp(base: &Path, port: u16) -> Result<(PathBuf, McpChange), String> {
    let f = base.join(".mcp.json");
    let entry = json!({"type": "http", "url": format!("http://127.0.0.1:{port}/mcp")});
    let existing = read_config(&f)?;
    let created = existing.is_none();
    let mut m = existing.unwrap_or_default();
    let servers = m.entry("mcpServers").or_insert_with(|| json!({}));
    let servers = servers.as_object_mut().ok_or_else(|| format!("{}: mcpServers is not an object, left untouched", f.display()))?;
    let change = match servers.get(NAME) {
        Some(v) if *v == entry => McpChange::Unchanged,
        Some(_) => McpChange::Updated,
        None if created => McpChange::Created,
        None => McpChange::Added,
    };
    if !matches!(change, McpChange::Unchanged) {
        servers.insert(NAME.into(), entry);
        write_config(&f, m)?;
    }
    Ok((f, change))
}

// (SETUP-03)
/// Removes `mcpServers.spec11`; an `mcpServers` left empty goes with it, and a file left with no keys is deleted.
pub fn remove_mcp(base: &Path) -> Result<(PathBuf, McpChange), String> {
    let f = base.join(".mcp.json");
    let Some(mut m) = read_config(&f)? else { return Ok((f, McpChange::Absent)) };
    let Some(Value::Object(servers)) = m.get_mut("mcpServers") else { return Ok((f, McpChange::Absent)) };
    if servers.remove(NAME).is_none() {
        return Ok((f, McpChange::Absent));
    }
    if servers.is_empty() {
        m.remove("mcpServers");
    }
    if m.is_empty() {
        std::fs::remove_file(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        return Ok((f, McpChange::Deleted));
    }
    write_config(&f, m)?;
    Ok((f, McpChange::Removed))
}

/// The repo root for project installs (nearest ancestor with a specs folder; else `--root` or the cwd).
pub fn repo_root(root: Option<&Path>) -> Result<PathBuf, String> {
    spec11_core::scan::Project::open(root).map(|p| p.root).or_else(|_| root.map(Path::to_path_buf).map_or_else(|| std::env::current_dir().map_err(|e| e.to_string()), Ok))
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).ok_or_else(|| "HOME is not set".into())
}

fn skill_state(dir: &Path) -> &'static str {
    match std::fs::read_to_string(dir.join(NAME).join("SKILL.md")) {
        Ok(t) if t == SKILL => "current",
        Ok(_) => "outdated",
        Err(_) => "missing",
    }
}

/// `ok`: an http entry for this port; `other`: a spec11 entry that points elsewhere; `missing`.
fn mcp_state(config: &Path, port: u16) -> &'static str {
    let Ok(Some(m)) = read_config(config) else { return "missing" };
    match m.get("mcpServers").and_then(|s| s.get(NAME)) {
        None => "missing",
        Some(e) if e["url"] == format!("http://127.0.0.1:{port}/mcp") => "ok",
        Some(_) => "other",
    }
}

/// Where the skill and the MCP entry are installed: this repo (`.agents/skills`, `.mcp.json`) and the user's global skills
/// (`~/.agents/skills`; the user-scope MCP entry in `~/.claude.json` is only looked at, never written).
pub fn status(root: Option<&Path>, port: u16) -> Result<Value, String> {
    let base = repo_root(root)?;
    let pdir = base.join(".agents/skills");
    let gdir = home()?.join(".agents/skills");
    Ok(json!({
        "project": {"root": base, "skill": skill_state(&pdir), "skill_path": pdir.join(NAME).join("SKILL.md"), "mcp": mcp_state(&base.join(".mcp.json"), port), "mcp_path": base.join(".mcp.json")},
        "global": {"skill": skill_state(&gdir), "skill_path": gdir.join(NAME).join("SKILL.md"), "mcp": mcp_state(&home()?.join(".claude.json"), port)},
    }))
}

/// One line per thing a person should do after `status`; empty when the agent can already use spec11 here.
pub fn suggestions(st: &Value) -> Vec<String> {
    let (p, g) = (&st["project"], &st["global"]);
    fn skill(v: &Value) -> &str {
        v["skill"].as_str().unwrap_or("")
    }
    fn mcp(v: &Value) -> &str {
        v["mcp"].as_str().unwrap_or("")
    }
    let mut out = vec![];
    if skill(p) == "missing" && skill(g) == "missing" {
        out.push("skill not installed: `spec11 install` (this repo) or `spec11 install --global`".to_string());
    } else if skill(p) == "outdated" || (skill(p) == "missing" && skill(g) == "outdated") {
        out.push("installed skill is out of date: run `spec11 install`".to_string());
    }
    if mcp(p) != "ok" && mcp(g) != "ok" {
        out.push(if mcp(p) == "other" { "the spec11 entry in .mcp.json points at another port: `spec11 install --port N` fixes it" } else { "MCP entry missing from .mcp.json: `spec11 install`" }.to_string());
    }
    out
}

/// install (`port` = Some) or uninstall; returns one line per change, as the CLI prints them.
pub fn apply(root: Option<&Path>, global: bool, port: Option<u16>) -> Result<Vec<String>, String> {
    use McpChange::*;
    let dir = skills_dir(root, global)?;
    let mut out = vec![];
    match port {
        Some(_) => out.push(format!("skill  {}", install(&dir)?.display())),
        None => out.push(match uninstall(&dir)? {
            Some(f) => format!("skill  removed {}", f.display()),
            None => format!("skill  not installed in {}", dir.display()),
        }),
    }
    if global {
        out.push(format!("mcp    per repo: `spec11 {}` inside a repo handles its .mcp.json", if port.is_some() { "install" } else { "uninstall" }));
        return Ok(out);
    }
    let base = repo_root(root)?;
    let (f, change) = match port {
        Some(p) => add_mcp(&base, p)?,
        None => remove_mcp(&base)?,
    };
    let what = match change {
        Created => "created with spec11",
        Added => "added spec11",
        Updated => "updated spec11",
        Unchanged => "spec11 already set",
        Removed => "removed spec11",
        Deleted => "removed spec11, deleted the empty file",
        Absent => "no spec11 entry",
    };
    out.push(format!("mcp    {} ({what})", f.display()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("spec11-skill-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn embedded_skill_is_the_spec11_skill() {
        assert!(SKILL.starts_with("---\nname: spec11\n"));
        assert!(!SKILL.contains("python3") && !SKILL.contains(".py"), "no Python left in the skill");
    }

    #[doc = "[SETUP-01, SETUP-03]"]
    #[test]
    fn install_is_idempotent_and_uninstall_keeps_foreign_files() {
        let dir = tmp("a").join(".agents/skills");
        let f = install(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), SKILL);
        install(&dir).unwrap();
        std::fs::write(dir.join("spec11/notes.txt"), "mine").unwrap();
        assert_eq!(uninstall(&dir).unwrap(), Some(f.clone()));
        assert!(!f.exists() && dir.join("spec11/notes.txt").exists());
        assert_eq!(uninstall(&dir).unwrap(), None);
    }

    #[doc = "[SETUP-03]"]
    #[test]
    fn uninstall_removes_the_empty_folder() {
        let dir = tmp("b");
        install(&dir).unwrap();
        uninstall(&dir).unwrap();
        assert!(!dir.join("spec11").exists());
    }

    #[doc = "[SETUP-01]"]
    #[test]
    fn mcp_config_is_merged_never_clobbered() {
        let d = tmp("m");
        let (f, c) = add_mcp(&d, 7411).unwrap();
        assert!(matches!(c, McpChange::Created));
        // another server and an unrelated key, in this order
        std::fs::write(&f, r#"{"zeta": 1, "mcpServers": {"codegraph": {"type": "stdio", "command": "codegraph"}}}"#).unwrap();
        assert!(matches!(add_mcp(&d, 7411).unwrap().1, McpChange::Added));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["codegraph"]["command"], "codegraph");
        assert_eq!(v["mcpServers"]["spec11"]["url"], "http://127.0.0.1:7411/mcp");
        assert_eq!(v.as_object().unwrap().keys().next().unwrap(), "zeta", "key order kept");
        assert!(matches!(add_mcp(&d, 7411).unwrap().1, McpChange::Unchanged));
        assert!(matches!(add_mcp(&d, 7500).unwrap().1, McpChange::Updated));
        assert!(matches!(remove_mcp(&d).unwrap().1, McpChange::Removed));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert!(v["mcpServers"].get("spec11").is_none() && v["mcpServers"]["codegraph"].is_object() && v["zeta"] == 1);
        assert!(matches!(remove_mcp(&d).unwrap().1, McpChange::Absent));
    }

    #[doc = "[SETUP-02, SETUP-03]"]
    #[test]
    fn mcp_file_we_created_is_deleted_on_uninstall_and_bad_json_is_untouched() {
        let d = tmp("n");
        let (f, _) = add_mcp(&d, 7411).unwrap();
        assert!(matches!(remove_mcp(&d).unwrap().1, McpChange::Deleted) && !f.exists());
        std::fs::write(&f, "{ not json").unwrap();
        assert!(add_mcp(&d, 7411).is_err() && remove_mcp(&d).is_err());
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "{ not json");
    }

    #[test]
    fn status_and_suggestions_follow_install() {
        let repo = tmp("s");
        std::fs::create_dir_all(repo.join("specs")).unwrap();
        let st = status(Some(&repo), 7411).unwrap();
        assert_eq!(st["project"]["skill"], "missing");
        assert_eq!(st["project"]["mcp"], "missing");
        assert!(suggestions(&st).len() >= 1 || st["global"]["skill"] != "missing"); // a real ~/.agents may already hold it
        apply(Some(&repo), false, Some(7411)).unwrap();
        let st = status(Some(&repo), 7411).unwrap();
        assert_eq!((st["project"]["skill"].as_str(), st["project"]["mcp"].as_str()), (Some("current"), Some("ok")));
        assert!(suggestions(&st).is_empty(), "{:?}", suggestions(&st));
        assert_eq!(status(Some(&repo), 7500).unwrap()["project"]["mcp"], "other");
        std::fs::write(repo.join(".agents/skills/spec11/SKILL.md"), "old").unwrap();
        assert_eq!(status(Some(&repo), 7411).unwrap()["project"]["skill"], "outdated");
        apply(Some(&repo), false, None).unwrap();
        assert_eq!(status(Some(&repo), 7411).unwrap()["project"]["skill"], "missing");
    }

    #[test]
    fn project_dir_is_the_explicit_root() {
        let root = tmp("c");
        std::fs::create_dir_all(root.join("specs")).unwrap();
        assert_eq!(skills_dir(Some(&root), false).unwrap(), root.canonicalize().unwrap().join(".agents/skills"));
        // no specs folder yet: still installs where asked
        let bare = tmp("d");
        assert_eq!(skills_dir(Some(&bare), false).unwrap(), bare.join(".agents/skills"));
    }
}
