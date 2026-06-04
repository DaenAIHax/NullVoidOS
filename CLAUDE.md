# Claude Code — Project Instructions

NullVoidOS — agent-primary operating system, LFS-bootstrap path. Research
alpha, not production. The demo goal is a bootable VM whose **primary user is
an AI agent**, not a human. The project source lives at the repo root
(`system/`, `pkgs/`, the Nix flake).

**Authoritative reading order:**

1. `DESIGN.md` — full architecture, phase plan, locked decisions
2. `CHANGELOG.md` — what was done, when
3. Memory files in `~/.claude/projects/-home-daenaihax-projects-nullvoid/memory/`

## Workflow

- **Branching.** Day-to-day work lands on `lfs-bootstrap`. Merge path:
  `lfs-bootstrap` → `develop` → `main`, via pull request. `main` is
  protected — never push to it directly even though the user is solo dev.
- **Commit author.** Must use the GitHub noreply email (the account has
  email privacy on). Already configured globally.
- **Commit signing / hooks.** Never skip hooks (`--no-verify`,
  `--no-gpg-sign`). If a hook fails, investigate the root cause.
- **CHANGELOG discipline.** Every meaningful change (code, design
  decision, architecture note) appends an entry to `CHANGELOG.md` with
  date and brief description. Trivial edits (typos, formatting-only)
  skip it.
- **Confirm before risky ops.** Destructive or shared-state operations
  (force-push, reset --hard, branch delete, PR merge, public push) need
  explicit confirmation each time. One prior authorization does not
  extend to future operations.

## Communication conventions

- **Language.** Italian, with technical terms and identifiers kept in
  their original form. Full orthographic correctness (accents, special
  characters) required.
- **Git literacy.** User is solo dev, not deeply familiar with git
  internals. Explain non-trivial operations (rebase, force-push, reset,
  branch surgery) before running them.
- **Mental model bridge.** User runs NixOS daily and knows the language.
  Anchor explanations to NixOS analogies when applicable. Skip beginner
  Nix introductions.
- **Triangulation.** User routinely pastes critiques from other AIs
  (Gemini, Opus) for comparison. Engage substantively. Admit when the
  other AI is more correct rather than defending position.
- **Hobby vs product framing.** Before deep-building any new direction,
  ask "hobby or product?" — user starts many big ideas; Cullis
  (`~/projects/agent-trust`) stays the primary commercial bet. Visions
  can be written down, not necessarily built.

## Phase awareness

The project sits at Phase 0 entry as of 2026-05-28. All locked
decisions are in `DESIGN.md`:

- **Phase 0 decisions (LOCKED 2026-05-28):** libc, init, agent backend,
  build env, image format, kernel.
- **Layer 3 language model (LOCKED 2026-05-28, re-locked to Nullang):**
  Nullang is the system description language — one language, two modes.
  No translator, no separate DSL.
- **Open design questions:** four pieces deferred to Phase 2 (module
  shape, composition semantics, SystemManifest schema, activation
  capability primitives).

## What this project is NOT

- Not a daily-driver OS — bootable VM is the demo goal.
- Not a NixOS replacement — different model entirely.
- Not Cullis (`~/projects/agent-trust`) — separate project, do not
  conflate.
- Not mio-kernel (`~/projects/mio-kernel`) — that one is an explicit
  hobby toy kernel in Rust.
- Not nixagent (`~/projects/nixagent`) — parked JSON→NixOS PoC,
  superseded.
