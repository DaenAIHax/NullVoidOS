# nv-pkg

Package manager for NullVoidOS (Phase 1). Local-only: no network, no registry.
Consumes `.nvpkg` tarballs authored by the agent; stores them under a
content-addressed root.

## Commands

```
nv-pkg install <file.nvpkg>     validate + unpack into the store, print store path
nv-pkg resolve <name>-<version> print store path, exit 1 if not installed
nv-pkg list [--json]            list installed packages (human or JSON)
nv-pkg remove <name>-<version>  remove store path (warns: no ref-tracking in Phase 1)
nv-pkg verify <name>-<version>  re-hash installed contents, confirm hash match
```

Override the store root (default `/var/lib/nv-store`) with:
```
export NV_STORE_ROOT=/tmp/test-store
```

## Build (musl static binary — for use inside the VM)

```sh
# Cross-compile to x86_64-unknown-linux-musl (fully static, no glibc dependency):
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#musl -c \
  cargo build --release --target x86_64-unknown-linux-musl
# Binary: target/x86_64-unknown-linux-musl/release/nv-pkg
```

Add the target once:
```sh
nix shell nixpkgs#cargo -c rustup target add x86_64-unknown-linux-musl
```

Or with cross (easier on NixOS):
```sh
nix shell nixpkgs#cross.x86_64-unknown-linux-musl.buildPackages.cargo -c cargo build --release
```

## Run tests

```sh
# cargo must be in PATH — on NixOS use nix shell (gcc needed for the linker):
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo test
```

Tests create their own temp store roots via `NV_STORE_ROOT`; no root required.

## Gotchas

- `NV_STORE_ROOT` is a process-global env var. Tests set it in-process; if
  you run tests in parallel with `-- --test-threads=N` and more than one test
  sets it simultaneously, they may share state. The tests are written to use
  `with_store()` which sets a per-test temp dir, but the env var is truly
  global. This is safe for `cargo test`'s default single-threaded-per-crate
  mode, but keep it in mind if parallelism is increased.
- The store hash (first 32 hex chars of SHA-256 of the raw tarball) is
  embedded in the directory name. `nv-pkg verify` re-hashes the *unpacked
  contents*, not the original tarball, so it detects post-install tampering
  but cannot reconstruct the original tarball.
- Symlink validation is lexical (no filesystem resolution): a symlink
  `payload/a/../../etc/passwd` is correctly rejected as escaping, even if
  the filesystem path would be harmless.
