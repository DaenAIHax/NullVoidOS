# net-enforce — runtime capability enforcement, `!net` slice (Traccia A)

The first capability NullVoidOS enforces *at runtime*, not just records.

Until now the capability vocabulary (`!net`, `!fs.read."…"`, `!tty`, …) was
**recorded-only**: `system.null` granted capabilities, packages declared the
ones they consume, and `null` type-checked `requires ⊆ caps` — but nothing
stopped a process from doing what it never declared. This slice closes that
gap for `!net`.

## The claim (falsifiable)

One binary, one package, declared as **two services that differ only in their
granted capabilities**:

| Service | `requires` | Result |
|---|---|---|
| `net-granted` | `[ !net !tty ]` | stays in the host network namespace → has a network |
| `net-denied`  | `[ !tty ]`     | launched in a fresh, empty netns → loopback only |

`nv-rebuild run <service>` reads the granted set from the active generation's
descriptor and confines the process accordingly. The probe reports whether it
can see a non-loopback interface and exits `0` (reachable) or `7` (isolated).
**Same code, opposite outcome — decided solely by the declared capability.**

## Mechanism

- **Kernel** (`pkgs/kernel.nix`): `NET_NS` (+ the rest of the namespace family
  and `SECCOMP`) enabled. tinyconfig shipped none of these — `CONFIG_NAMESPACES
  depends on MULTIUSER`, which tinyconfig disables, so the deps are enabled
  first or olddefconfig silently drops the whole block.
- **Activation engine** (`system/nv-rebuild`): `null eval`'s per-service
  `requires` is now deserialized, persisted into the generation descriptor
  (`etc/services/<name>`, line `requires=<tokens>`), and consumed by the new
  `nv-rebuild run <service>` command. No `!net` capability → the service is
  launched via `unshare -n` (busybox applet); otherwise it stays in the host
  netns.
- **Probe**: `/proc/net/dev` is per-netns (it resolves through `/proc/self/net`
  to the reader's network namespace), so it reflects the `unshare -n`
  isolation without remounting anything and without needing the network to be
  up — offline-safe. `/sys/class/net` does **not** (it is tied to the netns
  that mounted sysfs) — an early version of the probe used it and silently
  reported "reachable" inside an isolated netns.

## Run it (inside the booted VM, as root)

```sh
sh /path/to/net-enforce-test.sh
```

Expect: `net-granted` exits 0 (REACHABLE), `net-denied` exits 7 (isolated),
final `PASS`.

## Scope / honest limits

- Enforces `!net` only. `!fs.*` (Landlock), `!proc.*` / `!rand` (seccomp) are
  the next increments; the kernel already has the primitives compiled in.
- `!net.localhost` (loopback-only: isolated netns with `lo` brought up) is
  treated as full `!net` for now — documented in `Capability::grants_net`.
- Supervision currently rides in `nv-rebuild run` (one-shot, manual). A real
  boot-time supervisor with restart policies is a separate piece — likely a
  dedicated `nv-init`. `nv-rebuild`'s contract is the activation engine; this
  is a deliberate, documented overlap for the slice.
