# AGENTS.md

spec11: Rust workspace (`core`, `server`) + Svelte dashboard (`web`) + the agent skill (`skills/spec11/SKILL.md`, embedded in the binary by `server/src/skill.rs`). See README.md for usage.

## Commands

- `cargo test --workspace` (debug) and `SKIP_WEB_BUILD=1 cargo test --release --workspace` (the embedded-UI test runs only in release; needs `web/dist`)
- `make parity` (optional) compares with the old Python scripts if they are vendored in `ROOT/.agents/skills/spec-driven`; the scripts are no longer in this repo.
- `cd web && pnpm run build && pnpm run check`
- `make dev` / `make run`; override `ROOT`, `PORT`, `OPEN=0`

## Rules

- **Evidence, not truth.** Tool descriptions, UI and docs must not claim a requirement is implemented or a test passes. No LLM in the core.
- Output is deterministic: same tree, same text. Text renderers in `core/src/report.rs` kept the Python output byte for byte; changing them changes what `SKILL.md` describes, so update it too.
- **All SQL lives in `core/src/codegraph.rs`**, read-only (`SQLITE_OPEN_READ_ONLY`). Schema drift is a typed `SchemaDrift` error rendered as "not checked", never a panic. A missing index is not an error either: indexing is the user's decision.
- Tools are defined once in `server/src/ops.rs`; the CLI and `/mcp` both go through it, and the dashboard calls `/mcp` too (there is no separate tool API under `/api`, which keeps only `/api/info`). Add a tool there, not per interface.
- `Analysis` uses `Rc` (not `Send`): run it inside `spawn_blocking`.
- The server binds 127.0.0.1 only; the Host/Origin guard in `server/src/main.rs` covers `/api` and `/mcp` (DNS-rebinding). Don't loosen it.
- Web: plain Svelte + Vite, no SvelteKit, pnpm (no npm, no Bun; `packageManager` in `web/package.json` pins it). `vite-plugin-singlefile` output must stay one `index.html`: it is embedded and also served as the MCP Apps resource. `web/src/transport.ts` is the only place that knows fetch vs MCP Apps bridge.
- No hardcoded layout: empty `spec11.json` fields are detected in `core/src/detect.rs` (a table of ecosystem profiles, specs folder names, CodeGraph file list via `Graph::file_paths`, else a walk). Ecosystem profiles (extensions, comments, declarations, tests, test-case markers) live in `core/resources/profiles.toml`: support a new language by adding a `[[profile]]` there, no Rust change. All walking goes through `detect::walk`: symlinks only into the root or `ALLOWED_PATHS`; keep every file listing on it. `detect::Ignore` (`.aiignore`, `.spec11ignore`, built-in `.agents`) is applied in `walk` and to the CodeGraph file list and module data; a new listing of graph files must filter with `Project::ignored`. Rust inline tests make auto-detected `tests` include `src`, so pin `tests` when doc examples look like tags.

- `spec11 install/uninstall` (`server/src/skill.rs`): `.mcp.json` is merged, never rewritten wholesale (keep `serde_json` `preserve_order`; other servers stay; invalid JSON is left alone). Only `SKILL.md` and, if it ends up empty, its folder are removed. Edit `skills/spec11/SKILL.md` to change the skill: it is `include_str!`-ed. The same install logic backs the CLI, the `spec_install*` tools and the dashboard Setup tab (`skill::apply`/`status`); global MCP config (`~/.claude.json`) is only read, never written. The server's workspace is resolved once at startup (`Project::open`), not per request.

- `spec11 doctor` (`server/src/doctor.rs`) `run` only reads. The one writer is `doctor::index` (`codegraph init`/`sync`), reached only by an explicit user action: `doctor --fix`, `spec_codegraph`, the Setup tab button (with a confirm). Never call it from a check, a startup path or a scan. Add new checks to `run`; the CLI, `spec_doctor` and the Setup tab share it.

- Spec places (`core/src/detect.rs::spec_paths`): a specs folder, single files (`SPEC.md`), any markdown with a requirement line as a last resort; `Spec.file` is repo-root-relative. `[ID](x.md)` links and decision folders give `ref` docs (`Project::docs`): never due.

- **Dogfooding.** `specs/*.md` are this repo's requirements (`SCAN-`, `GRAPH-`, `API-`, `SETUP-`, `WEB-`), configured by `spec11.json` for Rust. A Rust test is tagged with `#[doc = "[API-02]"]` above its `#[test]` (a `// [ID]` comment would count as comment-only); code cites with `// (API-02)`. Fixture tags in tests must not look real: write `[DEM\x2D01]`. Run `make report`; new behaviour gets a spec bullet first.

## Sandbox notes

Port binding may be blocked (EPERM); `make dev` then cannot run in-session. Server behaviour is covered by in-process tests in `server/src/main.rs` (`tower::ServiceExt::oneshot`). Use `CARGO_HOME`/`npm_config_cache`/`pnpm --store-dir` in the scratchpad if `~/.cargo` is read-only.
