# Dashboard

- **WEB-01** MUST — the URL is the view: tab, selected requirement, spec file, code path and line, filters; back, forward, reload and shared links work. (manual)
- **WEB-02** MUST — build to one `index.html`, which the binary embeds and MCP hosts get as the dashboard resource. (manual)
- **WEB-03** MUST — never claim a requirement is implemented or a test passes: evidence, not truth. (manual)
- **WEB-05** SHOULD — open on the dashboard (the whole-project status) at `/`, and offer a modules tab (a table, and a page per module with its symbols, dependencies, dependents and detected tests) and a tests tab (every detected test by module and file, with the requirement IDs tagged on it, filterable by module, tagged or untagged, and text), both labelled detected, not run. (manual)
- **WEB-04** SHOULD — link every test, citation and spec location to the file viewer or spec it names. (manual)
