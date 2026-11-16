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
@Test(\"[DEM-01] works\") func a() {
    #expect(true)
}
@Test(\"[DEM-05] built already\") func b() {
    let x = 1
}
@Test(\"[DEM-99] unknown\") func c() {
    #expect(true)
}
@Test(\"[DEM-10] retired\") func e() {
    #expect(true)
}
// [DEM-03] only in a comment
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
