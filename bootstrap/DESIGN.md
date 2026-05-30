# NullVoidOS Bootstrap — Design Document

> Experimental research alpha. Not production-targeted.

## What this is

An LFS-bootstrap path toward an operating system designed from boot moment 0
around the assumption that the primary user is an AI agent, not a human at a
terminal.

This is exploratory work. The goal is a demonstrable prototype, not a
shippable distribution.

## Thesis

Every general-purpose OS in use today inherited the assumption that the
primary user is a human logging in via terminal — a 1970s timeshare model.
Containers, sandboxing, security-hardened distros, "AI assistants on the
desktop" are all patches above that base assumption.

In the AI era this assumption is wrong. Increasingly, the entity *driving*
computation is an agent, with humans supervising at a higher level. The OS
that fits this world has four architectural primitives baked in, not added:

1. **Capability** as the authorization model — not UID/groups, which encode
   "human identity logged in via shell"
2. **Audit** as structured machine-readable trace — not freeform text logs
   meant for humans to grep
3. **Provenance** for every artifact — who or what created it, from which
   inputs, with which model, under which capability grants
4. **CAS** (content-addressable storage) — identity by content hash, so the
   agent reasons about artifacts the way it composes (not by filesystem
   location)

None of the four are individually novel. The bet is that combining all four
**as the OS's native primitives**, with the agent as primary user from the
boot moment, produces a coherent system nobody has built yet.

## Why LFS-bootstrap (vs NixOS-based)

Two paths were considered.

**NixOS-based:** Use NixOS as substrate, build the agent layer above as Nix
modules. Time to bootable agent layer: 2-4 hours. Inherits NixOS's
declarative model, atomic generations, rollback, store.

**LFS-bootstrap:** Build the minimum (kernel + libc + tiny userland + Zero +
LLM runtime) by hand. From the boot moment, the AI takes over and builds
the rest of the upper stack from within the system itself. Time to minimum
bootable: 5-7 focused days.

The LFS path was chosen because:

- **Demo narrative.** *"We gave the AI a kernel, the Zero compiler, and a
  local LLM. From boot, it built every layer above."* This is memorable and
  precedent-free. The NixOS-based version reads as "we configured NixOS to
  run an agent layer."
- **Thesis fidelity.** AI is primary user from boot moment 0, not from
  "after NixOS finishes setting up the system."
- **Self-hosting pattern.** Precedents exist (Lisp machines, Smalltalk
  image, self-hosting compilers). The pattern works; nobody has applied it
  to AI-as-primary-user.

The cost is 5-7 days of low-level work before the AI can take over. The
user has prior toy-kernel experience (mio-kernel boots Hello World in
QEMU), so this is within reach.

## Architecture

```
┌──────────────────────────────────────────────────┐
│ Layer 4: Apps                                    │
│   distributed as (prompt + spec + capabilities)  │
│   AI rebuilds locally per substrate              │
├──────────────────────────────────────────────────┤
│ Layer 3: .null + activation engine               │
│   declarative system state in .null DSL          │
│   (agent-native, Nix-shaped, eval-only)          │
│   nv-rebuild renders generations + atomic swap   │
├──────────────────────────────────────────────────┤
│ Layer 2: Agent backend (pluggable)               │
│   agent_backend interface: send(prompt, caps)    │
│   default impl: ClaudeCodeBackend (hosted)       │
│   alt impls: OllamaBackend, LlamaCppBackend,     │
│              AnthropicAPIBackend                  │
│   Capability sandbox for AI-generated code       │
├──────────────────────────────────────────────────┤
│ Layer 1: Substrate (~30 curated packages)        │
│   C libraries (crypto, codec, sqlite, etc.)      │
│   wrapped with capability-typed Zero APIs        │
│   AI never sees C directly                       │
├──────────────────────────────────────────────────┤
│ Layer 0: Minimum bootstrap                       │
│   Linux kernel + musl libc + busybox             │
│   ZeroLang compiler + llama.cpp runtime          │
│   sh-based init that launches the agent          │
└──────────────────────────────────────────────────┘
```

Layers 0-1 are built by the human (Phase 0-1). Layers 2-4 are built by the
agent from inside the running system.

## Distribution model — prompt as package

Apps are not distributed as compiled binaries. The distributable unit is:

```
mrblunder.app/
├── intent.prompt        # what the app does, structured NL
├── spec.test            # behavioral specification the impl must pass
├── capabilities.cap     # which substrate primitives the app needs
├── substrate.dep        # minimum substrate version required
└── provenance.sig       # author, signature, hash of the prompt
```

When user X receives `mrblunder.app`:

1. X's agent reads `intent.prompt`
2. Agent generates implementation matching X's substrate
3. Generated code is executed against `spec.test` to validate behavior
4. App runs sandboxed per `capabilities.cap`
5. Provenance is recorded: prompt hash + model version + capability grants

Two users' implementations of `mrblunder.app` are **behaviorally equivalent
but textually distinct**. Reproducibility is behavior-exact (via tests),
not bit-exact (via binaries).

This inverts how shared infrastructure scales. Instead of "package binary
N times for N users," the AI generates per-user. Updates are prompt
updates; the AI rebuilds.

## Language choices — Zero (programs) + `.null` (system declaration)

> **Superseded 2026-05-28 (later) by the Nullang re-lock** — see the
> RE-LOCK box in [the Layer 3 section](#layer-3-language-model--null-agent-native-config-dsl-revised-2026-05-28).
> Both roles below are now one language, Nullang, in two modes; ZeroLang
> is the *source of ideas*, not an external dependency. The section is
> kept for history.

Two agent-native languages, two roles, no overlap.

### ZeroLang — implementation language (layers 1-2, layer 4 apps)

ZeroLang (Vercel Labs, currently v0.1.4, Apache 2.0, May 2026) is the
implementation language for: substrate wrappers (Layer 1), agent
backend internals and activation engine when rewritten in Zero
(Layer 2), and agent-authored applications (Layer 4).

**Why Zero for programs:**

- Capability-based I/O native (matches thesis primitive #1)
- JSON-structured compiler diagnostics with stable error codes and
  typed repair plans (`zero fix --plan --json`)
- Version-matched skill bundles (`zero skills list`) — the compiler
  ships its own docs, so agents without training data can learn the
  language from the binary alone
- Designed for AI agents as consumers from day one
- Compiles to small native musl binaries (~10KB target range)
- Apache 2.0, forkable if vendor abandons

**Vendor risk handling:**

- For research alpha, vendor risk is acceptable
- If Vercel pivots or kills Zero: fork (Apache 2.0) or migrate to
  other emerging AI-native languages (Roc, Unison, future entrants)
- **No fallback to Rust/Go for the program layer** — using
  human-designed languages for agent-authored programs would
  contradict the thesis. Phase 1 tooling (`nv-pkg`, `nv-rebuild`)
  is currently Rust as a pragmatic bootstrap; rewrite-in-Zero is a
  later question once the substrate is solid.

### `.null` — system description language (layer 3)

`.null` is the declarative DSL for `system.null` files. It does **not**
overlap with Zero's role: Zero builds programs, `.null` describes the
system that runs them. See the [Layer 3 section](#layer-3-language-model--null-agent-native-config-dsl-revised-2026-05-28)
below for the full design and [SPEC.md](system/null/SPEC.md) for the
authoritative spec.

The same agent-first recipe (JSON diagnostics, repair IDs, embedded
skills bundle, single-form-per-concept, capability-explicit syntax)
applies independently to `.null`, transposed to the configuration
domain.

## Layer 3 language model — `.null` (agent-native config DSL) (REVISED 2026-05-28)

> **History.** This section was first locked on 2026-05-28 as
> *"Zero native, no translator"* — the Layer 3 DSL would be ZeroLang
> itself. The same day, after surveying ZeroLang's actual surface
> (`mut`, `set`, `World`, generics, ownership, native codegen — a
> systems language in the Rust/Zig family), it became clear that
> forcing it into the system-declaration role is a category error.
> Zero is for *building software*; a system manifest is *data*. The
> decision is revised below. Authoritative spec:
> [`bootstrap/system/null/SPEC.md`](system/null/SPEC.md).

> **RE-LOCK (2026-05-28, later — supersedes everything below in this
> section).** Layer 3 is now **Nullang declaration mode**, not an
> isolated `.null` plus an external Zero. Decision and rationale:
>
> - **One language, two modes.** Nullang is a single agent-native
>   language. *Declaration mode* (eval-only, no functions/effects) is
>   exactly the `.null` profile described below — it does not change.
>   *Construction mode* (functions, effects, native codegen) replaces
>   ZeroLang for Layers 1-2 and Layer 4 apps, incrementally. The
>   `.null`-vs-Zero split below was correct as a *local* call but missed
>   the level above: both roles are the same language in two modes, and
>   the seam is the capability vocabulary (declaration *grants*;
>   construction *consumes* via `World`).
> - **Sovereignty.** ZeroLang is Vercel Labs' — an external project whose
>   death would force rewriting Layers 1-4. NullVoidOS cannot stand on
>   that. Owning the spec + compiler is non-negotiable *now*;
>   self-sufficiency of the ecosystem (stdlib, TLS, …) is a separate,
>   incremental, years-long goal. The two must not be conflated.
> - **Codegen to C.** The substrate already ships a C compiler, so a C
>   backend adds no new external dependency that can die — unlike
>   LLVM/Cranelift. Floor = kernel + libc + cc.
> - **Status (verified 2026-05-28).** Nullang v0.1 exists at
>   [`bootstrap/system/nullang/SPEC.md`](system/nullang/SPEC.md): the
>   closed loop `source → C → cc → ELF → run` is green, with functions,
>   the capability/effect discipline, arithmetic/`if`, and
>   `enum`/`match`; 12 integration tests pass. Deferred: CAS+provenance
>   wiring, `mut`/ownership, generics, self-hosting.
>
> The text below this box documents the now-superseded two-language
> model; it is kept for history and because Nullang's *declaration mode*
> is precisely the `.null` design it describes.

The Layer 3 DSL is `.null` — a small, declarative, Nix-shaped
configuration language. It is **eval-only**: no functions, no `let`,
no control flow, no runtime, no IO. A `system.null` file evaluates to
a typed `SystemManifest` JSON value, which `nv-rebuild` (Gamma)
renders into generations and atomically activates.

`.null` is **not** Zero, and not a fork of Zero. It is a separate,
deliberately tiny language that **inherits ZeroLang's agent-first
tooling recipe** transposed to the configuration domain:

| Zero (systems language) | `.null` (config language) |
|---|---|
| Stable JSON diagnostics + repair IDs | Stable JSON diagnostics + repair IDs |
| `zero explain CODE` from embedded docs | `null explain CODE` from embedded docs |
| `zero skills list` — version-matched skill bundles | `null skills list` — version-matched skill bundles |
| `World` capability + effect annotations | Capabilities as values in syntax (`!net`, `!fs.read."/etc"`); system grants, services require |
| Small surface, one way per concept | Small surface, one way per concept |
| Token-efficient, fast startup | Token-efficient, fast eval |

**Why two languages and not one:**

- A declaration is categorically different from a program. Programs
  have control flow, mutation, IO, generics. Declarations are values.
  One language for both is *one language doing two jobs poorly* —
  exactly the trap NixOS avoids by separating Nix-the-evaluator from
  the C++/Rust modules it activates.
- Both can be agent-native at once. The recipe (typed JSON
  diagnostics, repair IDs, embedded skills bundle, capability-explicit
  syntax, single-form-per-concept) applies independently to each.
- Narrative integrity is preserved differently than the original
  framing claimed: *"the agent builds programs in Zero and declares
  the system in `.null` — and both languages publish their docs and
  repair plans the same way."* This is in fact stronger than
  "everything in Zero", because it admits the categorical difference
  the agent will encounter regardless.

The cost is a second toolchain (lexer + parser + typechecker + CLI
for `.null`) — minor, since the surface is deliberately tiny
(see SPEC.md §2 for the anti-feature list).

### Mental model — how the layers relate (NixOS analogy)

The user already runs NixOS daily. The mechanics here are identical
in shape — only the languages and where the type-checking happens
change.

| NixOS construct | NullVoidOS equivalent |
|---|---|
| Nix the language | `.null` the language |
| Module system (`mkOption`, runtime types, merges) | Fixed `SystemManifest` schema in `.null`'s compiler — no runtime module layer |
| `imports = [ ./foo.nix ];` | Deferred to `.null` v2.1 (see SPEC §12) |
| Evaluation → attribute set | Evaluation → typed `SystemManifest` JSON |
| Nix derivation (sandboxed build → hashed output) | LFS-style sandboxed build → CAS artifact (built using Zero) |
| `/nix/store` | CAS substrate (content-addressed) |
| System derivation → `/etc`, units, kernel | `nv-rebuild` activation engine, reads manifest, materializes outputs |
| `nixos-rebuild switch` | `nv-rebuild switch` (capability-gated by `!activate.system`) |

Implementation languages remain Zero for layers 1-2 (substrate
wrappers, services, activation engine internals if rewritten) and for
agent-authored applications. `nv-rebuild` is currently a Rust crate
per Phase 1 contracts; rewriting it in Zero is a later question.

The imperative side never disappears — building openssl from source is
imperative work. It is wrapped in a sandboxed CAS-producing build (the
LFS-bootstrap analogue of a Nix derivation). The declarative side
references those built artifacts by hash.

### Substrate ↔ Zero boundary (how Linux talks to Zero)

For each C package in the substrate there is a Zero wrapper module:

```
substrate/openssl.zero       ← typed Zero API, capability-annotated
        │
        │  FFI boundary
        ▼
libcrypto.so                 ← unsafe raw C, never exposed above
```

Wrapper functions declare their capability requirements in the type
signature (e.g. `fn aes_encrypt(...) requires cap[crypto]`). The Zero
compiler rejects calls without the required capability. The C layer is
unaware of capabilities — enforcement is purely at the Zero boundary.

Kernel syscalls follow the same pattern. A `substrate/syscall.zero`
module wraps the relevant syscalls behind capability types:
`cap[net]` gates `socket()`, `cap[fs:/path]` gates `open()` on a
specific path, and so on.

## Trust model & sandboxing (agent-primary ≠ agent-trusted)

The design assumes a *trusted* agent. That assumption does not survive
contact with reality, and "trusted" is not even a stable state: the
moment the agent processes untrusted input (a web page, a file with
injected instructions), it becomes a confused deputy — honest but
steered. So "untrusted agent" is the **default**, not a rare adversarial
case. This section states where the real boundary is.

**The language capability discipline is audit, not security.** Wrapper
signatures like `fn aes_encrypt(...) requires cap[crypto]` and the
compiler rejecting un-capability'd calls only bind code that *goes
through* the language. A malicious or steered agent is not obligated to
use Zero/Nullang — it has a shell, can write raw C, can call `socket()`/
`open()`/`ptrace()` directly. Capability-in-the-type is the Nix analogue
of writing the sandbox *inside* the Nix expression: it constrains nobody
who declines to write that expression. It buys ergonomics and a
provenance trail. It is not a containment boundary.

**Two distinct subjects, two different stories:**

1. **Generated apps** — *sandboxed today.* Capabilities declared in
   `.null` are enforced at runtime by the kernel (Traccia A, alive in
   VM): `!net` via network namespaces, `!fs` via Landlock, `!proc.spawn`
   + `!rand` via seccomp. Here the model holds — the app cannot escape
   what the kernel grants it.
2. **The agent itself** — *not sandboxed.* In an agent-primary OS the
   agent *is* the system author: by construction it holds the activation
   capabilities (`nv-rebuild switch`). You cannot have, at 100%, both
   "agent-primary" and "untrusted agent" — that is a genuine tension, not
   a bug to fix. You choose a point on the spectrum.

**The only real boundary is the kernel / hypervisor.** Two ways to
position the agent against it:

- *In-guest confinement* — run the agent under the same seccomp/Landlock/
  netns it imposes on apps, strip `!activate.system`, and split off a
  small **trusted activation gate** (`nv-rebuild`, the 7 activation
  primitives) as the TCB. The agent *proposes* (`.null` manifests,
  `.nvpkg` builds); the gate *disposes* (optionally human-gated). Agent
  runs unprivileged. Cost: the fully-autonomous agent-primary dream
  breaks into "agent designs via a mediated proposal channel."
- *Perimeter-as-jail* — the cleaner model for a research alpha: don't
  confine the agent inside; treat the **VM/hypervisor as the prison**.
  Agent is god inside; blast radius is the box. TCB shrinks to QEMU/KVM
  (small, audited VM-escape surface). This is *less* work than in-guest
  seccomp — but only holds under three conditions, two of which our
  current `boot-vm` violates:

  1. **The brain is the network.** A Claude-backed agent thinks via
     `api.anthropic.com`, which lives *outside*. "Cut the internet" does
     not sandbox the agent — it lobotomises it. The honest forms are
     either a **local model** (llama.cpp in substrate) for true air-gap,
     or **controlled egress**: exactly one mediated, logged hole to the
     model endpoint and nothing else. Not "no network" — "single
     surveilled exit."
  2. **The perimeter must be clean.** Air-gap is meaningless if a host
     directory is mounted RW into the guest: that is a filesystem door
     back home, independent of the network. *"Inside" is not inside if a
     directory is shared RW with the outside.* No passthrough of host
     secrets; RO and minimal where unavoidable.
  3. **Outputs stay trusted afterward.** The perimeter protects the host
     *during* the run, not the **artifacts** that leave it — the `.null`
     manifests and the built image, which you later boot on real
     hardware *outside* the sandbox. Hence **provenance** (prompt hash +
     model version + capability grants) is non-optional even in the
     air-gap model: it is what lets you trust what came out.

**Current status & known sharp edge.** Phase 0 (a) `boot-vm` violates
condition 1 (user-mode NAT gives the guest general network, not a single
egress) and condition 2 (the host's `~/.claude` is mounted **RW** via
9P `claudefs` — a god-inside agent can read host credentials *and* write
back into the host's Claude config: inject MCP servers/hooks that then
run on the **host**). This is accepted for a single-user research alpha
but is the first thing to tighten before any multi-tenant or untrusted
use: narrow the share (RO credentials + separate writable scratch) and
replace NAT with a whitelisted egress proxy.

## Horizon — "everything in the VM" (vision, not a current task)

The natural end-state of an agent-primary OS is that the agent does *all* of
its own work inside the VM, with the host out of the loop entirely. We are
deliberately **not** building this yet (research alpha; the bootable VM demo is
the goal, see Phase plan). Recorded here so the direction is fixed and the
preconditions are explicit — visions get written down, not necessarily built.

**Where we already are (2026-05-30).** The compiler's *evolution* already
happens in the VM: the self-improvement loop edits Nullang's source in `/var`,
`cargo build`s it, packages it as `nv-toolchain`, hot-swaps via `nv-rebuild
switch`, smoke-probes, rolls back on red. The in-VM agent has authored builtins
unaided (`char_at`, then `index_of` + `split` — the latter the first builtin to
*produce* a `List`). So "language work happens in the VM" is largely already
true.

**What still lives on the host, and why each is load-bearing — not laziness:**

1. **Git integration (commit + push).** The VM has *no* GitHub access by
   design (Via B: private repo, host SSH key not authorised, no PAT inside).
   The agent pastes a diff; the host applies, commits, pushes. Moving this
   into the VM means putting a write credential to the repo in the hands of an
   **unsandboxed, potentially prompt-steered agent** — which directly
   contradicts the Trust model section above. This is the single hardest line
   to cross and the last one that should move.
2. **Parser/typer surgery.** Work outside the builtin boundary (`List`,
   `struct`, `mut`/`while`) is total-blast-radius; `BUILTINS_CONTRACT.md`
   keeps the agent to builtins precisely because a broken parser is not
   smoke-probe-recoverable the way a broken builtin is. Lifting this is the
   real **self-hosting** step (§Thesis, "self-hosting pattern"): it requires
   either a much stronger in-VM verification net than the current smoke-probe,
   or the compiler rewritten in Nullang (so a bad edit fails to compile rather
   than miscompiling silently).
3. **The cooked `/bin/nullang` floor.** Rebuilt by the host; it is the
   rollback floor when a generation is bad. If the VM becomes the source of
   truth, the floor and the "truth lives on origin/host" safety both weaken
   (note `/` is RAM-only; only `/var` persists).

**Preconditions before any of this is worth building** (all gating, none met):

- The Trust-model sharp edges closed first (RO credentials, egress proxy) —
  you cannot widen the agent's authority while the perimeter still leaks.
- A verification net strong enough to let the agent touch parser/typer:
  realistically, **self-hosting** the compiler in Nullang, so the bootstrap
  build itself is the check.
- A mediated, audited path for work to leave the VM that is *not* a raw git
  credential — e.g. a proposal channel the host (or a human gate) approves,
  consistent with "agent proposes, trusted gate disposes."

**Order, if/when pursued:** self-hosting the compiler (removes reason #2 and
shrinks the host floor) → in-guest confinement + clean perimeter (makes the
agent safe to widen) → a mediated egress for artifacts (removes reason #1
without handing over a repo credential). Git-in-the-VM is the *last* step, not
the first — and may never be the right trade for a single-user research alpha.

## Open design questions (Layer 3, to resolve in Phase 2)

These are deliberately deferred until the bootstrap is alive and the
agent can participate in the design. (Several have first answers in
`.null` SPEC §12 — they remain open as larger architectural
questions.)

1. **Composition / imports.** `.null` v2.0 is single-file; SPEC §12
   reserves `import "./other.null"` for v2.1. The bigger question is
   whether composition is just file concatenation, attrset merging
   with a defined combinator, or something module-shaped (named units
   with explicit interfaces).
2. **Shape of a module.** If `.null` grows beyond single-file, what
   *is* a module? A named attrset literal, a typed record interface,
   or a function from `SystemContext` to a partial manifest? Resolving
   this answers what reuse looks like in `.null`.
3. **Variants beyond `SystemManifest`.** Should `.null` also describe
   user-level artifacts (`.nvpkg` recipes, app manifests)? Either the
   evaluator becomes schema-pluggable or each artifact gets its own
   tiny tool. SPEC §12 q2 flags this.
4. **Activation capability primitives.** Minimum set `nv-rebuild`
   needs: `write_file`, `start_unit`, `mount`, `symlink`, `chmod`,
   `chown`, `link_into_store`. Each gated by a distinct capability
   in `.null`'s capability vocabulary (SPEC §5.5 has the user-facing
   set; the activation-host set is separate and currently fixed in
   `nv-rebuild`).

None of these block Phase 0 (bootable VM) or Phase 1 (substrate
selection + `.null` v2 implementation). They become central in Phase
2-3 when the agent designs Layer 3 from inside the running system.

## What the substrate covers (the irreducible C layer)

The agent does not regenerate cryptography, codecs, kernel drivers, or SQL
engines. These took decades of expert work and AI cannot reproduce them
with confidence. The substrate is the small set of C libraries that fit
this profile, wrapped behind Zero capability-typed APIs.

Tentative substrate (~30 packages — to be finalized in Phase 1):

- **Crypto:** openssl, libsodium
- **Codec:** ffmpeg
- **Storage:** sqlite
- **Network:** curl
- **LLM runtime:** llama.cpp (or alternative)
- **Filesystem:** existing kernel FS via libc
- **Process management:** systemd or runit
- **JS engine:** V8 (if browser-class apps in scope)
- **TLS:** rustls or openssl
- (final list TBD)

The agent calls these only through Zero wrappers that declare capability
requirements explicitly. C is hidden infrastructure — invisible to the
agent the way microcode is invisible to Rust.

## Phase plan

| Phase | Owner | Deliverable | Duration |
|-------|-------|-------------|----------|
| **0** | Human | Bootable VM: kernel + musl + busybox + Zero + LLM + agent loop alive | 5-7 focused days |
| **1** | Human, AI-assisted | Substrate package selection + Zero capability wrappers | 3-5 days |
| **2** | Agent in-system | Agent runtime, LLM client, sandbox builder | 1-2 weeks |
| **3** | Agent in-system | DSL parser/evaluator + activation engine | 2-3 weeks |
| **4** | Agent in-system | First app end-to-end (prompt → built → sandboxed → running) | 2-3 days |

Total to first demonstrable wow: **6-10 weeks of focused work** with
serious AI assistance throughout.

## What's NOT in scope

- **Production readiness.** No SLA, no security warranty, no stability
  guarantees. Alpha research.
- **Replacing nixpkgs / apt / homebrew.** The substrate is intentionally
  small; the AI generates everything above it per-user.
- **Daily-driver OS.** Booting in a VM is the goal. Bare metal much later,
  maybe never.
- **Replacing the main branch direction.** The Fedora Atomic + container
  cybersecurity workbench in `main` is a separate concern with its own
  scope. The two directions coexist; the user decides over time.

## Existing landscape (what this is not)

| Project | What it actually is |
|---------|---------------------|
| ZeroLang (Vercel Labs) | Language. We *use* it; we are not Vercel. |
| AIOS / Qualixar OS | Academic agent orchestrators at application layer on top of existing OSes. |
| rabbit OS | Failed AI hardware device with custom UI; not an OS rewrite. |
| Anthropic Claude Cowork | SaaS product, not an installable OS. |
| Open Interpreter, Agent-S | Frameworks running on existing OS. |
| Microsoft Copilot+ PCs / Apple AI / Google Gemini | AI features added to existing consumer OSes. |
| NixOS, Guix | Declarative OS, but human-primary, not agent-primary. |
| Genode, seL4, Fuchsia | Capability-based OSes but not designed for AI as primary user. |

No project combines: bootable Linux + LFS-bootstrap + capability+audit+
provenance+CAS as primitives + agent as primary user + prompt-as-distribution.

The combination is the bet.

## Phase 0 decisions (LOCKED 2026-05-28)

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| 1 | libc | **musl** | Static linking, ~5MB, matches Zero target, Alpine-proven |
| 2 | Init | **sh-based custom** | Smallest, easiest to modify, no service supervision needed |
| 3 | Agent backend | **Pluggable** (default Claude Code) | Try multiple models; ClaudeCode/Ollama/LlamaCpp/AnthropicAPI swappable via config |
| 4 | Build env | **Nix cross-compile on host** | Reproducible, integrates with existing flake, no host pollution |
| 5 | VM image | **initramfs + qcow2 /var** | Fast boot from RAM + persistence for agent memory/generated apps |
| 6 | Kernel | **Vanilla Linux LTS, minimal .config** | Stable, small attack surface, predictable behavior |

These are revisable but lock the starting position. Override later via
config or by replacing the relevant subsystem.

### Phase 0 (a) — documented deviation: glibc in the bootstrap initramfs

The default `ClaudeCodeBackend` requires the upstream `claude-code` Node
binary, which nixpkgs ships as a wrapper hard-linked against
`/nix/store/.../glibc/lib/ld-linux-x86-64.so.2`. The Phase 0 (a) image
therefore ships the **entire `claude-code` Nix closure** (~30 store paths,
~100 MB compressed) under `/nix/store` inside the initramfs, alongside the
static-musl busybox + Zero binary.

This is an intentional shortcut, not the steady-state design:

- It contradicts decision #1 (musl-only) for one specific binary, in
  exchange for skipping a fragile cross-compile of Node + `npm install`
  in the bootstrap toolchain.
- It uses the real `/nix/store` as the CAS substrate (see the NixOS
  analogy table above: `/nix/store` ↔ CAS substrate). Phase 1+ replaces
  this with the LFS-style sandboxed-build CAS the design calls for.
- Other backends (`OllamaBackend`, `LlamaCppBackend`, `LlamaCppBackend`)
  remain pure-musl candidates when we wire them later.

Revisit when (a) Claude Code ships a standalone musl binary, or (b) the
Phase 1 CAS is online and any binary — glibc or not — is a hashed
artifact addressed the same way.

## Agent backend abstraction

The agent runtime layer exposes a minimal interface so any LLM-driving
agent can be swapped in:

```
agent_backend.send(prompt: str, capabilities: CapSet) -> Response
```

Initial implementations:

| Backend | Description | Network | API key | Cost |
|---------|-------------|---------|---------|------|
| `ClaudeCodeBackend` | Invokes `claude` CLI from substrate | Required | Required | Per-token |
| `OllamaBackend` | REST to localhost:11434 | Local | None | Compute |
| `LlamaCppBackend` | In-process llama.cpp invocation | Local | None | Compute |
| `AnthropicAPIBackend` | Direct HTTP to Anthropic API | Required | Required | Per-token |

Default is `ClaudeCodeBackend` because the user already runs Claude Code
daily and it provides the most capable agent loop without rebuilding one.
Swap to local backends when sovereignty/offline matters, or for benchmark
testing across models.

## Deferred: graphical UI / desktop

Current Phase 0-4 plan ends at a TTY console. The agent lives in the
shell and apps it generates default to CLI or headless web servers. This
is invisible to non-technical observers — they see a black terminal.

Whether to add a graphical layer (and which one) is **deferred**, not
ruled out. Options to revisit once the base boots and the agent loop
is alive:

| Option | Substrate added | What the user sees | Effort |
|--------|-----------------|--------------------|--------|
| (a) TTY only | none | console with text | 0 (current plan) |
| (b) Browser-as-desktop kiosk | Wayland + sway minimal + Chromium | full-screen Chromium serving agent's web UI | 3-5 days |
| (c) Minimal Wayland desktop | + bar + terminal emulator + font | sway with floating windows | 1-2 weeks |
| (d) Full DE (GNOME/KDE) | massive | traditional desktop | 3-4 weeks; substrate explodes |

**Provisional preference** (revisit when Phase 0-1 done): option (b).
Reasoning: the AI is already strong at generating web UIs (HTML/CSS/JS),
substrate stays small, browser becomes the window manager, converges
with how user's other apps (Cullis, mrblunder) already look. ChromeOS
is essentially this pattern at consumer level — without the AI generating
the apps.

**Honest constraint on what the AI can build at this layer:** the
compositor (Wayland protocol, sway/hyprland) is irreducible — like
crypto and codecs, it lives in the substrate. The AI can configure it,
generate apps that render to it, design UI/layout, but cannot
meaningfully rewrite the compositor itself.

Decision deferred until Phase 0-1 base is demonstrably booting.

## Layer 4 vision — voice-orchestrated agentic interface (fast/slow)

> **Status: vision, not a task.** This is a Layer 4 / Phase 4+ direction,
> written down to preserve a coherent design — *ahead* of where the build
> is (Phase 0/1 bootstrap). It is not built and is not next. Build only
> once the bootstrap demonstrably breathes. Recorded per the
> visions-are-written-not-necessarily-built frame; Cullis stays the
> primary commercial bet. Provoked by the Reachy Mini robot question
> (2026-05-30): once the brain is local (substrate `llama.cpp`), the
> human-facing interface of an agent-primary OS need not be a desktop or
> a TTY — it can be **voice**, with a heavy agent behind it.

This reframes the "Deferred: graphical UI" question above. The interface
of an agent-primary OS is not graphical-vs-terminal — it is **one agentic
backend with several frontends as transports**. The same shape as Reachy's
daemon (daemon owns brain/hardware, clients are thin transports) and as
our own `agent_backend.send()` interface — now with the frontend abstracted
too.

```
   FRONTEND = transports (dumb, UNTRUSTED)
   ┌─────────┬───────────┬──────────────┐
   │  voice  │ terminal  │    robot     │
   │ STT/TTS │   text    │  mic + body  │
   └────┬────┴─────┬─────┴──────┬───────┘
        │          │ (bypasses) │   robot = voice frontend + !motor/!camera
        ▼          │            ▼
   ┌──────────────┐│
   │  router LLM  ││   ← FRONT of the BACKEND (System-1)
   │   (small)    ││
   └──────┬───────┘│
   ─ ─ ─ ─│─ ─ ─ ─ │ ─ escalation boundary = capability + provenance ─ ─
          ▼        ▼
       ┌────────────────┐
       │ Opus (worker)  │   ← System-2
       └────────────────┘
     AGENTIC BACKEND (trusted — capability & provenance live here)
```

**Fast/slow split (System-1 / System-2).** Two models, two latency
profiles that are *incompatible in one model*: conversation needs
sub-second turn-taking; deep agentic work takes seconds-to-minutes. A
single model doing both means the voice goes dead while the worker grinds.
So the split is structural, not an optimization:

- **Small router LLM (System-1):** always-live, low-latency, holds the
  dialogue, does turn-taking and backchanneling, gives status ("still
  working, found X"), and decides *when* to escalate. Local
  (`LlamaCppBackend`/`OllamaBackend`).
- **Opus worker (System-2):** the heavy reasoner that does the actual
  multi-step work. `ClaudeCodeBackend`. Cloud — so the "brain is the
  network" / controlled-egress discipline of the Trust model applies to
  this half.

This is not a new primitive: it is **two existing pluggable backends wired
in a fast/slow topology**. NixOS analogy: the small model is the
interactive shell (must echo *now*); Opus is `nixos-rebuild switch`
running — you do not freeze the shell while the rebuild works.

**The router is backend, not frontend.** A frontend is a dumb transport;
the moment a component *decides* (when to escalate, what to mediate, which
capabilities), it is backend. STT/TTS/keyboard/mic are untrusted
frontends; the small router LLM is the *front of the trusted backend*.
Putting the router in the frontend would push decision-making *outside*
the trust boundary — the error to avoid.

Three consequences fall out of the diagram:

1. **Terminal mode is a transport that reaches deeper** — it bypasses the
   router and talks to Opus directly. Not just convenience: it is the
   high-trust escape hatch / "root shell" to the voice frontend's "GUI".
   When you must authorize something the router should not mediate (a
   dangerous capability, `!activate.system`), drop to terminal.
2. **The robot is not a third system** — it is the voice frontend *plus a
   capability set that happens to be physical* (`!motor`, `!camera`). One
   line more than voice. Which loops back to the capability story: a robot
   behavior downloaded from an untrusted registry, voice-driven, confined
   by declared capabilities at the kernel (Traccia A).
3. **The escalation boundary is the supervision boundary.** When the
   router hands to Opus, that is the moment to record provenance (voice
   request → escalated to Opus → these capabilities → this result), check
   capability grants, and optionally **voice-confirm** with the human
   ("Opus wants to modify the system, needs `!activate.system` — proceed?").
   Voice is not just UX — it is the human-in-the-loop authorization channel
   the Trust model's trusted activation gate already wants. This is where
   it stops being a voice assistant and becomes an OS feature.

**The open research question (the crown jewel).** The protocol between
small router and big worker — *what* gets passed, how state is shared, how
interruption mid-task works — is the actual contribution; everything else
(STT, TTS, transports) is plumbing that already exists. Two early
constraints: the router must **route, not rewrite** (forward the raw
transcript and decide *when* to escalate, never re-summarize the task in
its own words — a small model is a lossy router); and the duplex case is
the hard part (while Opus works, the human keeps talking — status, mid-task
corrections injected into the running worker, chit-chat — a live duplex
between two models and the human). Deferred to design-in-system, not now.

**Substrate compatibility (why this does not blow up Layer 0-1).** The
speech-to-speech stack runs as a *system service*, and its pieces fit the
musl-minimal substrate ethos already chosen: `llama.cpp` is already in the
planned substrate; `whisper.cpp` (STT) is the same author, same build
style; **Piper** (TTS) is small C++; **Silero VAD** is light enough to sit
on the frontend. No new irreducible dependency. Air-gap bonus: with the
small model local, the voice frontend needs no external network — only the
Opus-escalation path crosses the controlled-egress hole.

**What the OS adds (vs a plain voice-agent app on Linux).** The keep-honest
question: this could be built on stock Linux today. The OS justification is
*only* the three consequences above — capability + provenance + voice-
mediated supervision on the escalation boundary, plus the "agent-primary,
no desktop" coherence. If those do not matter, it is an app, not a kernel
feature. That line is the design's north star here.
