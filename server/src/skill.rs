//! `spec11 install` / `uninstall`: the skill ships inside the binary and is written to `<base>/skills/spec11/SKILL.md`.
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

const NAME: &str = "spec11";
const CODEGRAPH: &str = "codegraph";
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
    add_server(base, NAME, json!({"type": "http", "url": format!("http://127.0.0.1:{port}/mcp")}), true)
}

/// Adds the `codegraph` entry (SETUP-10), as `codegraph install --print-config claude` prints it. One that is already there is the user's: never replaced.
pub fn add_codegraph(base: &Path) -> Result<(PathBuf, McpChange), String> {
    add_server(base, CODEGRAPH, json!({"type": "stdio", "command": "codegraph", "args": ["serve", "--mcp"], "alwaysLoad": true}), false)
}

fn add_server(base: &Path, name: &str, entry: Value, replace: bool) -> Result<(PathBuf, McpChange), String> {
    let f = base.join(".mcp.json");
    let existing = read_config(&f)?;
    let created = existing.is_none();
    let mut m = existing.unwrap_or_default();
    let servers = m.entry("mcpServers").or_insert_with(|| json!({}));
    let servers = servers.as_object_mut().ok_or_else(|| format!("{}: mcpServers is not an object, left untouched", f.display()))?;
    let change = match servers.get(name) {
        Some(v) if *v == entry || !replace => McpChange::Unchanged,
        Some(_) => McpChange::Updated,
        None if created => McpChange::Created,
        None => McpChange::Added,
    };
    if !matches!(change, McpChange::Unchanged) {
        servers.insert(name.into(), entry);
        write_config(&f, m)?;
    }
    Ok((f, change))
}

// (SETUP-03)
/// Removes `mcpServers.spec11`; an `mcpServers` left empty goes with it, and a file left with no keys is deleted.
pub fn remove_mcp(base: &Path) -> Result<(PathBuf, McpChange), String> {
    remove_server(base, NAME)
}

/// Removes the `codegraph` entry (SETUP-10), whatever it holds: the user asked. The rest follows `remove_mcp`.
pub fn remove_codegraph(base: &Path) -> Result<(PathBuf, McpChange), String> {
    remove_server(base, CODEGRAPH)
}

fn remove_server(base: &Path, name: &str) -> Result<(PathBuf, McpChange), String> {
    let f = base.join(".mcp.json");
    let Some(mut m) = read_config(&f)? else { return Ok((f, McpChange::Absent)) };
    let Some(Value::Object(servers)) = m.get_mut("mcpServers") else { return Ok((f, McpChange::Absent)) };
    if servers.remove(name).is_none() {
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

/// The repo root for project installs (`--root`, else the nearest ancestor with specs or a git work tree; else the cwd).
pub fn repo_root(root: Option<&Path>) -> Result<PathBuf, String> {
    spec11_core::scan::Project::open(root).map(|p| p.root).or_else(|_| root.map(Path::to_path_buf).map_or_else(|| std::env::current_dir().map_err(|e| e.to_string()), Ok))
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).ok_or_else(|| "HOME is not set".into())
}

#[derive(Clone, Copy, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SkillState {
    Current,
    Outdated,
    Missing,
}

/// `Ok`: an http entry for this port; `Other`: a spec11 entry that points elsewhere. For the `codegraph` entry only `Ok` and `Missing` apply.
#[derive(Clone, Copy, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum McpState {
    Ok,
    Other,
    Missing,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProjectInstall {
    pub root: PathBuf,
    pub skill: SkillState,
    pub skill_path: PathBuf,
    pub mcp: McpState,
    pub mcp_path: PathBuf,
    /// Whether `.mcp.json` has a `codegraph` server (SETUP-10).
    pub codegraph_mcp: McpState,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct GlobalInstall {
    pub skill: SkillState,
    pub skill_path: PathBuf,
    pub mcp: McpState,
    pub codegraph_mcp: McpState,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct InstallStatus {
    pub project: ProjectInstall,
    pub global: GlobalInstall,
}

fn skill_state(dir: &Path) -> SkillState {
    match std::fs::read_to_string(dir.join(NAME).join("SKILL.md")) {
        Ok(t) if t == SKILL => SkillState::Current,
        Ok(_) => SkillState::Outdated,
        Err(_) => SkillState::Missing,
    }
}

fn mcp_state(config: &Path, port: u16) -> McpState {
    let Ok(Some(m)) = read_config(config) else { return McpState::Missing };
    match m.get("mcpServers").and_then(|s| s.get(NAME)) {
        None => McpState::Missing,
        Some(e) if e["url"] == format!("http://127.0.0.1:{port}/mcp") => McpState::Ok,
        Some(_) => McpState::Other,
    }
}

fn codegraph_state(config: &Path) -> McpState {
    match read_config(config) {
        Ok(Some(m)) if m.get("mcpServers").is_some_and(|s| s.get(CODEGRAPH).is_some()) => McpState::Ok,
        _ => McpState::Missing,
    }
}

/// Where the skill and the MCP entry are installed: this repo (`.agents/skills`, `.mcp.json`) and the user's global skills
/// (`~/.agents/skills`; the user-scope MCP entry in `~/.claude.json` is only looked at, never written).
pub fn status(root: Option<&Path>, port: u16) -> Result<InstallStatus, String> {
    let base = repo_root(root)?;
    let pdir = base.join(".agents/skills");
    let gdir = home()?.join(".agents/skills");
    let (pmcp, gmcp) = (base.join(".mcp.json"), home()?.join(".claude.json"));
    Ok(InstallStatus {
        project: ProjectInstall { skill: skill_state(&pdir), skill_path: pdir.join(NAME).join("SKILL.md"), mcp: mcp_state(&pmcp, port), codegraph_mcp: codegraph_state(&pmcp), mcp_path: pmcp, root: base },
        global: GlobalInstall { skill: skill_state(&gdir), skill_path: gdir.join(NAME).join("SKILL.md"), mcp: mcp_state(&gmcp, port), codegraph_mcp: codegraph_state(&gmcp) },
    })
}

/// One line per thing a person should do after `status`; empty when the agent can already use spec11 here.
pub fn suggestions(st: &InstallStatus) -> Vec<String> {
    use {McpState as M, SkillState as S};
    let (p, g) = (&st.project, &st.global);
    let mut out = vec![];
    if p.skill == S::Missing && g.skill == S::Missing {
        out.push("skill not installed: `spec11 install` (this repo) or `spec11 install --global`".to_string());
    } else if p.skill == S::Outdated || (p.skill == S::Missing && g.skill == S::Outdated) {
        out.push("installed skill is out of date: run `spec11 install`".to_string());
    }
    if p.mcp != M::Ok && g.mcp != M::Ok {
        out.push(if p.mcp == M::Other { "the spec11 entry in .mcp.json points at another port: `spec11 install --port N` fixes it" } else { "MCP entry missing from .mcp.json: `spec11 install`" }.to_string());
    }
    out
}

/// `install` (`add`) or `uninstall` with `server=codegraph` (SETUP-10): only the CodeGraph entry of the repo's `.mcp.json`; the skill and the spec11 entry are left alone.
pub fn apply_codegraph(root: Option<&Path>, add: bool) -> Result<Vec<String>, String> {
    let base = repo_root(root)?;
    let (f, change) = if add { add_codegraph(&base)? } else { remove_codegraph(&base)? };
    let what = match change {
        McpChange::Created => "created with codegraph",
        McpChange::Added => "added codegraph",
        McpChange::Removed => "removed codegraph",
        McpChange::Deleted => "removed codegraph, deleted the empty file",
        McpChange::Absent => "no codegraph entry",
        _ => "codegraph already set, kept as it is",
    };
    Ok(vec![format!("mcp    {} ({what})", f.display())])
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

    #[doc = "[SETUP-01]"]
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

    #[doc = "[SETUP-01, SETUP-03]"]
    #[test]
    fn status_and_suggestions_follow_install() {
        let repo = tmp("s");
        std::fs::create_dir_all(repo.join("specs")).unwrap();
        let st = status(Some(&repo), 7411).unwrap();
        assert!(st.project.skill == SkillState::Missing && st.project.mcp == McpState::Missing);
        assert!(suggestions(&st).len() >= 1 || st.global.skill != SkillState::Missing); // a real ~/.agents may already hold it
        apply(Some(&repo), false, Some(7411)).unwrap();
        let st = status(Some(&repo), 7411).unwrap();
        assert!(st.project.skill == SkillState::Current && st.project.mcp == McpState::Ok);
        assert!(suggestions(&st).is_empty(), "{:?}", suggestions(&st));
        assert!(status(Some(&repo), 7500).unwrap().project.mcp == McpState::Other);
        std::fs::write(repo.join(".agents/skills/spec11/SKILL.md"), "old").unwrap();
        assert!(status(Some(&repo), 7411).unwrap().project.skill == SkillState::Outdated);
        apply(Some(&repo), false, None).unwrap();
        assert!(status(Some(&repo), 7411).unwrap().project.skill == SkillState::Missing);
    }

    #[doc = "[SETUP-01]"]
    #[test]
    fn project_dir_is_the_explicit_root() {
        let root = tmp("c");
        std::fs::create_dir_all(root.join("specs")).unwrap();
        assert_eq!(skills_dir(Some(&root), false).unwrap(), root.canonicalize().unwrap().join(".agents/skills"));
        // no specs folder yet (SCAN-28): still installs where asked
        let bare = tmp("d");
        assert_eq!(skills_dir(Some(&bare), false).unwrap(), bare.canonicalize().unwrap().join(".agents/skills"));
    }

    #[doc = "[SETUP-10]"]
    #[test]
    fn the_codegraph_entry_is_added_beside_the_others_and_never_replaced() {
        let d = tmp("g");
        let f = d.join(".mcp.json");
        let (_, c) = add_codegraph(&d).unwrap();
        assert!(matches!(c, McpChange::Created));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["codegraph"], json!({"type": "stdio", "command": "codegraph", "args": ["serve", "--mcp"], "alwaysLoad": true}));
        assert!(status(Some(&d), 7411).unwrap().project.codegraph_mcp == McpState::Ok);
        // a customised entry and other servers stay as they are
        std::fs::write(&f, r#"{"mcpServers": {"codegraph": {"command": "mine", "env": {"A": "1"}}, "other": {"command": "x"}}}"#).unwrap();
        assert!(matches!(add_codegraph(&d).unwrap().1, McpChange::Unchanged));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!((v["mcpServers"]["codegraph"]["command"].as_str(), v["mcpServers"]["other"]["command"].as_str()), (Some("mine"), Some("x")));
        // the spec11 entry is added by its own install, and does not touch codegraph
        add_mcp(&d, 7411).unwrap();
        assert!(status(Some(&d), 7411).unwrap().project.mcp == McpState::Ok);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["codegraph"]["command"], "mine");
        // invalid JSON is left alone (SETUP-02)
        std::fs::write(&f, "{ nope").unwrap();
        assert!(add_codegraph(&d).is_err() && std::fs::read_to_string(&f).unwrap() == "{ nope");
        assert!(status(Some(&d), 7411).unwrap().project.codegraph_mcp == McpState::Missing);
        assert!(remove_codegraph(&d).is_err() && std::fs::read_to_string(&f).unwrap() == "{ nope");
    }

    #[doc = "[SETUP-10]"]
    #[test]
    fn the_codegraph_entry_is_removed_alone_customised_or_not() {
        let d = tmp("h");
        let f = d.join(".mcp.json");
        assert!(matches!(remove_codegraph(&d).unwrap().1, McpChange::Absent)); // no file, nothing to do
        std::fs::write(&f, r#"{"mcpServers": {"codegraph": {"command": "mine", "env": {"A": "1"}}, "spec11": {"type": "http", "url": "http://127.0.0.1:7411/mcp"}, "other": {"command": "x"}}, "keep": 1}"#).unwrap();
        assert!(matches!(remove_codegraph(&d).unwrap().1, McpChange::Removed));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert!(v["mcpServers"].get("codegraph").is_none() && v["mcpServers"]["other"]["command"] == "x" && v["keep"] == 1);
        assert!(status(Some(&d), 7411).unwrap().project.mcp == McpState::Ok && status(Some(&d), 7411).unwrap().project.codegraph_mcp == McpState::Missing);
        assert!(matches!(remove_codegraph(&d).unwrap().1, McpChange::Absent)); // twice is fine
        // a file that held only this entry goes with it, like spec11's own (SETUP-03)
        std::fs::remove_file(&f).unwrap();
        add_codegraph(&d).unwrap();
        assert!(matches!(remove_codegraph(&d).unwrap().1, McpChange::Deleted) && !f.exists());
    }
}
