#!/usr/bin/env python3
"""Trace one requirement: spec text, its tests, source comments, and the code those tests touch.

usage: trace.py DOC-25 [--codegraph]
Related code = types in app sources named in the tagged tests' bodies; each source citation shows the declaration it sits in.
"""
import re, subprocess, sys
from pathlib import Path
sys.dont_write_bytecode = True  # no __pycache__ beside the scripts
sys.path.insert(0, str(Path(__file__).resolve().parent))
import common

args = [x for x in sys.argv[1:] if not x.startswith("--")]
if not args:
    sys.exit(__doc__)
want = args[0].upper()
base = common.root()
S, T, C = common.specs(base), common.tests(base), common.sources(base)
s = S.get(want)
if not s:
    print(f"{want}: not in specs" + (" (tests/sources still cite it)" if want in T or want in C else ""))
    if want not in T and want not in C:
        sys.exit(1)
else:
    print(f"{want} {s['level']} {s['file']}:{s['line']}{' ('+s['mark']+')' if s['mark'] else ''}\n  {s['text']}")
    refs = sorted(set(common.MENTION_RE.findall(s["text"])) - {want})
    if refs: print("  cites:", " ".join(refs))

idents = set()
print("tests:")
for rel, n, text in T.get(want, []):
    print(f"  {rel}:{n}  {text[:110]}")
    for line in common.body(common.lines(base / rel), n):
        idents.update(re.findall(r"\b[A-Za-z_]\w{3,}\b", line))
if not T.get(want):
    print("  none")

print("source cites (file:line  declaration):")
cited, names = set(), set()
for rel, n, text in C.get(want, []):
    cited.add(rel)
    decl = common.enclosing(common.lines(base / rel), n)
    if decl: names.add(decl)
    print(f"  {rel}:{n}  {decl or '-'}  {text[:90]}")
if not C.get(want):
    print("  none")

# types the tests name, declared in app sources; a bare func name like `parse` is too generic to match on its own
defs = {}
for f in common.swift_files(base, common.SRC_DIRS):
    types = set()
    for line in common.lines(f):
        m = common.DECL_RE.match(line)
        if m and m.group(1) in idents and re.search(r"\b(?:struct|class|enum|actor|protocol)\b", line):
            types.add(m.group(1))
    if types:
        defs[str(f.relative_to(base))] = sorted(types)
        names |= types
print("related code (types the tests name; * = also cites the ID):")
for rel in sorted(defs, key=lambda r: (r not in cited, r))[:12]:
    print(f"  {'*' if rel in cited else ' '} {rel}  {' '.join(defs[rel])}")
if not defs:
    print("  none found")
if len(defs) > 12:
    print(f"  … {len(defs) - 12} more files")

q = " ".join(sorted(names)[:8])
if q:
    print(f'next: codegraph explore "{q}"')
    if "--codegraph" in sys.argv:
        subprocess.run(["codegraph", "explore", q], cwd=base)
