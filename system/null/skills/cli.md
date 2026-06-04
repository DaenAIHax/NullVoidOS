---
name: cli
description: Command surface and JSON output contracts for the null binary.
---

# `null` CLI

```
null check    <file.null>          typecheck against schema, exit 0 if ok
null eval     <file.null>          typecheck + emit SystemManifest JSON to stdout
null fmt      <file.null>          canonical format, in-place
null parse    --json <file.null>   AST as JSON

null explain  <code>               docs for a diagnostic code (e.g. CAP004)
null explain  list                 list all known diagnostic codes

null skills   list                 list embedded skill bundles
null skills   get <name>           dump a skill bundle (markdown)
```

## Exit codes

| Code | Meaning                                                          |
|------|------------------------------------------------------------------|
| 0    | success                                                          |
| 1    | one or more diagnostics emitted (parse/type/schema/cap/ref error)|
| 2    | usage error (bad flags, unknown subcommand, unknown code)        |
| 3    | environment error (nv-pkg unresolvable when needed, etc.)        |

## Diagnostic output

All diagnostics go to **stderr** as NDJSON — one JSON object per line.
Stdout is reserved for the actual command output (JSON manifest, AST,
formatted source, skill markdown, etc.).

The shape of each diagnostic line:

```json
{
  "level":    "error",
  "code":     "<NAMESPACE><N>",
  "message":  "<human-readable summary>",
  "expected": "<what would have been valid>",
  "actual":   "<what was found>",
  "file":     "<path>",
  "span":     { "line": N, "col": N, "end_line": N, "end_col": N },
  "repair":   { "id": "<kebab-case-id>", "args": { ... } }   // optional
}
```

The `repair` field, when present, is the most important: agents apply it
by `id + args` deterministically, without parsing prose.

## Common pipelines

```sh
# Validate before commit:
null check system.null && echo OK

# Get the evaluated manifest for nv-rebuild:
null eval system.null > /var/lib/nv-system/staging/manifest.json

# Format in place:
null fmt system.null

# Programmatic error handling:
null check system.null 2> /tmp/diags.ndjson
while read line; do
  code=$(jq -r .code <<<"$line")
  null explain "$code"
done < /tmp/diags.ndjson
```
