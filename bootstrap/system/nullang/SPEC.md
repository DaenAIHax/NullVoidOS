# Nullang — Specification v0.1

> Status: design draft. Authored 2026-05-28. Pre-implementation.
>
> Scope: the **native construction language** of NullVoidOS. Nullang is
> *one language with two modes* — **construction** (full: functions,
> effects, compiles to native code) and **declaration** (restricted: the
> `.null` profile, eval-only). This document specs the construction core
> and the agent-native foundation shared by both modes. The declaration
> profile is `null/SPEC.md` (`.null` v2), re-framed here as a subset (§3).

## 0 — Why Nullang exists

`DESIGN.md` locked *"Layer 3 DSL is not Zero"* on 2026-05-28. That decision
was right **given Zero**. Nullang changes the premise: NullVoidOS cannot
depend on an external language (Zero / Vercel Labs) whose death forces a
rewrite of Layers 1–4. The language must be ours and must grow with the OS.

Two goals, kept deliberately separate — conflating them is how ambitious
projects fail to ship:

- **Sovereignty.** NullVoidOS owns the spec and the compiler. No external
  project can kill the OS. *Needed now.*
- **Self-sufficiency.** The ecosystem (stdlib, TLS, crypto, …) is
  reimplemented in Nullang, one library at a time, **only when the OS
  actually needs each piece**. *A destination reached over years, never
  big-bang.*

Sovereignty does **not** require self-sufficiency. Nullang delivers the
first immediately. The second is the long arc: when the OS needs a library,
an agent reads the logical flow of an existing implementation and rebuilds
it in Nullang — freely for low-risk code (parsers, data structures,
HTTP/1.1, file formats), and **last, with a verification strategy**, for
high-risk code (TLS, crypto, anything timing/side-channel sensitive). Until
then, high-risk capabilities stay wrapped at the substrate (Layer 1).

**One language, not two.** The earlier Zero/`.null` split is preserved as
two *modes of one grammar*, not two grammars (§3). A declaration is simply
Nullang with functions, bindings, control flow, and effects forbidden.

## 1 — The five tricks (inherited)

Nullang keeps the recipe that makes a never-seen language usable by an agent:

| Trick | Nullang realization |
|---|---|
| Compiler emits structured JSON, never prose-only | NDJSON diagnostics on stderr, both modes (§9) |
| Stable error codes + typed repair IDs | `DiagCode` namespaces + closed repair set (§9) |
| `<tool> explain CODE` reads embedded docs | `null explain CODE` from embedded skill bundle |
| Version-matched skill bundles | `null skills list` / `get` (§8) |
| Effects in signatures / capabilities visible | `uses` clause + `World`; same `!cap` vocabulary as `.null` §5.5 (§5) |
| One way to express each concept | §10 anti-features hold even in construction mode |
| Small surface, no historical baggage | v0.1 omits generics, traits, Float, borrow checker (§11) |
| Token-efficient | emit C, no heavy backend; small binary, fast startup (§7) |

## 2 — Non-negotiable foundation: agent-native tooling

This is the proven part of the project and it is **load-bearing** — per
`null/SPEC.md` §10, *"without typed repair IDs the rest of the recipe is
theater."* Nullang inherits, unchanged:

1. **NDJSON diagnostics** on stderr — one JSON object per diagnostic.
2. **Stable error codes** in fixed namespaces (§9.2).
3. **Typed repair IDs** — a repair is an AST transformation applied by
   `id + args`, never string manipulation. The set is closed and versioned.
4. **Embedded skills** — the full language model is recoverable from the
   binary alone, no network (`null skills get language`).

A feature that cannot emit a structured diagnostic with a repair path does
not ship. This section overrides convenience everywhere it conflicts.

## 3 — Two modes

Nullang has one grammar and one type system. A *mode* is selected by the
command, and the checker enforces the mode's restrictions:

| | **Declaration mode** (`null eval`, `null check`) | **Construction mode** (`null build`, `null run`) |
|---|---|---|
| Output | a value (a `SystemManifest` JSON) | a native ELF on the substrate |
| Allowed | attrsets, lists, primitives, enums (`.sym`), capability values (`!cap`), field access | everything in declaration + `fn`, `let`, `if`/`match`, function calls |
| Forbidden | `fn`, `let`, control flow, effects | nothing (full grammar) |
| Termination | strict, top-down eval against a schema | compiled, then executed |

Declaration mode **is** `.null` v2 (`null/SPEC.md`). Its anti-features
(no functions, no `let in`, no `if then else`, no recursion, no lazy eval,
one-way-to-express) are not a separate language — they are the construction
grammar with the effect/control-flow productions disabled. Using a
forbidden production in declaration mode is a typed error (`MOD001`,
repair `extract-to-construction`).

The two modes meet at the **capability vocabulary** (§5): declaration mode
*grants* capabilities to the system; construction mode *consumes* them via
`World`. This is the seam that makes one language coherent across Layers 1–4.

## 4 — Surface syntax (construction core, minimal)

### 4.1 Lexical

Extends `.null` lexical (`null/SPEC.md` §3.1) with construction keywords.
One whitespace policy: significant only as a separator, no indentation rules.

```
keyword     = fn | let | type | enum | if | else | match | use | return
identifier  = [a-z][a-z0-9_]*        (snake_case for values/functions)
typename    = [A-Z][A-Za-z0-9]*      (PascalCase for types)
string      = "..."                  (\n \t \" \\ escapes; no interpolation)
int         = -?[0-9]+               (i64; no underscores, no hex/oct/bin)
bool        = true | false
symbol      = .identifier            (enum values, as in .null §5.3)
capability  = !identifier(.identifier)*(."string")?   (.null §5.5)
```

Separators have **distinct, non-overlapping roles** (this does not violate
the "one way per concept" rule — `;` and `,` are never interchangeable):

- `;` terminates a statement / a binding / an attrset entry.
- `,` separates function parameters, call arguments, and `match` arms.

### 4.2 Types

```nullang
# Primitives (no Float in v0.1 — see §11):
Int      # i64
Bool
String   # UTF-8, owned
Bytes    # raw byte buffer
Unit     # the empty result, written ()

# Struct — nominal record (v0.4). Reference semantics; fields are scalar
# (Int/Bool/String) or another struct (enum/List fields deferred, §11):
type Point = { x: Int, y: Int };
type Line  = { from: Point, to: Point, label: String };

# Enum — closed symbol set. A variant may carry a single typed payload
# (v0.2); payload types are Int/Bool/String (enum/World/Unit deferred, §11):
enum Restart = .always | .on_failure | .never;
enum Status  = .code(Int) | .message(String) | .none;

# List — built-in growable container (v0.3). Element type is scalar
# (Int/Bool/String) or a struct (v0.4); nested lists / lists of enums are
# deferred (§11).
List<Int>
List<String>
List<Point>
```

A `List<T>` has **reference semantics**: a value is a handle to a heap buffer,
so `push`/`set` mutate it in place (and therefore require a `let mut` binding,
§4.4). The element type `T` is a scalar (`Int`/`Bool`/`String`) or a **struct**
(v0.4): a struct is itself a heap handle (a pointer), so it fits the same uniform
64-bit slot a `String` pointer does — `List<Point>` is nearly free, and a field
written through an element (`xs[i]` then `e.field = v`) mutates the record in the
list. Nested lists (`List<List<T>>`) and lists of enums stay deferred (§11).
Construct one with a literal `[a, b, c]`; an empty `[]` takes its element
type from an annotation (`let mut xs: List<String> = []`). Read an element with
`xs[i]`; write with `set` (§4.7) — there is no `xs[i] = v` lvalue (one way per
concept, §10). Indices are total: a read past the end returns the element
default, a write is a no-op (like `substr`, §4.7).

A **struct** (v0.4) is a nominal record declared at top level with `type Name =
{ field: Type, ... };`. Like `List`, it has **reference semantics** — a value is
a heap handle (a pointer), so a struct also fits the uniform list slot, and a
binding copy aliases the same record. Construct one with **named fields**,
`Point { x: 1, y: 2 }` (all fields required, each once, order free); read a field
with `p.x`; write one with the lvalue `p.x = v` (chains too: `p.a.b = v`). A
field write requires the chain's **root** to be a `let mut` binding (same surface
discipline as `push`/`set`). v0.4 fields are `Int`/`Bool`/`String` or **another
struct** (by handle, including self- and mutual reference); enum-typed and
List-typed fields are deferred (§11). There is no field-update expression and no
positional construction — one way per concept (§10).

A payload variant is **constructed** with its argument (`.code(42)`,
`.message("oops")`); a bare variant takes none (`.none`). A mismatch — a
payload supplied to a bare variant, or omitted from a payload variant — is
`TYP021`. Enums with no payloads lower to a bare integer; enums with at
least one payload lower to a tagged union (§7), so flag-style enums pay
nothing for the feature.

No type inference for declarations; each value is checked against its
expected type by position (inherited from `.null` §5.6). Local `let`
bindings in construction mode *do* infer from the initializer (§4.4).
No subtyping, no coercion: `42` is not a `String`.

### 4.3 Functions

```nullang
fn add(a: Int, b: Int) -> Int {
  a + b
}
```

- `fn name(params) -> ReturnType uses <effects> { body }`.
- The `uses` clause lists the capabilities the body may exercise (§5).
  Omitted means **pure** — the function performs no effects, and the
  checker rejects any effectful call inside it (`EFF001`).
- The body is an expression; the last expression is the return value.
  `return expr;` is the single early-exit form.
- No overloading, no default arguments, no variadics (one way per concept).

### 4.4 Bindings (construction mode only)

```nullang
let n = add(2, 3);
let label: String = "count";   # annotation optional, inferred otherwise
let mut i = 0;                 # reassignable binding
i = i + 1;                     # assignment; only `let mut` is assignable
```

- `let name = expr;` introduces an **immutable** binding in the current block.
- `let mut name = expr;` introduces a **mutable** binding; `name = expr;`
  reassigns it (same type). Assigning a non-`mut` binding is a type error;
  the assignment value's type must match the binding's.
- No `let in`; no shadowing in the same block (`MOD002`, repair `rename-binding`).
- Mutable state pairs with `while` (§4.5) to iterate without recursion — the
  way to scan a large input without exhausting the stack.

### 4.5 Control flow

Exactly one form each — no `then`, no ternary, no `switch`/`case` alias:

```nullang
if cond { a } else { b }          # both branches required; an expression

while cond { ... }                # loop while cond (Bool) holds; a statement

match color {
  .red   => 1,
  .green => 2,
  .blue  => 3,
}                                  # exhaustive; missing arm is TYP020

match status {                     # payload variants bind their payload (v0.2)
  .code(n)    => n,
  .message(m) => length(m),
  .none       => 0,
}                                  # use `_` to discard: `.message(_) => 0`
```

`if` is an expression (both branches must yield the same type). A block-like
expression (`if`/`match`) used in **statement position** — i.e. not as the
block's trailing value — needs no terminating `;`; the next statement may
follow directly, as in Rust. (`;` is still required to end a `let`, an
assignment, or a call statement.) `match`
must be exhaustive over the enum; a non-exhaustive match is a type error
with the missing symbols in `expected`. A payload variant's arm **must**
bind the payload (`.code(n)` or `.code(_)`); the bound name is in scope in
the arm body with the payload's type. A bare arm must not bind, and a
payload arm must — either error is `TYP021`.

### 4.6 The entry point

```nullang
fn main(world: World) -> Int uses !tty {
  print(world, "hello from nullang");
  0
}
```

`main` receives a `World` — the runtime token carrying the capabilities the
*system* granted this process (§5). Its `uses` set must be a subset of what
`World` was constructed with; the rest of the program threads `world` to
reach effectful stdlib functions.

### 4.7 Builtins

Available without declaration. The set is deliberately tiny and grows only
when the OS needs it (§11):

```nullang
print(world: World, s: String) -> ()              uses !tty       # writes s + newline
concat(a: String, b: String) -> String                            # pure; BINARY only
str_of_int(n: Int) -> String                                      # pure; decimal
str_len(s: String) -> Int                                         # pure (Tier 0)
substr(s: String, start: Int, len: Int) -> String                 # pure; indices clamp (Tier 0)
char_at(s: String, i: Int) -> String                              # pure; 1-char, O(i), "" out of range
char_code(s: String, i: Int) -> Int                               # pure; byte at i, -1 out of range (P0)
int_of_str(s: String) -> Int                                      # pure; decimal parse, total, 0 on junk (P0)

read_file(world: World, path: String) -> String   uses !fs.read   # "" on error (Tier 0)
write_file(world: World, path: String, content: String) -> ()   uses !fs.write   # (Tier 0)
argc() -> Int                                                     # pure; arg count incl. argv(0)
argv(i: Int) -> String                                            # pure; "" out of range

# List<T> ops (v0.3). Polymorphic in the element type T (Int/Bool/String),
# so they are compiler intrinsics, not ordinary SigTable builtins. `push`/`set`
# mutate and require a `let mut` list; all are pure (no World, no effect).
list_len(xs: List<T>) -> Int                                      # element count
push(xs: List<T>, v: T) -> ()                                     # append; xs must be `let mut`
set(xs: List<T>, i: Int, v: T) -> ()                             # in-place write; no-op out of range
# read:  xs[i]   (postfix, §4.2)   literal: [a, b, c]
```

**List intrinsics** (`list_len`/`push`/`set`) and the literal/index syntax are
the v0.3 collection surface. They are *polymorphic* — the only polymorphism in
the language — handled as a built-in special case, **not** user generics (§11).
The names `push`/`set`/`list_len` are reserved.

`print`/`read_file`/`write_file` are the effectful builtins; the rest are pure
and need no `uses`. There is **no string interpolation and no `+` overload for
strings** (§10): build dynamic strings by composing `concat`/`str_of_int`
explicitly. `concat` is strictly binary (§10), so nesting is the intended cost.

**Tier 0 (string decomposition + file I/O).** `concat` builds strings up;
`str_len`/`substr`/`char_at` take them apart. `char_at(s,i)` returns the same
1-char string as `substr(s,i,1)` but is O(i) (it stops at `i`) rather than O(n)
(`substr` does a full `strlen`), so a left-to-right scan is O(n) not O(n²) —
the first builtin **authored by the in-VM agent itself** (see
`BUILTINS_CONTRACT.md`).
`substr` clamps its indices so it is total (no panics, no error type). File I/O
is effectful and **path-less in the language**: an fn that reads files declares
`uses !fs.read` (no path) — the path is a runtime `String`, and the *system*
grant in `system.null` (`!fs.read."/dir"`) scopes it, which is what Landlock
enforces at `nv-rebuild run`. So the language effect maps 1:1 onto the runtime
capability. `read_file` returns `""` on error today; a `Result`-returning
variant (§10) is the follow-up. `str_of_bool` and friends are still deferred.

**P0 stdlib — the String↔Int seam.** Two pure, total builtins that two
independent probes (the self-host lexer and a config-parser) both needed and
hand-rolled identically. `char_code(s,i)` is the missing `char→Int`: the byte
value at index `i` (0..255), or `-1` out of range — so character classes become
arithmetic ranges (`code >= 48 && code <= 57`) instead of 10-arm `==` chains.
`int_of_str(s)` parses a decimal (optional leading `-`, digits, stops at the
first non-digit; `""`/junk → `0`) — the headline deterministic gap, confirmed
3/3, every config-parser otherwise re-implementing the same ~22-LOC parse. Both
total (no panic, no error type); a `Result`-returning `int_of_str` that
distinguishes `"0"` from a parse error is the §10 follow-up, as for `read_file`.
Still deferred in this cluster: `split`/`index_of` (P1 — both probes reached for
them), `str_of_bool` and `else if` ergonomics (P2).

**String equality.** `==` and `!=` work on `String` (lowered to `strcmp`), in
addition to `Int`/`Bool` — needed to recognise commands and keystrokes. No
ordering (`<`, `>`) on strings yet.

**Process arguments.** `argc()`/`argv(i)` give the command line (gate for
`cat <file>`/`grep`/`sed`-likes). They are **pure** — argv is startup data, not
an ongoing effect, so no `World` and no `uses` (and nothing to add to `null`'s
capability vocabulary). C convention: `argv(0)` is the program name. The
ergonomic `fn main(world, args: List<String>)` form waits on `List<T>` (§11).

## 5 — Capabilities and effects

The heart of the language, and the seam between the two modes.

- A capability is named with the **same vocabulary as `.null` §5.5**:
  `!net`, `!net.localhost`, `!fs.read."<path>"`, `!fs.write."<path>"`,
  `!tty`, `!proc.spawn`, `!proc.exec`, `!time`, `!rand`, `!activate.system`.
- A function declares the capabilities it may exercise in its `uses` clause.
  This is a *static effect annotation*, checked transitively: a function may
  only `uses` what its callees `uses` (or a superset it holds via `World`).
- Calling an effectful function without holding the required capability is
  `EFF001` (repair `add-uses-clause` on the caller, or `thread-world` if the
  caller has `World` but did not pass it).
- `World` is the root capability token. It is **constructed at process
  start from the `SystemManifest`** — i.e. from the `.null` declaration that
  granted the system its `caps`. A program can never exercise a capability
  the declaration did not grant. Effect reasoning is fully local and the
  grant is visible in the system file.

This unifies Zero's effects-in-signatures with `.null`'s capabilities-as-
values: in declaration mode capabilities are *values that grant*; in
construction mode they are *effects that consume*. One vocabulary, two roles.

### 5.1 Runtime representation of `World` (v0.1 — important)

In v0.1, **`World` has no runtime representation.** It is a compile-time
token only: the checker verifies that every effect a function exercises is
declared in its `uses` clause, and codegen **erases** every `World`
parameter and argument (you will not find `world` in the emitted C). The
effect system is therefore a purely *static* discipline in v0.1.

This is deliberate and consistent with `CONTRACTS.md §4`: capabilities are
*"declared but not enforced in Phase 1… Phase 2 will wire them to actual
sandboxing (seccomp / landlock / cgroups)."* So:

- **Who constructs `World` at process start?** In Phase 1, *no one* — it
  does not exist at runtime. The `.null` manifest's `caps` are the grant
  *on paper*; the compiler's `uses` check is the enforcement *on paper*.
- **Phase 2** will make `World` real: the activation engine (`nv-rebuild`)
  constructs the token from the granted `caps`, init passes it to `main`,
  and the substrate wrappers refuse syscalls outside the held set. Only
  then does the static `uses` annotation gain runtime teeth.

Until Phase 2, the value of the effect system is that an agent reading a
program can see exactly which capabilities it *could* exercise, locally and
without running it — which is the agent-native property that matters now.

## 6 — Memory model (v0.1)

Deliberately minimal to keep the first compiler small and the C codegen
trivial:

- Values are owned and copied at binding; no references, no aliasing in
  user-visible semantics.
- Allocation is arena/region-scoped per `main` invocation; freed on exit.
  Long-running services get a region reset hook (deferred detail).
- **No borrow checker, no manual free.** `let mut` exists (§4.4), but ownership
  and lifetimes are §11 deferrals. This trades runtime efficiency for a
  compiler an agent can fully model today; the effect system (§5), not the
  memory model, is where v0.1 invests.

## 7 — Codegen: emit C

The single most important sovereignty decision.

- Nullang lowers to **C**, which the substrate's existing C compiler
  (Layer 0) turns into an ELF. The backend adds **no new third-party
  dependency that can die** — a C compiler will not disappear. LLVM /
  Cranelift would reintroduce exactly the external-death risk Nullang
  exists to escape.
- Pipeline: `source.null → typed AST → C → cc → ELF`.
- **CAS + provenance** wrap every stage: the content hash of the source,
  of the emitted C, and of the resulting ELF are recorded, with provenance
  noting which agent built it from which inputs (the four OS primitives).
- Effects (§5) lower to calls into the Layer-1 capability-typed C wrappers.
  A program physically cannot reach a syscall the wrapper does not expose.

## 8 — CLI surface

A superset of `.null`'s CLI (`null/SPEC.md` §6):

```
# Construction mode
null build <file.null>          compile to ELF via C; provenance + CAS
null run   <file.null>          build (if stale) then execute
null emit-c <file.null>         dump the generated C (inspection)
null package <file.null> --name N --version V [--author A] [--install]
                                build, then emit `<N>-<V>.nvpkg` (manifest +
                                ELF + recipe.null). Capabilities derive from
                                main's `uses`. `--install` → `nv-pkg install`.

# Declaration mode (unchanged from .null v2)
null check <file.null>          typecheck against schema
null eval  <file.null>          emit SystemManifest JSON

# Shared
null fmt   <file.null>          canonical format, in-place
null parse --json <file.null>   typed AST as JSON
null explain <code>             docs for an error code (embedded)
null skills list | get <name>   version-matched skill bundle
null doctor [--json]            environment check (cc present, substrate
                                wrappers resolvable, schema loaded)
null fix --plan --json <file>   machine-readable repair plan, all diagnostics
```

Exit codes inherit `.null` §6: `0` ok, `1` diagnostic, `2` usage,
`3` environment error.

## 9 — Diagnostics

### 9.1 Format

Identical to `.null` §7.1 — one NDJSON object per diagnostic on stderr,
with `code`, `level`, `message`, `expected`, `actual`, `file`, `span`,
and an optional typed `repair { id, args }`.

### 9.2 Error code namespaces

Inherits `.null`'s `PAR` / `TYP` / `SCH` / `REF` / `CAP`, and adds two for
construction:

| Prefix | Domain | Examples |
|---|---|---|
| `PAR` | lexical / parse | unexpected token, bad string |
| `TYP` | type mismatch | branch types differ, non-exhaustive match (`TYP020`), enum payload arity (`TYP021`) |
| `SCH` | schema (declaration mode) | unknown / missing manifest field |
| `REF` | reference resolution | unknown `pkgs.X`, undefined binding |
| `CAP` | capability vocabulary | unknown cap name, service requires ungranted cap |
| `EFF` | **effect discipline** | effectful call in pure fn (`EFF001`), `uses` not a subset of `World` |
| `MOD` | **mode violation** | construction production used in declaration mode (`MOD001`), rebinding (`MOD002`) |
| `CGN` | **codegen** | C emission / `cc` invocation failure |

### 9.3 Repair set (v0.1 additions)

Inherits `.null` §7.3 repairs, adds:

```
add-uses-clause          {fn: string, cap: capability-name}
thread-world             {fn: string}
extract-to-construction  {span}        # move forbidden decl-mode code to a fn
rename-binding           {from: string, to: string}
add-missing-arm          {enum: typename, symbol: string}
supply-payload           {symbol: string, ty: typename}   # .code → .code(<Int>)
drop-payload             {symbol: string}                 # .none(x) → .none
bind-payload             {symbol: string, ty: typename}   # .code => → .code(n) =>
```

The set is closed and versioned; adding a repair is a minor bump.

## 10 — Anti-features (hold in both modes)

Even in full construction mode, regularity beats cleverness:

- Mutation only via explicit `let mut` (§4.4) — no implicit or hidden mutation.
- No exceptions / panics as control flow — fallible operations return an
  enum result; the caller matches it.
- No global mutable state. No implicit `main` globals.
- No implicit numeric/string coercion.
- No operator overloading, no overloading, no default args, no variadics.
- No macros, no preprocessor, no conditional compilation.
- No string interpolation (compose explicitly via `concat`/`str_of_int`,
  §4.7), no multiple list/record syntaxes (one bracket per concept).
- One control-flow form each (`if`/`match`/`while`); no `then`, ternary, or `switch`.

## 11 — What v0.1 deliberately omits (the roadmap)

Each is deferred, not rejected; each ships *only when the OS needs it*:

- ~~**`mut` / mutable state**~~ — LANDED (`let mut` + `while`, §4.4/§4.5).
- **Ownership / borrow checker** — when arena allocation stops being enough.
- **Float** — when a workload needs numeric computation (none in v0.1).
- ~~**`List<T>`**~~ — **landed in v0.3** (§4.2, §4.7): a built-in **monomorphic**
  container over scalar elements (Int/Bool/String) — **and structs since v0.4**
  (`List<Point>`, an element is the struct's 64-bit handle, so it reuses the
  same slot at no extra cost). Reference semantics (a heap handle), so
  `push`/`set` mutate in place and require a `let mut` target; literal
  `[a, b, c]`, read `xs[i]`, total bounds (out-of-range read → default,
  write → no-op). Nested lists and lists of enums are deferred.
- **Generics / parametric types** — still deferred. `List<T>` is a *built-in
  special case* (the compiler knows it), **not** user-defined generics
  (`fn f<T>`); those wait until the stdlib actually demands them.
- ~~**`struct` / nominal records**~~ — **landed in v0.4** (§4.2): reference
  semantics (heap handle), named-field construction `Point { x: 1, y: 2 }`,
  field read `p.x` and lvalue write `p.x = v` (chains too), `let mut` root
  required to write. Fields are Int/Bool/String or another struct; enum/List
  fields deferred. With List (v0.3) this is the data-modelling core needed to
  express the compiler's own tables — the precondition for self-hosting (§12).
- **Traits / interfaces** — when polymorphism over types is unavoidable.
- ~~**Enum payloads** (`.some(Int)`)~~ — **landed in v0.2** (§4.2, §4.5): a
  variant carries at most one Int/Bool/String payload; enum/Unit/World
  payloads remain deferred (would need indirection or carry nothing).
- **Module / import system** — coordinate with `.null` §12.1 (namespaced,
  *no* automatic merge).
- **Self-hosting** — the milestone that certifies sovereignty (§12).
- **Package model** — "create, don't install": agent declares an internal
  package built from the substrate, CAS-identified; users consume it.

## 12 — Bootstrap path

Three tempi, each leaving a working closed loop behind:

1. **Host compiler in Rust**, emitting C. Fastest route to a green loop;
   reuses the existing `.null` crate's lexer/diagnostics/skills discipline.
2. **Grow** the language + a minimal stdlib until Nullang can express its
   own compiler — driven strictly by what the OS needs next, never ahead.
3. **Self-host** — rewrite the compiler in Nullang, bootstrapped by the
   Rust build, then drop Rust. The only third-party floor that remains is
   **kernel + libc + C compiler**. That is the honest, minimal, won't-die
   substrate. "No third-party" is true *above* this floor, never through it.

## 13 — The v0.1 milestone: one closed loop

v0.1 is done when this loop is green — nothing more:

```
agent writes hello.null
  → null build emits C
  → substrate cc compiles it to an ELF
  → it runs and prints via the !tty wrapper
  → CAS stores the binary, provenance records (agent, source-hash, c-hash)
```

When that loop closes, every later feature in §11 is "turn the crank."
Discipline that keeps the project alive: **always have a working loop and
widen it; never build TLS before hello-world runs.**

**Status (2026-05-28):** the loop is green, and `null package` extends it to
the CAS: it emits a CONTRACTS.md §1.1 `.nvpkg` (manifest + `payload/bin/<name>`
+ `recipe.null`), with `capabilities` derived from `main`'s `uses` and
provenance (`authoredBy`, `createdAt`, source/C `sha256`) in the manifest.
`nv-pkg install` then content-addresses it — closing the CAS+provenance half
of this section. The `.null` source ships inside the package, so the artifact
carries the exact recipe that produced it.
