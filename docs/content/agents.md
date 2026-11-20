---
title: Agents
weight: 30
---

An agent reads spec17's evidence through MCP. The skill built into the binary tells it to use the MCP tools, with the CLI as a fallback.

## Install the skill and an MCP entry

```sh
spec17 install              # repo skill + HTTP entry in <repo>/.mcp.json
spec17 install --global     # ~/.agents/skills/spec17/SKILL.md + HTTP entry in ~/.mcp.json
spec17 install --codegraph  # only adds the codegraph MCP server
spec17 uninstall            # remove the repo skill and HTTP entry
```

`.mcp.json` is merged, never rewritten: other servers stay and invalid JSON is left alone. The Setup tab of the dashboard installs or removes each entry separately, for this repo or globally, and can also add the stdio entry below.

## Connect a client

HTTP, to a running `spec17` (default port 7411; `spec17 install --port N` to match):

```json
{"mcpServers": {"spec17": {"type": "http", "url": "http://127.0.0.1:7411/mcp"}}}
```

stdio, so each agent session launches its own process and binds no port:

```json
{"mcpServers": {"spec17-stdio": {"type": "stdio", "command": "spec17", "args": ["--stdio-mcp", "--no-http-mcp", "--no-ui"]}}}
```

Add `"--root", "/path/to/repo"` to `args` if the client starts spec17 outside the repo.

## Tools

`report`, `status`, `changes`, `diff`, `history`, `commits`, `blame`, `trace`, `search_tests`, `search_requirements`, `search_modules`, `overlap`, `uncovered`, `file`, `source`, `doctor`, `codegraph`, `install_status`, `install`, `uninstall`, `module`, `dashboard`. The tools that write files are `codegraph`, `install`, `uninstall` and `module`; the rest only read. `commits`, `blame` and the `search_*` tools have no CLI command.

Plain-text resources return the same output as the matching tools: `spec17://report`, `spec17://status`, `spec17://changes`, `spec17://uncovered`, `spec17://overlap`. JSON versions, typed by schemas at `spec17://schemas/<name>`, add `.json`. Two more resources are JSON only: `spec17://modules` (modules, with CodeGraph symbols and dependencies) and `spec17://tests` (test cases detected, with the IDs tagged on each; detected, not run).
