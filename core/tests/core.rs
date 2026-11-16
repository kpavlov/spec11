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
    let l = |fs: &[&str]| { let v: Vec<String> = fs.iter().map(|s| s.to_string()).collect(); layout(&v).map(|l| (l.profile.name, l.src, l.tests)) };
    let s = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(l(&["RunSolo/RunSolo/A.swift", "RunSolo/RunSoloTests/ATests.swift"]), Some(("swift", s(&["RunSolo"]), s(&["RunSolo/RunSoloTests"]))));
    assert_eq!(l(&["Sources/App/A.swift", "Tests/AppTests/ATests.swift"]), Some(("swift", s(&["Sources"]), s(&["Tests"]))));
    assert_eq!(l(&["core/src/a.rs", "core/tests/t.rs", "server/src/b.rs", "web/src/x.ts"]), Some(("rust", s(&["core/src", "server/src"]), s(&["core/src", "core/tests", "server/src"]))));
    assert_eq!(l(&["cmd/x/main.go", "internal/a.go", "internal/a_test.go"]), Some(("go", s(&["cmd", "internal"]), vec![])));
    assert_eq!(l(&["src/main/java/A.java", "src/test/java/ATest.java"]), Some(("jvm", s(&["src/main"]), s(&["src/test"]))));
    assert_eq!(l(&["web/src/a.ts", "web/src/a.test.ts", "web/e2e/b.ts", "web/node_modules/x/y.js"]), Some(("typescript", s(&["web/src"]), s(&["web/e2e"]))));
    assert_eq!(l(&["lib/a.rb", "spec/a_spec.rb", ".hidden/x.rb"]), Some(("ruby", s(&["lib"]), s(&["spec"]))));
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
