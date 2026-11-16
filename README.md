# spec11

Deterministic spec-driven-development evidence for a repo: which requirement IDs in `specs/*.md` have tests, code citations, spec problems, or overlap with other requirements. No LLM in the core.

> **Evidence, not truth.** A citation or test tag does not prove a requirement is implemented or that its test passes. Output is a candidate list for a human or an assessor to judge.

One Rust binary, three interfaces over the same core:

| Interface | How |
|---|---|
| CLI | `spec11 report`, `spec11 status [flags]`, `spec11 trace ID` (`--json`, `--root DIR`) |
| MCP (streamable HTTP) | `spec11` / `spec11 serve` → `http://127.0.0.1:7411/mcp` |
| Web dashboard | `spec11` / `spec11 serve` → `http://127.0.0.1:7411/` (same or separate port) |

It replaces the earlier Python scripts (output is identical; `make parity` can diff against a vendored copy of them).

## Conventions it reads

- Spec bullets: `- **ABC-12** MUST|SHOULD|MAY — text` in `specs/*.md` (also single files such as `SPEC.md`, IDs up to 4 digits, links `[ADR-0003](decisions/0003-x.md)` as reference docs, plain/backtick IDs, `ID:`, `*`/numbered lists, `SHALL`, and `| ID | MUST | text |` rows)
- Test tags: `[ABC-12]` in test titles; source citations `(ABC-12)` in `//` comments
- States: `tested`, `src-only`, `none`, `manual`, `later`; check-marked requirements get a verdict `ok`, `NO-TEST`, `NO-LINK`
- Optional [CodeGraph](https://github.com/colbymchenry/codegraph) index (`.codegraph/codegraph.db`, opened read-only) adds test→code links, overlap and uncovered-code analysis. Without it those are "not checked", never an error.
- No config needed: the specs folder (`specs/`, `spec/`, `docs/specs/`, `openspec/specs/`, `.kiro/specs/`, a `docs/` with requirement bullets ...) and the source and test folders, extensions and regexes (Swift, Rust, TypeScript/JavaScript, Python, Go, Java/Kotlin/Scala, C#, Ruby, PHP, C/C++) are detected, from the CodeGraph file list when `.codegraph/` exists, else from the repo's files. Mixed repos use the dominant language; override any field in `spec11.json` at the repo root (see `spec11.example.json`).
- Symlinks are followed only when they resolve inside the repo root or a path in the `ALLOWED_PATHS` environment variable (path list, `:`-separated).
- Paths listed in `.aiignore` or `.spec11ignore` (gitignore syntax, repo root; the second is for spec11 only) are left out of every scan, test list, module and spec search, and so are `.agents` and everything the CodeGraph index lists there.

## Run

```sh
make dev                      # Vite HMR + Rust server on :7411, opens the browser
make run                      # release binary with the UI embedded, serves on :7411
make report                   # text report
make test                     # cargo tests
make parity                   # Rust output == the old Python scripts' output (needs them vendored in ROOT)
make dev ROOT=/path/to/repo PORT=7500 OPEN=0
```

`ROOT` defaults to this repo (spec11 scans itself: see `specs/` and `spec11.json`); point it at another repo with `ROOT=...`. Needs Rust, Node and pnpm; the release binary needs neither at runtime.

`spec11` with no command runs `serve` for the repo that holds the current directory (nearest ancestor with `specs/`, or `--root`) and opens the dashboard; `--no-open` skips the browser. At startup it checks whether the skill and MCP entry are installed for this repo or globally and prints what to run if not; the dashboard shows the same in a banner and a Setup tab with Install/Uninstall buttons.

`spec11 serve [--port 7411] [--ui-port N] [--no-open]`: one listener serves `/`, `/api` and `/mcp`; with `--ui-port` the UI and `/api` move to that port and the main port carries only `/mcp`. Binds 127.0.0.1 only and rejects unknown `Host`/`Origin` (403).

MCP client config:

```json
{"mcpServers": {"spec11": {"type": "http", "url": "http://127.0.0.1:7411/mcp"}}}
```

Tools: `spec_report`, `spec_status`, `spec_trace`, `spec_overlap`, `spec_uncovered`, `spec_file`, `spec_source`, `spec_dashboard`, and `spec_install_status` / `spec_install` / `spec_uninstall` / `spec_codegraph` (the last three write files). Read-only JSON resources: `spec11://modules` (modules by build manifest, with CodeGraph symbols and the modules each depends on) and `spec11://tests` (test cases detected by marker, grouped by module, with the requirement IDs tagged on each; detected, not run). The dashboard is also an MCP Apps resource (`ui://spec11/dashboard`) for hosts that render it; elsewhere use the URL from `spec_dashboard`.

## Dashboard URLs

The URL is the view, so back/forward, reload and shared links work: `/report`, `/requirements/DOC-25?state=none&q=text`, `/specs?file=04-documents.md&id=DOC-25`, `/code?path=src/main.rs&line=12`, `/overlap`, `/uncovered`, `/setup`. Tests, citations and spec locations link to the file viewer or spec; IDs inside code and markdown link to the requirement.

## Doctor

`spec11 doctor` checks the repo: specs found, test and source folders, git, whether the `codegraph` binary is on PATH and its index is initialized, readable and not older than the sources, and whether the skill and MCP entry are installed. It prints a hint per problem and exits 1 only when spec11 cannot work (no `specs/`). It never indexes or installs anything on its own. When the index is missing or stale, `spec11 doctor --fix` runs `codegraph init` / `codegraph sync` for you, and the Setup tab has the same button (asks to confirm); both are your call, nothing indexes automatically. The same list is the MCP tool `spec_doctor` and the Setup tab.

## Install the skill

```sh
spec11 install              # <repo>/.agents/skills/spec11/SKILL.md  +  spec11 entry in <repo>/.mcp.json
spec11 install --global     # ~/.agents/skills/spec11/SKILL.md (the MCP entry is per repo)
spec11 uninstall [--global] # removes only what install added
```

The skill is built into the binary and tells the agent to use the MCP tools (CLI as fallback). `.mcp.json` is created if missing; other servers and keys are kept as they are, and a file with invalid JSON is left untouched. `--port N` sets the port the entry points at (default 7411). Start the server with `spec11`.

## Layout

```
core/     spec11-core: scan, config, report, coverage, codegraph (the only SQL)
server/   spec11 binary: CLI, axum API, MCP (rmcp), UI serving
web/      Svelte 5 + Vite (no SvelteKit), builds to one dist/index.html
skills/   the agent skill (SKILL.md), embedded in the binary
```

In debug builds the server proxies the UI to Vite (`localhost:5173`); in release it serves the embedded `web/dist`.
