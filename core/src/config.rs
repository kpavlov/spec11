//! `spec11.json` at the repo root. Every field is optional: an empty one is detected from the repo (see `detect.rs`).
use crate::detect::{self, Layout};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Folder of `*.md` specs, relative to the root (also in subfolders).
    pub specs: String,
    pub tests: Vec<String>,
    pub src: Vec<String>,
    /// File extensions scanned in `tests` and `src`.
    pub ext: Vec<String>,
    /// Line-comment markers; a source citation is an ID after one of them.
    pub comment: Vec<String>,
    /// Declaration regex, group 1 = name (used to name the code a citation sits in).
    pub decl: String,
    /// A test body containing a match has a direct assertion.
    pub assert: String,
    /// A line matching this opens a test case (default: the ecosystem's marker `#[test]`, `@Test`, `it(` …); read by the `spec11://tests` resource.
    pub case: String,
    /// A test head containing a match is switched off.
    pub off: String,
    /// A file whose repo-relative path matches is a test, also inside a `src` folder (`a_test.go`, `a.test.ts`).
    pub test_files: String,
    pub retired_prefixes: Vec<String>,
    /// CodeGraph index; empty = `.codegraph/codegraph.db`.
    pub codegraph: String,
    /// Path prefixes under `src[0]` that `uncovered` treats as plumbing.
    pub infra_paths: Vec<String>,
}

impl Config {
    // (SCAN-06)
    pub fn load(root: &Path) -> Result<Config, String> {
        let f = root.join("spec11.json");
        if !f.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))
    }

    /// Fill what the file left empty from the repo's file list (CodeGraph's, else a walk); what it set stays as it is.
    /// (SCAN-09)
    pub fn detect(&mut self, files: &[String]) {
        if self.codegraph.is_empty() {
            self.codegraph = ".codegraph/codegraph.db".into();
        }
        let Some(Layout { profile: p, src, tests }) = detect::layout(files) else { return self.never() };
                let (explicit_src, explicit_tests) = (!self.src.is_empty(), !self.tests.is_empty());
        for (field, auto) in [(&mut self.src, src), (&mut self.tests, tests), (&mut self.ext, p.ext.clone()), (&mut self.comment, p.comment.clone())] {
            if field.is_empty() {
                *field = auto;
            }
        }
        // a profile's test regex describes its own layout; a hand-written src/tests list keeps the user's meaning
        for (field, auto) in [(&mut self.decl, &p.decl), (&mut self.assert, &p.assert), (&mut self.off, &p.off)] {
            if field.is_empty() {
                *field = auto.into();
            }
        }
        if self.test_files.is_empty() && !(explicit_src && explicit_tests) {
            self.test_files = p.test.clone();
        }
        self.never();
    }

    /// Regexes that are still empty match nothing (an ecosystem spec11 does not know).
    fn never(&mut self) {
        for r in [&mut self.decl, &mut self.assert, &mut self.off] {
            if r.is_empty() {
                *r = detect::NEVER.into();
            }
        }
        if self.codegraph.is_empty() {
            self.codegraph = ".codegraph/codegraph.db".into();
        }
    }
}
