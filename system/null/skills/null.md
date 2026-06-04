---
name: null
description: Top-level "how to use me" for the .null configuration language.
---

# `.null` — the NullVoidOS system description language

`.null` is a small declarative configuration language. It describes the
state of a NullVoidOS system: which packages are installed, which
services run, which capabilities the system grants. It is **eval-only**:
no functions, no IO, no runtime — a `.null` file evaluates to a typed
`SystemManifest` JSON value, consumed by `nv-rebuild`.

## When to use which skill

Load these in order when authoring a `system.null` from scratch:

  1. `null skills get language` — surface syntax (literals, attrsets, lists, symbols, capabilities)
  2. `null skills get schema`   — the SystemManifest type the file must evaluate to
  3. `null skills get caps`     — capability vocabulary and the system-grants/service-requires model
  4. `null skills get cli`      — how to invoke `null check`, `null eval`, etc.
  5. `null skills get diagnostics` — error codes and repair IDs you may encounter

When fixing a specific error, use `null explain <CODE>` instead — it gives
focused docs for one code plus the repair IDs typically applied.

## Three things to know first

1. **The schema is fixed**, not declared by the user. The top-level value
   must be a `SystemManifest`; every field's type comes from the schema,
   not inferred from the literal you write.

2. **Capabilities live in the syntax**, not in a separate manifest. The
   system declares what it grants (`caps`), services declare what they
   require (`requires`), and the compiler enforces subset.

3. **There is one way to express each concept.** No `,` vs `;`. No `=`
   vs `:`. No `[]` vs `()`. Pick the obvious form; if you're choosing
   between two ways, look again at this skill — the second way doesn't
   exist.

## Authoritative spec

`system/null/SPEC.md` is the source of truth for everything
in this and the other skills. If a skill and SPEC disagree, SPEC wins.
