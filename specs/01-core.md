# Core: scanning and CodeGraph

Deterministic evidence only: same tree, same text. No LLM in the core.

## Scanning

- **SCAN-01** MUST — read requirements from `- **ABC-12** MUST|SHOULD|MAY — text` bullets in the specs folder and its subfolders, joining a wrapped bullet's continuation lines.
- **SCAN-02** MUST — find test tags `[ABC-12]` (also `[ABC-12, ABC-14]`) in the test folders.
- **SCAN-03** MUST — count a source citation only when the ID follows a line-comment marker; block comments and code are not scanned.
- **SCAN-04** MUST — give each requirement one state: `tested`, `src-only`, `none`, `manual` or `later`; struck-out bullets are retired and never due.
- **SCAN-05** MUST — print the same text for the same tree: no clocks, randomness or map-order dependence.
- **SCAN-06** SHOULD — take folders, extensions and regexes from an optional `spec11.json`, and fall back to defaults when it is missing.
- **SCAN-07** MUST — report invalid test tags (unknown, retired, comment-only, disabled, no assertion) and dangling citations as problems to judge, not as verdicts.
- **SCAN-08** MUST — trace one ID to its spec text, tests and code citations.
- **SCAN-09** SHOULD — detect what `spec11.json` leaves out: the specs folder (`specs/`, `spec/`, `docs/specs/`, `openspec/specs/`, `.kiro/specs/` and the like, a generic `docs/` only with a requirement bullet), and the source and test folders, extensions, comment markers and regexes of the dominant ecosystem, from the CodeGraph file list when an index exists, else from the repo's files.
- **SCAN-11** SHOULD — also accept the common spec spellings: `*`, `+` and numbered lists, a plain, `**bold**` or `` `code` `` ID with an optional colon, the level anywhere in the line (`MUST`, `SHOULD`, `MAY`, `SHALL`), and `| ID | MUST | text |` table rows.
- **SCAN-12** SHOULD — find specs beyond a specs folder of bullets: IDs of up to four digits (`REQ-0001`), single-file specs (`SPEC.md`, `REQUIREMENTS.md`, also in `docs/`, or a `specs` setting that names a file), and, when nothing else holds a requirement line, any markdown file of the repo that does.
- **SCAN-13** SHOULD — treat a markdown link `[ABC-12](path.md)` to an existing markdown file, and a decision-folder file `0003-title.md` (`adr/`, `decisions/` …), as a reference doc with that ID: state `ref`, never due, with the files that link to it in its trace; report a link to a missing file as a spec issue.
- **SCAN-14** SHOULD — list the test cases of the test files (a Rust `#[test]`, a JUnit `@Test`, a Python `def test_`, a JS `it(`, a Go `func Test` and the like, or the `case` regex of `spec11.json`) with the requirement IDs tagged on each, grouped by module; a marker inside a string literal is not a case. Detected, not run: it says nothing about passing.
- **SCAN-10** MUST — follow a symlink only when it resolves inside the repo root or a path listed in the `ALLOWED_PATHS` environment variable.
- **SCAN-16** SHOULD — list the requirements a git diff touches (`status --changed <ref>`): staged, unstaged and untracked files, mapped from their changed line ranges to the spec lines and cited code they hit. A pure deletion marks the line it sat before.
- **SCAN-17** SHOULD — print consecutive IDs of a list as one range (`DOC-01..03`), with the others separated by commas, in the summary and problem lines.
- **SCAN-19** SHOULD — give the unified diff of one changed file (against a ref or in one commit; a rename shows both paths, an untracked file is all additions, cut at 3000 lines) for the files `changes` lists, and under each hunk the requirements it touches (SCAN-18); refuse any other path, and a ref or sha that looks like a git option.
- **SCAN-18** SHOULD — list what changed as `changes`: against a git ref (`ref`, HEAD by default) or in one commit (`commit`, against its first parent), every changed file with what happened to it (`added`, `modified`, `deleted`, `renamed` with its old path), where (`staged`, `unstaged`, `both`, `untracked`, or `committed` for what is in the compared commits) and its kind (`spec`, `test`, `src`), and the requirements the changed lines touch, down to the line: a hunk in its bullet (`spec`: added, edited, removed: a removed bullet is listed as deleted), in a test that carries its `[ID]` tag (`test`) or in the code unit its `// (ID)` citation sits on (`src`; the whole file when the language has no braces). `staged` narrows to the index, `include_untracked=false` leaves untracked files out, `ignore_whitespace` leaves out changes that only touch whitespace, `show_ignored` adds the files `.aiignore`, `.spec11ignore` and `.agents` hide, `show_all` adds the files spec11 does not scan (kind `other`). Evidence of what was edited, not that it is done.
- **SCAN-20** SHOULD — list the commits of a branch that are not on the base branch (`history`: main or master by default, newest first, with the number of uncommitted files), so each one can be explained with `changes commit=<sha>`; on the base branch itself it lists the latest commits. A commit that changes no file the filters show (whitespace-only changes, ignored files, files spec11 does not scan) is left out.

## CodeGraph

- **SCAN-15** MUST — leave out what the repo's `.aiignore` and `.spec11ignore` (gitignore syntax) hide, and always `.agents`, from every file walk and every file the CodeGraph index lists, so they reach no test, source, module or spec list.
- **GRAPH-01** MUST — open the CodeGraph index read-only, with all SQL in `core/src/codegraph.rs`.
- **GRAPH-02** MUST — turn a schema the code does not know into a typed error shown as "not checked", never a panic.
- **GRAPH-03** MUST — treat a missing index as "not checked", not as an error: indexing is the user's decision.
- **GRAPH-05** SHOULD — group the repo's files into modules (the nearest folder with a build manifest, else the folder above `src`), count a file as a test file only when it is test-named or in a tests folder that is not also a source folder (inline unit tests stay source), and from the index count each module's symbols and the dependencies between modules.
- **GRAPH-04** SHOULD — link tests to code through the index, and answer callers, impact and search queries from it.
