# NullVoidOS — Agent-Primary OS (Bootstrap)

[![self-host fixpoint](https://github.com/DaenAIHax/NullVoidOS/actions/workflows/selfhost.yml/badge.svg)](https://github.com/DaenAIHax/NullVoidOS/actions/workflows/selfhost.yml)

> Experimental research alpha. Not production. The demo goal is a bootable VM
> whose **primary user is an AI agent**, not a human.

The question this repo asks: *what does an operating system look like when an
autonomous agent is the primary user from boot moment 0?* — and what has to be
true for that to be safe.

## The trust loop

The thesis is a closed loop: **autonomy ⊗ enforcement ⊗ audit.**

1. **Autonomy is real.** Inside the VM an agent extended its own toolchain:
   it self-hosted a compiler to a byte-identical fixpoint, and authored its
   own system services. Not a scripted demo — an agent modifying the system
   it runs on.
2. **Every action runs behind a kernel capability perimeter.** Services
   declare what they need (`!net`, `!fs.read`, `!proc.spawn`, `!rand`); the
   supervisor enforces exactly that set at runtime via network namespaces,
   Landlock, and seccomp. The *declared* capability set **is** the *enforced*
   set — and that is falsifiable (see below).
3. **Audit is the next layer** — and it is exactly why
   [Cullis](https://github.com/DaenAIHax) exists. Autonomy without a trust
   perimeter is a liability; the perimeter without an inspectable audit trail
   is only half the loop. The kernel experiment and the product are the same
   idea at different altitudes.

## Reproduce it yourself (no trust required)

Two commands, two acts. Both are host-side and run to a hard pass/fail gate.
Requirements: Nix with flakes; `/dev/kvm` recommended for the VM (falls back
to slow TCG).

**Act 1 — autonomy.** Certify the self-hosting compiler reaches a
byte-identical fixpoint (the Rust seed becomes removable):

```sh
nix develop --command bash system/nullang/selfhost-bootstrap.sh
# → PUNTO FISSO RAGGIUNTO — self0.c == self1.c == self2.c
```

**Act 2 — enforcement.** Boot a headless VM that runs the capability tests and
powers off with a machine-readable verdict. It mounts **nothing** from your
host (no `~/.claude`, no `~/.ssh`):

```sh
nix run .#verify-capabilities
# → NVTEST-VERDICT: PASS (3/3) — net=PASS fs=PASS procrand=PASS
```

Each slice ships the *same* binary as two services differing only in their
declared capabilities; one reaches the resource, the other is confined. That
is the test that enforcement is real, not advisory.

## What's real today vs. what's next

| Piece | Status |
|---|---|
| Self-hosting compiler (byte-identical fixpoint) | ✅ reproducible — `selfhost-bootstrap.sh` |
| Runtime capability enforcement (netns / Landlock / seccomp) | ✅ reproducible — `nix run .#verify-capabilities` |
| Declarative system loop (`system.null` → packages → activation) | ✅ working (Phase 1) |
| **Dynamic, inspectable audit trail of effects → Cullis** | ⏳ **next layer** — today the supervisor announces capabilities pre-launch; a structured effect log is the MVP that closes the loop |

We don't claim the audit layer is built. Naming the hard part precisely is the
point: it's the bridge from this kernel experiment to the product.

## Read next

[`DESIGN.md`](./DESIGN.md) — full thesis, architecture, phase plan, and the
locked decisions. [`CHANGELOG.md`](./CHANGELOG.md) — what was built, when.
