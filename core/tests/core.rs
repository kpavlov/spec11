use rusqlite::Connection;
use spec11_core::{Analysis, StatusOpts, codegraph::{Graph, GraphError}};
use std::{fs, path::{Path, PathBuf}};

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// A tiny repo that exercises every state and issue kind.
fn fixture(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("spec11-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    write(&d, "spec11.json", r#"{"tests":["tests"],"src":["src"],"ext":["swift"]}"#);
    write(&d, "specs/01-demo.md", "\
- **DEM-01** MUST — tested thing.
- **DEM-02** MUST — only cited in src.
- **DEM-03** SHOULD — tagged only in a comment.
- **DEM-04** MAY — checked by a person (manual)
- **DEM-05** MUST — built already (later)
- **DEM-06** MUST — first definition.
- **DEM-06** MUST — second definition.
- **DEM-07** has no level.
- ✅ **DEM-08** MUST — marked done without tests.
- **DEM-09** MUST — wraps
  onto a second line (later)
- ~~**DEM-10** MUST — retired.~~
- **DEM-11** MAY — nothing at all.
");
    write(&d, "tests/DemoTests.swift", "\
@Test(\"[DEM\x2D01] works\") func a() {
    #expect(true)
}
@Test(\"[DEM\x2D05] built already\") func b() {
    let x = 1
}
@Test(\"[DEM\x2D99] unknown\") func c() {
    #expect(true)
}
@Test(\"[DEM\x2D10] retired\") func e() {
    #expect(true)
}
// [DEM\x2D03] only in a comment
");
    write(&d, "src/Demo.swift", "\
/// (DEM-02) the rule
struct Demo {
    func run() {}
}
// DEM-98 typo
");
    d
}

#[doc = "[SCAN-01, SCAN-02, SCAN-03, SCAN-04]"]
#[test]
fn states_and_issues() {
    let a = Analysis::load(Some(&fixture("states"))).unwrap();
    let st = |i: &str| a.state[i].as_str();
    assert_eq!((st("DEM-01"), st("DEM-02"), st("DEM-03"), st("DEM-04"), st("DEM-05")), ("tested", "src-only", "tested", "manual", "later"));
    assert_eq!(a.s["DEM-09"].mark, "later");
    assert_eq!(a.s["DEM-09"].text, "wraps onto a second line (later)");
    assert!(!a.s.contains_key("DEM-07") && !a.s.contains_key("DEM-10"));
    assert!(a.issues.iter().any(|x| x.starts_with("DEM-06 is defined twice")));
    assert!(a.issues.iter().any(|x| x.starts_with("DEM-07 ") && x.contains("no MUST/SHOULD/MAY")));
    assert_eq!(a.verdict("DEM-08"), "NO-TEST");
    assert!(a.built("DEM-05"));
}

#[doc = "[SCAN-05]"]
#[test]
fn same_tree_same_text() {
    let d = fixture("determinism");
    let text = || {
        let a = Analysis::load(Some(&d)).unwrap();
        (a.report().text(&a, 12), a.status(&StatusOpts { all: true, ..Default::default() }).unwrap().text(&StatusOpts { all: true, ..Default::default() }))
    };
    let first = text();
    assert_eq!(first, text());
    assert_eq!(first, text());
}

#[doc = "[SCAN-06]"]
#[test]
fn config_comes_from_spec11_json_or_defaults() {
    let a = Analysis::load(Some(&fixture("config"))).unwrap();
    assert_eq!((a.p.cfg.tests.clone(), a.p.cfg.src.clone(), a.p.cfg.ext.clone()), (vec!["tests".to_string()], vec!["src".to_string()], vec!["swift".to_string()]));

    let bare = std::env::temp_dir().join(format!("spec11-{}-bare", std::process::id()));
    let _ = fs::remove_dir_all(&bare);
    write(&bare, "specs/01-a.md", "- **AAA-01** MUST — one.\n");
    let a = Analysis::load(Some(&bare)).unwrap();
    assert_eq!(a.p.cfg.specs, "specs");
    assert!(a.p.cfg.ext.is_empty() && a.p.cfg.src.is_empty()); // nothing to detect from

    write(&bare, "spec11.json", "{not json");
    let err = Analysis::load(Some(&bare)).err().expect("a broken spec11.json is an error");
    assert!(err.contains("spec11.json"), "{err}");
}

#[doc = "[GRAPH-03]"]
#[test]
fn a_missing_index_is_not_checked_not_an_error() {
    let d = fixture("noindex");
    assert!(Graph::open(&d.join(".codegraph/codegraph.db")).unwrap().is_none());
    let a = Analysis::load(Some(&d)).unwrap();
    let r = a.report();
    assert!(!r.index);
    assert!(r.text(&a, 12).contains("not checked"), "{}", r.text(&a, 12));
}

#[doc = "[SCAN-07]"]
#[test]
fn report_problems() {
    let a = Analysis::load(Some(&fixture("report"))).unwrap();
    let r = a.report();
    assert_eq!(r.unimplemented, ["DEM-06", "DEM-08", "DEM-11"]);
    assert_eq!(r.untested, ["DEM-02"]);
    assert_eq!(r.built, ["DEM-05"]);
    assert_eq!(r.dangling_tests, ["DEM-10", "DEM-99"]); // a retired ID is a leftover too
    assert_eq!(r.dangling_sources, ["DEM-98"]);
    let bad = |k: &str| r.invalid_tests[k].iter().map(|p| p.id.as_str()).collect::<Vec<_>>();
    assert_eq!(bad("unknown"), ["DEM-99"]);
    assert_eq!(bad("retired"), ["DEM-10"]);
    assert_eq!(bad("no-assert"), ["DEM-05"]);
    assert_eq!(bad("comment-only"), ["DEM-03"]);
    assert!(!r.index); // no CodeGraph index: reported as not checked, never an error
    assert!(r.text(&a, 12).contains("[code with no requirement] not checked"));
}

#[doc = "[SCAN-08]"]
#[test]
fn status_listing_and_trace() {
    let a = Analysis::load(Some(&fixture("status"))).unwrap();
    let o = StatusOpts { missing: true, effort: "low".into(), ..Default::default() };
    let rows = a.status(&o).unwrap().rows;
    assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["DEM-02", "DEM-06", "DEM-08", "DEM-11"]);
    assert!(a.status(&StatusOpts { prefix: Some("NOPE".into()), ..Default::default() }).is_err());
    let t = a.trace("dem-02").unwrap();
    assert_eq!(t.source_cites[0].decl, "Demo");
    assert!(a.trace("ZZZ-1").is_none());
}

/// A minimal CodeGraph index: the tagged test calls `Demo.run`.
fn index(d: &Path, with_edges_kind: bool) {
    fs::create_dir_all(d.join(".codegraph")).unwrap();
    let c = Connection::open(d.join(".codegraph/codegraph.db")).unwrap();
    c.execute_batch(&format!("
        create table nodes(id text primary key, kind text, name text, qualified_name text, file_path text, start_line int, end_line int, visibility text);
        create table edges(id integer primary key, source text, target text, {});
        create virtual table nodes_fts using fts5(name, qualified_name, content='nodes', content_rowid='rowid');
        create table schema_versions(version int, applied_at int, description text);
        insert into schema_versions values (11, 0, 'x');
        insert into nodes values ('t1','function','a','a','tests/DemoTests.swift',1,3,null),
                                 ('n1','struct','Demo','Demo','src/Demo.swift',2,4,'internal'),
                                 ('n2','method','run','Demo::run','src/Demo.swift',3,3,'internal');
        {}
        insert into nodes_fts(rowid, name, qualified_name) select rowid, name, qualified_name from nodes;
    ", if with_edges_kind { "kind text" } else { "other text" },
        if with_edges_kind { "insert into edges(source,target,kind) values ('t1','n2','calls'),('n2','n1','references');" } else { "" })).unwrap();
}

#[doc = "[GRAPH-01, GRAPH-04]"]
#[test]
fn codegraph_links_callers_impact_fts() {
    let d = fixture("graph");
    index(&d, true);
    let a = Analysis::load(Some(&d)).unwrap();
    assert_eq!(a.graph.as_ref().unwrap().version, Some(11));
    assert_eq!(a.links.as_ref().unwrap()["DEM-01"].iter().collect::<Vec<_>>(), ["Demo.swift"]);
    assert_eq!(a.verdict("DEM-01"), "ok");
    let g = a.graph.as_ref().unwrap();
    assert_eq!(g.callers("n2").unwrap()[0].id, "t1");
    assert_eq!(g.impact("n2", 3).unwrap().len(), 2); // itself and its caller
    assert_eq!(g.search("Demo", 5).unwrap()[0].name, "Demo"); // FTS5 is compiled in
}

#[doc = "[GRAPH-02]"]
#[test]
fn codegraph_schema_drift_is_an_error_not_a_panic() {
    let d = fixture("drift");
    index(&d, false);
    match Graph::open(&d.join(".codegraph/codegraph.db")) {
        Err(GraphError::SchemaDrift(m)) => assert!(m.contains("edges lacks kind")),
        other => panic!("{:?}", other.map(|_| ())),
    }
    let a = Analysis::load(Some(&d)).unwrap(); // the rest still works
    assert!(a.graph.is_none() && a.graph_note.unwrap().contains("schema not supported"));
}

#[doc = "[SCAN-09]"]
#[test]
fn every_profile_compiles() {
    for p in spec11_core::detect::PROFILES.iter() {
        for rx in [&p.decl, &p.assert, &p.off, &p.test, &p.case] {
            regex::Regex::new(rx).unwrap_or_else(|e| panic!("{}: {rx}: {e}", p.name));
        }
        assert!(p.decl.contains('('), "{}: decl needs a name group", p.name);
    }
}

#[doc = "[SCAN-09]"]
#[test]
fn layout_is_detected_when_spec11_json_is_missing() {
    use spec11_core::detect::{layout, spec_dir};
    let d = std::env::temp_dir().join(format!("spec11-{}-detect", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    write(&d, "docs/specs/01-a.md", "- **AAA-01** MUST — one.\n");
    write(&d, "docs/guide.md", "no requirements here\n");
    write(&d, "pkg/mod.py", "# (AAA-01)\ndef f(): pass\n");
    write(&d, "tests/test_mod.py", "# [AAA\x2D01]\ndef test_f():\n    assert True\n");
    let a = Analysis::load(Some(&d)).unwrap();
    let c = &a.p.cfg;
    assert_eq!((c.specs.as_str(), c.src.clone(), c.tests.clone(), c.ext.clone(), c.comment.clone()), ("docs/specs", vec!["pkg".to_string()], vec!["tests".to_string()], vec!["py".to_string()], vec!["#".to_string()]));
    assert_eq!(a.s.len(), 1);
    assert!(a.c.contains_key("AAA-01"));

    // spec folders: a generic docs/ counts only with a requirement bullet; Ruby's spec/ holds tests, not specs
    let allow = |p: &Path| spec11_core::detect::allow_list(p);
    let e = std::env::temp_dir().join(format!("spec11-{}-detect2", std::process::id()));
    let _ = fs::remove_dir_all(&e);
    write(&e, "docs/guide.md", "prose\n");
    write(&e, "spec/a_spec.rb", "describe 1\n");
    assert_eq!(spec_dir(&e, &allow(&e)), None);
    write(&e, "openspec/specs/auth/spec.md", "- **AUT-01** MUST — x.\n");
    assert_eq!(spec_dir(&e, &allow(&e)).as_deref(), Some("openspec/specs"));

    // ecosystems, from file lists alone
    let l = |fs: &[&str]| { let v: Vec<String> = fs.iter().map(|s| s.to_string()).collect(); layout(&v).map(|l| (l.profile.name.as_str(), l.src, l.tests)) };
    let s = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(l(&["RunSolo/RunSolo/A.swift", "RunSolo/RunSoloTests/ATests.swift"]), Some(("swift", s(&["RunSolo"]), s(&["RunSolo/RunSoloTests"]))));
    assert_eq!(l(&["Sources/App/A.swift", "Tests/AppTests/ATests.swift"]), Some(("swift", s(&["Sources"]), s(&["Tests"]))));
    assert_eq!(l(&["core/src/a.rs", "core/tests/t.rs", "server/src/b.rs", "web/src/x.ts"]), Some(("rust", s(&["core/src", "server/src"]), s(&["core/src", "core/tests", "server/src"]))));
    assert_eq!(l(&["cmd/x/main.go", "internal/a.go", "internal/a_test.go"]), Some(("go", s(&["cmd", "internal"]), vec![])));
    assert_eq!(l(&["src/main/java/A.java", "src/test/java/ATest.java"]), Some(("jvm", s(&["src/main"]), s(&["src/test"]))));
    assert_eq!(l(&["web/src/a.ts", "web/src/a.test.ts", "web/e2e/b.ts", "web/node_modules/x/y.js"]), Some(("typescript", s(&["web/src"]), s(&["web/e2e"]))));
    assert_eq!(l(&["lib/a.rb", "spec/a_spec.rb", ".hidden/x.rb"]), Some(("ruby", s(&["lib"]), s(&["spec"]))));
    assert_eq!(l(&["lib/a.dart", "test/a_test.dart"]), Some(("dart", s(&["lib"]), s(&["test"]))));
    assert_eq!(l(&["R/a.R", "tests/testthat/test-a.R"]), Some(("r", s(&["R"]), s(&["tests"]))));
    assert_eq!(l(&["README.md"]), None);

    // colocated tests (Go, TS) are told apart from sources by name, so a citation in a test is not a source citation
    let g = std::env::temp_dir().join(format!("spec11-{}-detect3", std::process::id()));
    let _ = fs::remove_dir_all(&g);
    write(&g, "specs/01.md", "- **GOO-01** MUST — x.\n");
    write(&g, "internal/a.go", "package a\n// (GOO-01)\nfunc A() {}\n");
    write(&g, "internal/a_test.go", "package a\n// [GOO\x2D01]\nfunc TestA(t *testing.T) {\n\tt.Fatal(1)\n}\n");
    let a = Analysis::load(Some(&g)).unwrap();
    assert_eq!((a.p.src_files(), a.p.test_files()), (vec!["internal/a.go".to_string()], vec!["internal/a_test.go".to_string()]));
    assert_eq!(a.state["GOO-01"], "tested");
}

#[cfg(unix)]
#[doc = "[SCAN-11]"]
#[test]
fn lenient_spec_formats() {
    let d = std::env::temp_dir().join(format!("spec11-{}-lenient", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    write(&d, "specs/a.md", "\
- LEN-01 MUST — plain.
* **LEN-02**: The system SHALL do it.
1. `LEN-03` SHOULD: numbered.
- **LEN-04:** MAY wrap
  onto a line (manual)
| ID | Level | Text |
|---|---|---|
| LEN-05 | MUST | in a table |
- LEN-06 has no level.
");
    let a = Analysis::load(Some(&d)).unwrap();
    let got: Vec<(&str, &str, &str)> = a.s.values().map(|s| (s.id.as_str(), s.level.as_str(), s.text.as_str())).collect();
    assert_eq!(got, [("LEN-01", "MUST", "plain."), ("LEN-02", "SHALL", "The system SHALL do it."), ("LEN-03", "SHOULD", "numbered."), ("LEN-04", "MAY", "wrap onto a line (manual)"), ("LEN-05", "MUST", "in a table")]);
    assert_eq!(a.s["LEN-04"].mark, "manual");
    assert!(a.issues.iter().any(|x| x.starts_with("LEN-06") && x.contains("has no MUST")));
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("spec11-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}

#[doc = "[SCAN-12]"]
#[test]
fn specs_in_a_single_file_or_anywhere() {
    let one = scratch("onefile");
    write(&one, "SPEC.md", "- REQ-0001 MUST — one.\n");
    let a = Analysis::load(Some(&one)).unwrap();
    assert_eq!(a.s.keys().collect::<Vec<_>>(), ["REQ-0001"]);
    assert_eq!(a.s["REQ-0001"].file, "SPEC.md");

    let named = scratch("namedfile");
    write(&named, "spec11.json", r#"{"specs":"design/all.md"}"#);
    write(&named, "design/all.md", "1. NAM-01 SHALL do it.\n");
    assert_eq!(Analysis::load(Some(&named)).unwrap().s["NAM-01"].file, "design/all.md");

    let any = scratch("anymd");
    write(&any, "notes/deep/a.md", "- **ANY-01** MUST — somewhere.\n");
    let a = Analysis::load(Some(&any)).unwrap();
    assert_eq!(a.s["ANY-01"].file, "notes/deep/a.md");
}

#[doc = "[SCAN-13]"]
#[test]
fn linked_docs_and_adrs_are_reference_docs() {
    let d = scratch("adr");
    write(&d, "spec11.json", r#"{"src":["src"],"tests":["tests"],"ext":["kt"],"comment":["//"]}"#);
    write(&d, "docs/arch/drivers.md", "\
The fold ([state-machine.md](state-machine.md)) decides nothing; see
[ADR\x2D0003](../../decisions/0003-patterns.md) \u{a7}5 and [ADR\x2D0009](../../decisions/0009-gone.md).
Also [site](https://example.com/ADR\x2D0004.md).
");
    write(&d, "docs/arch/state-machine.md", "# Fold\n");
    write(&d, "decisions/0003-patterns.md", "# Architectural patterns\n");
    write(&d, "decisions/0007-other.md", "# Other\n");
    write(&d, "src/A.kt", "// (ADR-0003) the driver\nclass A\n");
    let a = Analysis::load(Some(&d)).unwrap();
    assert_eq!(a.s.keys().collect::<Vec<_>>(), ["ADR-0003", "ADR-0007"]);
    let adr = &a.s["ADR-0003"];
    assert_eq!((adr.file.as_str(), adr.level.as_str(), adr.text.as_str(), a.state["ADR-0003"].as_str()), ("decisions/0003-patterns.md", "DOC", "Architectural patterns", "ref"));
    assert!(a.issues.iter().any(|x| x.starts_with("ADR-0009") && x.contains("does not exist")), "{:?}", a.issues);
    let r = a.report();
    assert_eq!((r.requirements, r.refs, r.due), (0, 2, 0));
    let t = a.trace("ADR-0003").unwrap();
    assert_eq!((t.linked_from.len(), t.source_cites.len()), (1, 1));
    assert_eq!(t.linked_from[0].file, "docs/arch/drivers.md");
}

#[doc = "[SCAN-14, GRAPH-05]"]
#[test]
fn modules_and_test_cases() {
    let d = scratch("modules");
    write(&d, "spec11.json", r#"{"specs":"SPEC.md","src":["a/src","b/src","web/src"],"tests":["a/tests","b/src"],"ext":["rs","ts"],"comment":["//"],"test_files":"\\.test\\.ts$"}"#);
    write(&d, "SPEC.md", "- MOD-01 MUST — x.\n");
    for m in ["a", "b"] {
        write(&d, &format!("{m}/Cargo.toml"), "[package]\n");
        write(&d, &format!("{m}/src/lib.rs"), "pub fn f() {}\n");
    }
    // unit tests inline in `src`: source that declares a test; a marker in a string is not a test
    write(&d, "b/src/lib.rs", "pub fn f() {}\n#[test]\nfn inline() {}\nconst S: &str = \"#[test]\\nfn fake() {}\";\n");
    write(&d, "web/package.json", "{}\n");
    write(&d, "a/tests/t.rs", "#[doc = \"[MOD\x2D01]\"]\n#[test]\nfn works() {\n    assert!(true);\n}\n\n#[test]\nfn plain() {}\n");
    write(&d, "web/src/x.test.ts", "describe('x', () => {\n  it('does x', () => {});\n});\n");
    let a = Analysis::load(Some(&d)).unwrap();
    let t = a.tests_view();
    assert_eq!(t.total, 4);
    let names = |m: &str| t.modules.iter().find(|x| x.module == m).unwrap().files[0].tests.iter().map(|c| (c.name.clone(), c.line, c.ids.clone())).collect::<Vec<_>>();
    assert_eq!(names("a"), [("works".to_string(), 2, vec!["MOD-01".to_string()]), ("plain".to_string(), 7, vec![])]);
    assert_eq!(names("b"), [("inline".to_string(), 2, vec![])]);
    assert_eq!(names("web"), [("does x".to_string(), 2, vec![])]);

    let m = a.modules_view();
    assert!(!m.index && m.note.unwrap().contains("no CodeGraph index"));
    let by = |p: &str| m.modules.iter().find(|x| x.path == p).unwrap();
    assert_eq!((by("a").manifest.as_deref(), by("a").src_files, by("a").test_files, by("a").tests), (Some("Cargo.toml"), 1, 1, 2));
    assert_eq!((by("b").src_files, by("b").test_files, by("b").tests), (1, 0, 1));
    assert_eq!((by("web").manifest.as_deref(), by("web").languages.clone()), (Some("package.json"), vec!["typescript".to_string()]));

    // with an index: symbols per module and the dependency between modules
    fs::create_dir_all(d.join(".codegraph")).unwrap();
    Connection::open(d.join(".codegraph/codegraph.db")).unwrap().execute_batch("
        create table nodes(id text primary key, kind text, name text, qualified_name text, file_path text, start_line int, end_line int, visibility text);
        create table edges(id integer primary key, source text, target text, kind text);
        create table schema_versions(version int, applied_at int, description text);
        insert into schema_versions values (11, 0, 'x');
        insert into nodes values ('a1','function','f','a::f','a/src/lib.rs',1,1,null), ('b1','function','f','b::f','b/src/lib.rs',1,1,null),
                                 ('w1','function','works','a::works','a/tests/t.rs',3,5,null);
        insert into edges(source,target,kind) values ('a1','b1','calls'), ('w1','a1','calls');
    ").unwrap();
    let m = Analysis::load(Some(&d)).unwrap().modules_view();
    assert!(m.index);
    let a = m.modules.iter().find(|x| x.path == "a").unwrap();
    assert_eq!((a.symbols["function"], a.depends_on.iter().map(|x| (x.module.as_str(), x.edges)).collect::<Vec<_>>()), (2, vec![("b", 1)]));
    let q = Analysis::load(Some(&d)).unwrap().tests_view();
    assert_eq!(q.modules.iter().find(|x| x.module == "a").unwrap().files[0].tests[0].qualified, "a::works");
}

#[doc = "[SCAN-10]"]
#[test]
fn symlinks_are_followed_only_inside_root_or_allowed_paths() {
    let d = std::env::temp_dir().join(format!("spec11-{}-links", std::process::id()));
    let out = std::env::temp_dir().join(format!("spec11-{}-links-out", std::process::id()));
    let ok = std::env::temp_dir().join(format!("spec11-{}-links-ok", std::process::id()));
    for x in [&d, &out, &ok] {
        let _ = fs::remove_dir_all(x);
    }
    write(&d, "specs/01.md", "- **LNK-01** MUST — x.\n");
    write(&d, "src/a.py", "# (LNK-01)\n");
    write(&d, "real/b.py", "# (LNK-01)\n");
    write(&out, "c.py", "# (LNK-01)\n");
    write(&ok, "e.py", "# (LNK-01)\n");
    write(&d, "spec11.json", r#"{"src":["src"],"tests":["tests"],"ext":["py"]}"#);
    let (inside, outside, allowed) = (d.join("src/inside"), d.join("src/outside"), d.join("src/allowed"));
    std::os::unix::fs::symlink(d.join("real"), &inside).unwrap();
    std::os::unix::fs::symlink(&out, &outside).unwrap();
    std::os::unix::fs::symlink(&ok, &allowed).unwrap();
    std::os::unix::fs::symlink(out.join("c.py"), d.join("src/file.py")).unwrap(); // a file link, too
    let files = || Analysis::load(Some(&d)).unwrap().p.src_files();
    unsafe { std::env::remove_var("ALLOWED_PATHS") };
    assert_eq!(files(), ["src/a.py", "src/inside/b.py"]);
    unsafe { std::env::set_var("ALLOWED_PATHS", std::env::join_paths([&ok]).unwrap()) };
    assert_eq!(files(), ["src/a.py", "src/allowed/e.py", "src/inside/b.py"]);
    unsafe { std::env::remove_var("ALLOWED_PATHS") };
}

#[doc = "[SCAN-15]"]
#[test]
fn ignore_files_and_agents_are_left_out() {
    let d = scratch("ignored");
    write(&d, "spec11.json", r#"{"src":["src"],"tests":["tests"],"ext":["kt"],"comment":["//"]}"#);
    write(&d, "specs/a.md", "- AAA-01 MUST — one.\n");
    write(&d, ".aiignore", "# generated\nsrc/gen/\n");
    write(&d, ".spec11ignore", "src/skip.kt\n*.tmp.kt\n!src/keep.tmp.kt\n");
    for f in ["src/a.kt", "src/skip.kt", "src/x.tmp.kt", "src/keep.tmp.kt", "src/gen/g.kt", "src/.agents/s.kt"] {
        write(&d, f, "class X\n");
    }
    let p = spec11_core::scan::Project::open(Some(&d)).unwrap();
    assert_eq!(p.src_files(), ["src/a.kt", "src/keep.tmp.kt"]);
    for hidden in ["src/skip.kt", "src/gen/g.kt", ".agents/skills/spec11/SKILL.md", "src/.agents"] {
        assert!(p.ignored(hidden), "{hidden}");
    }
    assert!(!p.ignored("src/a.kt") && !p.ignored("src/keep.tmp.kt"));
}

#[doc = "[SCAN-16]"]
#[test]
fn diff_ranges_map_to_changed_lines() {
    let d = spec11_core::scan::parse_diff("+++ b/specs/a.md\n@@ -1,0 +2,3 @@\n@@ -9 +11 @@\n+++ /dev/null\n@@ -1 +1 @@\n");
    assert_eq!(d["specs/a.md"], vec![(2, 4), (11, 11)]);
    assert_eq!(d.len(), 1);
    let gone = spec11_core::scan::parse_diff("+++ b/x.rs\n@@ -5,2 +4,0 @@\n");
    assert_eq!(gone["x.rs"], vec![(4, 4)]);
}

#[doc = "[SCAN-17]"]
#[test]
fn consecutive_ids_print_as_a_range() {
    assert_eq!(spec11_core::scan::ranges(["DOC-01", "DOC-02", "DOC-03", "DOC-07", "SEC-4"]), "DOC-01..03,DOC-07,SEC-04");
}

#[doc = "[SCAN-08]"]
#[test]
fn a_tests_body_stops_at_its_closing_brace() {
    let ls: Vec<String> = ["@Test(\"[A-1]\")", "func f() {", "  x()", "}", "other"].iter().map(|s| s.to_string()).collect();
    assert_eq!(spec11_core::scan::body(&ls, 1, 200).len(), 4);
}

#[doc = "[SCAN-18, SCAN-19]"]
#[test]
fn changes_split_staged_unstaged_and_untracked() {
    use spec11_core::ChangeOpts;
    let d = scratch("changes");
    write(&d, "spec11.json", r#"{"src":["src"],"tests":["tests"],"ext":["kt"],"comment":["//"]}"#);
    write(&d, "specs/a.md", "- **AAA-01** MUST — one.\n- **AAA-02** MUST — two.\n");
    write(&d, "src/a.kt", "// (AAA-01)\nclass A\n");
    write(&d, "src/b.kt", "// (AAA-02)\nclass B\n");
    let git = |a: &[&str]| assert!(std::process::Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(a).current_dir(&d).output().unwrap().status.success(), "{a:?}");
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-qm", "x"]);
    write(&d, "src/a.kt", "// (AAA-01)\nclass A2\n"); // staged below
    git(&["add", "src/a.kt"]);
    write(&d, "specs/a.md", "- **AAA-01** MUST — one.\n- **AAA-02** MUST — two, edited.\n"); // unstaged
    write(&d, "src/c.kt", "// (AAA-01)\nclass C\n"); // untracked
    let changes = |o: ChangeOpts| Analysis::load(Some(&d)).unwrap().changes(&o).unwrap();
    let states = |c: &spec11_core::Changes| c.files.iter().map(|f| format!("{}:{}:{}:{}", f.path, f.change, f.stage, f.kind)).collect::<Vec<_>>();
    let all = changes(ChangeOpts::default());
    assert_eq!(states(&all), ["specs/a.md:modified:unstaged:spec", "src/a.kt:modified:staged:src", "src/c.kt:added:untracked:src"]);
    assert_eq!(all.requirements.iter().map(|r| (r.id.as_str(), r.changed.join(","))).collect::<Vec<_>>(), [("AAA-01", "src".to_string()), ("AAA-02", "spec".to_string())]);
    let tracked = changes(ChangeOpts { no_untracked: true, ..Default::default() });
    assert_eq!(states(&tracked), ["specs/a.md:modified:unstaged:spec", "src/a.kt:modified:staged:src"]);
    let staged = changes(ChangeOpts { staged: true, ..Default::default() });
    assert_eq!(states(&staged), ["src/a.kt:modified:staged:src"]);
    assert_eq!(staged.requirements.len(), 1);
    let a = Analysis::load(Some(&d)).unwrap();
    let o = ChangeOpts::default();
    let c = a.file_diff("src/c.kt", &o).unwrap();
    assert!(c.change == "added" && c.diff.contains("+class C") && c.diff.contains("@@ -0,0 +1,2 @@"), "{}", c.diff);
    assert!(a.file_diff("src/a.kt", &o).unwrap().diff.contains("+class A2"));
    assert!(a.file_diff("src/b.kt", &o).is_err() && a.file_diff("../x", &o).is_err());
    // status takes the same switches
    let o = StatusOpts { staged: true, ..Default::default() };
    assert_eq!(Analysis::load(Some(&d)).unwrap().status(&o).unwrap().rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["AAA-01"]);
    let o = StatusOpts { changed: Some("HEAD".into()), no_untracked: true, ..Default::default() };
    assert_eq!(Analysis::load(Some(&d)).unwrap().status(&o).unwrap().rows.len(), 2);
}

#[doc = "[SCAN-18, SCAN-19, SCAN-20]"]
#[test]
fn changes_by_ref_commit_rename_delete_whitespace_and_history() {
    use spec11_core::ChangeOpts;
    let d = scratch("changes2");
    write(&d, "spec11.json", r#"{"src":["src"],"tests":["tests"],"ext":["kt"],"comment":["//"]}"#);
    write(&d, ".spec11ignore", "hidden/\n");
    write(&d, "specs/a.md", "- **AAA-01** MUST — one.\n- **AAA-02** MUST — two.\n- **AAA-03** MUST — three.\n");
    write(&d, "tests/AT.kt", "// [AAA\x2D01]\nfun t1() {\n    check(1)\n}\n\n// [AAA\x2D02]\nfun t2() {\n    check(2)\n}\n");
    write(&d, "src/old.kt", "// (AAA-01)\nfun f() {\n    1\n}\n");
    write(&d, "notes.txt", "n\n");
    write(&d, "hidden/h.kt", "x\n");
    let git = |a: &[&str]| {
        let r = std::process::Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(a).current_dir(&d).output().unwrap();
        assert!(r.status.success(), "{a:?}");
        String::from_utf8_lossy(&r.stdout).trim().to_string()
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["add", "."]);
    git(&["commit", "-qm", "base"]);
    git(&["checkout", "-qb", "feature"]);
    // commit 1: edit the AAA-02 test only, drop AAA-03, rename old.kt
    write(&d, "tests/AT.kt", "// [AAA\x2D01]\nfun t1() {\n    check(1)\n}\n\n// [AAA\x2D02]\nfun t2() {\n    check(22)\n}\n");
    write(&d, "specs/a.md", "- **AAA-01** MUST — one.\n- **AAA-02** MUST — two.\n");
    git(&["mv", "src/old.kt", "src/new.kt"]);
    write(&d, "notes.txt", "n2\n");
    write(&d, "hidden/h.kt", "y\n");
    git(&["add", "."]);
    git(&["commit", "-qm", "work"]);
    let sha = git(&["rev-parse", "HEAD"]);
    // working tree: whitespace-only edit
    write(&d, "src/new.kt", "// (AAA-01)\nfun f() {\n      1\n}\n");
    let a = Analysis::load(Some(&d)).unwrap();
    let ids = |c: &spec11_core::Changes| c.requirements.iter().map(|r| format!("{}:{}", r.id, r.changed.join("+"))).collect::<Vec<_>>();
    let paths = |c: &spec11_core::Changes| c.files.iter().map(|f| format!("{}:{}:{}", f.path, f.change, f.stage)).collect::<Vec<_>>();

    // against HEAD: only the whitespace edit; ignoring whitespace leaves nothing
    let head = a.changes(&ChangeOpts::default()).unwrap();
    assert_eq!(paths(&head), ["src/new.kt:modified:unstaged"]);
    assert!(a.changes(&ChangeOpts { ignore_whitespace: true, ..Default::default() }).unwrap().files.is_empty());

    // against main: committed + working tree; renames, deleted requirement, one test touched (not the other)
    let o = ChangeOpts { reference: "main".into(), ..Default::default() };
    let c = a.changes(&o).unwrap();
    assert_eq!(paths(&c), ["specs/a.md:modified:committed", "src/new.kt:renamed:unstaged", "tests/AT.kt:modified:committed"]);
    assert_eq!(c.files[1].old_path.as_deref(), Some("src/old.kt"));
    assert_eq!(ids(&c), ["AAA-01:src", "AAA-02:test", "AAA-03:spec"], "{c:?}");
    let gone = c.requirements.iter().find(|r| r.id == "AAA-03").unwrap();
    assert!(gone.deleted && gone.state == "deleted" && gone.text.contains("three"), "{gone:?}");
    assert!(c.text().contains("(from src/old.kt)"));
    // show_ignored and show_all widen the list
    let wide = a.changes(&ChangeOpts { show_ignored: true, show_all: true, ..o.clone() }).unwrap();
    assert!(paths(&wide).contains(&"hidden/h.kt:modified:committed".to_string()) && paths(&wide).contains(&"notes.txt:modified:committed".to_string()), "{:?}", paths(&wide));
    assert!(!paths(&c).iter().any(|p| p.starts_with("notes.txt") || p.starts_with("hidden")));

    // the diff of the test file says which requirement each hunk touches
    let fd = a.file_diff("tests/AT.kt", &o).unwrap();
    assert!(fd.diff.contains("+    check(22)") && fd.hunks.len() == 1, "{}", fd.diff);
    assert_eq!(fd.hunks[0].requirements.iter().map(|r| format!("{}:{}", r.id, r.role)).collect::<Vec<_>>(), ["AAA-02:test"]);
    assert!(fd.text().contains("↳ AAA-02 test changed"), "{}", fd.text());
    assert!(a.file_diff("src/new.kt", &o).unwrap().diff.contains("rename from src/old.kt"));
    assert!(a.file_diff("notes.txt", &o).is_err(), "an unlisted (non-scanned) file is refused");

    // one commit explained
    let one = a.changes(&ChangeOpts { commit: sha.clone(), ..Default::default() }).unwrap();
    assert_eq!(one.commit.as_deref(), Some(sha.as_str()));
    assert_eq!(ids(&one), ["AAA-02:test", "AAA-03:spec"]);
    assert!(one.files.iter().all(|f| f.stage == "committed"));
    assert!(a.changes(&ChangeOpts { commit: "--output=x".into(), ..Default::default() }).is_err());

    // history: the branch commit is not on main
    let h = a.p.history("", "", 10).unwrap();
    assert_eq!((h.base.as_deref(), h.on_base, h.commits.len(), h.commits[0].subject.as_str()), (Some("main"), false, 1, "work"));
    assert_eq!(h.uncommitted, 1);
    git(&["checkout", "-qf", "main"]);
    let h = Analysis::load(Some(&d)).unwrap().p.history("", "", 10).unwrap();
    assert!(h.on_base && h.commits[0].subject == "base");
}

#[doc = "[SCAN-18]"]
#[test]
fn braces_in_strings_do_not_stretch_a_unit() {
    use spec11_core::ChangeOpts;
    let d = scratch("braces");
    write(&d, "spec11.json", r#"{"src":["src"],"tests":["tests"],"ext":["kt"],"comment":["//"]}"#);
    write(&d, "specs/a.md", "- **AAA-01** MUST — one.\n- **AAA-02** MUST — two.\n");
    // the first test opens a brace inside a string and never closes it in code
    let t = |b: &str| format!("// [AAA\\x2D01]\nfun t1() {{\n    check(\"{{\")  // {{\n}}\n\n// [AAA\\x2D02]\nfun t2() {{\n    check({b})\n}}\n").replace("\\x2D", "-");
    write(&d, "tests/AT.kt", &t("1"));
    let git = |a: &[&str]| assert!(std::process::Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(a).current_dir(&d).output().unwrap().status.success(), "{a:?}");
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-qm", "x"]);
    write(&d, "tests/AT.kt", &t("2"));
    let c = Analysis::load(Some(&d)).unwrap().changes(&ChangeOpts::default()).unwrap();
    assert_eq!(c.requirements.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["AAA-02"], "{c:?}");
}
