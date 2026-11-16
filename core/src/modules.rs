//! Modules and test cases: where the repo's code is grouped (build manifests, CodeGraph symbols and dependencies) and
//! which test cases its test files declare. Detected, not run: a listed test says nothing about passing.
use crate::detect;
use crate::report::Analysis;
use regex::Regex;
use serde::Serialize;
use std::{collections::{BTreeMap, BTreeSet, HashMap}, sync::LazyLock};

const NOTE: &str = "Detected from markers in test files, not run: a listed test does not mean it passes.";

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TestCase {
    pub name: String,
    /// CodeGraph's qualified name of the declaration, when the index has it.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub qualified: String,
    pub line: usize,
    /// Requirement IDs tagged on the test.
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TestFile {
    pub file: String,
    pub tests: Vec<TestCase>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModuleTests {
    pub module: String,
    pub tests: usize,
    pub files: Vec<TestFile>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TestsView {
    pub note: &'static str,
    pub total: usize,
    pub modules: Vec<ModuleTests>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Dep {
    pub module: String,
    /// Call, reference, instantiate, extend and import edges from this module to that one.
    pub edges: i64,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModuleInfo {
    pub path: String,
    pub manifest: Option<String>,
    pub languages: Vec<String>,
    pub src_files: usize,
    pub test_files: usize,
    pub tests: usize,
    /// Symbols per kind, from the CodeGraph index.
    pub symbols: BTreeMap<String, i64>,
    pub depends_on: Vec<Dep>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModulesView {
    /// Whether symbols and dependencies come from a CodeGraph index.
    pub index: bool,
    pub note: Option<String>,
    pub modules: Vec<ModuleInfo>,
    /// Modules hidden by `.spec11ignore` (API-12): not scanned, so only their paths are known.
    pub hidden: Vec<String>,
}

static QUOTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"["'`]([^"'`]+)["'`]"#).unwrap());

impl Analysis {
    /// The test cases one file declares: a case starts on a line matching `marker`; its IDs are the `[ID]` tags from three lines
    /// above it to its declaration. `call`: the marker line is a call, so the name is its quoted string.
    fn cases_in(&self, file: &str, marker: &Regex, call: bool) -> Vec<TestCase> {
        let decls = self.graph.as_ref().and_then(|g| g.decls_in(file).ok()).unwrap_or_default();
        let ls = self.p.lines(file);
        let mut tests = vec![];
        // a marker inside a string literal (a fixture that writes a test file) is not a test case
        let unescaped_quotes = |t: &str| t.matches('"').count() - t.matches("\\\"").count();
        let opens = |l: &str| marker.find_iter(l).any(|f| call || unescaped_quotes(&l[..f.start()]) % 2 == 0);
        for (m, line) in ls.iter().enumerate().filter(|(_, l)| opens(l)) {
            // the declaration is on the marker line or just below an attribute / annotation
            let decl = (m..(m + 4).min(ls.len())).find_map(|i| self.p.rx.decl.captures(&ls[i]).map(|c| (i, c[1].to_string())));
            let (at, name) = match (decl, call) {
                (Some((i, n)), false) => (i, n),
                _ => (m, QUOTED.captures(line).map_or_else(|| line.trim().to_string(), |c| c[1].to_string())),
            };
            let ids: Vec<String> = (m.saturating_sub(3)..=at).flat_map(|i| self.p.tagged(&ls[i])).collect::<BTreeSet<_>>().into_iter().collect();
            let qualified = decls.iter().filter(|(lo, hi, _)| *lo <= at as i64 + 1 && at as i64 + 1 <= *hi).min_by_key(|(lo, hi, _)| hi - lo).map(|d| d.2.clone()).unwrap_or_default();
            tests.push(TestCase { name, qualified, line: m + 1, ids });
        }
        tests
    }

    // (SCAN-14)
    /// The test cases of the test files, grouped by module and file (`case` in `spec11.json` replaces the built-in markers).
    pub fn tests_view(&self) -> TestsView {
        let (mut rx, mut seen): (HashMap<&str, Regex>, _) = (HashMap::new(), HashMap::new());
        let custom = Some(&self.p.cfg.case).filter(|c| !c.is_empty()).and_then(|c| Regex::new(c).ok());
        let mut by: BTreeMap<String, Vec<TestFile>> = BTreeMap::new();
        for file in self.p.test_files() {
            let Some(case) = detect::case_of(&file) else { continue };
            let marker = custom.clone().unwrap_or_else(|| rx.entry(case.marker).or_insert_with(|| Regex::new(case.marker).unwrap()).clone());
            let tests = self.cases_in(&file, &marker, case.call);
            if !tests.is_empty() {
                by.entry(detect::module_of(&self.p.root, &file, &mut seen).0).or_default().push(TestFile { file, tests });
            }
        }
        let modules: Vec<ModuleTests> = by.into_iter().map(|(module, files)| ModuleTests { module, tests: files.iter().map(|f| f.tests.len()).sum(), files }).collect();
        TestsView { note: NOTE, total: modules.iter().map(|m| m.tests).sum(), modules }
    }

    // (GRAPH-05)
    /// Modules of the repo with their files, languages, tests, CodeGraph symbols and the modules they depend on.
    pub fn modules_view(&self) -> ModulesView {
        let mut seen = HashMap::new();
        let mut mods: BTreeMap<String, ModuleInfo> = BTreeMap::new();
        let blank = |path: &str, manifest: Option<&str>| ModuleInfo { path: path.to_string(), manifest: manifest.map(str::to_owned), languages: vec![], src_files: 0, test_files: 0, tests: 0, symbols: BTreeMap::new(), depends_on: vec![] };
        let mut module = |file: &str, mods: &mut BTreeMap<String, ModuleInfo>| -> String {
            let (path, manifest) = detect::module_of(&self.p.root, file, &mut seen);
            mods.entry(path.clone()).or_insert_with(|| blank(&path, manifest));
            path
        };
        let mut langs: HashMap<String, BTreeSet<&str>> = HashMap::new();
        for f in &self.p.files {
            let Some(case) = detect::case_of(f) else { continue };
            let m = module(f, &mut mods);
            let info = mods.get_mut(&m).unwrap();
            if self.p.is_test_only(f) { info.test_files += 1 } else { info.src_files += 1 }
            langs.entry(m).or_default().insert(&case.profile.name);
        }
        for t in self.tests_view().modules {
            mods.entry(t.module.clone()).or_insert_with(|| blank(&t.module, None)).tests = t.tests; // also when its files are not in the list
        }
        let (mut index, mut note) = (false, self.graph_note.clone());
        if let Some(g) = &self.graph {
            match (g.symbols_by_file(), g.file_deps()) {
                (Ok(symbols), Ok(deps)) => {
                    index = true;
                    for (file, kind, n) in symbols.into_iter().filter(|(f, _, _)| !self.p.ignored(f)) {
                        let m = module(&file, &mut mods);
                        *mods.get_mut(&m).unwrap().symbols.entry(kind).or_default() += n;
                    }
                    let mut edges: BTreeMap<(String, String), i64> = BTreeMap::new();
                    for (from, to, n) in deps.into_iter().filter(|(a, b, _)| !self.p.ignored(a) && !self.p.ignored(b)) {
                        let (a, b) = (module(&from, &mut mods), module(&to, &mut mods));
                        if a != b {
                            *edges.entry((a, b)).or_default() += n;
                        }
                    }
                    for ((a, b), n) in edges {
                        mods.get_mut(&a).unwrap().depends_on.push(Dep { module: b, edges: n });
                    }
                }
                (Err(e), _) | (_, Err(e)) => note = Some(self.graph_err(e)),
            }
        } else if note.is_none() {
            note = Some("no CodeGraph index, so symbols and module dependencies are not checked".into());
        }
        let mut modules: Vec<ModuleInfo> = mods.into_values().collect();
        for m in &mut modules {
            m.languages = langs.get(&m.path).map(|s| s.iter().map(|x| x.to_string()).collect()).unwrap_or_default();
            m.depends_on.sort_by(|a, b| b.edges.cmp(&a.edges).then_with(|| a.module.cmp(&b.module)));
        }
        ModulesView { index, note, modules, hidden: detect::Ignore::hidden_modules(&self.p.root) }
    }
}
