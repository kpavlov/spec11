---
title: spec17
description: Deterministic spec-driven-development evidence for a repo. No LLM in the core.
---

spec17 reads the requirement IDs in your specs and reports which ones have tests, code citations, spec problems, or overlap with other requirements. The output is the same for the same tree.

> **Evidence, not truth.** A citation or test tag does not prove a requirement is implemented or that its test passes. The output is a candidate list for a human or an assessor to judge.

One Rust binary gives three interfaces over the same core: a CLI, an MCP server (streamable HTTP or stdio) and a web dashboard.

## Get started

The [Quickstart](quickstart/) covers installing spec17 (Homebrew on Apple Silicon, or from source), scanning a repo, and a three-file example that shows a requirement, a test and a code citation meeting in the report.

No config is needed: spec folders, source and test folders and the language are detected. Then: [every command](usage/), [how requirements and tests are recognised](specs/), and [connecting an agent](agents/).
