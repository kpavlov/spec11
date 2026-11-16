# Setup: install and doctor

- **SETUP-01** MUST — `install` writes the skill to `<repo>/.agents/skills/spec11` and merges a spec11 entry into `.mcp.json`, keeping every other server and key.
- **SETUP-02** MUST — leave a `.mcp.json` that is not valid JSON untouched.
- **SETUP-03** MUST — `uninstall` removes only what `install` added, and deletes `.mcp.json` only if install created it.
- **SETUP-04** MUST — only read the global MCP config (`~/.claude.json`), never write it. (manual)
- **SETUP-05** MUST — `doctor` only reads: unless asked with `--fix`, it never runs `codegraph init` or `codegraph sync` and never writes files.
- **SETUP-06** SHOULD — `doctor` fails only when spec11 cannot work (no specs folder); everything else is a warning with a hint.
- **SETUP-07** MUST — build or refresh the CodeGraph index only when the user asks (`doctor --fix`, the Setup tab button): `init` only when there is no index, `sync` only when there is one, and never automatically.
