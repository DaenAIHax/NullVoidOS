---
name: diagnostics
description: Error code table, when each fires, and the repair IDs they emit.
---

# Diagnostics

All diagnostics share the shape in `null skills get cli`. This skill
focuses on the **error code table** and **repair IDs** — the load-bearing
piece for automated agent repair (SPEC §10).

## Code namespaces

| Prefix | Domain                                    |
|--------|-------------------------------------------|
| `PAR`  | Lexical / parse                           |
| `TYP`  | Type mismatch                             |
| `SCH`  | Schema (missing fields)                   |
| `REF`  | Reference resolution (`pkgs.X`)           |
| `CAP`  | Capability (vocabulary, system whitelist) |

Codes are stable across patch versions. Adding a new code is a minor
version bump.

## Codes in v2.0

| Code     | When                                                           | Typical repair                |
|----------|----------------------------------------------------------------|-------------------------------|
| PAR001   | Generic parse error / anti-feature used                        | varies                        |
| TYP001   | Value type does not match schema position                      | wrap-int-as-string, homogenize-list |
| TYP004   | `restart` is not a valid Restart symbol                        | fix-enum-symbol               |
| SCH001   | Schema-required field missing                                  | add-required-field            |
| REF002   | Unknown identifier or `pkgs.<name>` resolution failure         | quote-bare-identifier         |
| CAP001   | Capability literal not in the SPEC §5.5 vocabulary             | (manual: pick valid cap)      |
| CAP004   | `services.<n>.requires` contains a cap not in `system.caps`    | add-system-cap, remove-unused-cap |

Use `null explain <CODE>` for the full doc on any one code.

## Repair IDs (v2.0)

Each repair is a typed AST transformation. The `args` shape is per
repair; mismatching args are a tooling bug to file against the agent.

| Repair ID                  | Args                                  | Effect                                                  |
|----------------------------|---------------------------------------|---------------------------------------------------------|
| `wrap-int-as-string`       | `{value: int}`                        | Quote the int literal                                   |
| `unwrap-string-as-int`     | `{value: string}`                     | Parse the string as int (verifies first)                |
| `add-system-cap`           | `{cap, path, arg}`                    | Append `cap` to `system.caps`                           |
| `remove-unused-cap`        | `{cap}`                               | Drop `cap` from the service's `requires`                |
| `quote-bare-identifier`    | `{ident}`                             | Replace `<ident>` with `"<ident>"`                      |
| `add-required-field`       | `{field, type}`                       | Insert a stub `<field> = <empty-of-type>;`              |
| `remove-unknown-field`     | `{field}`                             | Delete the duplicate / unknown field entry              |
| `fix-enum-symbol`          | `{got, valid: [string]}`              | Replace value with the symbol from `valid` named `got`  |
| `homogenize-list`          | `{target-type}`                       | Convert non-conforming list elements to `target-type`   |

The set is closed and versioned. New repairs are a minor version bump.

## Discipline for agents

Two strict rules when applying repairs:

1. **Apply by ID, not by prose.** Never derive the fix from `message`
   or `expected/actual` strings — those rot. Always use `repair.id`
   and `repair.args`.

2. **One repair at a time, then re-check.** Capability fixes can
   cascade (adding `!net` to `system.caps` may unblock other services
   that were also failing). Re-run `null check` after each repair.
