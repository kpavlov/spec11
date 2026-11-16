# AGENTS.md

spec11: Rust workspace (`core`, `server`) + Svelte dashboard (`web`) + the agent skill (`skills/spec11/SKILL.md`, embedded in the binary by `server/src/skill.rs`). See README.md for usage.

## Commands

- `cargo test --workspace` (debug) and `SKIP_WEB_BUILD=1 cargo test --release --workspace` (the embedded-UI test runs only in release; needs `web/dist`)
- `make parity` (optional) compares with the old Python scripts if they are vendored in `ROOT/.agents/skills/spec-driven`; the scripts are no longer in this repo.
- `cd web && npm run build && npx svelte-check`
- `make dev` / `make run`; override `ROOT`, `PORT`, `OPEN=0`

## Rules

- **Evidence, not truth.** Tool descriptions, UI and docs must not claim a requirement is implemented or a test passes. No LLM in the core.
- Output is deterministic: same tree, same text. Text renderers in `core/src/report.rs` kept the Python output byte for byte; changing them changes what `SKILL.md` describes, so update it too.
- **All SQL lives in `core/src/codegraph.rs`**, read-only (`SQLITE_OPEN_READ_ONLY`). Schema drift is a typed `SchemaDrift` error rendered as "not checked", never a panic. A missing index is not an error either: indexing is the user's decision.
- Tools are defined once in `server/src/ops.rs`; the CLI, `/api/call/{name}` and `/mcp` all go through it. Add a tool there, not per interface.
- `Analysis` uses `Rc` (not `Send`): run it inside `spawn_blocking`.
- The server binds 127.0.0.1 only; the Host/Origin guard in `server/src/main.rs` covers `/api` and `/mcp` (DNS-rebinding). Don't loosen it.
- Web: plain Svelte + Vite, no SvelteKit, npm (no Bun). `vite-plugin-singlefile` output must stay one `index.html`: it is embedded and also served as the MCP Apps resource. `web/src/transport.ts` is the only place that knows fetch vs MCP Apps bridge.
- Defaults are Swift/run-solo (`core/src/config.rs`); other languages go through `spec11.json`, not code changes.

- `spec11 install/uninstall` (`server/src/skill.rs`): `.mcp.json` is merged, never rewritten wholesale (keep `serde_json` `preserve_order`; other servers stay; invalid JSON is left alone). Only `SKILL.md` and, if it ends up empty, its folder are removed. Edit `skills/spec11/SKILL.md` to change the skill: it is `include_str!`-ed. The same install logic backs the CLI, the `spec_install*` tools and the dashboard Setup tab (`skill::apply`/`status`); global MCP config (`~/.claude.json`) is only read, never written. The server's workspace is resolved once at startup (`Project::open`), not per request.

- `spec11 doctor` (`server/src/doctor.rs`) only reads: it must never run `codegraph init/sync` or write files. Add new checks there; the CLI, `spec_doctor` and the Setup tab share it.

## Sandbox notes

Port binding may be blocked (EPERM); `make dev` then cannot run in-session. Server behaviour is covered by in-process tests in `server/src/main.rs` (`tower::ServiceExt::oneshot`). Use `CARGO_HOME`/`npm_config_cache` in the scratchpad if `~/.cargo` is read-only.
