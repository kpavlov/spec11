---
title: spec17
description: Find tagged tests and code references for project requirements.
---

spec17 scans a project's requirements, tests, and source code. It shows which requirements have tagged tests or references in code, flags problems in the specifications, and can flag possible overlap between requirements. A tag or reference shows where a requirement is mentioned. spec17 does not run tests or verify that a requirement is met.

## Get started

If you have [Homebrew](https://brew.sh/) on an Apple Silicon Mac or on Linux (arm64 or x86_64), run these commands in Terminal:

```sh
brew tap kpavlov/spec17 https://github.com/kpavlov/spec17
brew install kpavlov/spec17/spec17
```

Follow the [Quickstart](quickstart/) to scan a project, open the dashboard, or try a sample project. It also covers building from source.

## How it works

![You can use spec17 directly from the dashboard or terminal, or ask an AI agent that connects through MCP. spec17 reads the project files and can also read an optional CodeGraph index.](img/overview.svg)

The [CodeGraph index](usage/#codegraph) helps spec17 find related code. You can scan requirements and find tagged tests without it.

![spec17 dashboard showing requirements, tagged test coverage, and recent changes](img/dashboard.png)

See [Usage](usage/) for commands and dashboard options, [Specs and tests](specs/) for supported formats, and [Agents](agents/) to connect an AI coding assistant.
