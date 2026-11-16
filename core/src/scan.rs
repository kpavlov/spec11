//! File scanning: spec IDs, test tags `[ABC-12]`, source comment mentions. Port of scripts/common.py (minus SQL).
use crate::codegraph::Graph;
use crate::config::Config;
use crate::detect;
use indexmap::IndexMap;
use regex::Regex;
use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, path::{Path, PathBuf}, process::Command, rc::Rc};

const ID: &str = r"[A-Z]{2,5}-\d{1,4}";
pub const LEVEL: &str = r"\b(?:MUST|SHOULD|MAY|SHALL)\b";
/// Start of a requirement line: a bullet (`-`, `*`, `+`, `1.`) or table row, an optional check mark, then the ID, plain, bold or in
/// backticks, with an optional `:`. Group 1 is the ID.
pub fn head() -> String {
    format!(r"^\s*(?:(?:[-*+]|\d+[.)])\s+|\|\s*)(?:{DONE}\s*)?[*_`]*({ID})\b[*_`]*[:.]?[*_`]*")
}
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
    head: Regex,
    level: Regex,
    link: Regex,
    adr: Regex,
    code_span: Regex,
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
    test: Option<Regex>,
}

impl Rx {
    fn new(cfg: &Config) -> Result<Rx, String> {
        let r = |s: String| Regex::new(&s).map_err(|e| format!("bad regex {s}: {e}"));
        Ok(Rx {
            head: r(format!("{}(.*)", head()))?,
            level: r(LEVEL.into())?,
            link: r(format!(r"\[({ID})\]\(([^)#\s]+)(?:#[^)]*)?\)"))?, // `[ADR-0003](../decisions/0003-x.md#why)`
            adr: r(r"^(\d{1,4})-".into())?,
            code_span: r(r"`[^`]*`".into())?,
            block_end: r(r"^\s*([-*+] |\d+[.)] |#|\|)".into())?, // a wrapped bullet continues until a blank line, bullet, heading or table row
            tag: r(format!(r"\[({ID}(?:\s*[,;]\s*{ID})*)\]"))?, // `[ABC-12]`, `[ABC-12][ABC-14]` and `[ABC-12, ABC-14]`
            mention: r(format!(r"\b({ID})\b"))?,
            mark: r(r"\((later|deferred|manual)\)$".into())?, // `(deferred)` means the same as `(later)`
            done: r(DONE.into())?,
            struck: r(r"~~.*?~~".into())?,
            later_inside: r(r"\((?:later|deferred|manual)\)".into())?,
            retired: r(format!(r"^\s*(?:[-*+]|\d+[.)])\s+~~[*_`]*({ID})"))?,
            decl: r(cfg.decl.clone())?,
            assert: r(cfg.assert.clone())?,
            off: r(cfg.off.clone())?,
            test: if cfg.test_files.is_empty() { None } else { Some(r(cfg.test_files.clone())?) },
        })
    }
}

/// The text of a requirement's first line: what follows the ID, minus separators and a level that opens it.
fn lead(rest: &str, level: &str) -> String {
    let sep = |c: char| c.is_whitespace() || "|:—–-".contains(c);
    let rest = rest.trim_start_matches(sep);
    rest.strip_prefix(level).unwrap_or(rest).trim_start_matches(sep).trim_end_matches(|c: char| c.is_whitespace() || c == '|').to_string()
}

pub struct Project {
    pub root: PathBuf,
    pub cfg: Config,
    pub rx: Rx,
    /// Where a symlink may point: the root and `ALLOWED_PATHS`.
    allow: Vec<PathBuf>,
    /// Folders and files (repo-relative) that hold requirement lines, and decision-record folders.
    spec_paths: Vec<String>,
    doc_dirs: Vec<String>,
    /// Repo-relative files of the repo: the CodeGraph file list, else a walk.
    pub files: Vec<String>,
    ignore: detect::Ignore,
    cache: RefCell<HashMap<String, Rc<Vec<String>>>>,
}

/// The repo is the nearest ancestor of `from` that has the `specs` path, or, when none is configured, any known spec place;
/// failing that the nearest git work tree.
pub fn find_root(from: &Path, specs: &str) -> Option<PathBuf> {
    from.ancestors().find(|d| if specs.is_empty() { detect::has_spec_marker(d, &detect::allow_list(d)) } else { d.join(specs).exists() })
        .or_else(|| from.ancestors().find(|d| d.join(".git").exists()))
        .map(Path::to_path_buf)
}

/// `from`'s folder joined with a link target, `..` resolved; `None` when it leaves the repo.
fn resolve(from: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    if target.starts_with('/') {
        parts.clear();
    }
    for c in target.split('/') {
        match c {
            "" | "." => {}
            ".." => { parts.pop()?; }
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
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
        let hint = |at: &str| format!("spec11: no specs (specs/, spec/, docs/specs/, SPEC.md, decisions/ ... or markdown with requirement lines) in {at}; set \"specs\" in spec11.json");
        let root = if root.is_some() { start.clone() } else { find_root(&start, &cfg.specs).ok_or_else(|| hint(&format!("{} or above", start.display())))? };
        let mut cfg = Config::load(&root)?;
        let allow = detect::allow_list(&root);
        let doc_dirs = detect::decision_dirs(&root);
        // (SCAN-12) `specs` may name a folder or a file; left empty, the places come from the repo
        let spec_paths = if cfg.specs.is_empty() {
            let v = detect::spec_paths(&root, &allow);
            if v.is_empty() && doc_dirs.is_empty() {
                return Err(hint(&root.display().to_string()));
            }
            cfg.specs = v.first().cloned().unwrap_or_default(); // the main one, as spec11.json would name it
            v
        } else if root.join(&cfg.specs).exists() {
            vec![cfg.specs.clone()]
        } else {
            return Err(format!("spec11: no {} in {}", cfg.specs, root.display()));
        };
        // (SCAN-09) what spec11.json leaves out comes from the CodeGraph file list, else from a walk of the repo
        let indexed = Graph::open(&root.join(if cfg.codegraph.is_empty() { ".codegraph/codegraph.db" } else { &cfg.codegraph })).ok().flatten().and_then(|g| g.file_paths().ok());
        let ignore = detect::Ignore::load(&root);
        let files: Vec<String> = indexed.map(|v| v.into_iter().filter(|f| !ignore.hides(f)).collect()).unwrap_or_else(|| detect::walk(&root, &allow).iter().filter_map(|f| f.strip_prefix(&root).ok()).map(|f| f.to_string_lossy().replace('\\', "/")).collect());
        cfg.detect(&files);
        let rx = Rx::new(&cfg)?;
        Ok(Project { root, cfg, rx, allow, spec_paths, doc_dirs, files, ignore, cache: RefCell::default() })
    }

    /// Whether `.aiignore`, `.spec11ignore` or the built-in `.agents` rule hides this repo-relative path.
    pub fn ignored(&self, rel: &str) -> bool {
        self.ignore.hides(rel)
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

    /// Files with a configured extension under each dir, sorted per dir. Symlinks count only inside the root or `ALLOWED_PATHS`.
    pub fn files(&self, dirs: &[String]) -> Vec<String> {
        let ext = |p: &PathBuf| p.extension().and_then(|x| x.to_str()).is_some_and(|x| self.cfg.ext.iter().any(|y| y == x));
        dirs.iter().flat_map(|d| detect::walk(&if d == "." { self.root.clone() } else { self.root.join(d) }, &self.allow).into_iter().filter(ext).map(|p| self.rel(&p)).collect::<Vec<_>>()).collect()
    }

    fn is_test(&self, rel: &str) -> bool {
        self.rx.test.as_ref().is_some_and(|r| r.is_match(rel))
    }

    /// A file in a tests folder, or one that `test_files` says is a test.
    pub fn is_test_path(&self, rel: &str) -> bool {
        self.is_test(rel) || self.cfg.tests.iter().any(|d| d == "." || rel.starts_with(&format!("{d}/")))
    }

    /// A file that exists to hold tests: test-named, or in a tests folder that is not also a source folder (Rust keeps unit tests
    /// inline in `src`, so those files are source that happens to declare tests).
    pub fn is_test_only(&self, rel: &str) -> bool {
        let in_src = self.cfg.src.iter().any(|d| d != "." && rel.starts_with(&format!("{d}/")));
        self.is_test(rel) || (self.is_test_path(rel) && !in_src)
    }

    /// The test files: those under `tests`, plus test-named files that sit in `src` folders.
    pub fn test_files(&self) -> Vec<String> {
        let mut v = self.files(&self.cfg.tests);
        v.extend(self.files(&self.cfg.src).into_iter().filter(|f| self.is_test(f)));
        let mut seen = std::collections::HashSet::new();
        v.retain(|f| seen.insert(f.clone()));
        v
    }

    /// The source files: those under `src` that are not tests.
    pub fn src_files(&self) -> Vec<String> {
        self.files(&self.cfg.src).into_iter().filter(|f| !self.is_test(f)).collect()
    }

    /// Folders and files the requirement lines are read from, plus decision folders, repo-relative.
    pub fn spec_paths(&self) -> Vec<&str> {
        self.spec_paths.iter().chain(&self.doc_dirs).map(String::as_str).collect()
    }

    /// Every `*.md` file of `dir` and its subfolders, repo-relative.
    fn md_in(&self, dir: &str) -> Vec<String> {
        detect::walk(&if dir == "." { self.root.clone() } else { self.root.join(dir) }, &self.allow).iter().filter(|p| p.extension().is_some_and(|x| x == "md")).map(|p| self.rel(p)).collect()
    }

    /// The markdown files that may hold requirements: the spec paths (folders, files) and decision folders, repo-relative.
    pub fn spec_files(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        self.spec_paths.iter().chain(&self.doc_dirs).flat_map(|p| if self.root.join(p).is_file() { vec![p.clone()] } else { self.md_in(p) }).filter(|f| seen.insert(f.clone())).collect()
    }

    // (SCAN-13)
    /// Reference docs and who links to them. `[ABC-12](path.md)` to an existing markdown file names that file by ID, and so does
    /// a decision-folder file `0003-title.md` (`ADR-0003`); IDs a bullet defines are left alone. Links to missing files go to `problems`.
    pub fn docs(&self, s: &IndexMap<String, Spec>, problems: &mut Vec<String>) -> (Vec<Spec>, Refs) {
        let mds = self.md_in(".");
        let known: std::collections::HashSet<&str> = mds.iter().map(String::as_str).collect();
        let (mut found, mut linked): (IndexMap<String, String>, Refs) = Default::default();
        for f in &mds {
            for (idx, line) in self.lines(f).iter().enumerate() {
                // a link shown in `code` is an example, not a link
                for c in self.rx.link.captures_iter(&self.rx.code_span.replace_all(line, "")) {
                    let (id, target) = (c[1].to_string(), &c[2]);
                    if target.contains(':') {
                        continue; // https:, mailto: …
                    }
                    linked.entry(id.clone()).or_default().push(Ref { file: f.clone(), line: idx + 1, text: line.trim().to_string() });
                    let Some(t) = resolve(f, target).filter(|t| t.ends_with(".md")) else { continue };
                    if !known.contains(t.as_str()) {
                        if !self.root.join(&t).exists() {
                            problems.push(format!("{id} {f}:{} links to {target}, which does not exist", idx + 1));
                        }
                    } else if !s.contains_key(&id) {
                        match found.get(&id) {
                            Some(was) if *was != t => problems.push(format!("{id} is linked to two files: {was} and {t}")),
                            Some(_) => {}
                            None => { found.insert(id, t); }
                        }
                    }
                }
            }
        }
        for d in &self.doc_dirs {
            for f in self.md_in(d) {
                let stem = f.rsplit('/').next().unwrap_or(&f);
                if let Some(m) = self.rx.adr.captures(stem) {
                    let id = format!("ADR-{}", &m[1]);
                    if !s.contains_key(&id) && !found.contains_key(&id) {
                        found.insert(id, f);
                    }
                }
            }
        }
        let docs = found.into_iter().map(|(id, file)| {
            let stem = file.rsplit('/').next().unwrap_or(&file).trim_end_matches(".md").to_string();
            let text = self.lines(&file).iter().find_map(|l| l.strip_prefix("# ").map(|h| h.trim().to_string())).unwrap_or(stem);
            Spec { id, file, line: 1, end: 1, level: "DOC".into(), text, mark: "ref".into(), done: false }
        }).collect();
        (docs, linked)
    }

    // (SCAN-01)
    /// {id: Spec} from `- **ABC-12** MUST — text` bullets (also `- ABC-12: text MUST …`, `1.`/`*` lists, `| ABC-12 | MUST | text |` rows), wrapped lines joined. A bullet ending in `(manual)`, `(later)` or
    /// `(deferred)` carries mark "manual" or "later"; struck-out bullets (`- ~~**ABC-12**`) are retired.
    /// Spec mistakes found on the way (duplicate ID, no level, marker not at the end) go to `problems`.
    pub fn specs(&self, problems: &mut Vec<String>) -> IndexMap<String, Spec> {
        let mut found: IndexMap<String, Spec> = IndexMap::new();
        for f in self.spec_files() {
            let ls = self.lines(&f);
            for (idx, line) in ls.iter().enumerate() {
                let n = idx + 1;
                let Some(m) = self.rx.head.captures(line) else { continue };
                let id = m[1].to_string();
                let Some(lv) = self.rx.level.find(&m[2]) else {
                    problems.push(format!("{id} {f}:{n} has no MUST/SHOULD/MAY, so it is not counted"));
                    continue;
                };
                let m1 = m.get(1).unwrap();
                let mut parts = vec![lead(&m[2], lv.as_str())];
                let mut j = n;
                // a table row is one line; a bullet wraps
                while !line.trim_start().starts_with('|') && j < ls.len() && !ls[j].trim().is_empty() && !self.rx.block_end.is_match(&ls[j]) {
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
                found.insert(id.clone(), Spec { id, file: f.clone(), line: n, end: j, level: lv.as_str().to_string(), text, mark: mark.into(), done });
            }
        }
        found
    }

    /// The requirement IDs of every `[ID]` or `[ID, ID]` tag on `line`, in order.
    pub fn tagged(&self, line: &str) -> Vec<String> {
        self.rx.tag.captures_iter(line).flat_map(|g| self.rx.mention.captures_iter(&g[1]).map(|m| m[1].to_string()).collect::<Vec<_>>()).collect()
    }

    // (SCAN-02)
    /// {id: [ref]} for every `[ID]` or `[ID, ID]` tag in the test folders.
    pub fn tests(&self) -> Refs {
        let mut found = Refs::new();
        for rel in self.test_files() {
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

    // (SCAN-03)
    /// {id: [ref]} for IDs mentioned after a line-comment marker in app sources (block comments are not scanned).
    pub fn sources(&self) -> Refs {
        let mut found = Refs::new();
        for rel in self.src_files() {
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
            for line in self.lines(&f).iter() {
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
