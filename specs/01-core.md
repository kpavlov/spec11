# Core: scanning and CodeGraph

Deterministic evidence only: same tree, same text. No LLM in the core.

## Scanning

- **SCAN-01** MUST — read requirements from `- **ABC-12** MUST|SHOULD|MAY — text` bullets in the specs folder, joining a wrapped bullet's continuation lines.
- **SCAN-02** MUST — find test tags `[ABC-12]` (also `[ABC-12, ABC-14]`) in the test folders.
- **SCAN-03** MUST — count a source citation only when the ID follows a line-comment marker; block comments and code are not scanned.
- **SCAN-04** MUST — give each requirement one state: `tested`, `src-only`, `none`, `manual` or `later`; struck-out bullets are retired and never due.
- **SCAN-05** MUST — print the same text for the same tree: no clocks, randomness or map-order dependence.
- **SCAN-06** SHOULD — take folders, extensions and regexes from an optional `spec11.json`, and fall back to defaults when it is missing.
- **SCAN-07** MUST — report invalid test tags (unknown, retired, comment-only, disabled, no assertion) and dangling citations as problems to judge, not as verdicts.
- **SCAN-08** MUST — trace one ID to its spec text, tests and code citations.

## CodeGraph

- **GRAPH-01** MUST — open the CodeGraph index read-only, with all SQL in `core/src/codegraph.rs`.
- **GRAPH-02** MUST — turn a schema the code does not know into a typed error shown as "not checked", never a panic.
- **GRAPH-03** MUST — treat a missing index as "not checked", not as an error: indexing is the user's decision.
- **GRAPH-04** SHOULD — link tests to code through the index, and answer callers, impact and search queries from it.
