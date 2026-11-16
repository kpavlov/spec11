---
name: spec-driven
description: >
  Report the SDD (spec-driven) project status deterministically, check which spec requirement IDs (ABC-12 in specs/*.md) have tests tagged [ABC-12] and source comments, list requirements that are not implemented, touched by the current changes or have spec issues, find `later` requirements that are already built and check-marked ones with no proof, find code that no requirement covers, find code covered by several requirements (conflicting or complementary specs), and trace one requirement to its tests and code. Use when asked to explore or report the SDD/spec status of the project, before changing behaviour, when asked what is implemented or untested, or to find the code a requirement affects.
---

# Spec-driven

`specs/` is the source of truth: each requirement is `- **ABC-12** MUST|SHOULD|MAY — text`, a test proves it by tagging its title `@Test("[ABC-12] …")` (specs/README.md), and source comments cite it as `(ABC-12)`. Change the spec first, then tests and code.

## Exploring the project status

When asked to explore, audit or report the SDD status of the project, **run the script and relay its output; do not rebuild the numbers by reading specs and tests yourself**. The report is plain text and identical on every run for the same tree, so two people (or models) get the same answer.

```sh
python3 .agents/skills/spec-driven/scripts/report.py [--limit N]
```

It finds every spec file and requirement and prints a headline, a one-line problem tally, and one section per problem kind (an empty one says `none`):

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
| code with no requirement | the top of `status.py --uncovered` (needs the CodeGraph index) |

Then:
1. Give the user the report (paste it, or the headline, tally and every non-empty section), then your reading of it: the two or three problems that matter most and why. Counts and ID lists come only from the script.
2. Drill down with the flags in the next section (`--missing`, `--unproven`, `--built`, `--issues`, `--overlap PATH`, `--uncovered PATH`, `trace.py ID`), never by grepping.
3. The script reports evidence, not truth. Never call two requirements conflicting from the report: see "Asked for conflicting requirements" below. Ask which effort the user wants for the judgement calls (`low`, `medium`, `high`, see below); default `low` is the report as it stands. Never change a spec marker or tag because of the report alone.

## Scripts

Scripts (Python 3 stdlib, ~50 ms, run from anywhere in the repo; paths are `.agents/skills/spec-driven/scripts/`):

```sh
report.py [--limit N]                                       # whole-project status, deterministic text
status.py [--spec 04] [--prefix DOC]                       # summary per spec file
status.py --all | --missing | --changed [REF] | --issues   # one line per requirement
status.py --built                                           # later/deferred requirements that already have tests or code
status.py --done | --unproven                               # check-marked (✅ ✔️ ☑️ ✳️) requirements, and those with no proof
status.py ... --effort low|medium|high                      # evidence per listed line (see effort modes below)
status.py --uncovered [PATH]                                # code no requirement covers (same as uncovered.py)
status.py --overlap [PATH]                                  # code covered by 2+ requirements (same as overlap.py)
trace.py DOC-25 [--codegraph]                               # one requirement in depth
selfcheck.py                                                # run after editing the scripts
```

## status.py — what is implemented

**Summary (no list flag):** one line per spec file, `tested/due`, then `src-only` (cited in app source, no test), `none` (neither), and counts of `manual` and `later`, the latter with how many already have tests or code, e.g. `later: 15 (3 with tests/code)`. Consecutive IDs print as ranges (`DOC-01..05`). A `TOTAL` line closes it, followed by spec issues (first 10) and a hint.

**Requirement lists:** `--all`, `--missing`, `--changed [REF]` and `--issues` print one line per requirement:

```
TRIP-02 src-only 18-trips.md:38 [changed: spec,src] [issue]  The card holds title …
```

`ID  state  spec-file:line  [flags]  first 100 characters`. Several flags together narrow the list (all must hold), and `--spec`/`--prefix` narrow it further.

| Flag | Lists |
|---|---|
| `--all` | every requirement, in ID order |
| `--missing` | due requirements with no test: state `src-only` or `none`. `later`, `deferred` and `manual` ones are not due |
| `--changed [REF]` | requirements touched by the working tree against `REF` (default `HEAD`; staged, unstaged and untracked files count): `spec` = its spec bullet was edited, `test` = a test file that tags it was edited, `src` = a source file that cites it was edited. The test and source match is per file, so it means "may be affected", not "its lines changed" |
| `--built` | requirements marked `later`/`deferred` that already have a `[ID]` test or a source citation, flagged `[built?: N tests, M src]`: the marker says not built, the code says otherwise. Evidence only, so the state still needs assessing (see below) |
| `--done` | requirements carrying a check mark (✅ ✔ ☑ ✳, with or without the variation selector) before the ID or anywhere in the bullet outside struck text. A check mark is a hint that the requirement is done, not proof. Each line gets `[DONE: verdict]` |
| `--unproven` | check-marked requirements whose verdict is `NO-TEST` or `NO-LINK` |
| `--issues` | requirements with a spec issue, the issue printed under the line |

Verdicts of a check-marked requirement: `NO-TEST` (no `[ID]` test anywhere: strong evidence it is not done), `NO-LINK` (tests exist, but the CodeGraph index cannot follow any of them, through test helpers up to 3 hops, to app code, so the test may prove nothing: verify), `ok` (a tagged test reaches app code), `unchecked` (no CodeGraph index). The summary always ends with `check-marked done: N | NO TEST: … | tests not linked to code: …`. The graph link is a hint too: it shows a test calls the code, not that the test checks the requirement.

States: `tested` (a `[ID]` tag exists, which does not mean the test passes: run the suite), `src-only`, `none`, `manual` (checked by a person), `later`. A requirement ending in `(later)` or `(deferred)` is `later`; both mean not built yet. A marker inside a struck-through sentence is ignored.

Always printed: **check-marked done** counts (above), **tests/sources citing IDs not in specs** (typos or retired IDs). **Spec issues** are: an ID defined twice, a bullet without MUST/SHOULD/MAY (not counted), a `(later)`/`(deferred)`/`(manual)` marker before the end of a requirement (it then counts as due), a requirement citing an ID no spec defines, a requirement with a check mark that is also marked `(later)`/`(deferred)`/`(manual)`.

Scanned: `specs/*.md` bullets (wrapped lines are joined; struck-through bullets are retired); tests in `RunSolo/RunSoloTests` and `RunSolo/RunSoloUITests` (`[ABC-12]`, `[ABC-12][ABC-14]` or `[ABC-12, ABC-14]` anywhere); sources in `RunSolo/RunSolo` (IDs after `//`, so block comments are not read).

## trace.py — what a requirement touches

For one ID prints the full spec text and the IDs it cites, the tests tagged with it, each source comment citing it with the declaration it sits in, and related code: types named in the tagged tests' bodies (`*` = the file also cites the ID). The last line is a `codegraph explore` query; `--codegraph` runs it. Use the output as a starting set, then read the real flow with `codegraph_explore`; name matching misses code a test reaches only indirectly. Exit 1 if the ID is unknown everywhere.

## Assessing the real state — effort modes

The scripts only find evidence. Two cases need judgement, and a quick model gets them wrong:
- **Marked later, maybe built** (`--built`): the marker says not built, tags or citations say otherwise. A citation can sit on a stub, and a test can cover one clause of a long requirement.
- **Marked done, maybe not** (`--unproven`, or `--done`): a check mark is only a hint. `NO-TEST` is strong evidence it is not done; `NO-LINK` means the tests may not exercise the code.

Effort sets who judges and how deep. Use `low` unless the user asks for more; "deep inspection" or "high effort" means `high`.

| Effort | Who | What |
|---|---|---|
| `low` (default) | the scripts | Report the lists and verdicts as they are, with `--effort low` counts. No judgement, no claim that anything is implemented. |
| `medium` | one assessor at Sonnet level or above (Agent tool, `model: "sonnet"`; never `haiku`), or you if you are that capable | Split each requirement into clauses; per clause name the code (`file:line`) and the tagged test; read the code with `codegraph_explore`, not only comments. Batches of up to 5 requirements per assessor, one assessor per spec file, each given `status.py ... --effort medium` and `trace.py ID` output. |
| `high` | the strongest model available (`model: "opus"`), one requirement per assessor | Everything in `medium`, plus: read each tagged test and say what it really asserts; for `NO-LINK` decide whether the test exercises app code at all (the graph misses dynamic dispatch and UI tests); for `NO-TEST` look for the behaviour in code anyway and say what test is missing; run the tagged tests (`RunSomeTests`, or `xcodebuild test -only-testing`) and report pass or fail; never infer from names or comments alone. Input: `status.py ... --effort high` (full spec text, test and code locations, graph links). |

The assessor does not edit. It returns, per requirement: `implemented` (every clause has working code and a test), `partial` (list the missing clauses), `stub` (cited or tagged, behaviour absent) or `not-built`, plus the evidence in one or two lines. Then act on it yourself, spec first:

- Marked later and `implemented`: remove the `(later)`/`(deferred)` marker, so it counts as due.
- Marked done and `implemented`: keep the check mark; write any missing test for a clause the code already satisfies (workflow step 4).
- Marked done but `partial`, `stub` or `not-built`: tell the user, then remove the check mark, or split the bullet into a done part and a `(later)` part. Do not leave a half-built requirement looking finished.
- Marked later and `partial`, `stub` or `not-built`: keep the marker; remove a misleading tag or citation.
- Edit specs on your own only for `implemented` marked later; confirm every other change with the user. Report one table: ID, verdict, one line of evidence.

## uncovered.py — code with no requirement

`status.py --uncovered [PATH]` (or `uncovered.py [PATH] [--fanin N] [--min N]`) reads the CodeGraph index (`.codegraph/codegraph.db`, read-only; if there is none it stops, because indexing is the user's decision) and lists top-level types and functions of `RunSolo/RunSolo` that no requirement covers, grouped by file and sorted by size, as `Name:first-last` line ranges. These are candidates for a **missing requirement**, not proof.

A unit is listed only if all of this holds, so shared plumbing does not drown the result:
- No spec ID is cited in a comment inside it or just above it.
- No test tagged `[ID]` names it in its body.
- Code that does cite an ID does not use it directly (one hop of calls, instantiates, references): a helper of a covered feature is covered.
- It is not shared infrastructure: used from fewer than `--fanin` other files (default 5), and not under `UI/Components/`, `UI/DataTypes/` or the app entry.
- It is `internal` or `public` and at least `--min` lines (default 10).

The first line shows how many units each rule skipped. The index follows file edits within about a second; run `codegraph sync` first if you just changed code.

**Triage each candidate before writing anything** (read it with `codegraph_explore "<Name>"`):
- *Business behaviour* — a rule, a computation, a saved or shown field, a user-visible flow or limit: write a requirement in the matching spec (`specs/README.md` index, next free ID, never a retired one), test it with `[ID]`, and cite `(ID)` at the code.
- *Plumbing* — framework glue, layout, styling, SwiftUI wiring with no rule of its own, generic helpers, platform adapters: ignore it. If a whole kind keeps showing up (a shared helper type), raise `--fanin` or `INFRA_PATHS` in `uncovered.py` instead of writing requirements for it.
- *Belongs to an existing requirement* — add `(ID)` in a comment at the code; the spec already says it.

## overlap.py — code several requirements cover

`status.py --overlap [PATH]` (or `overlap.py [PATH] [--all]`) lists declarations (method, function, type, property) of `RunSolo/RunSolo` that two or more requirements cover, from the CodeGraph index. A requirement covers a declaration when its ID is cited in a comment inside it or in the doc comment right above it. Each pair of IDs is marked:
- `linked`: one spec's text cites the other, or both cite a third ID. The specs already know each other, so they are probably complementary.
- `UNLINKED`: no declared relation. Read these first; the specs may conflict, repeat each other, or share code only by accident.

Pairs from the same spec file are hidden unless `--all`. The script finds the overlap; deciding what it means is a judgement (effort modes above: `medium` for `complementary`, `duplicate` and `incidental`; any claim of `conflicting` needs the strong-model review below). For each `UNLINKED` pair read both requirements (`trace.py ID`) and the declaration (`codegraph_explore "<Name>"`), then classify:

| Verdict | Test | Action |
|---|---|---|
| conflicting | they demand incompatible behaviour of this code: different limits, formats, defaults, ordering, or one forbids what the other requires | Only after the review below. Show the user both texts and the code. Never pick a side silently. After the user decides, fix the losing spec, its tests and the code |
| complementary | different aspects of one feature (one validates, one stores, one displays) or one refines the other | Add a reference in each bullet (`see ABC-12`), so the next run reports the pair as `linked` |
| duplicate | the same rule stated twice | Propose retiring one: strike it through and keep its ID reserved (specs/README.md); the user approves; move its tests' tags to the survivor |
| incidental | the code is shared plumbing and one citation is only a loose pointer | Move that citation to the code that really implements it, or remove it |

### Asked for conflicting requirements: a strong model must review first

An `UNLINKED` pair is a candidate, not a finding: two IDs on one declaration only prove that someone cited both. When the user asks which requirements conflict, or when you are about to call a pair `conflicting`, do not answer from the script output or from the spec texts alone. Hand each candidate pair to the strongest model available (Agent tool, `model: "opus"`, one pair per assessor; a Sonnet-level or smaller model is not enough, and if no strong model can be used, say so and report the pair only as an unverified candidate). The reviewer, who does not edit anything, must:

1. **Understand the current code first.** Read the declaration and what it does (`codegraph_explore "<Name>"`), its callers and callees, the tests tagged with either ID (`trace.py ID`), and any other code that cites either ID. Say in a few lines what the code does today for each requirement: implements A only, B only, both, or neither.
2. **Read both requirements in full** and extract their conditions: which entity, state, input, version or phase each one applies to, and whether either is `(later)`, `(deferred)`, struck or "after 1.0".
3. **Try to reconcile them.** Look for a reading in which both hold: disjoint conditions, one a refinement or exception of the other, different layers (storage vs display), different moments (on save vs on read).
4. **Only if no reading reconciles them, write the contradiction as a concrete case**: one input or situation, and the two outputs the specs demand from it. No such case, no conflict.
5. **Report** per pair: verdict (`conflicting`, `complementary`, `duplicate`, `incidental`, or `unclear` with what is missing), the concrete case for a conflict, what the code does today, and the `file:line` evidence for each claim. Never infer behaviour from names or comments.

Then relay the verdicts to the user. A conflict is reported with both bullets quoted and the case; fixing it is the user's decision (specs first, then tests and code).

A `linked` pair needs no action unless the texts disagree; check that when reading the code anyway. Report one table: declaration, IDs, verdict, one line of evidence.

## Workflow

0. Specs marked `(later)` may already be built (`status.py --built`) and check-marked specs may not be (`status.py --unproven`): assess per effort (above).
1. Starting work: `status.py --changed` shows which requirements the current changes already touch; `--missing --prefix X` shows what an area still lacks.
2. Behaviour change: `trace.py ID`, read the spec and its tests, edit the spec first.
3. New requirement: add the bullet, write the tests, and cite `(ID)` in a comment at the code that implements it.
4. Writing or changing a test: it must name the requirement it proves, in the form the scripts read, square brackets with the ID:
   - Title: `@Test("[ABC-12] what holds")`. One test per behaviour; list several IDs as `[ABC-12][ABC-14]` or `[ABC-12, ABC-14]` only when one assertion set really proves both.
   - Comment: in the body, a `// [ABC-13] why this line proves it` beside each assertion or setup that proves a *different* requirement than the title, and above a test helper or fixture that exists for one. A bare `(ABC-12)` in a test comment is not read.
   - No ID fits: the behaviour is unspecified. Write the requirement first (step 3), then the test. Never tag a test with a nearby ID that does not say what it checks.
   - After editing tests, `status.py --changed` should list the requirement as `tested` with `[changed: test]`.
5. Several specs on one piece of code: `status.py --overlap`, classify per the table above.
6. Cleaning up: `status.py --uncovered Domain/` finds behaviour nobody specified; triage as above.
7. Before finishing: `status.py --changed --missing` should be empty (nothing you touched is untested) and `status.py --issues` clean.
