---
name: schema
description: The SystemManifest schema that every system.null must satisfy.
---

# SystemManifest Schema

The top-level value of a `system.null` file **must** evaluate to a
`SystemManifest`. The shape is fixed at the compiler level — there is no
module system, no schema customization, no runtime merges.

```
type SystemManifest = {
  hostname:    String,
  caps:        [Capability],
  packages:    [String],              # form: "<name>-<version>"
  services:    { String: Service },
  environment: { String: String },
}

type Service = {
  exec:        String,
  restart:     Restart,
  requires:    [Capability],
}

enum Restart = .always | .on-failure | .never

type Capability = (see `null skills get caps`)
```

## Required vs optional

All listed fields are required (SCH001 if absent). Empty values are fine:
`packages = [];` is valid, `packages = [ ];` is also valid (same thing).

## Capabilities subset rule

For every `services.<name>.requires` entry, that capability must also
appear in `caps` at the top level. Violations produce CAP004 with repair
`add-system-cap`.

## Common errors against the schema

- `hostname = 42;`              → TYP001 (wrap-int-as-string)
- `restart = "always";`         → TYP004 (fix-enum-symbol: convert to `.always`)
- `packages = pkgs.bash;`       → TYP001 (expected [String], got String)
- `services = [];`              → TYP001 (expected AttrSet, got List)
- `environment = "PATH=...";`   → TYP001 (expected `{ String: String }`)
- missing `caps =`              → SCH001 (add-required-field)

## What is not in the schema (v2.0)

- Mounts, users, kernel cmdline, network interfaces, firewall rules —
  these are Phase 2 work, see SPEC §12.
- Service supervision policy beyond `restart` — Phase 2.
- Generation-scoped overrides — out of scope.
