#!/usr/bin/env python3
"""Code no requirement covers, from the CodeGraph index: candidates for a missing requirement.

usage: uncovered.py [PATH] [--fanin N] [--min N]        (also `status.py --uncovered [PATH]`)
A top-level type or function of the app is listed only if ALL hold:
  - no spec ID is cited in a comment inside it or just above it;
  - no test tagged `[ID]` names it;
  - cited code does not use it directly (one hop: calls, instantiates, references);
  - it is not shared infrastructure: used from fewer than --fanin other files (default 5), outside UI/Components, UI/DataTypes and the app entry;
  - it is internal or public, and at least --min lines (default 10).
"""
import argparse, re, sqlite3, sys
from pathlib import Path
sys.dont_write_bytecode = True  # no __pycache__ beside the scripts
sys.path.insert(0, str(Path(__file__).resolve().parent))
import common

APP = "RunSolo/RunSolo/"
INFRA_PATHS = ("UI/Components/", "UI/DataTypes/", "RunSoloApp.swift")  # generic controls, styling, app entry: no business rule of their own
UNIT_KINDS = ("struct", "class", "enum", "interface", "function")
USES = common.USES


def run(base: Path, only: str = "", fanin: int = 5, min_lines: int = 10) -> None:
    db = base / ".codegraph" / "codegraph.db"
    if not db.exists():
        sys.exit("spec-driven: no .codegraph index here; the user decides about indexing")
    c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    nodes = {r[0]: r[1:] for r in c.execute(
        "select id, kind, name, qualified_name, file_path, start_line, end_line, visibility from nodes where file_path like ?", (APP + "%",))}

    def unit_of(nid):  # the top-level declaration a node lives in
        n = nodes.get(nid)
        return None if not n or n[0] in ("file", "import") else (n[3], n[2].split("::")[0])

    units = {}  # (file, name) -> dict
    for nid, (kind, name, qn, f, lo, hi, vis) in nodes.items():
        if kind in UNIT_KINDS and "::" not in qn:
            units.setdefault((f, name), dict(name=name, file=f, lo=lo, hi=hi, vis=vis, kind=kind))

    S, T, C = common.specs(base), common.tests(base), common.sources(base)
    cited = {(rel, n) for refs in C.values() for rel, n, _ in refs}
    tested = set()  # names that appear in the body of a test tagged with a real requirement
    for i, refs in T.items():
        for rel, n, _ in refs:
            for line in common.body(common.lines(base / rel), n):
                tested.update(re.findall(r"\b[A-Za-z_]\w{3,}\b", line))

    covered = {k for k, u in units.items() if any(r == k[0] and u["lo"] - 4 <= n <= u["hi"] for r, n in cited)}
    fan, used_by_cited = {}, set()
    for src, dst, kind in c.execute("select source, target, kind from edges"):
        if kind not in USES:
            continue
        a, b = unit_of(src), unit_of(dst)
        if not a or not b or a == b:
            continue
        if a[0] != b[0]:
            fan.setdefault(b, set()).add(a[0])
        if a in covered:
            used_by_cited.add(b)

    out, skipped = [], dict(covered=0, infra=0, shared=0, small=0)
    for k, u in units.items():
        if k in covered or k in used_by_cited or u["name"] in tested:
            skipped["covered"] += 1
        elif u["file"].removeprefix(APP).startswith(INFRA_PATHS):
            skipped["infra"] += 1
        elif len(fan.get(k, ())) >= fanin:
            skipped["shared"] += 1
        elif u["vis"] in ("private", "fileprivate") or u["hi"] - u["lo"] + 1 < min_lines:
            skipped["small"] += 1
        elif only in u["file"]:
            out.append(u)

    by = {}
    for u in out:
        by.setdefault(u["file"].removeprefix(APP), []).append(u)
    size = lambda us: sum(u["hi"] - u["lo"] + 1 for u in us)
    print(f"{len(out)} units / {size(out)} lines in {len(by)} files have no requirement"
          f" (skipped: {skipped['covered']} covered, {skipped['shared']} shared by >={fanin} files, {skipped['infra']} controls/styling, {skipped['small']} small or private)")
    for f, us in sorted(by.items(), key=lambda kv: -size(kv[1]))[:30]:
        print(f"  {f}  {size(us)} lines: " + ", ".join(f"{u['name']}:{u['lo']}-{u['hi']}" for u in sorted(us, key=lambda u: u["lo"])))
    if len(by) > 30:
        print(f"  … {len(by) - 30} more files (give a PATH to narrow)")
    if out:
        print("next: codegraph explore \"<Name>\" to read one, then decide: business rule -> write the requirement; plumbing -> ignore")


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("path", nargs="?", default="", help="only files whose path contains this")
    ap.add_argument("--fanin", type=int, default=5)
    ap.add_argument("--min", type=int, default=10, dest="min_lines")
    a = ap.parse_args()
    run(common.root(), a.path, a.fanin, a.min_lines)
