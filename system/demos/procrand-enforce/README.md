# procrand-enforce — `!proc.spawn` + `!rand` slice (Traccia A, seccomp-bpf)

The third and fourth capabilities NullVoidOS enforces at runtime, after `!net`
(netns) and `!fs` (Landlock). These use **seccomp-bpf**: a classic-BPF filter
that returns `EPERM` for the syscalls a missing capability should forbid.

## The claim (falsifiable)

One package, two compiled probes, four services differing in one capability:

| Service | `requires` | Probe | Result |
|---|---|---|---|
| `rand-granted`  | `[ !net !rand !tty ]`       | `getrandom()` | exit 0 |
| `rand-denied`   | `[ !net !tty ]`             | `getrandom()` | EPERM → exit 7 |
| `spawn-granted` | `[ !net !proc.spawn !tty ]` | `fork()`      | exit 0 |
| `spawn-denied`  | `[ !net !tty ]`             | `fork()`      | EPERM → exit 7 |

All granted `!net`, so none is put in a netns — the only variable is the
seccomp filter.

## Mechanism

- **Kernel**: `CONFIG_SECCOMP` + `CONFIG_SECCOMP_FILTER`, already enabled with
  the `!net` slice — no kernel rebuild needed.
- **Activation engine** (`system/nv-rebuild`): `nv-rebuild run` builds a cBPF
  program (allow-all, with `EPERM` for the denied syscalls) and installs it in
  the child via `pre_exec` — `prctl(PR_SET_NO_NEW_PRIVS)` then
  `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, …)`. Installed post-fork /
  pre-execve, so the launch `execve` is never blocked, and `unshare(2)` (a
  distinct syscall from the clone family) still works for the `!net`
  trampoline. The filter is hand-rolled with `libc` — and `libc` is reached via
  `nix::libc` (already a dependency) so **no new crate** is pulled, which kept
  this buildable while crates.io was flaky.
  - missing `!proc.spawn` → deny `fork`/`vfork`/`clone`/`clone3`
  - missing `!rand` → deny `getrandom`

## Scope / honest limits

- **`!proc.exec` is NOT enforced here.** Stateless cBPF cannot allow only the
  launch `execve` while blocking subsequent ones — that needs seccomp
  `USER_NOTIF` or a ptrace supervisor (a larger architectural piece). Denying
  `!proc.spawn` already blocks the fork+exec helper-spawning pattern, which is
  the practical threat; a service could still `execve`-morph itself, which is
  benign. The audit line prints `exec=denied` honestly but the filter does not
  act on it.
- The arch guard kills on a non-x86_64 syscall ABI (defensive); the image is
  x86_64-only anyway.

## Run it (inside the booted VM)

```sh
sh /path/to/procrand-enforce-test.sh
```

Expect `rand-granted`/`spawn-granted` = 0, `rand-denied`/`spawn-denied` = 7,
final `PASS`.
