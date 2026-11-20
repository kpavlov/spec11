---
title: Quickstart
weight: 5
description: Install spec17, scan a repo, and see one requirement, one test and one citation meet.
---

## 1. Install

On an Apple Silicon Mac, install the prebuilt development version from the Homebrew tap:

```sh
brew tap kpavlov/spec17 https://github.com/kpavlov/spec17
brew install kpavlov/spec17/spec17-dev
spec17 --version
```

Anywhere else, build from source. You need Rust, Node and pnpm; the binary needs none of them at run time:

```sh
git clone https://github.com/kpavlov/spec17 && cd spec17
make install        # spec17 goes to ~/.cargo/bin, which must be on your PATH
```

The stable formula, `kpavlov/spec17/spec17`, has nothing to install until the first tagged release. Until then `brew install --HEAD kpavlov/spec17/spec17` builds the default branch from source and needs the same three tools.

## 2. Scan your repo

From the repo you want to look at:

```sh
cd /path/to/your/repo
spec17 doctor    # what spec17 found, and what is missing
spec17 report    # coverage, problems, where to look next
```

No config is needed: the specs folder, the source and test folders and the language are detected. `doctor` warns about optional things (a CodeGraph index, the agent skill and MCP entries, a Git work tree) and still works without them. A repo with no requirement lines opens with zero requirements; `doctor` tells you where spec17 looked.

## 3. Try it on a scratch repo

Three conventions connect a requirement to a test and to code. Make a folder with one file of each:

```sh
mkdir -p greeter/specs greeter/src greeter/tests && cd greeter
```

`specs/greeter.md`: a requirement is an ID and a level (`MUST`, `SHOULD`, `MAY`). A trailing `(later)` marks one that is not due yet.

```md
# Greeter

- **GRT-01** MUST — greet a person by name.
- **GRT-02** SHOULD — greet the world when no name is given.
- **GRT-03** MAY — support a shout flag. (later)
```

`src/greeter.py`: code cites an ID in a line comment.

```python
def greet(name=None):
    # (GRT-01) greet by name
    return f"Hello, {name or 'world'}!"
```

`tests/test_greeter.py`: a test is tagged with the ID in square brackets, here in its docstring.

```python
from src.greeter import greet

def test_greets_by_name():
    """[GRT-01] greets by name"""
    assert greet("Ada") == "Hello, Ada!"
```

Now ask what spec17 sees. Example output:

```sh
$ spec17 status
specs/greeter                1/2 tested | none: GRT-02 | later: 1
TOTAL 1/2 tested (50%) | src-only 0 | none 1 | manual 0 | later 1 (0 with tests/code)
next: --missing lists them; spec17 trace <ID> shows tests, citations and related code

$ spec17 status --missing
GRT-02 none     specs/greeter.md:4  greet the world when no name is given.
TOTAL 1/2 tested (50%) | src-only 0 | none 1 | manual 0 | later 1 (0 with tests/code)

$ spec17 trace GRT-01
GRT-01 MUST specs/greeter.md:3
  greet a person by name.
tests:
  tests/test_greeter.py:4  """[GRT-01] greets by name"""
source cites (file:line  declaration):
  src/greeter.py:2  greet  # (GRT-01) greet by name
related code (types the tests name; * = also cites the ID):
  none found
next: codegraph explore "greet"
```

`GRT-01` has a tagged test and a citation. `GRT-02` has neither, so it is listed as missing. `GRT-03` is `later` and not counted as due. spec17 only reads the files: it does not run the test, so a tag is evidence that a test claims the requirement, not that the requirement is met or the test passes. Add a `[GRT-02]` test and run `spec17 status` again to see it move.

Other styles of requirement line, such as tables, headings, OpenSpec and Spec Kit, are listed in [Specs and tests](../specs/#requirement-styles).

## 4. Open the dashboard

```sh
spec17
```

This starts the server on `127.0.0.1:7411` and opens the dashboard in your browser; stop it with Ctrl-C. To pick another port or skip the browser: `spec17 serve --port 7500 --no-open`.

![Changes view: requirements a working-tree edit may affect, with spec edited and spec added badges](../img/changes.png)

## 5. Connect an agent (optional)

```sh
spec17 install    # repo skill + HTTP entry in .mcp.json
spec17            # keep the server running for the agent
```

The agent then reads the same evidence through MCP. Details, a stdio setup that needs no running server, and the tool list are in [Agents](../agents/).

## Next

- [Usage](../usage/): every command, the dashboard, CodeGraph.
- [Specs and tests](../specs/): requirement styles, states, configuration.
- [Agents](../agents/): skill, MCP clients, tools.
