//! File scanning: spec IDs, test tags `[ABC-12]`, source comment mentions. Port of scripts/common.py (minus SQL).
use crate::config::Config;
use indexmap::IndexMap;
use regex::Regex;
use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, path::{Path, PathBuf}, process::Command, rc::Rc};
use walkdir::WalkDir;

const ID: &str = r"[A-Z]{2,5}-\d{1,3}";
const DONE: &str = r"[✅✔☑✳]\x{FE0F}?"; // check marks: a hint that the requirement is done

#[derive(Debug, Clone, Serialize)]
pub struct Spec {
    pub id: String,
    pub file: String,
    pub line: usize,
    pub end: usize,
    pub level: String,
    pub text: String,
    /// "" | "manual" | "later"
    pub mark: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ref {
    pub file: String,
    pub line: usize,
    pub text: String,
}

pub type Refs = HashMap<String, Vec<Ref>>;
pub type Diff = IndexMap<String, Vec<(i64, i64)>>;

pub struct Rx {
    spec: Regex,
    loose: Regex,
    block_end: Regex,
    tag: Regex,
    pub mention: Regex,
    mark: Regex,
    done: Regex,
    struck: Regex,
    later_inside: Regex,
    retired: Regex,
    pub decl: Regex,
    assert: Regex,
    off: Regex,
}

impl Rx {
    fn new(cfg: &Config) -> Result<Rx, String> {
        let r = |s: String| Regex::new(&s).map_err(|e| format!("bad regex {s}: {e}"));
        Ok(Rx {
            spec: r(format!(r"^\s*- (?:{DONE}\s*)?\*\*({ID})\*\*\s+(MUST|SHOULD|MAY)\b\s*(?:—\s*)?(.*)"))?,
            loose: r(format!(r"^\s*- (?:{DONE}\s*)?\*\*({ID})\*\*"))?,
            block_end: r(r"^\s*(- |#|\|)".into())?, // a wrapped bullet continues until a blank line, bullet, heading or table row
            tag: r(format!(r"\[({ID}(?:\s*[,;]\s*{ID})*)\]"))?, // `[ABC-12]`, `[ABC-12][ABC-14]` and `[ABC-12, ABC-14]`
            mention: r(format!(r"\b({ID})\b"))?,
            mark: r(r"\((later|deferred|manual)\)$".into())?, // `(deferred)` means the same as `(later)`
            done: r(DONE.into())?,
            struck: r(r"~~.*?~~".into())?,
            later_inside: r(r"\((?:later|deferred|manual)\)".into())?,
            retired: r(format!(r"^\s*- ~~\*\*({ID})\*\*"))?,
            decl: r(cfg.decl.clone())?,
            assert: r(cfg.assert.clone())?,
            off: r(cfg.off.clone())?,
        })
    }
}

pub struct Project {
    pub root: PathBuf,
    pub cfg: Config,
    pub rx: Rx,
    cache: RefCell<HashMap<String, Rc<Vec<String>>>>,
}

/// The repo is the nearest ancestor of `from` that has a specs folder.
pub fn find_root(from: &Path, specs: &str) -> Option<PathBuf> {
    from.ancestors().find(|d| d.join(specs).is_dir()).map(Path::to_path_buf)
}

impl Project {
    /// `root`: explicit repo root, else walk up from the current directory.
    pub fn open(root: Option<&Path>) -> Result<Project, String> {
        let start = match root {
            Some(r) => r.canonicalize().map_err(|e| format!("{}: {e}", r.display()))?,
            None => std::env::current_dir().map_err(|e| e.to_string())?,
        };
        // the config can move the specs folder, so look for spec11.json first
        let base = start.ancestors().find(|d| d.join("spec11.json").exists()).unwrap_or(&start);
        let cfg = Config::load(base)?;
        let root = if root.is_some() { start.clone() } else { find_root(&start, &cfg.specs).ok_or_else(|| format!("spec11: no {}/ folder in {} or above", cfg.specs, start.display()))? };
        if !root.join(&cfg.specs).is_dir() {
            return Err(format!("spec11: no {}/ folder in {}", cfg.specs, root.display()));
        }
        let cfg = Config::load(&root)?;
        let rx = Rx::new(&cfg)?;
        Ok(Project { root, cfg, rx, cache: RefCell::default() })
    }

    pub fn lines(&self, rel: &str) -> Rc<Vec<String>> {
        if let Some(l) = self.cache.borrow().get(rel) {
            return l.clone();
        }
        let text = std::fs::read(self.root.join(rel)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let l = Rc::new(text.lines().map(str::to_owned).collect::<Vec<_>>());
        self.cache.borrow_mut().insert(rel.to_owned(), l.clone());
        l
    }

    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.root).unwrap_or(p).to_string_lossy().replace('\\', "/")
    }

    /// Files with a configured extension under each dir, sorted per dir.
    pub fn files(&self, dirs: &[String]) -> Vec<String> {
        let mut out = vec![];
        for d in dirs {
            let mut fs: Vec<PathBuf> = WalkDir::new(self.root.join(d))
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file() && e.path().extension().and_then(|x| x.to_str()).is_some_and(|x| self.cfg.ext.iter().any(|y| y == x)))
                .map(|e| e.into_path())
                .collect();
            fs.sort();
            out.extend(fs.iter().map(|p| self.rel(p)));
        }
        out
    }

    fn spec_files(&self) -> Vec<String> {
        let mut fs: Vec<String> = std::fs::read_dir(self.root.join(&self.cfg.specs))
            .map(|rd| rd.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".md")).collect())
            .unwrap_or_default();
        fs.sort();
        fs
    }

    /// {id: Spec} from `- **ABC-12** MUST — text` bullets, wrapped lines joined. A bullet ending in `(manual)`, `(later)` or
    /// `(deferred)` carries mark "manual" or "later"; struck-out bullets (`- ~~**ABC-12**`) are retired.
    /// Spec mistakes found on the way (duplicate ID, no level, marker not at the end) go to `problems`.
    pub fn specs(&self, problems: &mut Vec<String>) -> IndexMap<String, Spec> {
        let mut found: IndexMap<String, Spec> = IndexMap::new();
        for f in self.spec_files() {
            let ls = self.lines(&format!("{}/{f}", self.cfg.specs));
            for (idx, line) in ls.iter().enumerate() {
                let n = idx + 1;
                let Some(m) = self.rx.spec.captures(line) else {
                    if let Some(l) = self.rx.loose.captures(line) {
                        problems.push(format!("{} {f}:{n} has no MUST/SHOULD/MAY, so it is not counted", &l[1]));
                    }
                    continue;
                };
                let id = m[1].to_string();
                let m1 = m.get(1).unwrap();
                let mut parts = vec![m[3].trim().to_string()];
                let mut j = n;
                while j < ls.len() && !ls[j].trim().is_empty() && !self.rx.block_end.is_match(&ls[j]) {
                    parts.push(ls[j].trim().to_string());
                    j += 1;
                }
                let text = parts.join(" ").trim().to_string();
                let mut seg = vec![line[m1.end()..].to_string()];
                seg.extend(parts[1..].iter().cloned());
                let hay = format!("{}{}", &line[..m1.start()], seg.join(" "));
                let done = self.rx.done.is_match(&self.rx.struck.replace_all(&hay, ""));
                let end = self.rx.mark.captures(&text);
                let mark = match &end {
                    None => "",
                    Some(e) if &e[1] == "manual" => "manual",
                    Some(_) => "later",
                };
                if let Some(was) = found.get(&id) {
                    problems.push(format!("{id} is defined twice: {}:{} and {f}:{n}", was.file, was.line));
                } else {
                    let before = match &end {
                        Some(e) => &text[..e.get(0).unwrap().start()],
                        None => &text[..],
                    };
                    if self.rx.later_inside.is_match(&self.rx.struck.replace_all(before, "")) {
                        problems.push(format!("{id} {f}:{n} has (later)/(deferred)/(manual) before the end, so it counts as due"));
                    }
                }
                if done && !mark.is_empty() {
                    problems.push(format!("{id} {f}:{n} has a check mark but is also marked ({}/deferred)", if mark == "manual" { "manual" } else { "later" }));
                }
                found.insert(id.clone(), Spec { id, file: f.clone(), line: n, end: j, level: m[2].to_string(), text, mark: mark.into(), done });
            }
        }
        found
    }

    /// The requirement IDs of every `[ID]` or `[ID, ID]` tag on `line`, in order.
    pub fn tagged(&self, line: &str) -> Vec<String> {
        self.rx.tag.captures_iter(line).flat_map(|g| self.rx.mention.captures_iter(&g[1]).map(|m| m[1].to_string()).collect::<Vec<_>>()).collect()
    }

    /// {id: [ref]} for every `[ID]` or `[ID, ID]` tag in the test folders.
    pub fn tests(&self) -> Refs {
        let mut found = Refs::new();
        for rel in self.files(&self.cfg.tests) {
            for (i, line) in self.lines(&rel).iter().enumerate() {
                for id in self.tagged(line) {
                    found.entry(id).or_default().push(Ref { file: rel.clone(), line: i + 1, text: line.trim().to_string() });
                }
            }
        }
        found
    }

    /// The text after the first line-comment marker of `line`.
    fn comment_of<'a>(&self, line: &'a str) -> Option<&'a str> {
        self.cfg.comment.iter().filter_map(|c| line.find(c.as_str()).map(|p| (p, c.len()))).min().map(|(p, l)| &line[p + l..])
    }

    pub fn is_comment(&self, text: &str) -> bool {
        self.cfg.comment.iter().any(|c| text.starts_with(c.as_str()))
    }

    /// {id: [ref]} for IDs mentioned after a line-comment marker in app sources (block comments are not scanned).
    pub fn sources(&self) -> Refs {
        let mut found = Refs::new();
        for rel in self.files(&self.cfg.src) {
            for (i, line) in self.lines(&rel).iter().enumerate() {
                if let Some(c) = self.comment_of(line) {
                    for m in self.rx.mention.captures_iter(c) {
                        found.entry(m[1].to_string()).or_default().push(Ref { file: rel.clone(), line: i + 1, text: line.trim().to_string() });
                    }
                }
            }
        }
        found
    }

    /// An issue for every requirement that cites an ID no spec defines (retired IDs live in specs/README.md, not in bullets).
    pub fn check_refs(&self, s: &IndexMap<String, Spec>, issues: &mut Vec<String>) {
        let known = self.known_prefixes(s);
        for (i, spec) in s {
            let mut refs: Vec<String> = self.rx.mention.captures_iter(&spec.text).map(|m| m[1].to_string()).filter(|r| !s.contains_key(r)).collect();
            refs.sort();
            refs.dedup();
            for r in refs {
                if known.contains(&prefix(&r)) {
                    issues.push(format!("{i} {}:{} cites {r}, which no spec defines", spec.file, spec.line));
                }
            }
        }
    }

    fn known_prefixes(&self, s: &IndexMap<String, Spec>) -> Vec<String> {
        s.keys().map(|i| prefix(i)).chain(self.cfg.retired_prefixes.iter().cloned()).collect()
    }

    /// IDs a test or source cites that no spec defines, when the prefix is a spec prefix or a retired one (typos, retired IDs).
    pub fn dangling(&self, s: &IndexMap<String, Spec>, cited: &Refs) -> Vec<String> {
        let known = self.known_prefixes(s);
        let mut v: Vec<String> = cited.keys().filter(|i| !s.contains_key(*i) && known.contains(&prefix(i))).cloned().collect();
        v.sort();
        v
    }

    /// IDs whose bullet is struck through (`- ~~**ABC-12** …~~`): retired, never reassigned.
    pub fn retired(&self) -> Vec<String> {
        let mut out = vec![];
        for f in self.spec_files() {
            for line in self.lines(&format!("{}/{f}", self.cfg.specs)).iter() {
                if let Some(m) = self.rx.retired.captures(line) {
                    out.push(m[1].to_string());
                }
            }
        }
        out
    }

    /// Tagged tests that prove nothing: kind -> [(id, file, line)], in a fixed kind order.
    /// unknown = ID in no spec; retired = struck-through spec; comment-only = every tag of the ID sits in a comment;
    /// disabled = every test is switched off; no-assert = no tagged test body has a direct assertion (it may assert through a helper: verify).
    pub fn test_problems(&self, s: &IndexMap<String, Spec>, t: &Refs) -> Vec<(&'static str, Vec<(String, String, usize)>)> {
        let gone = self.retired();
        let mut out: Vec<(&'static str, Vec<(String, String, usize)>)> = ["unknown", "retired", "comment-only", "disabled", "no-assert"].into_iter().map(|k| (k, vec![])).collect();
        let slot = |out: &mut Vec<(&'static str, Vec<_>)>, k: &str, v| out.iter_mut().find(|(n, _)| *n == k).unwrap().1.push(v);
        let mut ids: Vec<&String> = t.keys().collect();
        ids.sort();
        for i in ids {
            let refs = &t[i];
            let first = (i.clone(), refs[0].file.clone(), refs[0].line);
            if gone.contains(i) {
                slot(&mut out, "retired", first);
            } else if !s.contains_key(i) {
                slot(&mut out, "unknown", first);
            } else {
                for kind in ["comment-only", "disabled", "no-assert"] {
                    if refs.iter().all(|r| self.flag(r, kind)) {
                        slot(&mut out, kind, first.clone());
                    }
                }
            }
        }
        out
    }

    fn flag(&self, r: &Ref, kind: &str) -> bool {
        let ls = self.lines(&r.file);
        match kind {
            "comment-only" => self.is_comment(&r.text),
            "disabled" => {
                // the attribute: the tag line up to the `func` line
                let mut head = vec![];
                for x in ls.iter().skip(r.line - 1).take(4) {
                    head.push(x);
                    if x.contains("func ") {
                        break;
                    }
                }
                !self.is_comment(&r.text) && head.iter().any(|x| self.rx.off.is_match(x))
            }
            _ => !self.is_comment(&r.text) && !body(&ls, r.line, 200).iter().any(|x| self.rx.assert.is_match(x)),
        }
    }

    /// Name of the declaration a comment at 1-based line `n` belongs to: the next one for a `///` doc comment, else the nearest above.
    pub fn enclosing(&self, ls: &[String], n: usize) -> String {
        let hit = |i: usize| self.rx.decl.captures(&ls[i]).map(|m| m[1].to_string());
        if ls[n - 1].trim_start().starts_with("///") {
            (n..(n + 8).min(ls.len())).find_map(hit)
        } else {
            (0..n).rev().find_map(hit)
        }
        .unwrap_or_default()
    }

    /// Files changed in the working tree since `reference` (staged, unstaged, untracked), with their changed line ranges.
    pub fn changed(&self, reference: &str) -> Result<Diff, String> {
        let git = |args: &[&str]| -> Result<String, String> {
            let r = Command::new("git").args(args).current_dir(&self.root).output().map_err(|e| format!("git: {e}"))?;
            if !r.status.success() {
                return Err(format!("spec11: git {} failed: {}", args.join(" "), String::from_utf8_lossy(&r.stderr).trim()));
            }
            Ok(String::from_utf8_lossy(&r.stdout).into_owned())
        };
        let mut out = parse_diff(&git(&["diff", "-U0", "--no-color", "--no-ext-diff", reference, "--"])?);
        for p in git(&["ls-files", "--others", "--exclude-standard"])?.lines() {
            out.insert(p.to_string(), vec![(1, 1_000_000_000)]);
        }
        Ok(out)
    }
}

pub fn prefix(id: &str) -> String {
    id.split('-').next().unwrap_or("").to_string()
}

/// Sort key `(prefix, number)`.
pub fn id_key(id: &str) -> (String, u32) {
    let (p, n) = id.rsplit_once('-').unwrap_or((id, "0"));
    (p.to_string(), n.parse().unwrap_or(0))
}

pub fn sorted_ids<'a>(ids: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut v: Vec<String> = ids.into_iter().cloned().collect();
    v.sort_by_key(|i| id_key(i));
    v
}

/// Lines of the function a 1-based tagged line `n` belongs to: from the tag to the brace that closes it.
pub fn body(ls: &[String], n: usize, cap: usize) -> Vec<String> {
    let (mut out, mut depth, mut opened) = (vec![], 0i32, false);
    for line in ls.iter().skip(n - 1).take(cap) {
        out.push(line.clone());
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        opened = opened || line.contains('{');
        if depth < 0 || (opened && depth == 0) {
            break;
        }
    }
    out
}

/// DOC-01,DOC-02,DOC-03,DOC-07 -> DOC-01..03,DOC-07
pub fn ranges<S: AsRef<str>>(ids: impl IntoIterator<Item = S>) -> String {
    let mut by: Vec<(String, Vec<u32>)> = vec![];
    for i in ids {
        let (p, n) = i.as_ref().rsplit_once('-').unwrap_or((i.as_ref(), "0"));
        let n: u32 = n.parse().unwrap_or(0);
        match by.iter_mut().find(|(q, _)| q == p) {
            Some((_, v)) => v.push(n),
            None => by.push((p.to_string(), vec![n])),
        }
    }
    let mut out = vec![];
    for (p, mut ns) in by {
        ns.sort();
        let mut run = vec![ns[0]];
        for n in ns[1..].iter().map(|n| Some(*n)).chain([None]) {
            if let Some(n) = n && n == *run.last().unwrap() + 1 {
                run.push(n);
                continue;
            }
            out.push(if run.len() == 1 { format!("{p}-{:02}", run[0]) } else { format!("{p}-{:02}..{:02}", run[0], run.last().unwrap()) });
            run = n.into_iter().collect();
        }
    }
    out.join(",")
}

/// {path: [(first, last)]} new-file line ranges from `git diff -U0`; a pure deletion marks the line it sat before.
pub fn parse_diff(diff: &str) -> Diff {
    let hunk = Regex::new(r"@@ -\S+ \+(\d+)(?:,(\d+))? @@").unwrap();
    let (mut out, mut path): (Diff, Option<String>) = (IndexMap::new(), None);
    for line in diff.lines() {
        if line.starts_with("+++ ") {
            path = line.strip_prefix("+++ b/").map(str::to_owned);
        } else if line.starts_with("@@") && let Some(p) = &path && let Some(m) = hunk.captures(line) {
            let start: i64 = m[1].parse().unwrap();
            let count: i64 = m.get(2).map_or(1, |c| c.as_str().parse().unwrap());
            out.entry(p.clone()).or_default().push((start, start + count.max(1) - 1));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_join_consecutive() {
        assert_eq!(ranges(["DOC-01", "DOC-02", "DOC-03", "DOC-07", "SEC-4"]), "DOC-01..03,DOC-07,SEC-04");
    }

    #[test]
    fn diff_ranges() {
        let d = parse_diff("+++ b/specs/a.md\n@@ -1,0 +2,3 @@\n@@ -9 +11 @@\n+++ /dev/null\n@@ -1 +1 @@\n");
        assert_eq!(d["specs/a.md"], vec![(2, 4), (11, 11)]);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn body_stops_at_closing_brace() {
        let ls: Vec<String> = ["@Test(\"[A-1]\")", "func f() {", "  x()", "}", "other"].iter().map(|s| s.to_string()).collect();
        assert_eq!(body(&ls, 1, 200).len(), 4);
    }
}
