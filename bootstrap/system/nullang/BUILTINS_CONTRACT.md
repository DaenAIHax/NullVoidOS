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

## The forge ritual (build → package → switch → probe → rollback)

**The compiler is generation-managed, like every other package — no reboot, no
lost context.** `nullang` is just a userspace binary, and the init puts
`/run/current/bin` *ahead* of `/bin` on PATH. So you ship a new compiler the
same way you ship any package: build it, package it as `nv-toolchain`, declare
it in `system.null`, `nv-rebuild switch`. `/run/current/bin/nullang` then
shadows the baked `/bin/nullang`, it **persists in `/var` across reboots**, and
rollback is a real generation rollback. Do NOT `cp` over `/bin/nullang` — that
is RAM-only (lost on reboot) and bypasses generations.

```sh
# build into /var so target/ never pollutes the source tree
export CARGO_TARGET_DIR=/var/cargo-target
( cd /var/src/nullang && cargo build --release )
NEW=/var/src/nullang/target/x86_64-unknown-linux-gnu/release/nullang  # or the host triple

# package the fresh compiler as nv-toolchain-<V> (bump V each iteration)
V=0.1.1
mkdir -p /var/pkg/nt/payload/bin
cp "$NEW" /var/pkg/nt/payload/bin/nullang
cat > /var/pkg/nt/manifest.json <<EOF
{ "schemaVersion":1, "name":"nv-toolchain", "version":"$V",
  "description":"Nullang compiler", "authoredBy":"agent-in-vm",
  "createdAt":"$(date -u +%Y-%m-%dT%H:%M:%SZ)", "deps":[],
  "exposedBins":["nullang"], "capabilities":[], "sourceLanguage":"rust",
  "buildSteps":["cargo build --release"] }
EOF
( cd /var/pkg/nt && tar czf /var/nv-toolchain-$V.nvpkg manifest.json payload/ )
nv-pkg install /var/nv-toolchain-$V.nvpkg

# declare `pkgs.nv-toolchain` in /etc/nullvoid/system.null's `packages`, then:
nv-rebuild switch

# smoke-test the now-active compiler (`nullang` resolves to /run/current/bin)
if nullang run /var/src/smoke-probe.null ; then
  echo "probe passed — generation kept"
else
  nv-rebuild rollback   # real generation rollback to the previous nullang
  echo "probe FAILED — rolled back"
fi
```

The smoke probe must exercise every shipped feature (Wave 0 + Wave 1 + argv +
whatever you add). Switch only persists if the probe passes; otherwise
`nv-rebuild rollback` reverts the generation and `/run/current/bin/nullang`
goes back to the last good compiler — you never end up stuck on a broken one.

Safe because `nullang` (construction mode) is **not** in `nv-rebuild`'s critical
path — `system.null` is evaluated by `null` (declaration mode), a separate baked
binary — and `cargo` rebuilds `nullang` without needing `nullang`. Even a
totally broken `nv-toolchain` generation rolls back cleanly; the baked
`/bin/nullang` is the floor.

## Flow back to the canonical repo

Source delivery is **git**: clone the repo inside the VM (it lives on GitHub),
edit your clone's `bootstrap/system/nullang/{check.rs,codegen.rs}` inside the
two allowed regions, build/swap/probe, then commit and push to a **separate
branch** (e.g. `nullang-builtins-wip`). The host author reviews that branch as a
PR, recomputes `cargoHash` only if `Cargo.lock` changed — builtins add no
dependencies, so it will not — rebuilds the shipped initramfs `nullang`, and
merges. The branch is the review gate; keep edits inside the two regions so the
review is a glance, not an audit. (A 9P source mount is the tighter-loop
alternative, deferred — git keeps the host working tree untouched.)
