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

/// Canonical roots a symlink may point into: the repo root, plus every entry of `ALLOWED_PATHS` (path-list syntax).
pub fn allow_list(root: &Path) -> Vec<PathBuf> {
    let extra = std::env::var_os("ALLOWED_PATHS").map(|v| std::env::split_paths(&v).collect::<Vec<_>>()).unwrap_or_default();
    std::iter::once(root.to_path_buf()).chain(extra).filter_map(|p| p.canonicalize().ok()).collect()
}

fn inside(p: &Path, allow: &[PathBuf]) -> bool {
    p.canonicalize().is_ok_and(|c| allow.iter().any(|a| c.starts_with(a)))
}

fn wanted(e: &DirEntry, allow: &[PathBuf]) -> bool {
    let skip = e.depth() > 0 && e.file_type().is_dir() && {
        let n = e.file_name().to_string_lossy();
        n.starts_with('.') || SKIP.contains(&n.as_ref())
    };
    !skip && (!e.path_is_symlink() || inside(e.path(), allow))
}

/// Files under `dir`, sorted. Symlinks are followed only when they resolve inside `allow`; junk folders are skipped.
pub fn walk(dir: &Path, allow: &[PathBuf]) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = WalkDir::new(dir).follow_links(true).into_iter().filter_entry(|e| wanted(e, allow)).filter_map(Result::ok).filter(|e| e.file_type().is_file()).map(DirEntry::into_path).collect();
    v.sort();
    v
}

/// The first spec folder of `root`: one holding a requirement bullet, else a dedicated-name folder with any markdown,
/// else a bare `specs/`. `None` when the repo has none of them.
pub fn spec_dir(root: &Path, allow: &[PathBuf]) -> Option<String> {
    let bullet = Regex::new(&format!("{}.*{}", crate::scan::head(), crate::scan::LEVEL)).unwrap();
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
