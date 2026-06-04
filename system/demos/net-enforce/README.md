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
- **Probe**: tests for a **default route** in the current netns via
  `/proc/net/route` (per-netns, like all of `/proc/self/net`). The host netns
  has a default route via `eth0` (DHCP); a fresh `unshare -n` netns has none.
  Two earlier probe attempts were wrong and the in-VM run caught both:
  `/sys/class/net` is tied to the netns that mounted sysfs (not the reader's),
  so it never reflected the isolation; and "any non-`lo` interface in
  `/proc/net/dev`" false-positived because a fresh netns auto-creates `sit0`
  (the IPv6-in-IPv4 tunnel) alongside `lo`. The route is the semantically
  correct, offline-safe signal of off-link connectivity.

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
