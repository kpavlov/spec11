# spec11: Rust core + server + Svelte dashboard. Targets the run-solo repo by default.
SHELL := /bin/bash
ROOT ?= /Users/maestro/code/github/itstaffhq/run-solo
PORT ?= 7411
OPEN ?= 1
BIN  := target/release/spec11

.PHONY: help dev serve web build run report test parity clean

help:
	@echo "make dev      Vite (HMR, :5173) + Rust server on :$(PORT) proxying to it, scanning $(ROOT)"
	@echo "make serve    Rust server only (debug), scanning $(ROOT); run 'cd web && npm run dev' yourself"
	@echo "make build    release binary with the UI embedded: $(BIN)"
	@echo "make run      build, then serve the release binary on :$(PORT) for $(ROOT)"
	@echo "make report   print the text report for $(ROOT)"
	@echo "make test     cargo tests; make parity compares with the old Python scripts if vendored in $(ROOT)"
	@echo "override: make dev ROOT=/path/to/repo PORT=7500 OPEN=0   (OPEN=0: don't open the browser)"

web/node_modules: web/package.json
	cd web && npm install
	@touch $@

web: web/node_modules
	cd web && npm run build

dev: web/node_modules
	@trap 'kill 0' INT TERM EXIT; \
	(cd web && npm run dev) & \
	if [ "$(OPEN)" = 1 ]; then (until curl -sf -o /dev/null http://127.0.0.1:$(PORT)/api/info; do sleep 0.5; done; open http://127.0.0.1:$(PORT)/) & fi; \
	cargo run -p spec11 -- --root "$(ROOT)" serve --port $(PORT) --no-open

serve:
	cargo run -p spec11 -- --root "$(ROOT)" serve --port $(PORT) --no-open

# build.rs runs `npm run build` for release builds (SKIP_WEB_BUILD=1 to skip when web/dist is already built)
build: web/node_modules
	cargo build --release -p spec11

run: build
	$(BIN) --root "$(ROOT)" serve --port $(PORT)

report:
	cargo run -q -p spec11 -- --root "$(ROOT)" report

test:
	cargo test --workspace

# optional: Rust output vs the old Python scripts, if still vendored in $(ROOT)/.agents/skills/spec-driven (no longer part of this repo)
parity:
	@cargo build -q -p spec11
	@set -e; S="$(ROOT)/.agents/skills/spec-driven/scripts"; R="$(CURDIR)/target/debug/spec11"; cd "$(ROOT)"; \
	norm() { sed -e 's/status\.py/spec11 status/g; s/trace\.py/spec11 trace/g'; }; \
	for a in "report.py|report" "status.py|status" "status.py --missing|status --missing" "status.py --issues|status --issues" \
	         "status.py --built|status --built" "status.py --overlap|status --overlap" "status.py --uncovered|status --uncovered"; do \
	  py=$${a%%|*}; rs=$${a##*|}; \
	  if diff <(python3 -B $$S/$$py 2>&1 | norm) <($$R $$rs 2>&1) >/dev/null; then echo "OK   $$rs"; else echo "DIFF $$rs"; exit 1; fi; \
	done

clean:
	cargo clean
	rm -rf web/dist web/node_modules
