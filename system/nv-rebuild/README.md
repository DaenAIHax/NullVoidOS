# nv-rebuild

Activation engine for NullVoidOS. Evaluates a system manifest, resolves
packages, and atomically switches the running system to a new generation.

## Commands

```
nv-rebuild check        validate manifest + package store, no mutation
nv-rebuild build        prepare next generation directory, do not activate
nv-rebuild switch       build + atomically activate new generation
nv-rebuild rollback     revert to previous generation
nv-rebuild generations  list generations, mark current
```

## Environment overrides

| Variable | Default | Meaning |
|---|---|---|
| `NV_SYSTEM_ROOT` | `/var/lib/nv-system` | Root of the generation store |
| `NV_CONFIG` | `/etc/nullvoid/system.null` | System manifest path |

Setting these lets you run and test without root.

## Switch algorithm (pseudo-code)

```
1. null eval $NV_CONFIG           -> SystemManifest JSON
2. for each pkg in manifest.packages:
     nv-pkg resolve <pkg>         -> store_path  (abort if missing)
3. N = max(existing generation numbers) + 1
4. mkdir /var/lib/nv-system/generation-N/
5.   write manifest.json (the evaluated manifest)
6.   for each pkg's exposedBins:
       symlink bin/<name> -> store_path/payload/bin/<name>
       (conflict: last package in `packages` wins; warning to stderr)
7.   write etc/environment (KEY=value lines)
8.   write etc/services/<name> (one file per service)
9. rename(.current.new -> generation-N, then rename to current)
```

Step 9 is the only mutation that commits a generation. Every step before it
is reversible by deleting the half-built directory.

## Atomic swap semantics

The `current` symlink is updated via a two-step rename:

1. `symlink(generation-N, .current.new)` — create a fresh tmp symlink.
2. `rename(.current.new, current)` — atomic replace on the same filesystem.

Both paths live under `NV_SYSTEM_ROOT`, so they are always on the same
filesystem. POSIX `rename(2)` is atomic when src and dst are on the same
mount point; no process ever sees `current` absent.

`renameat2(RENAME_EXCHANGE)` (Linux >= 3.15) would allow simultaneously
swapping two live directory trees without either ever being absent. That
syscall is implemented in `src/swap.rs` (`try_renameat2_exchange`) but
**not used in the main path** because `current` is a symlink (not a
directory), and the rename approach is already atomic for symlinks. The
`RENAME_EXCHANGE` path is reserved for Phase 2 when live service trees
may need in-place swapping.

## Build

```sh
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc \
  -c cargo build --release --target x86_64-unknown-linux-musl
```

## Test

```sh
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo test
```

See `tests/README.md` for how the fixture stubs work.
