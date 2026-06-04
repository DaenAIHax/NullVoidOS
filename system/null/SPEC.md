# `.null` — Specification v2

> Status: design draft, supersedes `CONTRACTS.md §2`. Authored 2026-05-28.
>
> Scope: Layer 3 system description language for NullVoidOS. Eval-only.
> Not a general-purpose programming language. The implementation of
> programs, services, and wrappers happens in Zero (layers 1-2); `.null`
> describes the system that *runs* those programs.

## 0 — Why a separate language at all

The reflex when reading DESIGN.md's locked decision *"Layer 3 DSL is
ZeroLang itself"* is to reuse Zero. That decision was wrong on
realization: **Zero is a systems programming language** — it has
`mut`, `set`, `World`, generics, ownership, ABI, native codegen. It is
designed to *build software*. Writing a system declaration in Zero is
the same category error as writing a NixOS configuration in Rust:
possible in principle, deeply unergonomic in practice, and inverts
the relationship between description and implementation.

`.null` exists because a **declaration** is categorically different from
a **program**:

- A program runs and produces effects. A declaration is a value.
- A program has control flow. A declaration is a literal.
- A program needs ownership and lifetimes. A declaration is data.

The two roles deserve two languages. **But both should be agent-native.**

The goal of this spec is to take the same five tricks that make Zero
usable by an agent that has never seen Zero before, and apply them to
the declarative-config domain — producing a *Nix-shaped* language with
*Zero-shaped* tooling.

## 1 — The five tricks, transposed

Zero's agent-friendly recipe and how `.null` mirrors each piece:

| Zero (systems language) | `.null` (config language) |
|---|---|
| Compiler emits structured JSON, never prose-only | Evaluator emits structured JSON, never prose-only |
| Stable error codes (`NAM003`) + typed repair IDs | Stable error codes (`TYP001`) + typed repair IDs |
| `zero explain CODE` reads docs embedded in the binary | `null explain CODE` reads docs embedded in the binary |
| `zero skills list` — version-matched skill bundles | `null skills list` — version-matched skill bundles |
| `World` capability passed to `main`, effects in signatures | Capabilities are *values in the syntax*, type-checked against system whitelist |
| One way to express each concept | One way to express each concept |
| Small surface, no historical baggage | Small surface, no historical baggage |
| Token-efficient (small binary, fast startup) | Token-efficient (small binary, fast eval) |

## 2 — Anti-features (deliberately out)

The agent works better when the language *cannot* surprise it. Out
forever:

- Functions (`{ x }: x + 1`). Reuse comes from references and merges, not lambdas.
- `let in` / local bindings. Top-level only.
- `if then else`. Use enum cases or explicit field presence.
- String interpolation. Compose strings outside the language if needed.
- Recursion / fixpoints. No `rec { }`.
- Lazy evaluation. Strict, top-down.
- Multiple syntaxes for the same concept (no `; ` *and* `,` separators, no `=` *and* `:`, no `[]` *and* `()` for lists).

Out for v2.0, deferred to v2.1+:

- `import ./other.null`. Single-file only in v2.0.
- Composition / module system. Phase 2 question.
- Schema-defined variants beyond the built-in `SystemManifest`.

## 3 — Surface syntax

### 3.1 Lexical

```
# line comment, until end of line
identifier  = [a-z][a-z0-9-]*       (kebab-case, max 64 chars)
string      = "..."                 (no interpolation, \n \t \" \\ escapes)
int         = -?[0-9]+              (no underscores, no hex/oct/bin literals)
bool        = true | false
null        = null
symbol      = .identifier           (enum values, see §5.3)
capability  = !identifier(.identifier)*(."string")?   (see §5.5)
```

There is exactly one whitespace policy: significant only as a separator;
no indentation rules; newlines are no more meaningful than spaces.

### 3.2 Composite values

**Attribute set** — the only record-like construct:

```null
{
  hostname = "nullvoid";
  packages = [ "bash-5.3.9" "neovim-mini-0.1.0" ];
}
```

- Curly braces, `key = value`, semicolon terminator after each entry.
- No trailing-comma optionality; the terminator is always `;`.
- Order is irrelevant for evaluation, preserved for formatting.
- Duplicate keys are a parse error (`PAR021`).

**List** — homogeneous, whitespace-separated:

```null
packages = [ "bash-5.3.9" "neovim-mini-0.1.0" ];
caps = [ !net !fs.read."/etc" ];
```

- Square brackets, whitespace separators (no commas).
- All elements must have the same type — heterogeneous lists are a type error (`TYP010`).
- Empty list is `[ ]`.

**Field access** — dotted, left-to-right:

```null
agent-bin = pkgs.claude-code;
```

- LHS must be an attribute set; chained `.` is left-associative.
- Missing field is a type error (`REF002`).

### 3.3 References

Exactly one ambient identifier exists in v2.0: `pkgs`. It is the
projection of `nv-pkg list --json`, see §5.4. No user-defined
identifiers — the language has no `let` or function parameters.

## 4 — The schema

`.null` is **schema-driven, not inferred**. The evaluator is parameterised
by an expected type at the top level; for system descriptions, that
type is `SystemManifest`:

```
type SystemManifest = {
  hostname: String,
  caps: [Capability],
  packages: [PackageRef],
  services: { String: Service },
  environment: { String: String },
}

type Service = {
  exec: String,
  restart: Restart,
  requires: [Capability],
}

enum Restart = .always | .on-failure | .never

type PackageRef = String       # form: "<name>-<version>"
type Capability = (see §5.5)
```

The schema is the contract. The evaluator does not infer record types
from literal shape; it checks each literal against the *expected* type
at that position. Mismatches produce typed errors with repair IDs.

This is the most important departure from Nix: no module system, no
runtime merges, no option declarations — the schema is fixed at the
compiler level, versioned with the language.

## 5 — Type system

### 5.1 Primitive types

`String`, `Int`, `Bool`, `Null`. No `Float` — the system description
has no numerical computation. No coercion — `42` is not a `String`.

### 5.2 Composite types

- `List<T>` — homogeneous.
- `AttrSet` — exists only as a *schema position*, never inferred.
  Either the schema names a record type (`Service`) or it names a
  string-keyed map (`{ String: String }`).

### 5.3 Enums (closed symbol sets)

```null
restart = .always;
```

Enums are written as `.identifier`. The set of valid symbols comes from
the schema position; using an undeclared symbol is a type error with
the full valid set listed in `expected`.

### 5.4 The `pkgs` ambient

Built at evaluator start by calling `nv-pkg list --json`. Projects to:

```
pkgs: { String: PackageRef }
pkgs.<name> = "<name>-<version>"     # latest installed version
```

Multiple versions of the same package are addressable only by literal
`"<name>-<version>"` string in the `packages` list.

### 5.5 Capabilities

**The most important transposition from Zero.** In Zero, capabilities
live in function types (`world: World`, `raises Net`). In `.null`,
capabilities are *values* with their own syntax:

```null
caps = [
  !net
  !fs.read."/etc"
  !fs.write."/var/notes"
  !tty
];
```

The language defines a closed set of capability roots:

```
!net                       any outbound socket
!net.localhost             127.0.0.1 only
!fs.read."<path>"          read subtree
!fs.write."<path>"         write subtree
!tty                       controlling terminal
!proc.spawn                spawn children
!proc.exec                 exec other binaries
!time                      read system time
!rand                      /dev/urandom
!activate.system           switch generations (privileged)
```

A `Service` declares the capabilities it *requires*. The
`SystemManifest` declares the capabilities the *system grants*.
Type-checking is straightforward subset: if a service requires a
capability the system has not granted, the file is rejected with
`CAP004`, repair ID `add-system-cap`.

This makes the agent's reasoning about effects local: every capability
a service can exercise is visible in the file, no implicit grants, no
escape hatches.

### 5.6 No subtyping, no inference

Each value's type is fixed by its position in the schema. There is no
type variable, no parametric polymorphism in user code, no inference
chain. The evaluator either accepts or rejects each value; rejection
includes the precise expected type.

## 6 — CLI surface

```
null check <file.null>          typecheck against schema, exit 0 if ok
null eval <file.null>           typecheck + emit SystemManifest JSON
null fmt <file.null>            canonical format, in-place
null parse --json <file.null>   AST as JSON

null explain <code>             docs for an error code, from embedded skill
null skills list                list skill bundles shipped in this binary
null skills get <name>          dump a single skill (markdown)
null doctor [--json]            environment check (pkgs ambient resolvable,
                                schema loaded, etc.)

null fix --plan --json <file.null>
                                machine-readable repair plan covering all
                                diagnostics — one transformation per error
```

All commands write structured output to stdout when `--json` is set,
diagnostics always to stderr. Exit codes:

- `0` — success
- `1` — diagnostic emitted (parse / type / capability / schema error)
- `2` — usage error
- `3` — environment error (`pkgs` unresolvable, schema bundle missing)

## 7 — Diagnostics

### 7.1 Format

One JSON object per diagnostic, one per line on stderr (NDJSON):

```json
{
  "code": "CAP004",
  "level": "error",
  "message": "service 'agent' requires capability 'net' not granted by system",
  "expected": "system.caps contains !net",
  "actual": "system.caps = []",
  "file": "system.null",
  "span": {"line": 12, "col": 5, "end_line": 12, "end_col": 9},
  "repair": {
    "id": "add-system-cap",
    "args": {"cap": "net"}
  }
}
```

### 7.2 Error code namespaces

| Prefix | Domain | Examples |
|---|---|---|
| `PAR` | Lexical / parse | unexpected token, duplicate key, malformed string |
| `TYP` | Type mismatch | expected String got Int, heterogeneous list |
| `SCH` | Schema | unknown top-level field, missing required field |
| `REF` | Reference resolution | unknown `pkgs.X`, undefined identifier |
| `CAP` | Capability | service requires cap not granted, unknown cap name |

Codes are stable across patch versions. Adding a new code is a minor
version bump; renumbering existing codes is a major version bump.

### 7.3 Repair IDs

A repair is a **typed AST transformation**, not a textual hint. Each
repair has:

- A stable `id` (kebab-case verb-phrase: `add-system-cap`, `wrap-int-as-string`, `quote-bare-identifier`).
- A typed `args` object specific to that repair.
- A documented effect (in `null skills get diagnostics`).

The repair set is closed and versioned. Adding a new repair is a minor
version bump. The agent applies a repair by name + args, not by string
manipulation.

Initial repair set (v2.0):

```
wrap-int-as-string         {value: int}
unwrap-string-as-int       {value: string}
add-system-cap             {cap: capability-name}
remove-unused-cap          {cap: capability-name}
quote-bare-identifier      {ident: string}
add-required-field         {field: string, type: typename}
remove-unknown-field       {field: string}
fix-enum-symbol            {got: string, valid: [string]}
homogenize-list            {target-type: typename}
```

## 8 — Skills bundle

Version-matched docs embedded in the binary, accessed via
`null skills`. Each skill is a Markdown document keyed by name:

| Skill | Content |
|---|---|
| `language` | This document, minus implementation details — pure language surface. |
| `diagnostics` | Full table of error codes, when each fires, and which repairs are typically applied. |
| `caps` | Capability vocabulary, semantics, and the system-vs-service grant model. |
| `schema` | The `SystemManifest` schema in pseudo-code, with field-by-field commentary. |
| `cli` | This binary's command surface and JSON output contracts. |
| `null` | Top-level "how to use me" — equivalent of `zero skills get zero`. |

The agent can recover the full language model from the binary alone,
with no network access:

```sh
null skills list
null skills get language
null skills get caps
```

This is the property that makes a brand-new language usable by an
agent that was never trained on it.

## 9 — Reference example

A complete `system.null` for the Phase 0 (a) lab VM, expressed in v2:

```null
# Phase 0 (a) lab VM — agent-driven, single-user, dev tooling on.
{
  hostname = "nullvoid";

  caps = [
    !net
    !fs.read."/etc"
    !fs.write."/var"
    !tty
    !proc.spawn
    !proc.exec
    !time
    !rand
  ];

  packages = [
    pkgs.claude-code
    pkgs.bash
    pkgs.neovim
    pkgs.git
    pkgs.rustc
  ];

  services = {
    agent = {
      exec = "/run/current/bin/claude";
      restart = .always;
      requires = [ !net !tty !proc.spawn !proc.exec !fs.read."/etc" ];
    };
  };

  environment = {
    EDITOR = "nvim";
    PATH = "/run/current/bin:/bin";
    TERM = "xterm-256color";
  };
}
```

A type error in this file (say `restart = "always"` as a string instead
of a symbol) emits:

```json
{
  "code": "TYP004",
  "level": "error",
  "message": "expected Restart enum, got String",
  "expected": "one of .always, .on-failure, .never",
  "actual": "\"always\"",
  "file": "system.null",
  "span": {"line": 22, "col": 17, "end_line": 22, "end_col": 25},
  "repair": {
    "id": "fix-enum-symbol",
    "args": {"got": "always", "valid": [".always", ".on-failure", ".never"]}
  }
}
```

The agent can apply `fix-enum-symbol` deterministically without parsing
prose, replacing the string with the corresponding symbol.

## 10 — What the implementation owes

The current `system/null/` crate implements CONTRACTS.md §2
(v1). Migrating to v2 requires:

1. Lexer: add `.identifier` (symbols) and `!identifier` (capabilities) tokens.
2. Parser: enforce single-syntax rules (no `;` vs `,` alternatives, no `=` vs `:`).
3. Type-checker: load `SystemManifest` schema from a const table, drop any inference logic, attach repair IDs to each error path.
4. Evaluator: capability whitelist check (service `requires` ⊆ system `caps`).
5. CLI: add `null explain`, `null skills`, `null doctor`, `null fix --plan --json`.
6. Skills bundle: embed the markdown files at compile time (`include_str!`), serve them via `null skills`.
7. Diagnostics: switch to NDJSON on stderr, repair IDs in every error path.

Step 7 is the load-bearing one for the agent-native thesis. Without
typed repair IDs, the rest of the recipe is theater.

## 11 — What this language is *not*

- Not Turing-complete. (Intentional. Declarations should not need it.)
- Not a programming language. (Use Zero for that.)
- Not extensible by users. (No new types, no new capabilities. The
  language version controls the schema; bumping the language bumps the
  manifest.)
- Not concerned with how the manifest is *applied*. That is `nv-rebuild`'s
  job per CONTRACTS.md §3.
- Not concerned with how packages are *built*. That is the agent's job
  using Zero + the substrate.
- Not a stable promise yet. Versioned 0.x; breaking changes allowed
  until 1.0 ships.

## 12 — Open questions for v2.1

These are deferred to keep v2.0 small. Each will be resolved with a
prototype before committing to syntax:

1. **Imports.** Multi-file composition. Likely `import "./other.null"`
   yielding an `AttrSet` that can be merged into a parent expression.
2. **Variants beyond `SystemManifest`.** Should `.null` describe
   user-level things too (a packaged app's manifest, an `.nvpkg`
   recipe)? Schema-pluggable evaluator or single-purpose tool?
3. **Generation diff.** `null diff old.null new.null` emitting the
   structural change that `nv-rebuild` will apply. Useful for the
   agent to preview before `switch`.
4. **Activation capability primitives.** Does `.null` express *which*
   capabilities the activation engine uses on the host, or is that
   fixed in `nv-rebuild`?
