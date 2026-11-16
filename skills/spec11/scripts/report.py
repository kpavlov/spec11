#!/usr/bin/env python3
"""Whole-project SDD status as plain text, the same every run: coverage, problems and where to look next. No model involved.

usage: report.py [--limit N]     (N lines per problem section, default 12)
"""
import argparse, subprocess, sys
from pathlib import Path
sys.dont_write_bytecode = True  # no __pycache__ beside the scripts
here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
import common, overlap

ap = argparse.ArgumentParser()
ap.add_argument("--limit", type=int, default=12)
L = ap.parse_args().limit

base = common.root()
issues = []
S, T, C = common.specs(base, issues), common.tests(base), common.sources(base)
common.check_refs(S, issues)
state = common.states(S, T, C)
links = common.code_links(base, T)
by = lambda ids: sorted(ids, key=lambda x: (x.split("-")[0], int(x.split("-")[1])))
due = [i for i in S if state[i] in ("tested", "src-only", "none")]
none, src_only = by(i for i in due if state[i] == "none"), by(i for i in due if state[i] == "src-only")
built = by(i for i in S if state[i] == "later" and (i in T or i in C))
unproven = [i for i in by(S) if S[i]["done"] and common.verdict(i, T, links) in ("NO-TEST", "NO-LINK")]
bad_tests = common.test_problems(base, S, T)
dangling_src, dangling_test = common.dangling(S, C), common.dangling(S, T)
overlaps = overlap.find(base) or []
unlinked = [r for r in overlaps if not all(p[2] for p in r[4])]
files = sorted({s["file"] for s in S.values()})
n_tested = sum(state[i] == "tested" for i in due)


def per_spec(ids):
    """`15-contracts: CON-01..03, CON-08` lines, one per spec file."""
    out = {}
    for i in ids:
        out.setdefault(S[i]["file"][:-3], []).append(i)
    return [f"  {f}: {common.ranges(v)}" for f, v in sorted(out.items())]


def section(title, count, lines, hint=""):
    print(f"\n[{title}] {count}" if count else f"\n[{title}] none")
    for x in lines[:L]:
        print(x)
    if len(lines) > L:
        print(f"  … {len(lines) - L} more" + (f" ({hint})" if hint else ""))
    elif count and hint:
        print(f"  -> {hint}")


tests_bad = sum(len(v) for v in bad_tests.values())
problems = dict(
    unimplemented=len(none), untested=len(src_only), unproven_done=len(unproven), later_but_built=len(built),
    invalid_tests=tests_bad, spec_issues=len(issues), dangling_citations=len(dangling_src) + len(dangling_test), specs_to_compare=len(unlinked),
)
print("SDD STATUS")
print(f"specs: {len(files)} files, {len(S)} requirements | due {len(due)}: tested {n_tested} ({100 * n_tested // len(due) if due else 0}%), "
      f"src-only {len(src_only)}, none {len(none)} | manual {sum(state[i] == 'manual' for i in S)} | later {sum(state[i] == 'later' for i in S)} | retired {len(common.retired(base))}")
print("problems: " + " | ".join(f"{k.replace('_', ' ')} {v}" for k, v in problems.items()) + f" | total {sum(problems.values())}")
if links is None:
    print("note: no CodeGraph index, so test-to-code links and uncovered code are not checked")

section("unimplemented: due, no test and no code", len(none), per_spec(none), "status.py --missing")
section("untested: due, cited in code but no test", len(src_only), per_spec(src_only), "write the [ID] tests; status.py --missing")
section("check-marked done without proof", len(unproven),
        [f"  {i} {common.verdict(i, T, links)} {S[i]['file']}:{S[i]['line']}  {S[i]['text'][:70]}" for i in unproven],
        "NO-TEST = not done unless proven; NO-LINK = verify; status.py --unproven --effort high")
section("marked later but already has tests or code", len(built), per_spec(built), "assess: status.py --built, SKILL.md effort modes")
bt = [f"  {k}: " + ", ".join(f"{i} ({rel}:{n})" for i, rel, n in v[:6]) + (f", … {len(v) - 6} more" if len(v) > 6 else "") for k, v in bad_tests.items() if v]
section("invalid tests", tests_bad, bt, "unknown/retired = fix the tag; comment-only = no test is titled with it; disabled = switched off; no-assert = no direct assertion, verify")
section("specs to compare: code covered by 2+ requirements that do not cite each other", len(unlinked),
        [f"  {rel.removeprefix(overlap.APP)}:{lo} {name}  [{', '.join(ids)}]" for rel, lo, name, ids, _ in unlinked],
        "status.py --overlap; conflicting or complementary? see SKILL.md")
section("spec issues", len(issues), [f"  {x}" for x in issues], "fix the spec bullet")
section("dangling citations (ID in no spec)", len(dangling_src) + len(dangling_test),
        [f"  {k}: {common.ranges(v)}" for k, v in (("tests", dangling_test), ("sources", dangling_src)) if v], "repoint to a live ID or remove")

r = subprocess.run([sys.executable, str(here / "status.py"), "--uncovered"], capture_output=True, text=True)
out = r.stdout.splitlines()
print(f"\n[code with no requirement] {out[0] if out and r.returncode == 0 else 'not checked (no CodeGraph index)'}")
for x in out[1:min(len(out), 6)] if r.returncode == 0 else []:
    if x.startswith("  "): print(x[:140])
print("  -> status.py --uncovered [PATH]; triage per SKILL.md before writing requirements")
