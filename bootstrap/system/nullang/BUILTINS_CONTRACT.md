# Builtins contract — the in-VM agent's self-improvement boundary

This is the boundary for the **intermediate self-improvement loop**: the agent
running inside NullVoidOS may extend Nullang **by adding builtins only**, and
rebuild/swap `/bin/nullang` itself. Parser, type system, and code-generation
*structure* stay with the external language author (the host) — a bug there
breaks compilation of every program; a bug in a builtin breaks only that
builtin.

## You MAY edit

1. **`src/check.rs`, function `builtins()`** — add a `Sig` entry:
   ```rust
   t.insert("NAME".to_string(), Sig {
       params: vec![/* Ty::String, Ty::Int, ... ; Ty::World as first param if effectful */],
       ret: Ty::String,            // or Int / Bool / Unit
       effects: vec![],            // pure; OR vec!["fs.read".into()] etc. if it uses World
       c_name: "nullang_NAME".to_string(),
   });
   ```
2. **`src/codegen.rs`, the `PRELUDE` string** — add the matching C function
   `static <ctype> nullang_NAME(<args>) { ... }`. `World` params are erased:
   they are NOT C parameters (see `nullang_read_file`, which takes only `path`).

That is the whole surface. A builtin is either **pure** (no `World`,
`effects: vec![]`) or **effectful** (`World` as the first param, and one or more
capability keys in `effects` — the fn's `uses` clause must then cover them).

## You MUST NOT touch

- `src/lexer.rs`, `src/parser.rs` — surface syntax.
- `src/ast.rs` — the `Ty` enum, `BinOp`, expression shapes.
- `src/check.rs` *outside* `builtins()` — typing rules, `check_binary`,
  effect discipline, enum/match logic.
- `src/codegen.rs` *outside* `PRELUDE` — lowering, `emit_main`, `lower`.

Anything that needs new **syntax** (`mut`, `while`, `List` literals, `struct`,
new operators) or new **types** is language-core surgery → request it from the
host author. These are the §11 items (`List<T>`, `let mut`, `while`, `struct`).

## The forge ritual (build → swap → probe → rollback)

Never swap a compiler you have not smoke-tested. Suggested loop:

```sh
# build into /var so target/ never pollutes the (9P-mounted) source tree
export CARGO_TARGET_DIR=/var/cargo-target
cargo build --release            # in the nullang source dir
NEW=target/.../release/nullang   # the freshly built binary

cp /bin/nullang /var/nullang.prev          # backup the current good one
cp "$NEW" /bin/nullang                      # swap in the new one
if nullang run /var/src/smoke-probe.null ; then
  echo "probe passed — keep"
else
  cp /var/nullang.prev /bin/nullang         # ROLLBACK
  echo "probe FAILED — rolled back"
fi
```

The smoke probe must exercise every shipped feature (Wave 0 + Wave 1 + argv +
whatever you add). If the new compiler can't still build+run it, the swap is
reverted automatically — you never end up with a broken `/bin/nullang`.

This is safe because `nullang` (construction mode) is **not** in `nv-rebuild`'s
critical path — `system.null` is evaluated by `null` (declaration mode), a
separate binary — and `cargo` rebuilds `nullang` without needing `nullang`. So
a broken build is always recoverable.

## Flow back to the canonical repo

The source is the host repo's `bootstrap/system/nullang/` (9P-mounted). Edits
you make to `check.rs`/`codegen.rs` land in the host working tree, where the
host author reviews the diff, recomputes `cargoHash` if (and only if) `Cargo.lock`
changed — builtins add no dependencies, so it will not — and rebuilds the
shipped initramfs `nullang`. Keep edits inside the two allowed regions so the
review is a glance, not an audit.
