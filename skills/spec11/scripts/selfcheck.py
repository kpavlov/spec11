#!/usr/bin/env python3
"""Checks the parsers on tiny inputs: python3 selfcheck.py (prints ok, or fails with an assert)."""
import sys, tempfile
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import common

with tempfile.TemporaryDirectory() as d:
    b = Path(d)
    (b / "specs").mkdir()
    (b / "specs/01-x.md").write_text(
        "- **AAA-01** MUST — one line\n"
        "- **AAA-02** SHOULD — wrapped\n  on two lines (later)\n\n"
        "- **AAA-03** MAY — checked by a person (manual)\n"
        "- ~~**AAA-04** MUST — retired (later)~~\n"
        "- **AAA-05** MUST — before a table\n| a | b |\n"
        "- **AAA-06** MUST — (later) then more text\n"
        "- **AAA-07** is missing its level\n"
        "- **AAA-01** MUST — again\n"
        "- ✅ **AAA-10** MUST — done before the ID\n"
        "- **AAA-11** SHOULD — done ✳️ inside\n  and on a wrapped line ☑️\n"
        "- **AAA-12** MUST — done ✔️ yet (later)\n"
        "- **AAA-09** MAY — held back (deferred)\n"
        "- **AAA-08** MUST — kept. ~~Dropped sentence. (later)~~\n", encoding="utf-8")
    issues = []
    S = common.specs(b, issues)
    assert sorted(S) == ["AAA-01", "AAA-02", "AAA-03", "AAA-05", "AAA-06", "AAA-08", "AAA-09", "AAA-10", "AAA-11", "AAA-12"], sorted(S)
    assert [x.split()[0] for x in issues] == ["AAA-06", "AAA-07", "AAA-01", "AAA-12"], issues
    assert [i for i in S if S[i]["done"]] == ["AAA-10", "AAA-11", "AAA-12"], [i for i in S if S[i]["done"]]
    assert S["AAA-02"]["mark"] == "later" and S["AAA-02"]["text"] == "wrapped on two lines (later)"
    assert S["AAA-03"]["mark"] == "manual" and S["AAA-08"]["mark"] == "" and S["AAA-09"]["mark"] == "later"
    assert S["AAA-01"]["text"] == "again" and S["AAA-05"]["text"] == "before a table"

with tempfile.TemporaryDirectory() as d:
    b = Path(d)
    (b / "specs").mkdir()
    (b / "RunSolo/RunSoloTests").mkdir(parents=True)
    (b / "specs/x.md").write_text("".join(f"- **AAA-0{n}** MUST — x\n" for n in range(1, 5)) + "- ~~**AAA-09** MUST — gone~~\n", encoding="utf-8")
    (b / "RunSolo/RunSoloTests/T.swift").write_text(
        '@Test("[AAA-01] vacuous")\nfunc a() { let x = 1 }\n'
        '@Test("[ZZZ-9] unknown")\nfunc b() { #expect(true) }\n'
        '@Test("[AAA-09] retired")\nfunc c() { #expect(true) }\n'
        'func d() {\n    // [AAA-03] only a comment\n    #expect(true)\n}\n'
        '@Test("[AAA-04] off", .disabled("x"))\nfunc e() { #expect(true) }\n', encoding="utf-8")
    S2, T2 = common.specs(b), common.tests(b)
    got = {k: [i for i, _, _ in v] for k, v in common.test_problems(b, S2, T2).items()}
    assert got == {"unknown": ["ZZZ-9"], "retired": ["AAA-09"], "comment-only": ["AAA-03"], "disabled": ["AAA-04"], "no-assert": ["AAA-01"]}, got

swift = ['@Test("[AAA-01] a")', "func a() {", "    if x {", "    }", "    use(Foo)", "}", "", '@Test("[AAA-02] b")', "func b() { }"]
assert common.body(swift, 1) == swift[:6], common.body(swift, 1)
assert common.body(swift, 5) == swift[4:6]  # a tag inside the body stops at the closing brace
assert common.enclosing(["/// doc (AAA-01)", "func run() {}"], 1) == "run"
assert common.enclosing(["struct Box {", "    func go() {", "        // AAA-01"], 3) == "go"
assert common.tagged('@Test("[DOC-25][UI-7] x")') == ["DOC-25", "UI-7"]
assert common.tagged('@Test("[DOC-25, UI-7,TIME-02; REG-1] x") // [AB-1]') == ["DOC-25", "UI-7", "TIME-02", "REG-1", "AB-1"]
assert common.tagged('let a = [DOC-25 UI-7] [x] [DOC-25,]') == []  # not a tag: no separator, or a stray comma
assert common.ranges(["DOC-01", "DOC-02", "DOC-03", "DOC-07", "UI-5"]) == "DOC-01..03,DOC-07,UI-05"
diff = "+++ b/specs/a.md\n@@ -3,2 +3,4 @@\n+++ b/RunSolo/X.swift\n@@ -9 +9 @@\n@@ -20,3 +19,0 @@\n+++ /dev/null\n@@ -1 +0,0 @@\n"
assert common.parse_diff(diff) == {"specs/a.md": [(3, 6)], "RunSolo/X.swift": [(9, 9), (19, 19)]}, common.parse_diff(diff)
print("ok")
