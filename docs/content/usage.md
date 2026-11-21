---
title: Usage
weight: 10
---

Every command takes `--root DIR`; without it spec17 uses the nearest parent folder that holds specs, else the nearest Git work tree. The commands that print a result (all but `serve`, `install` and `uninstall`) also take `--json`.

| Command | What it shows |
| --- | --- |
| `spec17 report` | Whole-project status: coverage, problems, where to look next |
| `spec17 status` | Requirements by state; `--missing`, `--issues`, `--done`, `--unproven`, `--built`, `--all` |
| `spec17 status --changed [REF]` | Requirements the working tree (or a ref) touches; `--staged`, `--no-untracked`, `--ignore-whitespace` |
| `spec17 trace ID` | One requirement: spec text, tests, citations, related code |
| `spec17 changes`, `diff`, `history` | What changed against a ref, one changed file by hunk, commits not on the base branch |
| `spec17 overlap`, `uncovered` | Code covered by two or more requirements, where specs may conflict or complement each other; code no requirement covers. Candidates, not verdicts (need a CodeGraph index) |
| `spec17 doctor [--fix]` | Checks the setup; `--fix` runs `codegraph init` or `sync` |
| `spec17 install`, `uninstall` | The agent skill and MCP entries, see [Agents](../agents/) |

```sh
spec17 status --missing              # due requirements with no test
spec17 trace SCAN-01                 # one requirement
spec17 changes --ref main            # against a ref
spec17 report --json --root ../other # JSON for another repo
```

## Dashboard

`spec17` with no command is `spec17 serve`: it starts the server on `127.0.0.1:7411` for the repo in the current directory (or `--root`) and opens the dashboard. The server options belong to `serve`: `spec17 serve --port N` changes the port, `--no-open` skips the browser, and `--ui-port N` serves the dashboard on its own port while `--port` keeps only `/mcp`. The server binds to localhost only and rejects unknown `Host` and `Origin` values.

The URL is the view, so back/forward, reload and shared links work: `/changes`, `/modules`, `/tests`, `/requirements/ID`, `/requirements?view=docs`, `/code`, `/overlap`, `/uncovered`, `/settings`.

![Requirements view: API-02 selected, with its tagged test, code citation and a diff of its edited text](../img/requirements.png)

## CodeGraph

An optional [CodeGraph](https://github.com/colbymchenry/codegraph) index (`.codegraph/codegraph.db`, opened read-only) adds test-to-code links, overlap and uncovered-code analysis. Without it `report` and the dashboard say "not checked" or "not available", and everything else works. The commands and tools that need the index (`spec17 overlap`, `uncovered`, `status --overlap`, `status --uncovered`, and the matching MCP tools) fail with "no .codegraph index here" instead: the CLI exits 1, the tools return an error. `spec17 doctor --fix` or the dashboard's Settings tab can create or refresh it, only when you ask.

## Build from source

```sh
make build     # target/release/spec17, UI embedded
make install   # into ~/.cargo/bin
make dev       # Vite dev server (hot reload) + Rust server
make report    # text report for ROOT (default: this repo)
```

`SKIP_WEB_BUILD=1` skips the UI build when `web/dist` already exists. The README has [releases and dev builds](https://github.com/kpavlov/spec17#releases).
