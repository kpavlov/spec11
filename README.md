# spec11

Deterministic spec-driven-development evidence for a repo: which requirement IDs in `specs/*.md` have tests, code citations, spec problems, or overlap with other requirements. No LLM in the core.

> **Evidence, not truth.** A citation or test tag does not prove a requirement is implemented or that its test passes. Output is a candidate list for a human or an assessor to judge.

One Rust binary, three interfaces over the same core:

| Interface | How |
|---|---|
| CLI | `spec11 report`, `spec11 status [flags]`, `spec11 trace ID` (`--json`, `--root DIR`) |
| MCP (streamable HTTP) | `spec11 serve` → `http://127.0.0.1:7411/mcp` |
| Web dashboard | `spec11 serve` → `http://127.0.0.1:7411/` (same or separate port) |

It ports the Python skill in `skills/spec11/` (kept as fallback and as the oracle; `make parity` diffs the two).

## Conventions it reads

- Spec bullets: `- **ABC-12** MUST|SHOULD|MAY — text` in `specs/*.md`
- Test tags: `[ABC-12]` in test titles; source citations `(ABC-12)` in `//` comments
- States: `tested`, `src-only`, `none`, `manual`, `later`; check-marked requirements get a verdict `ok`, `NO-TEST`, `NO-LINK`
- Optional [CodeGraph](https://github.com/colbymchenry/codegraph) index (`.codegraph/codegraph.db`, opened read-only) adds test→code links, overlap and uncovered-code analysis. Without it those are "not checked", never an error.
- Defaults are Swift/run-solo. Override in `spec11.json` at the repo root (see `spec11.example.json`).

## Run

```sh
make dev                      # Vite HMR + Rust server on :7411, opens the browser
make run                      # release binary with the UI embedded, serves on :7411
make report                   # text report
make test                     # cargo tests
make parity                   # Rust output == Python scripts' output
make dev ROOT=/path/to/repo PORT=7500 OPEN=0
```

`ROOT` defaults to `/Users/maestro/code/github/itstaffhq/run-solo`. Needs Rust and Node/npm; the release binary needs neither at runtime.

`spec11 serve [--port 7411] [--ui-port N]`: one listener serves `/`, `/api` and `/mcp`; with `--ui-port` the UI and `/api` move to that port and the main port carries only `/mcp`. Binds 127.0.0.1 only and rejects unknown `Host`/`Origin` (403).

MCP client config:

```json
{"mcpServers": {"spec11": {"type": "http", "url": "http://127.0.0.1:7411/mcp"}}}
```

Tools: `spec_report`, `spec_status`, `spec_trace`, `spec_overlap`, `spec_uncovered`, `spec_dashboard`. The dashboard is also an MCP Apps resource (`ui://spec11/dashboard`) for hosts that render it; elsewhere use the URL from `spec_dashboard`.

## Layout

```
core/     spec11-core: scan, config, report, coverage, codegraph (the only SQL)
server/   spec11 binary: CLI, axum API, MCP (rmcp), UI serving
web/      Svelte 5 + Vite (no SvelteKit), builds to one dist/index.html
skills/   the agent skill (SKILL.md) and the Python reference scripts
```

In debug builds the server proxies the UI to Vite (`localhost:5173`); in release it serves the embedded `web/dist`.
