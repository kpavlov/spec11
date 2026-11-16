# AGENTS.md

spec11: Rust workspace (`core`, `server`) + Svelte dashboard (`web`) + the Python reference skill (`skills/spec11`). See README.md for usage.

## Commands

- `cargo test --workspace` (debug) and `SKIP_WEB_BUILD=1 cargo test --release --workspace` (the embedded-UI test runs only in release; needs `web/dist`)
- `make parity` must stay green: Rust output equals the Python scripts on run-solo (`ROOT`). Change both or neither.
- `cd web && npm run build && npx svelte-check`
- `make dev` / `make run`; override `ROOT`, `PORT`, `OPEN=0`

## Rules

- **Evidence, not truth.** Tool descriptions, UI and docs must not claim a requirement is implemented or a test passes. No LLM in the core.
- Output is deterministic: same tree, same text. Text renderers in `core/src/report.rs` mirror the Python output; a change there needs a matching change in `skills/spec11/scripts/` or the parity check fails.
- **All SQL lives in `core/src/codegraph.rs`**, read-only (`SQLITE_OPEN_READ_ONLY`). Schema drift is a typed `SchemaDrift` error rendered as "not checked", never a panic. A missing index is not an error either: indexing is the user's decision.
- Tools are defined once in `server/src/ops.rs`; the CLI, `/api/call/{name}` and `/mcp` all go through it. Add a tool there, not per interface.
- `Analysis` uses `Rc` (not `Send`): run it inside `spawn_blocking`.
- The server binds 127.0.0.1 only; the Host/Origin guard in `server/src/main.rs` covers `/api` and `/mcp` (DNS-rebinding). Don't loosen it.
- Web: plain Svelte + Vite, no SvelteKit, npm (no Bun). `vite-plugin-singlefile` output must stay one `index.html`: it is embedded and also served as the MCP Apps resource. `web/src/transport.ts` is the only place that knows fetch vs MCP Apps bridge.
- Defaults are Swift/run-solo (`core/src/config.rs`); other languages go through `spec11.json`, not code changes.

## Sandbox notes

Port binding may be blocked (EPERM); `make dev` then cannot run in-session. Server behaviour is covered by in-process tests in `server/src/main.rs` (`tower::ServiceExt::oneshot`). Use `CARGO_HOME`/`npm_config_cache` in the scratchpad if `~/.cargo` is read-only.
