//! Read-only SQL over the CodeGraph index (`.codegraph/codegraph.db`). Every query lives in this file,
//! so a CodeGraph schema change touches only this one.
use crate::scan::Refs;
use rusqlite::{Connection, OpenFlags, params};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt,
    path::Path,
};

/// Edge kinds that mean "uses".
const USES: &str = "'calls','instantiates','references','extends','implements'";
pub const UNIT_KINDS: [&str; 5] = ["struct", "class", "enum", "interface", "function"];
const DECL_KINDS: &str = "'method','function','field','struct','class','enum','interface','property','constant','variable','type_alias'";

#[derive(Debug)]
pub enum GraphError {
    /// The index exists but does not have the tables/columns this version reads.
    SchemaDrift(String),
    Sql(String),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GraphError::SchemaDrift(m) => write!(f, "CodeGraph index schema not supported ({m}); update spec11 or re-run `codegraph init`"),
            GraphError::Sql(m) => write!(f, "CodeGraph index unreadable: {m}"),
        }
    }
}

impl From<rusqlite::Error> for GraphError {
    fn from(e: rusqlite::Error) -> Self {
        GraphError::Sql(e.to_string())
    }
}

type R<T> = Result<T, GraphError>;

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub qualified: String,
    pub file: String,
    pub lo: i64,
    pub hi: i64,
    pub visibility: String,
}

pub struct Graph {
    c: Connection,
    pub version: Option<i64>,
}

const NEEDED: [(&str, &[&str]); 2] = [
    ("nodes", &["id", "kind", "name", "qualified_name", "file_path", "start_line", "end_line", "visibility"]),
    ("edges", &["source", "target", "kind"]),
];

impl Graph {
    // (GRAPH-01, GRAPH-02, GRAPH-03)
    /// `Ok(None)` when there is no index: indexing is the user's decision, never an error.
    pub fn open(db: &Path) -> R<Option<Graph>> {
        if !db.exists() {
            return Ok(None);
        }
        let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        for (table, cols) in NEEDED {
            let have: HashSet<String> = c.prepare(&format!("select name from pragma_table_info('{table}')"))?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
            let missing: Vec<&str> = cols.iter().copied().filter(|x| !have.contains(*x)).collect();
            if !missing.is_empty() {
                return Err(GraphError::SchemaDrift(format!("{table} lacks {}", missing.join(", "))));
            }
        }
        let version = c.query_row("select max(version) from schema_versions", [], |r| r.get(0)).ok().flatten();
        Ok(Some(Graph { c, version }))
    }

    /// {id: app files (relative to `app`)} each tagged test reaches through calls/instantiates/references, up to 3 hops, through test helpers.
    /// An id with tests but no files has tests the graph cannot connect to app code.
    pub fn code_links(&self, tests: &Refs, app: &str, is_test: &dyn Fn(&str) -> bool) -> R<HashMap<String, BTreeSet<String>>> {
        let mut inside = self.c.prepare("select id from nodes where file_path=?1 and start_line<=?2 and end_line>=?2 and kind in ('method','function') order by end_line-start_line limit 1")?;
        let mut out_edges = self.c.prepare(&format!("select e.target, n.file_path from edges e join nodes n on n.id=e.target where e.source=?1 and e.kind in ({USES})"))?;
        let mut out = HashMap::new();
        for (id, refs) in tests {
            let files: &mut BTreeSet<String> = out.entry(id.clone()).or_default();
            for r in refs {
                let mut frontier: Vec<String> = inside.query_map(params![r.file, r.line as i64], |x| x.get(0))?.collect::<Result<_, _>>()?;
                let mut seen = HashSet::new();
                for _ in 0..3 {
                    let mut next = vec![];
                    for src in &frontier {
                        let rows: Vec<(String, String)> = out_edges.query_map([src], |x| Ok((x.get(0)?, x.get(1)?)))?.collect::<Result<_, _>>()?;
                        for (dst, path) in rows {
                            if is_test(&path) {
                                if seen.insert(dst.clone()) {
                                    next.push(dst);
                                }
                            } else if let Some(rest) = path.strip_prefix(app) {
                                files.insert(rest.to_string());
                            }
                        }
                    }
                    frontier = next;
                }
            }
        }
        Ok(out)
    }

    /// Repo-relative paths of the files the index covers (generated ones left out): what layout detection starts from.
    pub fn file_paths(&self) -> R<Vec<String>> {
        let mut st = self.c.prepare("select path from files where generated=0 order by path")?;
        Ok(st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
    }

    /// (start, end, qualified name) of declarations in `file`.
    pub fn decls_in(&self, file: &str) -> R<Vec<(i64, i64, String)>> {
        let mut st = self.c.prepare(&format!("select start_line, end_line, qualified_name from nodes where file_path=?1 and kind in ({DECL_KINDS})"))?;
        Ok(st.query_map([file], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?)
    }

    /// Every node under `prefix`, in index order.
    pub fn nodes_under(&self, prefix: &str) -> R<Vec<Node>> {
        let mut st = self.c.prepare("select id, kind, name, qualified_name, file_path, start_line, end_line, coalesce(visibility,'') from nodes where file_path like ?1")?;
        let rows = st.query_map([format!("{prefix}%")], |r| {
            Ok(Node { id: r.get(0)?, kind: r.get(1)?, name: r.get(2)?, qualified: r.get(3)?, file: r.get(4)?, lo: r.get(5)?, hi: r.get(6)?, visibility: r.get(7)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// (source, target) of every "uses" edge.
    pub fn use_edges(&self) -> R<Vec<(String, String)>> {
        let mut st = self.c.prepare(&format!("select source, target from edges where kind in ({USES})"))?;
        Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?)
    }

    /// Direct callers of a symbol id.
    pub fn callers(&self, id: &str) -> R<Vec<Node>> {
        let mut st = self.c.prepare("select c.id, c.kind, c.name, c.qualified_name, c.file_path, c.start_line, c.end_line, coalesce(c.visibility,'') from edges e join nodes c on c.id=e.source where e.target=?1 and e.kind='calls'")?;
        let rows = st.query_map([id], node_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Transitive callers of a symbol id, depth-capped.
    pub fn impact(&self, id: &str, depth: i64) -> R<Vec<Node>> {
        let mut st = self.c.prepare(
            "with recursive up(id, depth) as (select ?1, 0 union select e.source, depth + 1 from edges e join up on e.target = up.id where e.kind = 'calls' and depth < ?2) \
             select n.id, n.kind, n.name, n.qualified_name, n.file_path, n.start_line, n.end_line, coalesce(n.visibility,'') from up join nodes n on n.id = up.id",
        )?;
        let rows = st.query_map(params![id, depth], node_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Full-text search over symbol names (`nodes_fts`).
    pub fn search(&self, query: &str, limit: i64) -> R<Vec<Node>> {
        let mut st = self.c.prepare(
            "select n.id, n.kind, n.name, n.qualified_name, n.file_path, n.start_line, n.end_line, coalesce(n.visibility,'') from nodes_fts f join nodes n on n.rowid = f.rowid where nodes_fts match ?1 order by rank limit ?2",
        )?;
        let rows = st.query_map(params![query, limit], node_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

fn node_row(r: &rusqlite::Row) -> rusqlite::Result<Node> {
    Ok(Node { id: r.get(0)?, kind: r.get(1)?, name: r.get(2)?, qualified: r.get(3)?, file: r.get(4)?, lo: r.get(5)?, hi: r.get(6)?, visibility: r.get(7)? })
}
