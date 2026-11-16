# Server: CLI, API and MCP

- **API-01** MUST — define every tool once in `server/src/ops.rs`, so the CLI, `/api/call/{name}` and `/mcp` return the same answer.
- **API-02** MUST — bind 127.0.0.1 only and answer 403 to an unknown `Host` or `Origin` on `/api` and `/mcp` (DNS rebinding).
- **API-03** MUST — read spec, test and source files only by names the scanner itself lists, so `..` and absolute paths are rejected.
- **API-04** MUST — serve MCP over streamable HTTP at `/mcp`, with the dashboard as the `ui://spec11/dashboard` resource.
- **API-07** SHOULD — offer the modules and the detected test cases as read-only JSON MCP resources, `spec11://modules` and `spec11://tests`. They are resources only, with no tool; `/mcp` also answers the 2026-07-28 revision (per-request `_meta`, no `initialize`, `server/discover`) so the dashboard reads them with the MCP client SDK.
- **API-05** MAY — move the dashboard and `/api` to a separate `--ui-port` while `/mcp` stays on the main port.
- **API-06** MUST — serve the embedded dashboard from release builds, falling back to `index.html` for client-side routes.
