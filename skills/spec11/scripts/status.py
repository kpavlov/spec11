#!/usr/bin/env python3
"""Spec coverage: which requirement IDs have a `[ID]` test, which only a source comment, which nothing.

usage: status.py [--spec 04] [--prefix DOC]                      per-spec summary
       status.py --all | --missing | --changed [REF] | --issues   one line per requirement (filters combine: all must hold)
       status.py --built                                         requirements marked later/deferred that already have tests or code
       status.py --done | --unproven                             requirements carrying a check mark (✅ ✔️ ☑️ ✳️), and those without test/code evidence
       status.py ... --effort low|medium|high                    how much evidence each listed line carries (low = counts)
       status.py --overlap [PATH]                                code covered by 2+ requirements: specs that may conflict or complement (needs the CodeGraph index)
       status.py --uncovered [PATH]                              code that no requirement covers (needs the CodeGraph index)
"""
import argparse, sys
from pathlib import Path
sys.dont_write_bytecode = True  # no __pycache__ beside the scripts
sys.path.insert(0, str(Path(__file__).resolve().parent))
import common

ap = argparse.ArgumentParser()
ap.add_argument("--spec", help="only spec files whose name contains this, e.g. 04 or documents")
ap.add_argument("--prefix", help="only IDs with this prefix, e.g. DOC")
ap.add_argument("--all", action="store_true", help="list every requirement")
ap.add_argument("--missing", action="store_true", help="list requirements that are due but have no test")
ap.add_argument("--changed", nargs="?", const="HEAD", metavar="REF", help="list requirements touched by the working tree vs REF (default HEAD): spec lines edited, or a test/source file that cites them edited")
ap.add_argument("--built", action="store_true", help="list later/deferred requirements that have a [ID] test or a source citation: probably built, wholly or in part")
ap.add_argument("--done", action="store_true", help="list requirements marked done with a check mark, each with its evidence verdict")
ap.add_argument("--unproven", action="store_true", help="list check-marked requirements with no test (strong) or tests the CodeGraph cannot connect to app code")
ap.add_argument("--effort", choices=("low", "medium", "high"), default="low", help="evidence per listed line: low counts, medium + test and code locations, high + full spec text (a dossier for a verifier)")
ap.add_argument("--issues", action="store_true", help="list requirements with a spec issue")
ap.add_argument("--overlap", nargs="?", const="", metavar="PATH", help="list declarations covered by two or more requirements, and whether those specs cite each other")
ap.add_argument("--uncovered", nargs="?", const="", metavar="PATH", help="list app code that no requirement covers, optionally only files whose path contains PATH")
a = ap.parse_args()

base = common.root()
if a.overlap is not None:
    import overlap
    sys.exit(overlap.run(base, a.overlap))
if a.uncovered is not None:
    import uncovered
    sys.exit(uncovered.run(base, a.uncovered))
issues = []
S, T, C = common.specs(base, issues), common.tests(base), common.sources(base)
common.check_refs(S, issues)
issue_ids = {}
for x in issues:
    issue_ids.setdefault(x.split()[0], []).append(x)

state = common.states(S, T, C)

ids = [i for i, s in S.items() if not (a.spec and a.spec not in s["file"] or a.prefix and not i.startswith(a.prefix + "-"))]
if not ids:
    sys.exit("spec-driven: no requirement matches --spec/--prefix")
ids.sort(key=lambda x: (x.split("-")[0], int(x.split("-")[1])))


def why_changed(i, diff):
    """Which of spec, test, source of requirement `i` the diff touches."""
    s, out = S[i], []
    spec = diff.get("specs/" + s["file"], [])
    if any(lo <= s["end"] and hi >= s["line"] for lo, hi in spec):
        out.append("spec")
    if any(rel in diff for rel, _, _ in T.get(i, [])):
        out.append("test")
    if any(rel in diff for rel, _, _ in C.get(i, [])):
        out.append("src")
    return out


links = common.code_links(base, T)  # None without a CodeGraph index
verdict = lambda i: common.verdict(i, T, links)
marked = [i for i in ids if S[i]["done"]]
built = lambda i: state[i] == "later" and (i in T or i in C)
listing = a.all or a.missing or a.issues or a.changed or a.built or a.done or a.unproven
if listing:
    diff = common.changed(base, a.changed) if a.changed else {}
    for i in ids:
        s, flags = S[i], ""
        if a.missing and state[i] not in ("src-only", "none"): continue
        if a.issues and i not in issue_ids: continue
        if a.built and not built(i): continue
        if (a.done or a.unproven) and i not in marked: continue
        if a.unproven and verdict(i) in ("ok", "unchecked"): continue
        if a.changed:
            w = why_changed(i, diff)
            if not w: continue
            flags += f" [changed: {','.join(w)}]"
        if built(i): flags += f" [built?: {len(T.get(i, []))} tests, {len(C.get(i, []))} src]"
        if S[i]["done"]: flags += f" [DONE: {verdict(i)}]"
        if i in issue_ids: flags += " [issue]"
        print(f"{i} {state[i]:<8} {s['file']}:{s['line']}{flags}  {s['text'] if a.effort == 'high' else s['text'][:100]}")
        if a.effort != "low":
            for rel, n, _ in T.get(i, [])[:5]: print(f"    test: {rel}:{n}")
            for rel, n, _ in C.get(i, [])[:5]: print(f"    cite: {rel}:{n}")
            if links and links.get(i): print(f"    graph: tests reach {', '.join(sorted(links[i])[:8])}")
        if a.issues or a.changed:
            for x in issue_ids.get(i, []):
                print(f"    issue: {x[len(i) + 1:]}")
    if a.issues:  # bullets with no level have no requirement entry
        for x in issues:
            if x.split()[0] not in S: print(f"{x.split()[0]} (not counted)  issue: {x.split(' ', 1)[1]}")
else:
    by = {}
    for i in ids:
        by.setdefault(S[i]["file"], []).append(i)
    for f, fids in sorted(by.items()):
        k = {key: [i for i in fids if state[i] == key] for key in ("tested", "src-only", "none", "manual", "later")}
        due = len(fids) - len(k["manual"]) - len(k["later"])
        line = f"{f[:-3]:<28} {len(k['tested'])}/{due} tested"
        if k["src-only"]: line += f" | src-only: {common.ranges(k['src-only'])}"
        if k["none"]: line += f" | none: {common.ranges(k['none'])}"
        if k["manual"]: line += f" | manual: {len(k['manual'])}"
        if k["later"]:
            line += f" | later: {len(k['later'])}" + (f" ({sum(map(built, k['later']))} with tests/code)" if any(map(built, k["later"])) else "")
        print(line)

n = {key: sum(state[i] == key for i in ids) for key in ("tested", "src-only", "none", "manual", "later")}
due = len(ids) - n["manual"] - n["later"]
print(f"TOTAL {n['tested']}/{due} tested ({100 * n['tested'] // due if due else 0}%) | src-only {n['src-only']} | none {n['none']} | manual {n['manual']} | later {n['later']} ({sum(map(built, ids))} with tests/code)")

if marked:
    bad = [i for i in marked if verdict(i) == "NO-TEST"]
    weak = [i for i in marked if verdict(i) == "NO-LINK"]
    print(f"check-marked done: {len(marked)} | NO TEST: {len(bad)}" + (f" ({common.ranges(bad)})" if bad else "") + f" | tests not linked to code: {len(weak)}" + (f" ({common.ranges(weak)})" if weak else "") + (" | no CodeGraph index, links unchecked" if links is None else ""))
if not listing:
    if issues:
        print(f"spec issues ({len(issues)}):")
        for x in issues[:10]:
            print(f"  {x}")
        if len(issues) > 10:
            print(f"  … {len(issues) - 10} more (--issues)")
    if n["src-only"] or n["none"]:
        print("next: --missing lists them; trace.py <ID> shows tests, citations and related code")
for label, d in (("tests", T), ("sources", C)):  # typos or retired IDs
    bad = common.dangling(S, d)
    if bad: print(f"{label} cite IDs not in specs: {common.ranges(bad)}")
