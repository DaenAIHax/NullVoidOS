# Agent loop — in-VM agent ⇄ host operator

How the agent *inside* NullVoid and the operator *on the host* divide the work,
and how a "wall" gets escalated. This is the contract for the autonomous-build
sessions.

## Scope of the in-VM agent — self-sufficient for two things

1. **Language implementation.** Extend the Nullang compiler: builtins first,
   then (with the self-host net, below) language features. The self-hosted
   compiler is `system/nullang/examples/selfhost-parser.null` (lexer + parser +
   codegen, in Nullang); the Rust compiler under `system/nullang/src/` is the
   seed.
2. **Environment construction.** Author software in Nullang, package it
   (`nv-pkg`), declare it in the system manifest, and apply it (`nv-rebuild
   switch`) — the CONTRACTS.md loop. Build the system the agent wants to live
   in.

The agent **owns the in-VM iteration loop**: write → build → run → smoke-probe
→ keep or **roll back** on red. It does not need the host for anything inside
this loop.

## Verification gates (what makes an edit safe to keep)

- **Builtins / programs:** build + run + smoke-probe; `nv-rebuild` keeps the
  previous generation as the rollback floor.
- **Compiler / language changes:** the **self-host fixpoint** is the gate —
  `system/nullang/selfhost-bootstrap.sh` must still pass (stage0→stage1→stage2,
  emitted C byte-identical). A bad edit breaks the stage1→stage2 build instead
  of miscompiling silently. *If the edit can't pass the fixpoint, it's not kept.*

## The wall protocol

A **wall** is anything the agent cannot *safely* resolve inside the VM:

- a bug in the **Rust seed compiler** (parser/typer/codegen) blocking progress;
- a parser/typer change that breaks the build in a way smoke-probe can't recover;
- a **missing tool or source** in the image;
- anything needing **git, network egress, or a capability/perimeter change**
  (Via B: the VM has no GitHub access by design).

When the agent hits a wall it does **not** thrash. It:

1. appends a dated, structured entry to **`/var/WALLS.md`** (persists across
   reboots): what it tried, the exact error, the minimal repro, and what it
   believes the host must change;
2. records progress so far in `/var/STATE.md`;
3. stops on that thread and either continues other in-scope work or halts.

The operator pastes the wall to the **host** Claude, who fixes it host-side
(edit Rust `src/`, the flake, or the image), rebuilds (`nix run .#boot-vm`
re-bakes the initramfs), and the agent resumes. **Source-of-truth lives on the
host/repo; the VM proposes, the host disposes** — consistent with the Trust
model in DESIGN.md.

## Boundaries (do not fight these — they're walls by design)

- `/` is RAM-only; only **`/var`** persists. Keep durable work and notes in `/var`.
- No git, no push, no outbound credential from the VM (Via B).
- `~/.claude` is a RW 9P share from the host — do not write outside the agent's
  own task scope into it.

## Operator-side (host) responsibilities

- Fix walls in the Rust seed / flake / image; re-bake and hand back.
- Apply, commit (Via B: the VM can't), and push when appropriate.
- Keep the self-host fixpoint green after host-side compiler edits
  (`nix develop --command bash system/nullang/selfhost-bootstrap.sh`).
