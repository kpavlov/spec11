"""Shared scanning for spec-driven: spec IDs, test tags `[ABC-12]`, source comment mentions."""
import functools, re, subprocess, sys
from pathlib import Path

ID = r"[A-Z]{2,5}-\d{1,3}"
DONE = r"[✅✔☑✳]\ufe0f?"  # check marks: a hint that the requirement is done
DONE_RE = re.compile(DONE)
SPEC_RE = re.compile(rf"^\s*- (?:{DONE}\s*)?\*\*({ID})\*\*\s+(MUST|SHOULD|MAY)\b\s*(?:—\s*)?(.*)")
LOOSE_RE = re.compile(rf"^\s*- (?:{DONE}\s*)?\*\*({ID})\*\*")  # a requirement-shaped bullet, with or without a level
BLOCK_END_RE = re.compile(r"\s*(- |#|\|)")  # a wrapped bullet continues until a blank line, bullet, heading or table row
TAG_RE = re.compile(rf"\[({ID}(?:\s*[,;]\s*{ID})*)\]")  # `[ABC-12]`, `[ABC-12][ABC-14]` and `[ABC-12, ABC-14]`
MENTION_RE = re.compile(rf"\b({ID})\b")
DECL_RE = re.compile(r"^\s*(?:(?:public|internal|private|fileprivate|final|nonisolated|indirect|static)\s+)*(?:struct|class|enum|actor|protocol|extension|func|typealias)\s+([A-Za-z_]\w*)")
TEST_DIRS = ["RunSolo/RunSoloTests", "RunSolo/RunSoloUITests"]
SRC_DIRS = ["RunSolo/RunSolo"]
RETIRED_PREFIXES = {"BKP", "DRD", "ROLE"}  # specs/README.md; their IDs are never defined, so a citation is a leftover


def root() -> Path:
    """The repo is where this file lives (.agents/skills/spec-driven/scripts), whatever the cwd."""
    base = Path(__file__).resolve().parents[4]
    if not (base / "specs").is_dir():
        sys.exit(f"spec-driven: no specs/ folder in {base}")
    return base


@functools.cache
def lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def swift_files(base: Path, dirs):
    for d in dirs:
        yield from sorted((base / d).rglob("*.swift"))


MARK_RE = re.compile(r"\((later|deferred|manual)\)$")  # `(deferred)` means the same as `(later)`


def specs(base: Path, problems: list | None = None):
    """{id: dict(file, line, end, level, text, mark, done)} from `- **ABC-12** MUST — text` bullets, wrapped lines joined.
    A bullet ending in `(manual)`, `(later)` or `(deferred)` carries mark "manual" or "later"; struck-out bullets (`- ~~**ABC-12**`) are retired.
    Spec mistakes found on the way (duplicate ID, no level, marker not at the end) are appended to `problems`."""
    found = {}
    problems = [] if problems is None else problems
    for f in sorted((base / "specs").glob("*.md")):
        ls = lines(f)
        for n, line in enumerate(ls, 1):
            m = SPEC_RE.match(line)
            if not m:
                loose = LOOSE_RE.match(line)
                if loose:
                    problems.append(f"{loose.group(1)} {f.name}:{n} has no MUST/SHOULD/MAY, so it is not counted")
                continue
            parts, j = [m.group(3).strip()], n
            while j < len(ls) and ls[j].strip() and not BLOCK_END_RE.match(ls[j]):
                parts.append(ls[j].strip())
                j += 1
            text = " ".join(parts).strip()
            done = bool(DONE_RE.search(re.sub(r"~~.*?~~", "", line[: m.start(1)] + " ".join([line[m.end(1):]] + parts[1:]))))  # a check mark in the bullet, outside struck text
            end = MARK_RE.search(text)
            mark = "" if not end else "manual" if end.group(1) == "manual" else "later"
            if m.group(1) in found:
                was = found[m.group(1)]
                problems.append(f"{m.group(1)} is defined twice: {was['file']}:{was['line']} and {f.name}:{n}")
            elif re.search(r"\((?:later|deferred|manual)\)", re.sub(r"~~.*?~~", "", text[: end.start()] if end else text)):  # a struck sentence is retired text
                problems.append(f"{m.group(1)} {f.name}:{n} has (later)/(deferred)/(manual) before the end, so it counts as due")
            if done and mark:
                problems.append(f"{m.group(1)} {f.name}:{n} has a check mark but is also marked ({'manual' if mark == 'manual' else 'later'}/deferred)")
            found[m.group(1)] = dict(file=f.name, line=n, end=j, level=m.group(2), text=text, mark=mark, done=done)
    return found


def tagged(line: str) -> list[str]:
    """The requirement IDs of every `[ID]` or `[ID, ID]` tag on `line`, in order."""
    return [i for group in TAG_RE.findall(line) for i in MENTION_RE.findall(group)]


def tests(base: Path):
    """{id: [(relpath, line, text)]} for every `[ID]` or `[ID, ID]` tag in the test folders."""
    found = {}
    for f in swift_files(base, TEST_DIRS):
        rel = str(f.relative_to(base))
        for n, line in enumerate(lines(f), 1):
            for i in tagged(line):
                found.setdefault(i, []).append((rel, n, line.strip()))
    return found


def sources(base: Path):
    """{id: [(relpath, line, text)]} for IDs mentioned after `//` in app sources (block comments are not scanned)."""
    found = {}
    for f in swift_files(base, SRC_DIRS):
        rel = str(f.relative_to(base))
        for n, line in enumerate(lines(f), 1):
            if "//" in line:
                for i in MENTION_RE.findall(line.split("//", 1)[1]):
                    found.setdefault(i, []).append((rel, n, line.strip()))
    return found


USES = ("calls", "instantiates", "references", "extends", "implements")  # the kinds code_links follows (written out in its SQL)


def code_links(base: Path, T: dict) -> dict[str, set[str]] | None:
    """{id: app files} each tagged test reaches through the CodeGraph index (calls, instantiates, references; up to 3 hops, through test helpers).
    None when there is no index. An id with tests but no files has tests the graph cannot connect to app code."""
    import sqlite3
    db = base / ".codegraph" / "codegraph.db"
    if not db.exists():
        return None
    c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    app = SRC_DIRS[0] + "/"
    out = {}
    for i, refs in T.items():
        files = out.setdefault(i, set())
        for rel, n, _ in refs:
            inside = c.execute("select id from nodes where file_path=? and start_line<=? and end_line>=? and kind in ('method','function') order by end_line-start_line limit 1", (rel, n, n)).fetchall()
            seen, frontier = set(), [r[0] for r in inside]
            for _ in range(3):
                nxt = []
                for src in frontier:
                    for dst, path in c.execute("select e.target, n.file_path from edges e join nodes n on n.id=e.target where e.source=? and e.kind in ('calls','instantiates','references','extends','implements')", (src,)):
                        if path.startswith(app):
                            files.add(path[len(app):])
                        elif dst not in seen and path.startswith(tuple(TEST_DIRS)):
                            seen.add(dst)
                            nxt.append(dst)
                frontier = nxt
    return out


def check_refs(S: dict, issues: list) -> None:
    """Append an issue for every requirement that cites an ID no spec defines (retired IDs live in specs/README.md, not in bullets)."""
    known = {i.split("-")[0] for i in S} | RETIRED_PREFIXES
    for i, spec in S.items():
        for ref in sorted(set(MENTION_RE.findall(spec["text"])) - set(S)):
            if ref.split("-")[0] in known:
                issues.append(f"{i} {spec['file']}:{spec['line']} cites {ref}, which no spec defines")


def dangling(S: dict, cited: dict) -> list[str]:
    """IDs a test or source cites that no spec defines, when the prefix is a spec prefix or a retired one (typos, retired IDs)."""
    known = {i.split("-")[0] for i in S} | RETIRED_PREFIXES
    return sorted(i for i in cited if i not in S and i.split("-")[0] in known)


def states(S: dict, T: dict, C: dict) -> dict[str, str]:
    """tested | src-only | none for a due requirement; manual | later when its bullet says so."""
    return {i: s["mark"] or ("tested" if i in T else "src-only" if i in C else "none") for i, s in S.items()}


def verdict(i: str, T: dict, links: dict | None) -> str:
    """Evidence for a check-marked requirement: NO-TEST is strong; NO-LINK means its tests reach no app code in the graph; unchecked = no index."""
    if i not in T: return "NO-TEST"
    if links is None: return "unchecked"
    return "ok" if links.get(i) else "NO-LINK"


def retired(base: Path) -> set[str]:
    """IDs whose bullet is struck through (`- ~~**ABC-12** …~~`): retired, never reassigned."""
    pat = re.compile(rf"^\s*- ~~\*\*({ID})\*\*")
    return {m.group(1) for f in sorted((base / "specs").glob("*.md")) for line in lines(f) if (m := pat.match(line))}


ASSERT_RE = re.compile(r"#expect|#require|XCTAssert|XCTFail|XCTUnwrap|Issue\.record|confirmation\(")
OFF_RE = re.compile(r"\.disabled\(|\.enabled\(if: false|XCTSkip")


def test_problems(base: Path, S: dict, T: dict) -> dict[str, list[tuple[str, str, int]]]:
    """Tagged tests that prove nothing: {kind: [(id, relpath, line)]}.
    unknown = ID in no spec; retired = struck-through spec; comment-only = every tag of the ID sits in a comment, so no test is titled with it;
    disabled = every test is switched off; no-assert = every tagged test body has no direct assertion (it may assert through a helper: verify)."""
    gone, out = retired(base), {k: [] for k in ("unknown", "retired", "comment-only", "disabled", "no-assert")}
    for i, refs in sorted(T.items()):
        first = (i, refs[0][0], refs[0][1])
        if i in gone: out["retired"].append(first)
        elif i not in S: out["unknown"].append(first)
        else:
            def flag(rel, n, text, kind):
                ls = lines(base / rel)
                if kind == "comment-only": return text.startswith("//")
                if kind == "disabled":  # the attribute: the tag line up to the `func` line
                    head = []
                    for x in ls[n - 1 : n + 3]:
                        head.append(x)
                        if "func " in x: break
                    return not text.startswith("//") and any(OFF_RE.search(x) for x in head)
                return not text.startswith("//") and not any(ASSERT_RE.search(x) for x in body(ls, n))
            for kind in ("comment-only", "disabled", "no-assert"):
                if all(flag(rel, n, text, kind) for rel, n, text in refs):
                    out[kind].append(first)
    return out


def body(ls: list[str], n: int, cap: int = 200) -> list[str]:
    """Lines of the Swift function a 1-based tagged line `n` belongs to: from the tag to the brace that closes it."""
    out, depth, opened = [], 0, False
    for line in ls[n - 1 : n - 1 + cap]:
        out.append(line)
        depth += line.count("{") - line.count("}")
        opened = opened or "{" in line
        if depth < 0 or (opened and depth == 0):
            break
    return out


def enclosing(ls: list[str], n: int) -> str:
    """Name of the declaration a comment at 1-based line `n` belongs to: the next one for a `///` doc comment, else the nearest above."""
    idx = range(n, min(n + 8, len(ls))) if ls[n - 1].lstrip().startswith("///") else range(n - 1, -1, -1)
    for i in idx:
        m = DECL_RE.match(ls[i])
        if m:
            return m.group(1)
    return ""


def ranges(ids) -> str:
    """DOC-01,DOC-02,DOC-03,DOC-07 -> DOC-01..03,DOC-07"""
    out, by = [], {}
    for i in ids:
        p, n = i.rsplit("-", 1)
        by.setdefault(p, []).append(int(n))
    for p, ns in by.items():
        ns.sort()
        run = [ns[0]]
        for n in ns[1:] + [None]:
            if n is not None and n == run[-1] + 1:
                run.append(n)
                continue
            out.append(f"{p}-{run[0]:02}" if len(run) == 1 else f"{p}-{run[0]:02}..{run[-1]:02}")
            run = [n]
    return ",".join(out)


def parse_diff(diff: str) -> dict[str, list[tuple[int, int]]]:
    """{path: [(first, last)]} new-file line ranges from `git diff -U0`; a pure deletion marks the line it sat before."""
    out, path = {}, None
    for line in diff.splitlines():
        if line.startswith("+++ "):
            path = line[6:] if line.startswith("+++ b/") else None
        elif line.startswith("@@") and path:
            m = re.match(r"@@ -\S+ \+(\d+)(?:,(\d+))? @@", line)
            start, count = int(m.group(1)), int(m.group(2) or 1)
            out.setdefault(path, []).append((start, start + max(count, 1) - 1))
    return out


def changed(base: Path, ref: str = "HEAD") -> dict[str, list[tuple[int, int]]]:
    """Files changed in the working tree since `ref` (staged, unstaged, untracked), with their changed line ranges."""
    def git(*args):
        r = subprocess.run(["git", *args], cwd=base, capture_output=True, text=True)
        if r.returncode:
            sys.exit(f"spec-driven: git {' '.join(args)} failed: {r.stderr.strip()}")
        return r.stdout
    out = parse_diff(git("diff", "-U0", "--no-color", "--no-ext-diff", ref, "--"))
    for path in git("ls-files", "--others", "--exclude-standard").splitlines():
        out[path] = [(1, 10**9)]
    return out
