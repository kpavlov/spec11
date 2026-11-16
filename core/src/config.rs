//! `spec11.json` at the repo root. Every field defaults to what the Python scripts hardcoded (run-solo, Swift).
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Folder of `*.md` specs, relative to the root.
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
    /// A test head containing a match is switched off.
    pub off: String,
    pub retired_prefixes: Vec<String>,
    pub codegraph: String,
    /// Path prefixes under `src[0]` that `uncovered` treats as plumbing.
    pub infra_paths: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect();
        Config {
            specs: "specs".into(),
            tests: v(&["RunSolo/RunSoloTests", "RunSolo/RunSoloUITests"]),
            src: v(&["RunSolo/RunSolo"]),
            ext: v(&["swift"]),
            comment: v(&["//"]),
            decl: r"^\s*(?:(?:public|internal|private|fileprivate|final|nonisolated|indirect|static)\s+)*(?:struct|class|enum|actor|protocol|extension|func|typealias)\s+([A-Za-z_]\w*)".into(),
            assert: r"#expect|#require|XCTAssert|XCTFail|XCTUnwrap|Issue\.record|confirmation\(".into(),
            off: r"\.disabled\(|\.enabled\(if: false|XCTSkip".into(),
            retired_prefixes: v(&["BKP", "DRD", "ROLE"]),
            codegraph: ".codegraph/codegraph.db".into(),
            infra_paths: v(&["UI/Components/", "UI/DataTypes/", "RunSoloApp.swift"]),
        }
    }
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
}
