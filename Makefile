# spec17: Rust core + server + Svelte dashboard. Scans the current directory unless ROOT is given.
SHELL := /bin/bash
ROOT ?= $(CURDIR)
PORT ?= 7411
OPEN ?= 1
BIN  := target/release/spec17
# macOS and Linux
OS   := $(shell uname -s | tr A-Z a-z)
ARCH := $(shell uname -m | sed s/arm64/aarch64/)
OPENER := $(shell command -v open || command -v xdg-open)
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
# archive label: the version, or "edge" (the rolling prerelease of main); TAG: the release a tagged upload goes to
LABEL ?= $(VERSION)
TAG ?= v$(VERSION)
# macOS signing identity: "-" is ad hoc (enough for Homebrew); a "Developer ID Application: ..." identity for wider distribution
SIGN_IDENTITY ?= -
archive = dist/spec17-$(1)-$(OS)-$(ARCH).tar.gz

.PHONY: help dev serve web build release-build run install uninstall dist sign check-tag upload upload-edge report test parity bench upgrade clean mcp-inspector

help:
	@echo "make dev      Vite (HMR, :5173) + Rust server on :$(PORT) proxying to it, scanning $(ROOT)"
	@echo "make serve    Rust server only (debug), scanning $(ROOT); run 'cd web && pnpm run dev' yourself"
	@echo "make build    release binary with the UI embedded: $(BIN)"
	@echo "make run      build, then serve the release binary on :$(PORT) for $(ROOT)"
	@echo "make install   build + install spec17 for your user into ~/.cargo/bin; make uninstall removes it"
	@echo "make dist      dist/spec17-$(VERSION)-$(OS)-$(ARCH).tar.gz (+ .sha256) for this machine (macOS or Linux); signed on macOS"
	@echo "make sign      macOS: codesign the release binary (SIGN_IDENTITY, default ad hoc)"
	@echo "make upload    add this machine's archive to the prerelease TAG (default v$(VERSION), must be pushed and checked out); creates it if missing"
	@echo "make upload-edge  add it to the rolling 'edge' prerelease: HEAD must be origin/main; moves the edge tag to HEAD for direct downloads"
	@echo "make report   print the text report for $(ROOT)"
	@echo "make test     cargo tests; make parity compares with the old Python scripts if vendored in $(ROOT)"
	@echo "make bench    time spec17 on a generated repo at REVS (default: main HEAD; . = working tree), SCALE=1 RUNS=10 JSON=1"
	@echo "make upgrade   bump web (pnpm, to latest) and Rust (cargo update; cargo upgrade --incompatible if cargo-edit is installed) dependencies"
	@echo "override: make dev ROOT=/path/to/repo PORT=7500 OPEN=0   (OPEN=0: don't open the browser)"

web/node_modules: web/package.json
	cd web && pnpm install
	@touch $@

web: web/node_modules
	cd web && pnpm run build

dev: web/node_modules
	@trap 'kill 0' INT TERM EXIT; \
	(cd web && pnpm run dev) & \
	if [ "$(OPEN)" = 1 ]; then (until curl -sf -o /dev/null http://127.0.0.1:$(PORT)/api/info; do sleep 0.5; done; $(OPENER) http://127.0.0.1:$(PORT)/) & fi; \
	cargo run -p spec17 -- --root "$(ROOT)" serve --port $(PORT) --no-open

serve:
	cargo run -p spec17 -- --root "$(ROOT)" serve --port $(PORT) --no-open

# build.rs runs `pnpm run build` for release builds (SKIP_WEB_BUILD=1 to skip when web/dist is already built)
build: web/node_modules
	cargo build --release -p spec17

run: build
	$(BIN) --root "$(ROOT)" serve --port $(PORT)

# the release build embeds the web UI (build.rs runs pnpm); needs ~/.cargo/bin on PATH
install: web/node_modules
	cargo install --path server --locked --force
	@echo "installed: $$(command -v spec17 || echo 'add ~/.cargo/bin to PATH')   then: spec17 doctor"

uninstall:
	cargo uninstall spec17

# one tarball per OS/arch: build on that OS (rusqlite is bundled, so only a C compiler is needed); CI does the others.
# On macOS the binary is signed first, so an archive always holds a signed one.
# (NFR-07) Distribution builds use both lockfiles and embed a freshly built dashboard.
release-build:
	cd web && pnpm install --frozen-lockfile && pnpm run build
	SKIP_WEB_BUILD=1 cargo build --release --locked -p spec17
	@set -o pipefail; version=$$($(BIN) --version | sed -n '1p') && [ "$$version" = "spec17 $(VERSION)" ]

dist: $(if $(filter darwin,$(OS)),sign,release-build)
	@mkdir -p dist
	tar -czf $(call archive,$(LABEL)) -C target/release spec17
	@cd dist && f=$(notdir $(call archive,$(LABEL))) && { sha256sum $$f 2>/dev/null || shasum -a 256 $$f; } > $$f.sha256
	@ls -la $(call archive,$(LABEL))*

sign: release-build
	@[ "$(OS)" = darwin ] || { echo "sign: macOS only (codesign)"; exit 1; }
	codesign --force --sign "$(SIGN_IDENTITY)" $(BIN)
	codesign --verify --verbose $(BIN)

# The uploads below do what .github/workflows/prerelease.yml does, so macOS and CI can add to one prerelease in any order. They
# never promote it (gh release edit $(TAG) --prerelease=false --latest). The build comes first: a failure changes nothing on GitHub.
check-tag:
	@case "$(TAG)" in "v$(VERSION)" | "v$(VERSION)"-?*) ;; *) echo "TAG $(TAG) must be v$(VERSION) or v$(VERSION)-<suffix>"; exit 1;; esac
	@[ "$$(git rev-parse HEAD)" = "$$(git rev-parse 'refs/tags/$(TAG)^{commit}')" ] || { echo "HEAD is not $(TAG): git checkout $(TAG)"; exit 1; }
	@[ -z "$$(git status --porcelain)" ] || { echo "uncommitted changes: the archive would not be $(TAG)"; exit 1; }

upload: check-tag
	@$(MAKE) --no-print-directory dist LABEL="$(VERSION)"
	gh release view "$(TAG)" >/dev/null 2>&1 || gh release create "$(TAG)" --prerelease --generate-notes --title "$(TAG)" --verify-tag || gh release view "$(TAG)" >/dev/null
	@[ "$$(gh release view "$(TAG)" --json isPrerelease,isDraft --jq '.isPrerelease and (.isDraft | not)')" = true ] \
	  || { echo "$(TAG) must be a published prerelease"; exit 1; }
	gh release upload "$(TAG)" $(call archive,$(VERSION)) $(call archive,$(VERSION)).sha256 --clobber

# (NFR-05) Rolling Mac archive for direct downloads of the latest main.
upload-edge:
	@[ "$(OS)" = darwin ] || { echo "upload-edge: macOS only"; exit 1; }
	@git fetch -q origin main
	@[ "$$(git branch --show-current)" = main ] || { echo "upload-edge runs on main"; exit 1; }
	@[ "$$(git rev-parse HEAD)" = "$$(git rev-parse origin/main)" ] || { echo "edge is the latest main: HEAD is not origin/main"; exit 1; }
	@[ -z "$$(git status --porcelain)" ] || { echo "uncommitted changes: the archive would not be main"; exit 1; }
	@$(MAKE) --no-print-directory dist LABEL=edge
	git push --force origin HEAD:refs/tags/edge
	@set -e; n="Rolling build of main at $$(git rev-parse HEAD), replaced on every run."; \
	gh release view edge >/dev/null 2>&1 || gh release create edge --prerelease --verify-tag --title edge --notes "$$n" || gh release view edge >/dev/null; \
	gh release edit edge --prerelease --notes "$$n"
	gh release upload edge $(call archive,edge) $(call archive,edge).sha256 --clobber

report:
	cargo run -q -p spec17 -- --root "$(ROOT)" report

test:
	cargo test --workspace

# optional: Rust output vs the old Python scripts, if still vendored in $(ROOT)/.agents/skills/spec-driven (no longer part of this repo)
parity:
	@cargo build -q -p spec17
	@set -e; S="$(ROOT)/.agents/skills/spec-driven/scripts"; R="$(CURDIR)/target/debug/spec17"; cd "$(ROOT)"; \
	norm() { sed -e 's/status\.py/spec17 status/g; s/trace\.py/spec17 trace/g'; }; \
	for a in "report.py|report" "status.py|status" "status.py --missing|status --missing" "status.py --issues|status --issues" \
	         "status.py --built|status --built" "status.py --overlap|status --overlap" "status.py --uncovered|status --uncovered"; do \
	  py=$${a%%|*}; rs=$${a##*|}; \
	  if diff <(python3 -B $$S/$$py 2>&1 | norm) <($$R $$rs 2>&1) >/dev/null; then echo "OK   $$rs"; else echo "DIFF $$rs"; exit 1; fi; \
	done

# pnpm goes to latest, past the ranges in package.json; plain cargo only updates Cargo.lock within Cargo.toml ranges, so
# major bumps need cargo-edit (cargo install cargo-edit). Run `make test` and `cd web && pnpm run check` after.
upgrade:
	cd web && pnpm update --latest
	cargo update
	@if cargo upgrade --help >/dev/null 2>&1; then cargo upgrade --incompatible && cargo update; \
	else echo "cargo-edit not installed: Cargo.toml ranges kept (cargo install cargo-edit for major bumps)"; fi

clean:
	cargo clean
	rm -rf web/dist web/node_modules

mcp-inspector: ## Launch MCP Inspector UI
	@echo "🧐 MCP Inspector"
	@export MCP_INSPECTOR_SECRET_STORE=memory && \
	npx -y @modelcontextprotocol/inspector --config mcp-inspector.json

REVS ?= main HEAD
SCALE ?= 1
RUNS ?= 10
bench:
	@python3 bench/bench.py $(REVS) --scale $(SCALE) --runs $(RUNS) $(if $(JSON),--json)
