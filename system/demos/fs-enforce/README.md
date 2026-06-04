# fs-enforce — runtime capability enforcement, `!fs.read` slice (Traccia A)

The second capability NullVoidOS enforces at runtime, after `!net`. Where `!net`
uses a network namespace, `!fs` uses **Landlock** — the kernel's path-scoped,
deny-by-default access control, which maps 1:1 onto the capability vocabulary
(`!fs.read."P"`, `!fs.write."P"`).

## The claim (falsifiable)

One binary, one package, declared as two services that differ only in their
granted filesystem capability:

| Service | `requires` | Result |
|---|---|---|
| `fs-granted` | `[ !net !fs.read."/srv" !tty ]` | reads `/srv/nv-canary` → exit 0 |
| `fs-denied`  | `[ !net !tty ]`                 | blocked by Landlock → exit 7 |

Both are granted `!net`, so neither is put in a network namespace — the only
variable is `!fs.read."/srv"`. Same code, opposite outcome.

## Mechanism

- **Kernel** (`pkgs/kernel.nix`): `CONFIG_SECURITY` + `CONFIG_SECURITY_LANDLOCK`
  + `CONFIG_LSM="landlock"`. tinyconfig ships `CONFIG_SECURITY` off, so Landlock
  must be turned on and added to the active LSM list or the syscalls ENOSYS.
- **Activation engine** (`system/nv-rebuild`): `nv-rebuild run` parses the
  `fs.read` / `fs.write` capabilities from the service descriptor, builds a
  Landlock ruleset and `restrict_self`s **before** spawning. The ruleset is
  inherited across fork+execve (and through the `unshare` trampoline used for
  `!net`), so both confinements compose on the final binary. Uses the
  `landlock` crate (0.4) — a new dependency; `cargoHash` recomputed.
- **Baseline**: deny-by-default would block a service from reading its *own*
  binary (execve resolves it under the ruleset), so the supervisor always
  grants a runtime baseline: read+execute on code/runtime trees (`/bin`,
  `/nix/store`, `/run`, `/var/lib/nv-store`, `/lib`, `/usr`, `/proc`) and
  read+write on `/dev` and `/tmp` (a read-only `/dev` breaks even a
  `2>/dev/null` redirect — the in-host smoke test caught exactly that). The
  capability vocabulary then protects *data* paths outside the baseline; the
  demo's canary lives in `/srv`, which is not baseline.

## Run it (inside the booted VM)

```sh
sh /path/to/fs-enforce-test.sh
```

Expect: `fs-granted` exits 0 (reads the canary), `fs-denied` exits 7 (Landlock
blocks the read), final `PASS`.

## Scope / honest limits

- Enforces `!fs.read` / `!fs.write` (subtree granularity, via `PathBeneath`).
- The runtime baseline is a coarse allowance of code/runtime trees; tightening
  it (e.g. only the specific store path of the service's own binary) is a
  follow-up. It deliberately errs toward "the service can run".
- `!proc.*` / `!rand` (seccomp) remain the next increment; the kernel already
  ships `SECCOMP`.
