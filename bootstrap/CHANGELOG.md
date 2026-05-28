# Bootstrap Changelog

All notable changes to the NullVoidOS `lfs-bootstrap` direction.

Format adapted from [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Dates are ISO 8601 (YYYY-MM-DD). Entries reference commit hashes when
applicable.

## [Unreleased]

### Milestone — Phase 1 stretch test passes (real Rust ELF + pkgs ambient) (2026-05-28)

`bootstrap/system/demos/hello-rust/stretch-test.sh` was driven
end-to-end inside the booted VM. Both gaps the original Phase 1 demo
left open are now closed:

1. **Real ELF binary.** `cargo build --release` (rustc 1.95.0 from the
   dev substrate, host-target nixpkgs glibc) compiled `hello-rust` in
   23.97s into a 301KB binary. No bash-script stand-in, no host-side
   cross-compilation.
2. **`pkgs.<name>` ambient exercised.** `system.null` referenced the
   package as `packages = [ pkgs.hello-rust ]`. The `.null` evaluator
   resolved it against the live `nv-pkg list --json` (which then
   listed both `hello-nv-0.1.0` from the earlier demo and the new
   `hello-rust-0.1.0`, confirming the store persists across reboots).
   The emitted `SystemManifest` carried the resolved literal
   `"hello-rust-0.1.0"` in its `packages` array.

The full loop ran clean:

```
cargo build --release             → 301 KB ELF in 23.97s
tar czf hello-rust-0.1.0.nvpkg    → 147 KB tarball
nv-pkg install                    → /var/lib/nv-store/b6b0bd5dbdf152d08fe0138fc4cb711c-hello-rust-0.1.0
null check + null eval            → SystemManifest with pkgs.hello-rust resolved
nv-rebuild switch                 → /var/lib/nv-system/current -> generation-2
which hello-rust                  → /run/current/bin/hello-rust
hello-rust                        → ran, printed pid/argv0/unix_ts/path_head
```

The generation counter bumped from 1 (from the previous demo) to 2,
confirming the activation engine's accumulator is monotonic across
reboots and the previous package (`hello-nv`) stayed installed but is
no longer in the active manifest — it's not garbage-collected, just
absent from `/run/current/bin/`, exactly as CONTRACTS §3.4 specifies.

Still not exercised (deliberately deferred, documented in the demo's
README):

- Static-musl target — needs `x86_64-unknown-linux-musl` added to the
  rustc target list in `devSubstrate`.
- Multi-package `deps` resolution — single-package install here.
- Runtime capability enforcement — recorded-only in Phase 1.

### Fix — `/etc/shells` + `/dev/pts` mount for PTY-aware services (2026-05-28)

Surfaced while driving the stretch test from the host: SSH into the
VM was failing in two distinct ways that the original Phase 0 (a)
boot had never tripped on (the interactive console seriale never
opened a fresh PTY or called `getusershell()`).

- **`/etc/shells` missing.** Dropbear validates each user's login
  shell against `getusershell(3)`. When `/etc/shells` does not exist,
  glibc returns a hardcoded `{/bin/sh, /bin/csh}` and dropbear
  rejects the login with *"user 'root' has invalid shell, rejected"*.
  Since `/etc/passwd` points root at `/bin/bash` (Phase 0 (a)
  contingency 1), every SSH from the host was permanently broken.
  Fixed by shipping `/etc/shells` with `/bin/sh` and `/bin/bash`.
- **`/dev/pts` not mounted.** `devtmpfs` populates `/dev/ptmx` but
  the slave nodes (`/dev/pts/N`) only appear under a separate
  `devpts` mount. Without it, any PTY-allocating program (`script`,
  interactive ssh sessions, tmux-like multiplexers) fails on the
  slave-open with ENOENT and the failure surfaces as something
  unrelated. Init now does `mkdir -p /dev/pts && mount -t devpts
  devpts /dev/pts` next to the other early filesystem mounts.

Both touch `bootstrap/pkgs/initramfs.nix`. A rebuild
(`nix build .#initramfs`) is needed before the next boot picks them
up. Known follow-up: even with these two fixes, dropbear still exits
with *"Failed to set euid"* after auth — likely a privsep / dropped-
privilege issue in the nixpkgs build that the in-VM agent never hit
(it uses the seriale directly). Tracked separately; SSH from host
is not on the Phase 1 critical path.

### Milestone — Phase 1 demo passes end-to-end (2026-05-28)

The six-step demo from CONTRACTS §5 — author → package → install
→ declare → switch → use — closes on the first try inside the
boot VM. The three Rust crates scaffolded by parallel sub-agents
in the previous session (`nv-pkg`, `null`, `nv-rebuild`) integrate
cleanly across the contracts they share, with no manual
glue-fixing between them.

Verified inside `nix run ./bootstrap`:

1. A package `hello-nv-0.1.0` was authored at `/tmp/pkg-src/`
   (manifest.json + `payload/bin/hello-nv`, a shell-script
   stand-in for a compiled binary).
2. `tar czf` produced a 447-byte `.nvpkg`.
3. `nv-pkg install` placed it at
   `/var/lib/nv-store/0fd5224c1a7c2b17f48218dcea2e3973-hello-nv-0.1.0/`.
4. A minimal `system.null` was written to `/etc/nullvoid/`:
   ```null
   {
     hostname = "nullvoid";
     caps = [ !tty ];
     packages = [ "hello-nv-0.1.0" ];
     services = {};
     environment = {};
   }
   ```
5. `null check` exited 0 silently; `null eval` emitted a clean
   `SystemManifest` JSON with the `!tty` capability serialised as
   `{"path":["tty"],"arg":null}`.
6. `nv-rebuild check` validated:
   `manifest ok: hostname=nullvoid`
   `[ok] hello-nv-0.1.0 -> /var/lib/nv-store/...`
7. `nv-rebuild switch` activated generation 1:
   `building generation 1...`
   `activated: /var/lib/nv-system/current -> generation-1`
8. `nv-rebuild generations` listed `generation-0` and
   `* generation-1 (current)`.
9. `/run/current/bin/hello-nv` resolved through the symlink chain
   to `/var/lib/nv-store/.../payload/bin/hello-nv`, `which`
   confirmed PATH lookup, and the binary ran:
   `hello from a package authored at 2026-05-28T18:15:57Z`.

This is the falsifiable test of Phase 1. The declarative loop
(edit `system.null` + `nv-rebuild switch` → atomic PATH change)
is closed end-to-end without an agent in the loop.

**Observations from the demo:**

- `null check` is silent on success (Unix convention). For
  agent-facing affordance, a future `--verbose` or non-JSON
  human summary on success would reduce "did it do anything?"
  doubt.
- `nv-rebuild generations` still lists `generation-0` (the empty
  bootstrap directory the initramfs creates at first boot).
  Cosmetic; a Phase 2 `nv-gc` would prune it.
- The `pkgs` ambient (SPEC §5.4 — populated by
  `nv-pkg list --json`) was not exercised: the demo references
  the package as a literal `"hello-nv-0.1.0"` string. Future
  test should use `packages = [ pkgs.hello-nv ];`.
- The package payload is a `bash`-script, not a compiled
  binary. The natural next stretch test is to compile a real
  Rust binary inside the VM (the dev substrate has rustc/cargo
  end-to-end), package it, switch, and run it — that exercises
  the build path inside the lab, not just the install path.

### Fix — /var probe is mount-first, not blkid-first (2026-05-28)

The smoke-test that verified the Phase 1 wire-up surfaced a latent
init bug. The /var bootstrap used `blkid /dev/vda` as the gate
between "mount existing fs" and "format then mount". `blkid` was
false-negativing on the existing qcow2 — likely because the
initramfs has no udev and blkid wanted a cache directory under
`/run` that the init script hadn't created yet — pushing init into
the mkfs branch every boot. `mkfs.ext4 -q` (no `-F`) then prompted
`Proceed anyway? (y,N)` on the existing fs and would have blocked
PID 1 forever; only the test's first piped character ("e" from
`echo`) accidentally answering "no" let the fs survive.

`bootstrap/pkgs/initramfs.nix`: switched to a mount-first probe.
The actual measure of "is this a usable ext4 fs?" is whether
`mount -t ext4` succeeds. Only fall through to `mkfs.ext4 -F` when
the mount truly failed; `-F` is safe at that point because there
is nothing to preserve on /dev/vda anyway.

Both branches verified by smoke-boot: existing-fs path mounts
cleanly with no mkfs noise, fresh-format path (qcow2 deleted)
runs `mkfs.ext4 -F`, mounts the new fs, and creates all four
Phase 1 directories under /var/lib/ (`dropbear nv-config nv-store
nv-system`).

### Milestone — Phase 1 tooling wired into the initramfs (2026-05-28)

The three Phase 1 crates scaffolded in the previous session
(`nv-pkg`, `null`, `nv-rebuild`) are now compiled by the Nix flake
and shipped inside the boot initramfs as standalone musl-static
binaries. The boot VM gains the full Phase 1 surface end-to-end:
`nv-pkg install` / `null eval` / `nv-rebuild switch`.

**New derivations:**

- `bootstrap/pkgs/null.nix` — `.null` CLI (1.1 MB stripped).
- `bootstrap/pkgs/nv-pkg.nix` — package manager (1.1 MB stripped).
- `bootstrap/pkgs/nv-rebuild.nix` — activation engine (1.6 MB stripped).

Each uses `pkgsStatic.rustPlatform.buildRustPackage`. Verified
`--version` runs on host; binaries are pure musl statics (no
`/lib/ld-musl-*` runtime dep, no `/nix/store` closure shipped).

**`bootstrap/pkgs/default.nix`** — exposes `nullLang`, `nv-pkg`,
`nv-rebuild` attrs (the `nullLang` name avoids the bare-`null` Nix
keyword clash; the binary on disk is still `null`). All three are
passed through to `initramfs` via `callPackage`.

**`bootstrap/pkgs/initramfs.nix`** — `cp`s the three binaries into
`/bin` alongside `zero`. The init script:

- Adds `/var/lib/nv-config/` to the persistent-`/var` mkdir set.
- Symlinks `/etc/nullvoid` → `/var/lib/nv-config/` on first boot so
  the agent-authored `system.null` survives reboots.
- Bootstraps an empty `generation-0/bin/` under
  `/var/lib/nv-system/` and points `current` at it, so
  `/run/current/bin` resolves to a real directory before the agent
  ever runs `nv-rebuild switch`. The first real generation will be
  `generation-1`.
- Creates `/run/` and symlinks `/run/current` →
  `/var/lib/nv-system/current` as specified in CONTRACTS §3.2.
- Bumps `PATH` to `/run/current/bin:/bin`, so a successful
  `nv-rebuild switch` immediately shadows the initramfs `/bin`.
- Banner now reports `null` / `nv-pkg` / `nv-rbld` versions.

**`pkgsStatic.rustPlatform.buildRustPackage` + `cargoLock.lockFile`
hit a crates.io regression** — the registry API endpoint
(`crates.io/api/v1/crates/<n>/<v>/download`) now returns HTTP 403
without a `User-Agent` header, and nixpkgs's `importCargoLock`-based
per-crate `fetchurl` doesn't set one. Switched to `cargoHash`, which
runs `cargo fetch` inside a fixed-output derivation — cargo's own
HTTP client sets a UA, the registry serves the bytes, and the
vendor tree is hashed as one FOD blob. Trade: a `cargoHash` line
per crate (set to `lib.fakeHash`, rebuild, paste the `got:` line
back) instead of an auto-derived hash from the lockfile. The
mechanism is the nixpkgs ≥25.05 default
(`useFetchCargoVendor = true` is implicit), no explicit flag
needed.

**Initramfs growth:** unchanged class — still 1.1 GB compressed.
The three binaries together add ~4 MB.

**Not yet wired and deferred to a future session:**

- A default `/etc/nullvoid/system.null` template — the agent is
  expected to author it the first time. Without a file, `nv-rebuild
  check` will refuse to evaluate; that's the intended starting
  point of the §5 demo flow.
- `examples/*.null` (v1-shaped) still present under `null/`, not
  yet migrated to v2.
- `null doctor` / `null fix --plan --json` (SPEC §6) still absent.

### Revised — Layer 3 language decision (2026-05-28, same day as lock)

The 2026-05-28 lock *"Layer 3 DSL is ZeroLang itself, no translator"*
has been revised the same day. Trigger: user surfaced that ZeroLang
is a systems programming language (`mut`, `set`, `World`, generics,
ownership, native codegen — Rust/Zig family), and forcing it into the
system-declaration role is a category error analogous to writing
NixOS configurations in Rust.

**New decision:** Layer 3 DSL is `.null` — a separate, deliberately
tiny, Nix-shaped declarative language that **inherits ZeroLang's
agent-first tooling recipe** (typed JSON diagnostics, repair IDs,
embedded skills bundle, single-form-per-concept, capability-explicit
syntax) transposed to the configuration domain. ZeroLang remains the
implementation language for layers 1-2 and layer 4 apps.

Authoritative spec: `bootstrap/system/null/SPEC.md` (new). DESIGN.md
section *"Layer 3 language model"* rewritten in place; the original
locked text is preserved verbatim in a `History` callout so the
reasoning trail stays visible. The mental-model NixOS-analogy table,
the layer-3 ASCII box, the *Language choice — ZeroLang* section, and
the *Open design questions* are all updated to match.

The CONTRACTS.md §2 sketch from the previous session (which had
already drifted to a separate `.null` language without flagging the
contradiction with the same-day lock) is now formally superseded by
SPEC.md v2.

### Migration — `.null` crate v1 → v2 (same session)

`bootstrap/system/null/` migrated to implement SPEC v2 in 8 steps,
each verified by build + smoke test before moving on:

1. **Lexer.** Added `Bang` (`!`) token. Symbol (`.identifier`) and
   capability (`!ident(.ident)*(."str")?`) literals are assembled at
   parse time from the dumb token stream, so the lexer stayed minimal.
2. **AST.** Added `Expr::Symbol { name, span }` and
   `Expr::Capability { path, arg, span }`. Field-access continues to
   work after a leading `Ident`; standalone `Dot` / `Bang` at the
   start of an expression now route to the new parsers.
3. **Parser.** `parse_symbol` and `parse_capability` added. Existing
   anti-feature detection (`let`, `if`, `import`) preserved. A
   pre-existing CLI bug (global `--json` clashing with `parse --json`)
   surfaced and was fixed by dropping the dual-mode toggle (v2 NDJSON
   is the default, in line with SPEC §6).
4. **Schema / types.** `types.rs` rewritten. `SystemManifest` gains
   `caps: [Capability]`; `Service` gains `requires: [Capability]` and
   `restart` becomes the enum-as-symbol `RestartPolicy`. The capability
   vocabulary from SPEC §5.5 is hardcoded in `known_capability()`.
   Subset rule enforced: every `service.requires` ⊆ `system.caps`
   (CAP004 with `repair = add-system-cap` if violated).
5. **Diagnostics.** Full rewrite to SPEC §7 shape: NDJSON on stderr,
   structured `expected` / `actual` / `span: SpanInfo` /
   `repair: Option<Repair>` (typed `id` + `args` JSON value). Stable
   error-code namespaces materialised: `PAR001`, `TYP001`, `TYP004`,
   `SCH001`, `REF002`, `CAP001`, `CAP004`. Initial repair-ID set from
   SPEC §7.3 wired in: `wrap-int-as-string`, `add-system-cap`,
   `fix-enum-symbol`, `add-required-field`, `quote-bare-identifier`,
   `homogenize-list`, `remove-unknown-field`.
6. **CLI `explain`.** `null explain <CODE>` and `null explain list`
   added. Per-code docs embedded as const strings in `src/explain.rs`
   — agent can recover the meaning of any diagnostic from the binary
   alone, no network access (SPEC §1).
7. **Skills bundle.** Six markdown documents at
   `bootstrap/system/null/skills/` (`null`, `language`, `schema`,
   `caps`, `cli`, `diagnostics`) embedded via `include_str!` and
   served by `null skills list` / `null skills get <name>`. This is
   the version-matched-skills-bundle property that lets an agent that
   has never seen `.null` author a correct `system.null` from the
   binary alone (SPEC §8).
8. **Test migration.** All 50 v1 integration tests adapted to v2
   schema (added `caps = []` to every `SystemManifest` test string,
   `requires = []` to every Service, `restart = .always` symbols
   instead of `"always"` strings, `err.span.line`/`err.repair`
   instead of `err.line`/`err.fix`). 3 example-file-bound tests
   deleted (the `examples/*.null` files are still v1-shaped — they
   parse but no longer typecheck under v2; restoration deferred).
   2 new schema-missing tests and 4 new capability tests added.
   **Final result: 53 tests passing, 0 failures.** Verified end-to-end
   with smoke files covering CAP001 (unknown cap), CAP004 (subset
   violation), TYP004 (string-instead-of-symbol restart). Each error
   now carries a typed `repair.id + args` payload an agent can apply
   without parsing prose.

Known gaps deferred to a future session:
- `null doctor` and `null fix --plan --json` (SPEC §6) not
  implemented.
- The `examples/*.null` files are still v1 shape; restore them or
  drop the dir.
- The v1 `./path` lexer shortcut is still accepted even though SPEC
  v2 §3.1 doesn't list it — either ban it in v2.1 or add it to SPEC.

### Added

- `bootstrap/system/null/SPEC.md` — authoritative spec for `.null`
  v2. Twelve sections including the five-tricks-transposed rationale,
  the anti-feature list, surface syntax, the `SystemManifest` schema,
  the capability-as-value system, CLI surface mirroring Zero's
  (`null explain`, `null skills`, `null fix --plan --json`),
  diagnostic NDJSON format with stable error-code namespaces
  (`PAR`/`TYP`/`SCH`/`REF`/`CAP`) and a closed initial repair-ID set,
  reference `system.null` example, and what the existing Rust crate
  owes to reach v2.

- **Layer 3 language model — Zero native, no translator** section in
  `DESIGN.md`. Decision locked: the system description language is
  ZeroLang itself; no separate DSL, no runtime module system bolted on
  top.
- **Mental model — how the layers relate** table in `DESIGN.md` mapping
  NixOS constructs to NullVoidOS equivalents (Nix language ↔ ZeroLang,
  module system ↔ static types, `/nix/store` ↔ CAS substrate, etc.).
- **Substrate ↔ Zero boundary** section in `DESIGN.md` explaining the
  per-package Zero wrapper pattern (`substrate/openssl.zero` over
  `libcrypto.so` via FFI; capability annotations enforced at Zero
  boundary).
- **Open design questions** section in `DESIGN.md` listing four pieces
  deferred to Phase 2: module shape, composition semantics,
  `SystemManifest` schema, activation capability primitives.
- `CLAUDE.md` at repo root scoping behaviour for this branch
  (workflow, communication conventions, phase awareness).
- `bootstrap/CHANGELOG.md` (this file).
- `bootstrap/flake.lock` generated by first `nix develop ./bootstrap`.
  Pins `nixpkgs` to `64c08a7` (2026-05-23) and `flake-utils` to
  `11707dc` (2024-11-13).

### Changed

- `bootstrap/flake.nix`: busybox source switched from cross-compiled
  dynamic (`pkgsMusl.busybox`) to fully static (`pkgs.pkgsStatic.busybox`).
  Verified: `statically linked`, `ldd` reports "not a dynamic executable".
  Rationale: initramfs cpio should not depend on shipping `ld-musl` as a
  runtime interpreter.
- `bootstrap/flake.nix`: dev shell now includes `zerolang` derivation
  (callPackage from `bootstrap/pkgs/`). Shell hook prints `zero --version`.
- `bootstrap/pkgs/zerolang.nix` added — ported from `nix-rewrite` branch
  (commit `d903bae`). Multi-platform derivation fetching Vercel's
  release binaries for v0.1.4 with SHA256 pinned (linux-musl-x64/arm64,
  darwin-x64/arm64). Verified: `zero --version` → `zero 0.1.4`. Binary
  itself is statically-linked musl ELF, ready to drop into the initramfs.
- `bootstrap/pkgs/default.nix` added — package set entry point for
  `callPackage` extensibility (next additions: llama.cpp, substrate
  wrappers).
- `bootstrap/pkgs/kernel.nix` added — minimal Linux 6.6.141 LTS kernel
  derivation. Starts from `make tinyconfig`, adds curated options via
  `scripts/config`, reconciles with `make olddefconfig`. Targets
  x86_64, VirtIO paravirt, serial ttyS0 console, basic TCP/IP, no
  modules. Build time on host: ~46 s after tarball cached. Result:
  `bzImage` **1.5 MB**, `.config` 52 KB. Verified: boots in QEMU
  through to "No working init found" panic — expected (initramfs is
  task #7). Includes `patchShebangs scripts` workaround for the Nix
  build sandbox.
- `bootstrap/pkgs/default.nix`: exposes `kernel` alongside `zerolang`.
- `bootstrap/pkgs/initramfs.nix` added — Phase 0 variant (d) initramfs.
  Assembles cpio.gz from: static-musl busybox + ~40 standard symlinks,
  the static `zero` binary, and an `/init` sh script that mounts
  `/proc /sys /dev`, prints kernel/hostname/zero versions, and drops
  to a busybox shell. Built via `runCommand` with `cpio` + `gzip` from
  nativeBuildInputs. Final size: **1.2 MB compressed**.
- `bootstrap/pkgs/default.nix`: exposes `initramfs`, wires it to the
  in-tree `zerolang` via `inherit (self) zerolang`.

### Milestone — Phase 0 boot pipeline alive (variant d)

End-to-end QEMU boot succeeds: SeaBIOS → kernel (1.5 MB bzImage) →
initramfs (1.2 MB cpio.gz) → `/init` → busybox shell. `zero --version`
runs inside the VM and prints `zero 0.1.4`. No AI in the loop yet —
this proves the kernel + initramfs + userland pipeline before adding
the agent backend.

Outstanding for full Phase 0 demo:
- Variant (a): Claude Code CLI inside the VM, consuming the user's
  Claude Max subscription. Plan in `bootstrap/PHASE0_A_PLAN.md`.
  Decisions locked: Node.js via `pkgsMusl.nodejs_22`, credentials
  passed in via 9P read-only mount of host's `~/.config/claude/`,
  init drops to busybox shell and user types `claude` manually.
  Delete the plan file when (a) ships.
- Cosmetic: `can't access tty; job control turned off` warning from
  busybox sh. Fix later with `setsid cttyhack`.

### Changed (within session)

- Replaced operational plan `PHASE0_B_PLAN.md` with `PHASE0_A_PLAN.md`.
  Reason: variant (b) called Anthropic API directly per-token, while
  user pays a Claude Max subscription that covers Claude Code usage.
  (b) would be double-paying. (a) reuses the subscription. See memory
  `feedback_claude_subscription`.
- `bootstrap/flake.nix`: added `apps.boot-vm` for one-command interactive
  boot of the Phase 0 (d) VM. Usage: `nix run ./bootstrap#boot-vm` or
  the shorter `nix run ./bootstrap` (alias as `apps.default`). Wraps
  `qemu-system-x86_64` with the kernel + initramfs derivations baked
  in; exit with `Ctrl-A x`.

### Milestone — Phase 0 (a) alive: Claude Code inside the VM

End-to-end boot of variant (a) succeeds. Final banner:

```
kernel:   Linux 6.6.141
zero:     0.1.4
claude:   2.1.148 (Claude Code)
IP:       10.0.2.15/24
creds:    yes  — backups cache debug ...
```

The VM mounts the host's `~/.claude/` directory over 9P/virtio at
`/root/.claude/` (read-only), brings up `eth0` via DHCP through QEMU
user networking, and `claude --version` runs the upstream Node-based
Claude Code CLI from inside the initramfs.

Artifact sizes:

- `bzImage`: 1.7 MB (unchanged class — added options are a few KB each)
- `initramfs.cpio.gz`: **100 MB** (up from 1.2 MB in variant (d); the
  entire `claude-code` Nix closure of ~312 MB uncompressed across 30
  store paths now lives under `/nix/store` inside the initramfs)

Interactive verification (manual, after `nix run ./bootstrap`):

- Send a prompt, confirm the Max-subscription token is consumed (and
  not a per-token API key).
- Send "create a file /tmp/test.txt with content hello", confirm tool
  use writes the file inside the VM.
- Stretch: ask `claude` to write a small Zero program and execute
  `zero run` on it.

### Changed (Phase 0 (a) work)

- `bootstrap/pkgs/kernel.nix`: enabled `VIRTIO_MENU` (parent Kconfig
  gate for `VIRTIO_PCI` / `VIRTIO_BLK` / `VIRTIO_CONSOLE`), `PCI_MSI`
  (required by modern virtio transports), `NET_9P`, `NET_9P_VIRTIO`,
  `9P_FS`, `9P_FS_POSIX_ACL`. Also added the userspace runtime block
  needed by modern glibc + Node.js: `FUTEX`, `EVENTFD`, `SIGNALFD`,
  `TIMERFD`, `EPOLL`, `INOTIFY_USER`, `AIO`, `POSIX_MQUEUE`,
  `PREEMPT_VOLUNTARY`. Without these, `claude --version` aborts with
  "futex facility returned an unexpected error code" and the virtio
  devices stay unbound (symptom: "no channels available for device
  claudefs"). bzImage stayed at 1.7 MB.
- `bootstrap/pkgs/initramfs.nix`: Phase 0 (a) initramfs. Ships the
  full `claude-code` Nix closure (30 paths) under `/nix/store`,
  symlinks `/bin/claude` to the wrapper, plus the `cacert`
  `ca-bundle.crt` at `/etc/ssl/certs/` and a hand-rolled
  `udhcpc/default.script` (busybox's bundled one hardcodes nix-store
  paths to its own bin/, useless inside the initramfs). Init script
  now mounts the 9P share, runs `udhcpc -i eth0`, exports
  `NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-bundle.crt`, and prints a
  variant-(a) banner with `claude --version` and the credentials
  listing.
- `bootstrap/pkgs/default.nix`: passes `claude-code` and `cacert`
  through to `initramfs` via `callPackage`.
- `bootstrap/flake.nix`: imports nixpkgs with
  `config.allowUnfree = true` (claude-code is unfree-licensed).
  `apps.boot-vm` extended for variant (a): preflight-checks
  `~/.claude/.credentials.json` on the host, mounts that directory as
  a read-only 9P share (`mount_tag=claudefs`), attaches a virtio user-
  network NIC, bumps memory to 1 GB, and adds `-cpu max` so the AVX2
  /  BMI2 instructions the nixpkgs glibc + bundled Node require don't
  trap as "Illegal instruction" on QEMU's default `qemu64` CPU.

### DESIGN.md

- Added "Phase 0 (a) — documented deviation: glibc in the bootstrap
  initramfs" under §Phase 0 decisions. Documents the trade-off
  (ship the full claude-code Nix closure + `/nix/store` as the
  CAS-of-convenience for now) and the revisit condition.

### Removed

- `bootstrap/PHASE0_A_PLAN.md` — superseded by this milestone entry.

### Fix — Phase 0 (a) interactive login

First interactive run blocked at `claude` startup with:

```
Claude configuration file not found at: /root/.claude.json
A backup file exists at: /root/.claude/backups/.claude.json.backup.<ts>
```

Root cause: Claude Code splits its on-disk state in two — `~/.claude/`
(credentials, history, cache) and `~/.claude.json` (config: project
trust, model prefs, MCP servers). Our 9P share only exposes the
directory, so the JSON config file at `$HOME/.claude.json` is missing
inside the VM and `claude` refuses to start. The on-screen rescue
command Claude prints (`cp …backup.<ts> ~/.claude.json`) gets line-
wrapped to 80 cols on the serial console, which is what made it look
like an OAuth `Missing code_challenge` problem.

Fix in two pieces:

- `bootstrap/pkgs/initramfs.nix`: init now seeds `/root/.claude.json`
  from the newest backup under `/root/.claude/backups/` on every boot.
- `bootstrap/flake.nix` and `initramfs.nix`: drop `readonly=on` from
  the 9P share so claude-code can refresh the OAuth token in-place.
  This is the trade documented as Phase 0 plan contingency 5 — the VM
  may now mutate the host's `~/.claude/` (token refresh + history
  writes). Accepted for Phase 0; a multi-tenant separation comes later.

### Performance + TTY polish (after first interactive Claude session)

User confirmed Phase 0 (a) works interactively (`claude` answers
prompts inside the VM), but flagged two issues:

- `claude` writes responses slowly — the boot-vm app was running in
  TCG (software emulation), which has to interpret every AVX2/BMI2
  instruction emitted by the nixpkgs glibc + bundled Node.js.
- Hitting `Ctrl-C` inside the Claude TUI left the serial terminal
  wedged in raw mode; the user had to close the host terminal window
  to recover. Symptom of the earlier `can't access tty; job control
  turned off` warning — the shell had no controlling tty, so signals
  bypassed its line discipline.

Fixes:

- `bootstrap/flake.nix` (boot-vm app): probes `/dev/kvm` at runtime
  and switches to `-accel kvm -cpu host` when usable (still falls
  back to `-accel tcg -cpu max` if not). KVM brings Node.js workloads
  back to native speed.
- `bootstrap/pkgs/initramfs.nix`: respawn loop now launches the
  shell as `setsid cttyhack /bin/sh` instead of bare `/bin/sh`. The
  `cttyhack` busybox applet grabs the first available tty
  (`/dev/console` here) and sets it as the controlling terminal, so
  job control works and TUIs that catch `SIGINT` (Claude Code's Ink
  UI) can restore the terminal cleanly on exit.

### Fix — Phase 0 (a) tool-use blocked without bash

User feedback after first real `claude` session: agent runs but
"cannot do commands". Claude Code invokes its Bash tool through
`bash -c "<cmd>"` rather than `sh -c`, and the initramfs only
shipped busybox ash at `/bin/sh`. No `/bin/bash` → every Bash
tool-use call fails (silently or with `ENOENT`).

- `bootstrap/pkgs/initramfs.nix`: closure root now includes `bash`
  alongside `claude-code`. Symlinks `/bin/bash` to the GNU bash
  wrapper and `/usr/bin/env -> /bin/env` (canonical shebang path,
  busybox's `env` applet covers the binary side).
- `bootstrap/pkgs/default.nix`: passes `bash` through to initramfs.
- Closure size impact: `+2 MB` compressed (47 MB uncompressed
  bash closure shares almost everything — glibc, ncurses, readline
  — with the already-shipped claude-code closure).

`bash --version` reports `5.3.9(1)-release` inside the VM; tool-use
should now reach a real GNU bash. Git, ripgrep, etc. are still
absent — add as needed when Claude reports the next missing tool.

### Milestone — Phase 0 (a) lab edition + Phase 1 components scaffolded

The project reframed mid-day, from "rewrite NixOS in Zero" (judged not
a real thesis) to a research lab for the question **"can an agent
author a working OS end-to-end?"** — with a path to a small specialised
model (NullAgent) eventually replacing the big general one on the
governance side. See DESIGN.md for the new framing.

Two parallel deliverables landed in this session:

**1. Lab substrate.** The initramfs now ships a developer toolchain
big enough for the agent to compile and package real software:
python313, rustc+cargo, nodejs_22, gcc, make, git, curl, jq, ripgrep,
fd, neovim, sqlite, GNU coreutils. Added dropbear (SSH server) and
e2fsprogs (ext4). A persistent `/var` on a qcow2 disk auto-provisioned
under `$XDG_CACHE_HOME/nullvoid/var.qcow2` (8 GB sparse) survives
reboots. Host SSH pubkey shared via 9P, dropbear authorized_keys
populated at boot, port 22 forwarded to host:2222. VM RAM bumped
1 GB → 8 GB (needed for compiling Rust + running Node + LLM in-VM).
Compressed initramfs grew 100 MB → 1.1 GB.

Kernel additions: `BLOCK`, `BLK_DEV`, `VIRTIO_BLK`, `EXT4_FS`. The
tinyconfig base disables CONFIG_BLOCK, which silently masks every
block driver and filesystem we tried to `--enable`; turning BLOCK on
first unblocks the rest. bzImage 1.7 MB → 2.0 MB.

`/etc/{passwd,group,shadow}` minimal stubs so dropbear's getpwnam
lookup for `root` succeeds.

**2. Phase 1 components built by 3 sub-agents in parallel.** Locked
the contracts in `bootstrap/system/CONTRACTS.md` so the three could
work without colliding. Results:

- `bootstrap/system/nv-pkg/` (Rust crate, package manager per
  CONTRACTS §1). 11 integration tests green. Install / resolve /
  list / remove / verify. Tarball-hash addressing in the store path,
  separate content-hash file for tamper detection.
- `bootstrap/system/null/` (Rust crate, configuration language per
  CONTRACTS §2). 50 integration tests green. Hand-rolled lexer +
  recursive-descent parser + single-pass typecheck/eval against the
  SystemManifest schema. `check` / `eval` / `fmt` / `parse --json`.
  Diagnostics with PAR/TYP error codes.
- `bootstrap/system/nv-rebuild/` (Rust crate, activation engine per
  CONTRACTS §3). 9 integration tests green. Atomic `rename(2)`-based
  symlink swap. `check` / `build` / `switch` / `rollback` /
  `generations`.

Each crate is self-contained, targets `x86_64-unknown-linux-musl`
when shipped. Not yet wired into the initramfs — that integration is
the next session's work.

### Polish (from first interactive session)

- `bootstrap/pkgs/initramfs.nix`: replaced the hand-curated busybox
  symlink list with auto-enumeration via `busybox --list`. Now every
  applet busybox was compiled with (~400, including `whoami`, `date`,
  `dmesg`, `vi`, `wget`, ...) gets a symlink in `/bin`. Also created
  `/root` and `/etc` dirs preemptively for variant (a).
- `bootstrap/pkgs/initramfs.nix`: init script now runs `dmesg -n 1`
  early to silence late kernel info-level messages that were leaking
  into the shell prompt during the first interactive session.
- `bootstrap/flake.nix` (boot-vm app): added `quiet` to the kernel
  cmdline. Suppresses boot info-level messages from the console.
- `bootstrap/pkgs/initramfs.nix` (init): replaced `exec /bin/sh` with
  a respawn loop. Typing `exit` (or any accidental shell death) no
  longer kills PID 1 and panics the kernel. Banner now suggests
  `poweroff` as the in-VM exit command (busybox applet, signals the
  kernel; QEMU `-no-reboot` makes that translate into a clean QEMU
  exit), with `Ctrl-A x` as the host-side fallback.
- `bootstrap/pkgs/kernel.nix`: enabled `CONFIG_ACPI`, `CONFIG_ACPI_BUTTON`,
  `CONFIG_ACPI_PROCESSOR`, `CONFIG_PNP`, `CONFIG_PNPACPI`. Without
  ACPI, busybox `poweroff` puts the kernel in halt but QEMU keeps
  running (the VM appears frozen). With ACPI, `poweroff` issues S5
  power-off, QEMU detects it and exits cleanly. bzImage grew from
  1.5 MB → 1.7 MB.

## 2026-05-28

### Added

- Branch `lfs-bootstrap` created off `main`.
- `bootstrap/README.md` and `bootstrap/DESIGN.md` scaffolded
  (commit `6759b53`).
- **Phase 0 decisions locked** in `DESIGN.md` (commit `5cd9531`):
  - libc → musl
  - init → sh-based custom
  - Agent backend → pluggable (default Claude Code)
  - Build env → Nix cross-compile on host
  - VM image → initramfs + qcow2 `/var`
  - Kernel → vanilla Linux LTS, minimal `.config`
- `bootstrap/flake.nix` cross-compile dev shell (commit `5cd9531`).
- Graphical UI decision **deferred** until Phase 0-1 base is booting.
  Provisional preference: browser-as-desktop kiosk (option b) once
  revisited. Documented in `DESIGN.md` (commit `7081559`).
