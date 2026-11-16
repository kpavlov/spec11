#!/usr/bin/env python3
"""Code that more than one requirement covers, from the CodeGraph index: where two specs may conflict or complement each other.

usage: overlap.py [PATH] [--all]            (also `status.py --overlap [PATH]`)
A declaration (method, function, type, property) counts as covered by every requirement ID cited in a comment inside it, or in the doc comment right above it.
It is listed when two or more IDs cover it. Each pair is `linked` when one spec's text cites the other (or both cite a third ID):
the specs already know each other, so they are probably complementary. An `unlinked` pair has no declared relation: read both, they may conflict.
Pairs from the same spec file are shown only with --all.
"""
import argparse, sqlite3, sys
from itertools import combinations
from pathlib import Path
sys.dont_write_bytecode = True  # no __pycache__ beside the scripts
sys.path.insert(0, str(Path(__file__).resolve().parent))
import common

APP = "RunSolo/RunSolo/"


def find(base: Path, only: str = "", same_file: bool = False):
    """[(file, line, name, [ids], [(a, b, linked)])] for declarations covered by two or more requirements, unlinked cross-file pairs first; None without an index."""
    db = base / ".codegraph" / "codegraph.db"
    if not db.exists():
        return None
    c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    S, C = common.specs(base), common.sources(base)
    nodes = {}
    for rel in {rel for refs in C.values() for rel, _, _ in refs}:
        nodes[rel] = c.execute("select start_line, end_line, qualified_name from nodes where file_path=? and kind in ('method','function','field','struct','class','enum','interface','property','constant','variable','type_alias')", (rel,)).fetchall()
    cites = {i: set(common.MENTION_RE.findall(s["text"])) - {i} for i, s in S.items()}
    covered = {}  # (file, lo, name) -> ids
    for i, refs in C.items():
        if i not in S:
            continue
        for rel, n, text in refs:
            ns = nodes.get(rel, [])
            if text.lstrip().startswith("///"):  # a doc comment belongs to the declaration below it
                ahead = [x for x in ns if n < x[0] <= n + 8]
                node = min(ahead, key=lambda x: x[0], default=None)
            else:
                inside = [x for x in ns if x[0] <= n <= x[1]]
                node = min(inside, key=lambda x: x[1] - x[0], default=None)
            if node:
                covered.setdefault((rel, node[0], node[2]), set()).add(i)
    out = []
    for (rel, lo, name), ids in covered.items():
        if len(ids) < 2 or only not in rel:
            continue
        pairs = []
        for a, b in combinations(sorted(ids, key=lambda x: (x.split("-")[0], int(x.split("-")[1]))), 2):
            linked = b in cites[a] or a in cites[b] or bool(cites[a] & cites[b])
            if S[a]["file"] != S[b]["file"] or same_file:
                pairs.append((a, b, linked))
        if pairs:
            out.append((rel, lo, name, sorted(ids), pairs))
    out.sort(key=lambda r: (all(p[2] for p in r[4]), r[0], r[1]))  # any unlinked pair first
    return out


def run(base: Path, only: str = "", same_file: bool = False, limit: int = 25) -> int:
    found = find(base, only, same_file)
    if found is None:
        sys.exit("spec-driven: no .codegraph index here; the user decides about indexing")
    S = common.specs(base)
    unlinked = [r for r in found if not all(p[2] for p in r[4])]
    print(f"{len(found)} declarations are covered by 2+ requirements; {len(unlinked)} have a pair whose specs do not cite each other (read those first)")
    for rel, lo, name, ids, pairs in found[:limit]:
        print(f"  {rel.removeprefix(APP)}:{lo}  {name}  [{', '.join(ids)}]")
        for a, b, linked in pairs[:6]:
            print(f"    {a} {'~' if linked else '?'} {b}  {'linked' if linked else 'UNLINKED'}  {S[a]['file'][:-3]} / {S[b]['file'][:-3]}")
    if len(found) > limit:
        print(f"  … {len(found) - limit} more (give a PATH to narrow)")
    if found:
        print("next: trace.py <ID> for each spec's text; decide conflicting / complementary / duplicate / incidental per SKILL.md")
    return 0


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("path", nargs="?", default="", help="only files whose path contains this")
    ap.add_argument("--all", action="store_true", help="also pairs from the same spec file")
    a = ap.parse_args()
    sys.exit(run(common.root(), a.path, a.all))
