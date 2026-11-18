# Acceptance criteria (kotlinconf-sdd style)

### HDG-1: Input directory from a property
**Covers:** B-1

When the application starts with `importer.input-dir` set, the importer shall use that path.

### HDG-2: Missing configuration fails startup
**Covers:** B-2

If `importer.input-dir` is unset, then the importer shall fail to start.

### RUL-1
**Covers:** project-wide
**MUST NOT** add a batch framework.
**Reason:** not needed.

## Cross-reference

| AC | Rules |
|---|---|
| HDG-1 | RUL-1 |
| HDG-2 | RUL-1 |
