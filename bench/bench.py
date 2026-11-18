#!/usr/bin/env python3
"""Time spec11 on a generated repo, at several git revisions: a before/after baseline.

    python3 bench/bench.py                      # main vs HEAD
    python3 bench/bench.py e7d83fd HEAD .       # any revs; `.` is the working tree, uncommitted changes included
    python3 bench/bench.py --scale 4 --runs 20  # a bigger repo, more runs
    python3 bench/bench.py --json               # one JSON document on stdout (progress goes to stderr)

Each rev is built once (release, UI not built) into target/bench/bin and cached by commit; `.` is rebuilt every time.
The repo is generated deterministically into target/bench/repo-<scale>: bullets only, so every rev reads it the same way,
with non-ASCII spec text (the em dash of a normal bullet, Cyrillic, CJK) and no CodeGraph index. Each command is a full
load (`Analysis::load`), so the numbers are mostly scan time. Outputs are compared across revs: a speed-up must not change them.
Stdlib only.
"""
import argparse, hashlib, json, os, shutil, statistics, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / "target" / "bench"
COMMANDS = [["report"], ["status", "--all"], ["trace", "GEN-0001"]]


def generate(scale: int) -> Path:
    repo = BENCH / f"repo-{scale}"
    if (repo / ".done").exists():
        return repo
    shutil.rmtree(repo, ignore_errors=True)
    n_specs, per_spec, n_src, n_tests, n_docs = 40 * scale, 50, 400 * scale, 100 * scale, 100 * scale
    w = lambda rel, text: ((repo / rel).parent.mkdir(parents=True, exist_ok=True), (repo / rel).write_text(text, encoding="utf-8"))
    w("Cargo.toml", '[package]\nname = "gen"\nversion = "0.1.0"\nedition = "2021"\n')
    w("README.md", "# gen\n\nA generated repo for timing spec11.\n")
    ids = [f"GEN-{i:04d}" for i in range(1, n_specs * per_spec + 1)]
    for s in range(n_specs):
        lines = [f"# Spec {s}", "", "Требования — 要件 — exigences.", ""]
        for k in range(per_spec):
            i = s * per_spec + k
            level = ["MUST", "SHOULD", "MAY"][i % 3]
            mark = " (later)" if i % 17 == 0 else ""
            lines.append(f"- **{ids[i]}** {level} — handle case {i}: café, naïve, 日本語, see `cfg.{i}`{mark}.")
            if i % 5 == 0:
                lines.append(f"  The wrapped line of {ids[i]} cites {ids[(i + 1) % len(ids)]}.")
        w(f"specs/{s:03d}-area.md", "\n".join(lines) + "\n")
    for f in range(n_src):
        body = [f"//! Module {f} — generated."]
        for k in range(20):
            i = (f * 20 + k) % len(ids)
            body += [f"// ({ids[i]}) why this exists — ошибка обработана", f"pub fn f{f}_{k}(x: u32) -> u32 {{", "    let y = x + 1; // plain comment", "    y * 2", "}", ""]
        w(f"src/m{f:04d}.rs", "\n".join(body))
    for f in range(n_tests):
        body = [f"use gen::*;", ""]
        for k in range(10):
            i = (f * 10 + k) % len(ids)
            body += [f'#[doc = "[{ids[i]}]"]', "#[test]", f"fn t{f}_{k}() {{", f"    assert_eq!(f0_0({k}), {2 * (k + 1)});", "}", ""]
        w(f"tests/t{f:04d}.rs", "\n".join(body))
    for f in range(n_docs):
        w(f"docs/guide/{f:03d}.md", f"# Guide {f}\n\nProse that mentions {ids[f % len(ids)]} in passing — and [a link](../guide/{(f + 1) % n_docs:03d}.md).\n" * 20)
    (repo / ".done").write_text("")
    return repo


def sha(rev: str) -> str:
    return subprocess.run(["git", "rev-parse", "--short", rev], cwd=ROOT, check=True, capture_output=True, text=True).stdout.strip()


def build(rev: str) -> Path:
    env = {**os.environ, "SKIP_WEB_BUILD": "1", "CARGO_TARGET_DIR": str(BENCH / "target")}
    out = BENCH / "bin"
    out.mkdir(parents=True, exist_ok=True)
    if rev == ".":
        # the repo's own target/: its files are older than the exported revs' builds, so the shared dir would look fresh
        subprocess.run(["cargo", "build", "-q", "--release", "-p", "spec11"], cwd=ROOT, env={**os.environ, "SKIP_WEB_BUILD": "1"}, check=True)
        shutil.copy2(ROOT / "target" / "release" / "spec11", out / "spec11-worktree")
        return out / "spec11-worktree"
    else:
        dst = out / f"spec11-{sha(rev)}"
        if dst.exists():
            return dst
        # an exported tree, not a worktree: nothing is written to .git. `-m` dates the files now: the shared target dir must
        # see them as newer than the last rev's build, or cargo reuses its spec11-core
        src = BENCH / f"src-{sha(rev)}"
        shutil.rmtree(src, ignore_errors=True)
        src.mkdir(parents=True)
        archive = subprocess.run(["git", "archive", rev], cwd=ROOT, check=True, capture_output=True).stdout
        subprocess.run(["tar", "-xm", "--exclude", ".mcp.json", "--exclude", ".claude", "-C", str(src)], input=archive, check=True)
        try:
            subprocess.run(["cargo", "build", "-q", "--release", "-p", "spec11"], cwd=src, env=env, check=True)
        finally:
            shutil.rmtree(src, ignore_errors=True)
    shutil.copy2(BENCH / "target" / "release" / "spec11", dst)
    return dst


def run(binary: Path, repo: Path, cmd: list, runs: int):
    argv = [str(binary), "--root", str(repo), *cmd]
    out = subprocess.run(argv, capture_output=True).stdout  # warm-up: file cache, page cache
    times = []
    for _ in range(runs):
        t = time.perf_counter()
        subprocess.run(argv, capture_output=True)
        times.append((time.perf_counter() - t) * 1000)
    return times, hashlib.sha256(out).hexdigest()[:12]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("revs", nargs="*", default=["main", "HEAD"])
    ap.add_argument("--scale", type=int, default=1, help="repo size: 1 = 2,000 requirements, 400 source and 100 test files")
    ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--json", action="store_true", help="print one JSON document instead of the Markdown table")
    a = ap.parse_args()
    repo = generate(a.scale)
    print(f"repo: {repo}", file=sys.stderr)
    bins = {}
    for rev in a.revs:
        print(f"building {rev} …", file=sys.stderr)
        bins[rev] = build(rev)
    revs = [{"rev": r, "commit": "worktree" if r == "." else sha(r)} for r in bins]
    rows = []
    for cmd in COMMANDS:
        for rev, b in bins.items():
            ts, h = run(b, repo, cmd, a.runs)
            rows.append({"command": " ".join(cmd), "rev": rev, "median_ms": round(statistics.median(ts), 1), "min_ms": round(min(ts), 1), "stdev_ms": round(statistics.pstdev(ts), 1), "output": h})
    base = a.revs[0]
    for r in rows:  # change of the median against the first rev, the baseline: negative is faster
        b = next(x for x in rows if x["command"] == r["command"] and x["rev"] == base)
        r["vs_baseline_pct"] = round((r["median_ms"] / b["median_ms"] - 1) * 100, 1)
    differs = sorted({r["command"] for r in rows if any(x["command"] == r["command"] and x["output"] != r["output"] for x in rows)})
    if a.json:
        print(json.dumps({"baseline": base, "revs": revs, "scale": a.scale, "runs": a.runs, "unit": "ms", "results": rows, "same_output": not differs, "output_differs": differs}, indent=2))
        return
    print(f"Baseline `{base}`; scale {a.scale}, {a.runs} runs, wall time per command (ms). Revs: " + ", ".join(f"`{r['rev']}` = {r['commit']}" for r in revs) + ".\n")
    print("| command | rev | median | min | stdev | vs baseline | output |")
    print("|---|---|--:|--:|--:|--:|---|")
    for r in rows:
        print(f"| {r['command']} | {r['rev']} | {r['median_ms']} | {r['min_ms']} | {r['stdev_ms']} | {r['vs_baseline_pct']:+.1f}% | {r['output']} |")
    print("\nOutput: " + ("same for every rev." if not differs else "DIFFERS for " + ", ".join(differs) + "."))


if __name__ == "__main__":
    main()
