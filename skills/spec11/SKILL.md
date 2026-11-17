---
name: spec11
description: >
  Report spec-driven-development (SDD) status deterministically with the spec11 MCP server: which requirement IDs (ABC-12 in specs/*.md) have tests tagged [ABC-12] and source citations, what is not implemented, what the current changes touch, `later` requirements that are already built, check-marked ones with no proof, code no requirement covers, code covered by several requirements (conflicting or complementary specs), and one requirement traced to its tests and code. Use when asked to explore or report the SDD/spec status of a project, before changing behaviour, when asked what is implemented or untested, or to find the code a requirement affects.
---

# Spec-driven with spec11

`specs/` (or `SPEC.md`, or markdown with such lines) is the source of truth: each requirement is `- **ABC-12** MUST|SHOULD|MAY — text` (also `- ABC-12: text … MUST`, `*`/numbered lists, `| ABC-12 | MUST | text |` rows, and `SHALL`), a test proves it by tagging its title with `[ABC-12]`, and source comments cite it as `(ABC-12)`. Change the spec first, then tests and code.

spec11 finds **evidence, not truth**: a tag or citation does not prove a requirement is implemented, nor that its test passes. Relay its output; never rebuild the numbers by reading specs and tests yourself, and never change a spec marker or tag because of the report alone.

## How to call it

**MCP (preferred).** Run `spec11` in the repo once (it starts the server for that repo and opens the dashboard; `spec11 serve --no-open` does not open it) and connect the client to `http://127.0.0.1:7411/mcp` (streamable HTTP):

```json
{"mcpServers": {"spec11": {"type": "http", "url": "http://127.0.0.1:7411/mcp"}}}
```

Add it to the client's MCP config if the spec11 tools are not available. `spec11 --root DIR serve` picks the repo; without it the server uses the directory it was started in.

| Tool | Arguments | Same as CLI |
|---|---|---|
| `report` | `limit` | `spec11 report [--limit N]` |
| `status` | `spec`, `prefix`, `all`, `missing`, `changed` (git ref), `staged`, `include_untracked`, `built`, `done`, `unproven`, `issues`, `effort`, `limit` | `spec11 status [flags]` |
| `changes` | `staged`, `include_untracked` | `spec11 status --changed [--staged] [--no-untracked]`, plus the changed files: every file the working tree changed against HEAD (change: added, modified, deleted or renamed; stage; kind) and the requirements it may affect |
| `diff` | `path` (a file `changes` lists) | `git diff HEAD -- PATH`: what exactly changed in one file (staged and unstaged; an untracked file is all additions) |
| `trace` | `id` | `spec11 trace ID [--codegraph]` |
| `overlap` | `path`, `all` | `spec11 overlap [PATH]` |
| `uncovered` | `path`, `fanin`, `min` | `spec11 uncovered [PATH]` |
| `file` | `name` (omit to list) | read `specs/NAME` |
| `source` | `path` | read a scanned test or source file |
| `dashboard` | `view` (optional page, e.g. `modules`, `changes`, `requirements/DOC-25`) | URL of the web dashboard (at that page) |
| `doctor` | none | `spec11 doctor`: repo setup, CodeGraph binary and index |
| `codegraph` | `action` (init\|sync) | builds/refreshes the CodeGraph index (`doctor --fix`); writes files, slow: only when the user asks |
| `module` | `path` (a module), `hidden` (true/false) | hides the module (adds its folder to `.spec11ignore` so scans leave it out) or shows it again (removes that line; the path is one of `hidden` in `spec11://modules`); writes a file: only when the user asks |
| `install_status` | none | whether the skill and MCP entry are installed here or globally |
| `install`, `uninstall` | `global`, `port` | `spec11 install/uninstall [--global]` (they write files: only when the user asks) |

**CLI (no server needed).** `spec11 [--root DIR] [--json] report|status|trace|overlap|uncovered`. Output is plain text, identical on every run for the same tree.

**Configuration.** None is needed in the usual layouts: the specs folder (`specs/`, `spec/`, `docs/specs/`, `openspec/specs/`, `.kiro/specs/`, a `docs/` holding requirement bullets) and the source and test folders, extensions and regexes of the dominant language (Swift, Rust, TypeScript/JavaScript, Python, Go, Java/Kotlin/Scala, C#, Ruby, PHP, C/C++) are detected, from the CodeGraph index when there is one. Symlinks count only inside the repo root or `ALLOWED_PATHS`. Anything else, or a second language, goes in an optional `spec11.json` at the repo root: `specs`, `tests`, `src`, `ext`, `comment`, `decl`, `assert`, `off`, `test_files`, `retired_prefixes`, `codegraph`, `infra_paths`; a field you set replaces its detected value.

## Exploring the project status

When asked to explore, audit or report the SDD status, call `report` and relay it. It prints a headline, a one-line problem tally, and one section per problem kind (an empty one says `none`):

| Section | Meaning |
|---|---|
| unimplemented | due requirements with no test and no code citation |
| untested | due requirements cited in code but with no `[ID]` test |
| check-marked done without proof | ✅-style marks with `NO-TEST` (strong) or `NO-LINK` |
| marked later but already has tests or code | probably built, wholly or in part |
| invalid tests | tags for an ID no spec defines (`unknown`) or a struck-out one (`retired`); an ID tagged only in comments (`comment-only`), only on disabled tests (`disabled`), or only on tests with no direct assertion (`no-assert`, may assert through a helper) |
| spec issues | duplicate IDs, bullets without MUST/SHOULD/MAY, misplaced markers, citations of undefined IDs, a check mark with `(later)` |
| dangling citations | IDs cited by tests or source that no spec defines |
| specs to compare | declarations covered by two or more requirements whose specs do not cite each other: possible conflict (needs the CodeGraph index) |
| code with no requirement | the top of `uncovered` (needs the CodeGraph index) |

Then:
1. Give the user the report (the headline, tally and every non-empty section), then your reading of it: the two or three problems that matter most and why. Counts and ID lists come only from spec11.
2. Drill down with `status` flags, `trace`, `overlap`, `uncovered`, never by grepping.
3. Ask which effort the user wants for judgement calls (`low`, `medium`, `high`, below); default `low` is the report as it stands. Never call two requirements conflicting from the report: see "Asked for conflicting requirements".

If something looks missing ("not checked", no tests found), run `doctor`: it reports the repo layout, whether the `codegraph` binary exists and its index is initialized and fresh, and what to run. The CodeGraph index (`.codegraph/codegraph.db`, opened read-only) adds test-to-code links, overlap and uncovered code. Without one those parts say "not checked": indexing is the user's decision, so do not create the index on your own.

## status — what is implemented

**Summary (no list flag):** one line per spec file, `tested/due`, then `src-only` (cited in source, no test), `none` (neither), and counts of `manual` and `later`, the latter with how many already have tests or code, e.g. `later: 15 (3 with tests/code)`. Consecutive IDs print as ranges (`DOC-01..05`). A `TOTAL` line closes it, followed by spec issues and a hint.

**Requirement lists** print one line per requirement: `ID  state  spec-file:line  [flags]  first 100 characters`. Several flags together narrow the list (all must hold), and `spec`/`prefix` narrow it further.

| Flag | Lists |
|---|---|
| `all` | every requirement, in ID order |
| `missing` | due requirements with no test: state `src-only` or `none`. `later`, `deferred` and `manual` ones are not due |
| `changed` | requirements touched by the working tree against a git ref (default `HEAD`; staged, unstaged and untracked files count): `spec` = its bullet was edited, `test` = a test file that tags it was edited, `src` = a source file that cites it was edited. `staged` narrows it to the index, `include_untracked=false` leaves untracked files out. Matching is per file: "may be affected", not "its lines changed" |
| `built` | requirements marked `later`/`deferred` that already have a `[ID]` test or a source citation, flagged `[built?: N tests, M src]`. Evidence only: the state still needs assessing (below) |
| `done` | requirements carrying a check mark (✅ ✔ ☑ ✳) before the ID or anywhere in the bullet outside struck text. A hint, not proof. Each line gets `[DONE: verdict]` |
| `unproven` | check-marked requirements whose verdict is `NO-TEST` or `NO-LINK` |
| `issues` | requirements with a spec issue, printed under the line |

Verdicts of a check-marked requirement: `NO-TEST` (no `[ID]` test anywhere: strong evidence it is not done), `NO-LINK` (tests exist, but the CodeGraph index cannot follow any of them, through helpers up to 3 hops, to app code, so the test may prove nothing: verify), `ok` (a tagged test reaches app code), `unchecked` (no CodeGraph index). The graph link is a hint too: it shows a test calls the code, not that the test checks the requirement.

States: `tested` (a `[ID]` tag exists; run the suite to know it passes), `src-only`, `none`, `manual` (checked by a person), `later`, `ref` (a reference doc: markdown linked as `[ADR-0003](decisions/0003-x.md)`, or a file in `decisions/`/`adr/`; never due, may be outdated, so read it before relying on it). A requirement ending in `(later)` or `(deferred)` is `later`; a marker inside struck-through text is ignored.

**Spec issues** are: an ID defined twice, a bullet without MUST/SHOULD/MAY (not counted), a `(later)`/`(deferred)`/`(manual)` marker before the end of a requirement (it then counts as due), a requirement citing an ID no spec defines, a `[ID](path.md)` link to a file that does not exist, an ID linked to two files, a check mark together with a `(later)`/`(deferred)`/`(manual)` marker.

Scanned: `specs/*.md` bullets (wrapped lines joined; struck-through bullets are retired); test tags `[ABC-12]`, `[ABC-12][ABC-14]` or `[ABC-12, ABC-14]` anywhere in the test folders; source IDs after the line-comment marker (block comments are not read). Folders and extensions come from `spec11.json`.

## trace — what a requirement touches

For one ID: the full spec text and the IDs it cites, the tests tagged with it, each source comment citing it with the declaration it sits in, and related code: types named in the tagged tests' bodies (`*` = the file also cites the ID). The last line is a `codegraph explore` query. Use the output as a starting set, then read the real flow with `codegraph_explore`; name matching misses code a test reaches only indirectly. An ID unknown everywhere is an error.

## Assessing the real state — effort modes

spec11 only finds evidence. Two cases need judgement, and a quick model gets them wrong:
- **Marked later, maybe built** (`built`): the marker says not built, tags or citations say otherwise. A citation can sit on a stub, and a test can cover one clause of a long requirement.
- **Marked done, maybe not** (`unproven`, or `done`): a check mark is only a hint. `NO-TEST` is strong evidence it is not done; `NO-LINK` means the tests may not exercise the code.

Effort sets who judges and how deep. Use `low` unless the user asks for more; "deep inspection" or "high effort" means `high`.

| Effort | Who | What |
|---|---|---|
| `low` (default) | spec11 | Report the lists and verdicts as they are. No judgement, no claim that anything is implemented. |
| `medium` | one assessor at Sonnet level or above (Agent tool, `model: "sonnet"`; never `haiku`), or you if you are that capable | Split each requirement into clauses; per clause name the code (`file:line`) and the tagged test; read the code with `codegraph_explore`, not only comments. Batches of up to 5 requirements per assessor, one assessor per spec file, each given `status ... effort=medium` and `trace` output. |
| `high` | the strongest model available (`model: "opus"`), one requirement per assessor | Everything in `medium`, plus: read each tagged test and say what it really asserts; for `NO-LINK` decide whether the test exercises app code at all (the graph misses dynamic dispatch and UI tests); for `NO-TEST` look for the behaviour in code anyway and say what test is missing; run the tagged tests and report pass or fail; never infer from names or comments alone. Input: `status ... effort=high` (full spec text, test and code locations, graph links). |

The assessor does not edit. It returns, per requirement: `implemented` (every clause has working code and a test), `partial` (list the missing clauses), `stub` (cited or tagged, behaviour absent) or `not-built`, plus the evidence in one or two lines. Then act on it yourself, spec first:

- Marked later and `implemented`: remove the `(later)`/`(deferred)` marker, so it counts as due.
- Marked done and `implemented`: keep the check mark; write any missing test for a clause the code already satisfies (workflow step 4).
- Marked done but `partial`, `stub` or `not-built`: tell the user, then remove the check mark, or split the bullet into a done part and a `(later)` part. Do not leave a half-built requirement looking finished.
- Marked later and `partial`, `stub` or `not-built`: keep the marker; remove a misleading tag or citation.
- Edit specs on your own only for `implemented` marked later; confirm every other change with the user. Report one table: ID, verdict, one line of evidence.

## uncovered — code with no requirement

Reads the CodeGraph index and lists top-level types and functions in the source folders that no requirement covers, grouped by file and sorted by size, as `Name:first-last` line ranges. These are candidates for a **missing requirement**, not proof.

A unit is listed only if all of this holds, so shared plumbing does not drown the result:
- No spec ID is cited in a comment inside it or just above it.
- No test tagged `[ID]` names it in its body.
- Code that does cite an ID does not use it directly (one hop of calls, instantiates, references): a helper of a covered feature is covered.
- It is not shared infrastructure: used from fewer than `fanin` other files (default 5), and not under the `infra_paths` of `spec11.json`.
- It is `internal` or `public` and at least `min` lines (default 10).

The first line shows how many units each rule skipped. The index follows file edits within about a second; run `codegraph sync` first if code just changed.

**Triage each candidate before writing anything** (read it with `codegraph_explore "<Name>"`):
- *Business behaviour* — a rule, a computation, a saved or shown field, a user-visible flow or limit: write a requirement in the matching spec (the specs index, next free ID, never a retired one), test it with `[ID]`, and cite `(ID)` at the code.
- *Plumbing* — framework glue, layout, styling, wiring with no rule of its own, generic helpers, platform adapters: ignore it. If a whole kind keeps showing up, raise `fanin` or add it to `infra_paths` instead of writing requirements for it.
- *Belongs to an existing requirement* — add `(ID)` in a comment at the code; the spec already says it.

## Resources — modules and tests

Read the MCP resource `spec11://modules` for the repo's module structure (nearest build manifest; with a CodeGraph index also symbols per kind and which modules each depends on) and `spec11://tests` for the test cases found in the test files, grouped by module and file, each with the `[ID]` tags on it. Tests are detected by marker (`#[test]`, `@Test`, `def test_`, `it(` …), not run: a listed test does not say it passes. Set `case` in `spec11.json` for an unusual marker.

## overlap — code several requirements cover

Lists declarations (method, function, type, property) that two or more requirements cover, from the CodeGraph index. A requirement covers a declaration when its ID is cited in a comment inside it or in the doc comment right above it. Each pair of IDs is marked:
- `linked`: one spec's text cites the other, or both cite a third ID. The specs already know each other, so they are probably complementary.
- `UNLINKED`: no declared relation. Read these first; the specs may conflict, repeat each other, or share code only by accident.

Pairs from the same spec file are hidden unless `all`. spec11 finds the overlap; deciding what it means is a judgement (`medium` for `complementary`, `duplicate` and `incidental`; any claim of `conflicting` needs the strong-model review below). For each `UNLINKED` pair read both requirements (`trace`) and the declaration (`codegraph_explore "<Name>"`), then classify:

| Verdict | Test | Action |
|---|---|---|
| conflicting | they demand incompatible behaviour of this code: different limits, formats, defaults, ordering, or one forbids what the other requires | Only after the review below. Show the user both texts and the code. Never pick a side silently. After the user decides, fix the losing spec, its tests and the code |
| complementary | different aspects of one feature (one validates, one stores, one displays) or one refines the other | Add a reference in each bullet (`see ABC-12`), so the next run reports the pair as `linked` |
| duplicate | the same rule stated twice | Propose retiring one: strike it through and keep its ID reserved; the user approves; move its tests' tags to the survivor |
| incidental | the code is shared plumbing and one citation is only a loose pointer | Move that citation to the code that really implements it, or remove it |

### Asked for conflicting requirements: a strong model must review first

An `UNLINKED` pair is a candidate, not a finding: two IDs on one declaration only prove that someone cited both. When the user asks which requirements conflict, or when you are about to call a pair `conflicting`, do not answer from the tool output or from the spec texts alone. Hand each candidate pair to the strongest model available (Agent tool, `model: "opus"`, one pair per assessor; a Sonnet-level or smaller model is not enough, and if no strong model can be used, say so and report the pair only as an unverified candidate). The reviewer, who does not edit anything, must:

1. **Understand the current code first.** Read the declaration and what it does (`codegraph_explore "<Name>"`), its callers and callees, the tests tagged with either ID (`trace`), and any other code that cites either ID. Say in a few lines what the code does today for each requirement: implements A only, B only, both, or neither.
2. **Read both requirements in full** and extract their conditions: which entity, state, input, version or phase each one applies to, and whether either is `(later)`, `(deferred)`, struck or "after 1.0".
3. **Try to reconcile them.** Look for a reading in which both hold: disjoint conditions, one a refinement or exception of the other, different layers (storage vs display), different moments (on save vs on read).
4. **Only if no reading reconciles them, write the contradiction as a concrete case**: one input or situation, and the two outputs the specs demand from it. No such case, no conflict.
5. **Report** per pair: verdict (`conflicting`, `complementary`, `duplicate`, `incidental`, or `unclear` with what is missing), the concrete case for a conflict, what the code does today, and the `file:line` evidence for each claim. Never infer behaviour from names or comments.

Then relay the verdicts to the user. A conflict is reported with both bullets quoted and the case; fixing it is the user's decision (specs first, then tests and code). A `linked` pair needs no action unless the texts disagree; check that when reading the code anyway. Report one table: declaration, IDs, verdict, one line of evidence.

## Workflow

0. Specs marked `(later)` may already be built (`built`) and check-marked specs may not be (`unproven`): assess per effort (above).
1. Starting work: `changes` (or `status changed=HEAD`) shows which requirements the current changes already touch; `missing=true prefix=X` shows what an area still lacks.
2. Behaviour change: `trace`, read the spec and its tests, edit the spec first.
3. New requirement: add the bullet, write the tests, and cite `(ID)` in a comment at the code that implements it.
4. Writing or changing a test: it must name the requirement it proves, in the form spec11 reads, square brackets with the ID:
   - Title: e.g. `@Test("[ABC-12] what holds")` (Swift Testing; use the test framework's own title or name). One test per behaviour; list several IDs as `[ABC-12][ABC-14]` or `[ABC-12, ABC-14]` only when one assertion set really proves both.
   - Comment: in the body, a `// [ABC-13] why this line proves it` beside each assertion or setup that proves a *different* requirement than the title, and above a test helper or fixture that exists for one. A bare `(ABC-12)` in a test comment is not read.
   - No ID fits: the behaviour is unspecified. Write the requirement first (step 3), then the test. Never tag a test with a nearby ID that does not say what it checks.
   - After editing tests, `status changed=HEAD` should list the requirement as `tested` with `[changed: test]`.
5. Several specs on one piece of code: `overlap`, classify per the table above.
6. Cleaning up: `uncovered path=Domain/` finds behaviour nobody specified; triage as above.
7. Before finishing: `status changed=HEAD missing=true` should be empty (nothing you touched is untested) and `status issues=true` clean.
8. For a visual overview open the dashboard (`dashboard` returns its URL).
