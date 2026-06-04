# hello-rust — Phase 1 stretch test

A self-contained shell script (`stretch-test.sh`) that closes two gaps
left open by the original Phase 1 end-to-end demo (CHANGELOG entry
*Phase 1 demo passes end-to-end*, 2026-05-28):

1. **Real ELF binary, not a bash-script payload.** The previous demo
   used a shell-script standing in for a compiled binary. This one
   compiles a tiny Rust program inside the VM with the dev substrate's
   `cargo` + `rustc`.
2. **`pkgs.<name>` ambient exercised.** The previous demo referenced
   the package as the literal string `"hello-nv-0.1.0"` in
   `system.null`. This one uses `packages = [ pkgs.hello-rust ]`,
   which forces the `.null` evaluator to call `nv-pkg list --json`
   and project the latest-installed version into the `pkgs` attrset
   (CONTRACTS §5.4).

## Running the test

```sh
# 1. Boot the bootstrap VM from the host:
nix run ./bootstrap

# 2. Inside the VM (you land at the busybox shell), open another
#    terminal on the host and copy the script in over SSH:
scp -P 2222 \
  system/demos/hello-rust/stretch-test.sh \
  root@localhost:/tmp/

# 3. Back inside the VM:
sh /tmp/stretch-test.sh
```

The script is idempotent — re-running it builds a new generation each
time (`generation-N+1`), but the package store path is content-addressed
(CONTRACTS §1.3) so re-installing the same tarball collapses to the
same store path.

## What success looks like

The final block prints something like:

```
=== 11. run hello-rust ===
hello-rust: ELF binary built from Rust, running on NullVoidOS
  pid:       143
  argv[0]:   /run/current/bin/hello-rust
  unix_ts:   1748469317
  path_head: /run/current/bin

=== stretch test passed ===
```

`argv[0]` resolves through the symlink chain
`/run/current/bin/hello-rust → /var/lib/nv-system/generation-N/bin/hello-rust
→ /var/lib/nv-store/<hash>-hello-rust-0.1.0/payload/bin/hello-rust`,
which proves PATH lookup, generation activation, and store resolution
all line up.

## What's still not exercised

- **Static-musl target.** `cargo build --release` uses the dev
  substrate's host target (nixpkgs glibc-dynamic). The binary works
  because glibc is in the closure shipped into the initramfs. Producing
  a true static-musl ELF would need
  `cargo build --release --target x86_64-unknown-linux-musl`, which
  requires the musl target to be installed in the toolchain — not
  available in the current devSubstrate. Left for a future test.
- **Service capabilities at runtime.** `capabilities` in the manifest
  is recorded but not enforced (CONTRACTS §4 — placeholder until
  Phase 2's seccomp/landlock wiring).
- **`deps` resolution beyond the empty array.** This package has
  `"deps": []`. Cross-package resolution is a separate test.
