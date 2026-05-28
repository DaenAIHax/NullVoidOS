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
