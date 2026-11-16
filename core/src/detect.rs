//! Layout detection: where the specs, sources and tests of a repo are, when `spec11.json` does not say.
//! Specs come from the folder conventions of spec-driven tools; sources and tests from the file list of the CodeGraph
//! index (else a directory walk) classified by ecosystem. Symlinks are followed only into the repo root or `ALLOWED_PATHS`.
use regex::Regex;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// Spec folders in priority order: dedicated names first, generic doc folders last (they need a requirement bullet to count).
/// Covers plain `specs/`, GitHub Spec Kit (`.specify/specs`), OpenSpec, Kiro and the usual docs layouts.
const SPEC_DIRS: [&str; 13] = [
    "specs", "spec", "specifications", "requirements", "docs/specs", "docs/spec", "docs/requirements", "doc/specs",
    "openspec/specs", ".specify/specs", ".kiro/specs", "docs", "doc",
];
const GENERIC: [&str; 2] = ["docs", "doc"];
/// File stems (upper case) of a single-file spec, at the root or in `docs/`.
const SPEC_STEMS: [&str; 6] = ["SPEC", "SPECS", "SPECIFICATION", "SPECIFICATIONS", "REQUIREMENTS", "PRD"];
/// Architecture decision records: reference docs, not bullets.
const DECISION_DIRS: [&str; 9] = ["decisions", "adr", "adrs", "docs/adr", "docs/adrs", "docs/decisions", "doc/adr", "doc/decisions", "architecture/decisions"];
/// Folders that never hold the project's own code.
const SKIP: [&str; 12] = ["node_modules", "target", "build", "dist", "vendor", "venv", "site-packages", "__pycache__", "Pods", "DerivedData", "obj", "coverage"];
/// Matches nothing: the regex of an ecosystem spec11 knows nothing about.
pub const NEVER: &str = r"[^\s\S]";

/// One ecosystem: how its files, comments, declarations and tests look.
pub struct Profile {
    pub name: &'static str,
    pub ext: &'static [&'static str],
    pub comment: &'static [&'static str],
    pub decl: &'static str,
    pub assert: &'static str,
    pub off: &'static str,
    /// Repo-relative path of a test file, also when it sits next to the code (`a_test.go`, `a.test.ts`).
    pub test: &'static str,
    /// Tests live in the source files too (Rust `mod tests`).
    pub inline: bool,
}

pub const PROFILES: [Profile; 10] = [
    Profile { name: "swift", ext: &["swift"], comment: &["//"],
        decl: r"^\s*(?:(?:public|internal|private|fileprivate|final|nonisolated|indirect|static)\s+)*(?:struct|class|enum|actor|protocol|extension|func|typealias)\s+([A-Za-z_]\w*)",
        assert: r"#expect|#require|XCTAssert|XCTFail|XCTUnwrap|Issue\.record|confirmation\(", off: r"\.disabled\(|\.enabled\(if: false|XCTSkip",
        test: r"(^|/)[^/]*Tests?(/|\.swift$)", inline: false },
    Profile { name: "rust", ext: &["rs"], comment: &["//"],
        decl: r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:fn|struct|enum|trait|impl|type|mod)\s+([A-Za-z_]\w*)",
        assert: r"assert!|assert_eq!|assert_ne!|panic!|matches!", off: r"#\[ignore",
        test: r"(^|/)(tests|benches)/|_test\.rs$", inline: true },
    Profile { name: "typescript", ext: &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"], comment: &["//"],
        decl: r"^\s*(?:export\s+)?(?:default\s+)?(?:declare\s+)?(?:async\s+)?(?:abstract\s+)?(?:function\*?|class|interface|enum|type|const)\s+([A-Za-z_$][\w$]*)",
        assert: r"expect\(|assert[.(]|\.should\b|\bexpect\.", off: r"\.skip\(|\bxit\(|\bxdescribe\(|\.todo\(",
        test: r"(^|/)(__tests__|tests?|e2e|cypress)/|\.(test|spec)\.[cm]?[jt]sx?$", inline: false },
    Profile { name: "python", ext: &["py"], comment: &["#"],
        decl: r"^\s*(?:async\s+)?(?:def|class)\s+([A-Za-z_]\w*)",
        assert: r"\bassert\b|pytest\.raises|self\.assert|\.assert_", off: r"@pytest\.mark\.skip|@unittest\.skip|pytest\.skip|pytest\.xfail",
        test: r"(^|/)(tests?|testing)/|(^|/)test_[^/]*\.py$|_test\.py$|(^|/)conftest\.py$", inline: false },
    Profile { name: "go", ext: &["go"], comment: &["//"],
        decl: r"^\s*(?:func(?:\s*\([^)]*\))?|type)\s+([A-Za-z_]\w*)",
        assert: r"t\.(?:Error|Errorf|Fatal|Fatalf|Fail|FailNow)\b|assert\.|require\.", off: r"t\.Skip",
        test: r"_test\.go$", inline: false },
    Profile { name: "jvm", ext: &["java", "kt", "scala"], comment: &["//"],
        decl: r"^\s*(?:(?:public|private|protected|internal|static|final|abstract|sealed|open|data|suspend|override|inline|case)\s+)*(?:class|interface|enum|record|object|trait|fun|def|void)\s+([A-Za-z_]\w*)",
        assert: r"\bassert\w*\(|assertThat|verify\(|\bexpect\w*\(|\bshould", off: r"@Disabled|@Ignore|@Ignored",
        test: r"(^|/)src/[A-Za-z]*[tT]ests?[A-Za-z]*/|(^|/)tests?/|(Tests?|IT|Spec)\.(java|kt|scala)$", inline: false },
    Profile { name: "csharp", ext: &["cs"], comment: &["//"],
        decl: r"^\s*(?:(?:public|private|protected|internal|static|sealed|abstract|partial|async|override|virtual)\s+)*(?:class|struct|interface|enum|record)\s+([A-Za-z_]\w*)",
        assert: r"Assert\.|\.Should\(|Expect\(", off: r"Skip\s*=|\[Ignore",
        test: r"(^|/)[^/]*(Tests?|Specs?)(/|\.cs$)|(^|/)tests?/", inline: false },
    Profile { name: "ruby", ext: &["rb"], comment: &["#"],
        decl: r"^\s*(?:def|class|module)\s+([\w:]+)",
        assert: r"\bexpect\b|\bassert\w*|\.should\b", off: r"\bskip\b|\bxit\b|\bxdescribe\b|\bpending\b",
        test: r"(^|/)(spec|test)/|_(spec|test)\.rb$", inline: false },
    Profile { name: "php", ext: &["php"], comment: &["//", "#"],
        decl: r"^\s*(?:(?:abstract|final|public|private|protected|static)\s+)*(?:class|interface|trait|enum|function)\s+([A-Za-z_]\w*)",
        assert: r"\$this->assert|\bassert\w*\(|\bexpect\(", off: r"markTestSkipped|markTestIncomplete",
        test: r"(^|/)tests?/|Tests?\.php$", inline: false },
    Profile { name: "c/c++", ext: &["c", "cc", "cpp", "cxx", "h", "hh", "hpp"], comment: &["//"],
        decl: r"^\s*(?:class|struct|enum|namespace)\s+([A-Za-z_]\w*)",
        assert: r"ASSERT_|EXPECT_|\bassert\(|\bREQUIRE\(|\bCHECK\(", off: r"DISABLED_|GTEST_SKIP",
        test: r"(^|/)(tests?|unittests?)/|[_.]?[tT]ests?\.(c|cc|cpp|cxx|h|hpp)$", inline: false },
];

/// Per ecosystem, a line that opens a test case (an attribute, annotation, call or declaration), and whether that line is a call
/// such as `it("name", ...)` (the name is its quoted string) rather than a marker above a declaration. Keyed by `Profile::name`.
const CASES: [(&str, &str, bool); 10] = [
    ("swift", r"^\s*@Test\b|^\s*(?:@\w+\s+)*func\s+test\w*", false),
    ("rust", r"#\[(?:\w+::)*(?:test|rstest|test_case)\b", false),
    ("typescript", r"^\s*(?:it|test)(?:\.\w+)*\s*\(", true),
    ("python", r"^\s*(?:async\s+)?def\s+test\w*", false),
    ("go", r"^func\s+(?:Test|Benchmark|Fuzz|Example)\w*\s*\(", false),
    ("jvm", r"^\s*@(?:\w+\.)*(?:Test|ParameterizedTest|RepeatedTest|TestFactory)\b", false),
    ("csharp", r"^\s*\[(?:Test|TestMethod|Fact|Theory|TestCase)\b", false),
    ("ruby", r"^\s*(?:(?:it|specify|scenario)\s*[(\x22']|def\s+test_)", true),
    ("php", r"^\s*(?:public\s+)?function\s+test\w*|^\s*#\[Test\]|@test\b", false),
    ("c/c++", r"^\s*(?:TEST|TEST_F|TEST_P|TYPED_TEST|TEST_CASE|SCENARIO)\s*\(", true),
];

/// How test cases are found in one file: its ecosystem, the line regex that opens one, and whether that line is a call.
pub struct Case {
    pub profile: &'static Profile,
    pub marker: &'static str,
    pub call: bool,
}

/// The test-case rule of a file's ecosystem, by extension.
pub fn case_of(file: &str) -> Option<Case> {
    let p = PROFILES.iter().find(|p| p.ext.contains(&ext_of(file)))?;
    CASES.iter().find(|(n, ..)| *n == p.name).map(|&(_, marker, call)| Case { profile: p, marker, call })
}

/// Build manifests that make a folder a module.
const MANIFESTS: [&str; 12] = ["Cargo.toml", "package.json", "build.gradle.kts", "build.gradle", "pom.xml", "go.mod", "pyproject.toml", "setup.py", "Package.swift", "composer.json", "CMakeLists.txt", "Gemfile"];

/// The module of a repo-relative file and its build manifest: the nearest folder above it that has one, else (also when only
/// the repo root has one) the folder above its `src`, else its top-level folder, else `.`. `seen` caches per folder.
pub fn module_of(root: &Path, file: &str, seen: &mut std::collections::HashMap<String, (String, Option<&'static str>)>) -> (String, Option<&'static str>) {
    let dir = file.rsplit_once('/').map_or("", |(d, _)| d);
    if let Some(v) = seen.get(dir) {
        return v.clone();
    }
    let manifest = |d: &str| MANIFESTS.iter().find(|m| root.join(d).join(m).is_file()).copied();
    let mut d = dir;
    let nested = loop {
        if d.is_empty() {
            break None;
        }
        if let Some(m) = manifest(d) {
            break Some((d.to_string(), Some(m)));
        }
        d = d.rsplit_once('/').map_or("", |(up, _)| up);
    };
    let found = nested.unwrap_or_else(|| {
        let comps: Vec<&str> = dir.split('/').filter(|c| !c.is_empty()).collect();
        let by_src = comps.iter().position(|c| matches!(*c, "src" | "Sources" | "Source" | "lib" | "app")).filter(|i| *i > 0).map(|i| comps[..i].join("/"));
        let m = by_src.or_else(|| comps.first().map(|c| c.to_string())).unwrap_or_else(|| ".".into());
        let own = manifest(if m == "." { "" } else { &m });
        (m, own)
    });
    seen.insert(dir.to_string(), found.clone());
    found
}

/// Canonical roots a symlink may point into: the repo root, plus every entry of `ALLOWED_PATHS` (path-list syntax).
pub fn allow_list(root: &Path) -> Vec<PathBuf> {
    let extra = std::env::var_os("ALLOWED_PATHS").map(|v| std::env::split_paths(&v).collect::<Vec<_>>()).unwrap_or_default();
    std::iter::once(root.to_path_buf()).chain(extra).filter_map(|p| p.canonicalize().ok()).collect()
}

fn inside(p: &Path, allow: &[PathBuf]) -> bool {
    p.canonicalize().is_ok_and(|c| allow.iter().any(|a| c.starts_with(a)))
}

/// Files with gitignore syntax that hide paths from spec11, read from the repo root: `.aiignore` (shared with other agent
/// tools) and `.spec11ignore` (this tool only). `.agents` (installed skills) is always hidden.
const IGNORE_FILES: [&str; 2] = [".aiignore", ".spec11ignore"];

pub struct Ignore(ignore::gitignore::Gitignore);

impl Ignore {
    pub fn load(root: &Path) -> Self {
        let mut b = ignore::gitignore::GitignoreBuilder::new(root);
        let _ = b.add_line(None, ".agents");
        for f in IGNORE_FILES {
            if root.join(f).is_file() {
                let _ = b.add(root.join(f)); // a bad line is skipped, the rest still applies
            }
        }
        Ignore(b.build().unwrap_or_else(|_| ignore::gitignore::Gitignore::empty()))
    }

    /// Whether a repo-relative path, or any folder above it, is hidden.
    pub fn hides(&self, rel: &str) -> bool {
        let rel = rel.trim_start_matches("./");
        !rel.is_empty() && rel != "." && self.0.matched_path_or_any_parents(rel, false).is_ignore()
    }
}

fn wanted(e: &DirEntry, allow: &[PathBuf]) -> bool {
    let skip = e.depth() > 0 && e.file_type().is_dir() && {
        let n = e.file_name().to_string_lossy();
        n.starts_with('.') || SKIP.contains(&n.as_ref())
    };
    !skip && (!e.path_is_symlink() || inside(e.path(), allow))
}

/// Files under `dir`, sorted. Symlinks are followed only when they resolve inside `allow` (its first entry is the repo root);
/// junk folders and what the root's `.aiignore` / `.spec11ignore` hide are skipped.
pub fn walk(dir: &Path, allow: &[PathBuf]) -> Vec<PathBuf> {
    let hidden = allow.first().and_then(|root| {
        let base = dir.canonicalize().ok()?.strip_prefix(root).ok()?.to_path_buf();
        Some((Ignore::load(root), base))
    });
    let hides = |e: &DirEntry| {
        hidden.as_ref().is_some_and(|(ig, base)| e.path().strip_prefix(dir).is_ok_and(|r| ig.hides(&base.join(r).to_string_lossy().replace('\\', "/"))))
    };
    let mut v: Vec<PathBuf> = WalkDir::new(dir).follow_links(true).into_iter().filter_entry(|e| wanted(e, allow) && !hides(e)).filter_map(Result::ok).filter(|e| e.file_type().is_file()).map(DirEntry::into_path).collect();
    v.sort();
    v
}

fn requirement_line() -> Regex {
    Regex::new(&format!("{}.*{}", crate::scan::head(), crate::scan::LEVEL)).unwrap()
}

fn is_md(p: &Path) -> bool {
    p.extension().is_some_and(|x| x == "md")
}

fn has_requirement(f: &Path, rx: &Regex) -> bool {
    std::fs::read(f).is_ok_and(|b| String::from_utf8_lossy(&b).lines().any(|l| rx.is_match(l)))
}

/// Decision-record folders of `root` that exist.
pub fn decision_dirs(root: &Path) -> Vec<String> {
    DECISION_DIRS.iter().filter(|d| root.join(d).is_dir()).map(|d| d.to_string()).collect()
}

/// Single-file specs (`SPEC.md`, `docs/REQUIREMENTS.md` …) of `root`, whatever the case of the name.
fn spec_file_names(root: &Path) -> Vec<String> {
    let mut v = vec![];
    for dir in ["", "docs"] {
        let mut names: Vec<String> = std::fs::read_dir(root.join(dir)).into_iter().flatten().filter_map(Result::ok).filter(|e| e.path().is_file() && is_md(&e.path())).filter_map(|e| e.file_name().into_string().ok()).filter(|n| SPEC_STEMS.contains(&n.trim_end_matches(".md").to_uppercase().as_str())).collect();
        names.sort();
        v.extend(names.into_iter().map(|n| if dir.is_empty() { n } else { format!("{dir}/{n}") }));
    }
    v
}

/// Cheap check for `find_root`: does `d` look like a repo with specs? No tree walk.
pub fn has_spec_marker(d: &Path, allow: &[PathBuf]) -> bool {
    spec_dir(d, allow).is_some() || !spec_file_names(d).is_empty() || !decision_dirs(d).is_empty()
}

/// Where the requirement lines of `root` are, repo-relative folders or files: the spec folder, single-file specs, and when
/// none of them holds a requirement line, every markdown file of the repo that does. Empty when the repo has no such place.
// ponytail: the fallback walks the whole repo's markdown once per load; add a cache if repos get huge.
pub fn spec_paths(root: &Path, allow: &[PathBuf]) -> Vec<String> {
    let mut v: Vec<String> = spec_dir(root, allow).into_iter().chain(spec_file_names(root)).collect();
    let rx = requirement_line();
    let holds = v.iter().any(|p| {
        let at = root.join(p);
        if at.is_file() { has_requirement(&at, &rx) } else { walk(&at, allow).iter().any(|f| is_md(f) && has_requirement(f, &rx)) }
    });
    if !holds {
        v.extend(walk(root, allow).iter().filter(|f| is_md(f) && has_requirement(f, &rx)).filter_map(|f| f.strip_prefix(root).ok()).map(|f| f.to_string_lossy().replace('\\', "/")));
    }
    v
}

/// The first spec folder of `root`: one holding a requirement bullet, else a dedicated-name folder with any markdown,
/// else a bare `specs/`. `None` when the repo has none of them.
pub fn spec_dir(root: &Path, allow: &[PathBuf]) -> Option<String> {
    let bullet = requirement_line();
    let mds = |d: &str| -> Vec<PathBuf> { walk(&root.join(d), allow).into_iter().filter(|p| p.extension().is_some_and(|x| x == "md")).collect() };
    let found: Vec<(&str, Vec<PathBuf>)> = SPEC_DIRS.iter().filter(|d| root.join(d).is_dir()).map(|d| (*d, mds(d))).collect();
    let has_bullet = |fs: &[PathBuf]| fs.iter().any(|f| std::fs::read(f).is_ok_and(|b| String::from_utf8_lossy(&b).lines().any(|l| bullet.is_match(l))));
    found.iter().find(|(_, fs)| has_bullet(fs)).map(|(d, _)| d.to_string())
        .or_else(|| found.iter().find(|(d, fs)| !GENERIC.contains(d) && !fs.is_empty()).map(|(d, _)| d.to_string()))
        .or_else(|| root.join("specs").is_dir().then(|| "specs".to_string()))
}

/// What the file list says about the code: the dominant ecosystem and the folders of its sources and tests.
pub struct Layout {
    pub profile: &'static Profile,
    pub src: Vec<String>,
    pub tests: Vec<String>,
}

fn ext_of(f: &str) -> &str {
    f.rsplit_once('.').map_or("", |(_, x)| x)
}

fn is_test_dir(c: &str) -> bool {
    matches!(c, "test" | "tests" | "spec" | "__tests__" | "e2e" | "cypress" | "testing") || c.ends_with("Test") || c.ends_with("Tests")
}

/// The folder a source file belongs to: up to its `src` (or `src/main`), else its top-level folder, else `.`.
fn src_root(f: &str) -> String {
    let dirs: Vec<&str> = f.split('/').collect::<Vec<_>>().split_last().map_or(vec![], |(_, d)| d.to_vec());
    match dirs.iter().position(|c| matches!(*c, "src" | "Sources" | "Source" | "lib" | "app")) {
        Some(i) => dirs[..=i + usize::from(dirs.get(i + 1) == Some(&"main"))].join("/"),
        None => dirs.first().map_or(".".to_string(), |d| d.to_string()),
    }
}

fn test_root(f: &str) -> Option<String> {
    let dirs: Vec<&str> = f.split('/').collect();
    let i = dirs[..dirs.len() - 1].iter().position(|c| is_test_dir(c))?;
    Some(dirs[..=i].join("/"))
}

/// Sorted, deduplicated, without folders that sit under another listed folder; `.` only when nothing else is left.
fn minimal(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    let others: Vec<String> = v.iter().filter(|d| *d != ".").cloned().collect();
    let v = if others.is_empty() { v } else { others };
    v.iter().filter(|d| !v.iter().any(|o| o != *d && d.starts_with(&format!("{o}/")))).cloned().collect()
}

/// `files`: repo-relative, `/`-separated. `None` when no known ecosystem has a file.
pub fn layout(files: &[String]) -> Option<Layout> {
    let mine: Vec<&String> = files.iter().filter(|f| !f.split('/').any(|c| c.starts_with('.') && c != "." || SKIP.contains(&c))).collect();
    let (profile, _) = PROFILES.iter().map(|p| (p, mine.iter().filter(|f| p.ext.contains(&ext_of(f))).count())).fold(None, |best: Option<(&Profile, usize)>, c| if c.1 > best.map_or(0, |b| b.1) { Some(c) } else { best })?;
    let test = Regex::new(profile.test).unwrap();
    let (tests, src): (Vec<&String>, Vec<&String>) = mine.into_iter().filter(|f| profile.ext.contains(&ext_of(f))).partition(|f| test.is_match(f));
    let src_dirs = minimal(src.iter().map(|f| src_root(f)).collect());
    let mut test_dirs: Vec<String> = tests.iter().filter_map(|f| test_root(f)).collect();
    if profile.inline {
        test_dirs.extend(src_dirs.iter().cloned());
    }
    Some(Layout { profile, src: src_dirs, tests: minimal(test_dirs) })
}
