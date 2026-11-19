# spec11

Deterministic spec-driven-development evidence for a repo: which requirement IDs in `specs/*.md` have tests, code citations, spec problems, or overlap with other requirements. No LLM in the core.

> **Evidence, not truth.** A citation or test tag does not prove a requirement is implemented or that its test passes. Output is a candidate list for a human or an assessor to judge.

One Rust binary, three interfaces over the same core:

| Interface | How |
|---|---|
| CLI | `spec11 report`, `spec11 status [flags]` (`--changed [REF] [--staged] [--no-untracked] [--ignore-whitespace]`), `spec11 trace ID`, `spec11 changes|diff|history`, `spec11 overlap|uncovered`, `spec11 doctor`, `spec11 install|uninstall`; every command takes `--json` and `--root DIR` |
| MCP (streamable HTTP) | `spec11` / `spec11 serve` → `http://127.0.0.1:7411/mcp` |
| MCP (stdio) | `spec11 --stdio-mcp --no-http-mcp --no-ui` |
| Web dashboard | `spec11` / `spec11 serve` → `http://127.0.0.1:7411/` (same or separate port) |

It replaces the earlier Python scripts (output is identical; `make parity` can diff against a vendored copy of them).

## Conventions it reads

- Spec bullets: `- **ABC-12** MUST|SHOULD|MAY — text` in `specs/*.md` (also single files such as `SPEC.md`, IDs up to 4 digits, links `[ADR-0003](decisions/0003-x.md)` as reference docs, plain/backtick IDs, `ID:`, `*`/numbered lists, `SHALL`, `| ID | MUST | text |` rows, `### ID: title` headings and `**ID** title` paragraphs with the level or EARS `shall` anywhere in the block, OpenSpec `### Requirement: ID name`, IDs like `FR-001`, `AC-1.2`, `REQ_001` but not `UTF-8`; UTF-8 files with or without a BOM; patterns in `core/resources/spec_format.toml`)
- Test tags: `[ABC-12]` in test titles; source citations `(ABC-12)` in `//` comments
- States: `tested`, `src-only`, `none`, `manual`, `later`; check-marked requirements get a verdict `ok`, `NO-TEST`, `NO-LINK`
- Optional [CodeGraph](https://github.com/colbymchenry/codegraph) index (`.codegraph/codegraph.db`, opened read-only) adds test→code links, overlap and uncovered-code analysis. Without it those are "not checked", never an error.
- No config needed: every specs folder with markdown (`specs/`, `spec/`, `docs/specs/`, `openspec/specs/` and in-flight `openspec/changes/*/specs/`, `.kiro/specs/`, `docs/superpowers/specs/`, a `docs/` with requirement lines ...) and the source and test folders, extensions and regexes (Swift, Rust, TypeScript/JavaScript, Python, Go, Java/Kotlin/Scala, C#, Ruby, PHP, C/C++) are detected, from the CodeGraph file list when `.codegraph/` exists, else from the repo's files. Mixed repos use the dominant language; override any field in `spec11.json` at the repo root (see `spec11.example.json`).
- Symlinks are followed only when they resolve inside the repo root or a path in the `ALLOWED_PATHS` environment variable (path list, `:`-separated).
- Paths listed in `.aiignore` or `.spec11ignore` (gitignore syntax, repo root; the second is for spec11 only) are left out of every scan, test list, module and spec search, and so are `.agents` and everything the CodeGraph index lists there.

## Run

```sh
make dev                      # Vite HMR + Rust server on :7411, opens the browser
make run                      # release binary with the UI embedded, serves on :7411
make report                   # text report
make test                     # cargo tests
make parity                   # Rust output == the old Python scripts' output (needs them vendored in ROOT)
make bench                    # time the scan at main vs HEAD (see Benchmark)
make dev ROOT=/path/to/repo PORT=7500 OPEN=0
```

Without `make`, build once (`cargo build --release`; `SKIP_WEB_BUILD=1` skips the UI build when `web/dist` exists) and call the binary:

```sh
spec11 report                          # whole-project status: coverage, problems, where to look next
spec11 status --missing                # due requirements with no test (also --issues, --done, --unproven, --built, --all)
spec11 status --changed HEAD           # requirements the working tree touches (--staged, --no-untracked, --ignore-whitespace)
spec11 trace SCAN-01                   # one requirement: spec text, tests, citations, related code
spec11 changes --ref main              # files and requirements changed against a ref (or one commit)
spec11 report --json --root ../other   # JSON, for another repo
```

`ROOT` defaults to this repo (spec11 scans itself: see `specs/` and `spec11.json`); point it at another repo with `ROOT=...`. Needs Rust, Node and pnpm; the release binary needs neither at runtime.

`spec11` starts the server for the nearest repo with specs or Git work tree. Use `--root DIR` to choose a workspace. Repos without specs still show their sources and tests. The dashboard opens in the browser by default. Use `spec11 serve --no-open` to skip opening it.

### Streamable HTTP

By default, `spec11 serve` serves the dashboard and `/mcp` together on `127.0.0.1:7411`. Use `--port N` to change the port. With `--ui-port N`, both ports serve `/mcp` and only the UI port serves the dashboard and `/api/info`. HTTP listeners bind to `127.0.0.1` and reject unknown `Host` and `Origin` values.

```json
{"mcpServers": {"spec11": {"type": "http", "url": "http://127.0.0.1:7411/mcp"}}}
```

### Stdio

Use stdio to let each agent session launch its own spec11 process without binding a TCP port. The process exits when the client closes stdin.

```sh
spec11 --stdio-mcp --no-http-mcp --no-ui
```

```json
{"mcpServers": {"spec11-stdio": {"type": "stdio", "command": "spec11", "args": ["--stdio-mcp", "--no-http-mcp", "--no-ui"]}}}
```

Add `"--root", "/path/to/repo"` to `args` if the MCP client launches spec11 outside the intended workspace. `--stdio-mcp` enables stdin and stdout for MCP. `--no-ui` disables the browser dashboard and `/api/info`. `--no-http-mcp` disables `/mcp` on every listener. The browser dashboard needs HTTP MCP to load its data.

Tools: `report`, `status`, `changes` (what changed against a ref or in one commit: files, renames, deleted files and requirements, and the requirements the changed lines touch; also `spec11://changes`), `diff` (one changed file, with the requirements each hunk touches), `history` (commits not on the base branch), `commits` (the newest commit that touched each requirement, with its author, date and subject, by git blame: who last changed the specification; the dashboard's "reviewed" flag is kept against it), `blame` (`path`, `lines`: who last changed those lines of a scanned file, e.g. a test), `trace`, `overlap`, `uncovered`, `file`, `source`, `dashboard`, `module` (hide or show a module: adds it to or removes it from `.spec11ignore`), and `install_status` / `install` / `uninstall` (`server=codegraph` adds or removes only the CodeGraph MCP entry) / `codegraph` (`action=init|sync|refresh`; `refresh` syncs whenever an index exists, also beside a running codegraph); the last three write files. Plain-text resources with the same output as `report`, `status`, `uncovered` and `overlap`: `spec11://report`, `spec11://status`, `spec11://uncovered`, `spec11://overlap`; prefer them where the client reads resources (a client on 2026-07-28 can also `subscriptions/listen` to them and is notified when the repo changes). The tools stay for clients without resource support and for arguments. The same four as JSON (`spec11://report.json`, `status.json`, `uncovered.json`, `overlap.json`), typed by JSON Schemas readable at `spec11://schemas/<name>`; the tools declare them as `outputSchema`. Read-only JSON resources: `spec11://modules` (modules by build manifest, with CodeGraph symbols and the modules each depends on) and `spec11://tests` (test cases detected by marker, grouped by module, with the requirement IDs tagged on each; detected, not run). The dashboard is also an MCP Apps resource (`ui://spec11/dashboard`) for hosts that render it: only the `dashboard` tool opens it, and its optional `view` (e.g. `modules`, `requirements/DOC-25?state=none`) picks the page; elsewhere use the URL from `dashboard`.

## Benchmark

`make bench` times the CLI on a generated repo at several git revisions, so a change can be compared with a baseline. Each command is a full load (`Analysis::load`), so the numbers are mostly scan time.

```sh
make bench                                      # main vs HEAD, scale 1, 10 runs
make bench REVS="main HEAD ." SCALE=4 RUNS=20   # `.` is the working tree, uncommitted changes included
make bench JSON=1                               # one JSON document instead of a Markdown table (for scripts and agents)
python3 bench/bench.py --help
```

- The repo is generated deterministically under `target/bench/repo-<scale>`: bullet specs only (so old and new revisions read it the same way) with non-ASCII text, source files citing requirements, test files tagging them, and markdown docs; no CodeGraph index. Scale 1 is 2,000 requirements, 400 source files, 100 test files and 100 docs.
- Each revision is built once (release, no web UI) and cached in `target/bench/bin`; `.` is rebuilt every run. Revisions are exported with `git archive`, so nothing is written to `.git`.
- The first revision listed is the baseline. The table has median, min and stdev in ms, the change of the median against the baseline, and a hash of each command's output; the run says whether the output is the same for every revision. A speed-up must not change the output.
- Stdlib Python only.

Results (scale 4: 8,000 requirements, 1,600 source files, 400 test files, 400 docs; median of 10 runs; one macOS machine, so compare the ratios, not the milliseconds):

| command | `main` | `e1755f6` (new spec formats) | `HEAD` (+ scan speed-ups) |
|---|--:|--:|--:|
| `report` | 98 ms | 167 ms (+70%) | 113 ms (+15%) |
| `status --all` | 125 ms | 180 ms (+44%) | 139 ms (+11%) |
| `trace GEN-0001` | 102 ms | 160 ms (+57%) | 117 ms (+15%) |

The output of every command is byte-identical across the three revisions. Supporting the new spec formats (heading and paragraph blocks, dotted IDs, OpenSpec deltas, UTF-8 cleaning) cost 44–70%; the speed-ups (one walk per folder, ASCII word boundaries, one pass for struck and removed IDs, slices instead of copies) won back about two thirds of it, and the scan is still 11–15% slower than before. The 1-second change poll behind `resources/updated` is not covered: the CLI does not expose it.

## Dashboard URLs

The URL is the view, so back/forward, reload and shared links work: `/` (the dashboard; `/report` still works), `/modules`, `/modules?module=core`, `/tests?module=core&state=untagged`, `/requirements/DOC-25?state=none&q=text`, `/specs?file=04-documents.md&id=DOC-25`, `/code?path=src/main.rs&line=12`, `/overlap`, `/uncovered`, `/setup`. Tests, citations and spec locations link to the file viewer or spec; IDs inside code and markdown link to the requirement.

## Doctor

`spec11 doctor` checks the repo: specs found, test and source folders, git, whether the `codegraph` binary is on PATH and its index is initialized, readable and not older than the sources, and whether the skill and MCP entry are installed. It prints a hint per problem and exits 1 only when spec11 cannot work at all (no repo, unreadable `spec11.json`); no specs is a warning. It never indexes or installs anything on its own (when the `codegraph` binary is missing it prints the install commands). When the index is missing or stale, `spec11 doctor --fix` runs `codegraph init` / `codegraph sync` for you, and the Setup tab has the same buttons (asks to confirm); the dashboard's ↻ rescan also runs `codegraph sync` (also while an agent's codegraph MCP server runs: its file watcher can be off); the index check compares the index's last write with the newest source or test file; all are your call, nothing indexes automatically. The same list is the MCP tool `doctor` and the Setup tab.

## Install the skill and connect an MCP client

```sh
spec11 install              # repo skill + HTTP entry in <repo>/.mcp.json
spec11 install --global     # ~/.agents/skills/spec11/SKILL.md + HTTP entry in ~/.mcp.json
spec11 install --codegraph  # only adds the codegraph MCP server to <repo>/.mcp.json (an existing entry is kept)
spec11 uninstall --codegraph # only removes that entry (customised or not), nothing else
spec11 uninstall            # remove the repo skill and HTTP MCP entry
spec11 uninstall --global   # remove the global skill
spec11 uninstall --only skill [--global] # remove the skill independently
spec11 uninstall --only mcp [--global]   # remove the HTTP entry in <repo>/.mcp.json or ~/.mcp.json
```

The skill is built into the binary and tells the agent to use the MCP tools, with the CLI as a fallback. For HTTP, set `spec11 install --port N` to match the port of the running server. The default is `7411`.

Open **Setup** in the dashboard to install or remove each entry independently under **This repo** or **Global**. The page shows the configured HTTP URL and the full stdio launch command for entries it installs. Both are selectable. The project stdio launcher includes `--root` for that repo. The global launcher uses the client's working directory.

| Entry | Key in `mcpServers` | How it connects |
| --- | --- | --- |
| HTTP MCP | `spec11` | Connects to a running spec11 server |
| stdio MCP | `spec11-stdio` | Launches a spec11 process for the agent session |

Both entries can coexist in `<repo>/.mcp.json` or `~/.mcp.json`. Setup preserves other servers and leaves invalid JSON untouched. It also displays Claude's user-scope entry from `~/.claude.json` as read-only. If Setup asks you to update spec11, restart the server with the updated binary to enable the transport controls.

The MCP `install` and `uninstall` tools accept `only: "skill"` or `only: "mcp"`. With `only: "mcp"`, use `transport: "stdio"` or `"http"` (the default). Set `global: true` to use the home directory.

## Layout

```
core/     spec11-core: scan, config, report, coverage, codegraph (the only SQL)
server/   spec11 binary: CLI, axum API, MCP (rmcp), UI serving
web/      Svelte 5 + Vite (no SvelteKit), builds to one dist/index.html
skills/   the agent skill (SKILL.md), embedded in the binary
bench/    bench.py: before/after timings (make bench)
```

In debug builds the server proxies the UI to Vite (`localhost:5173`); in release it serves the embedded `web/dist`.
