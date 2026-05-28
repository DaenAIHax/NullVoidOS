# NullVoidOS — Phase 1 Contracts

> **Purpose.** Lock the interfaces between the three components that
> together implement the "AI-authored declarative OS" thesis:
>
> - `nv-pkg` (Alpha) — package manager
> - `null` (Beta) — system description language
> - `nv-rebuild` (Gamma) — activation engine
>
> These three pieces can be built in parallel **only if their contracts
> are fixed first**. This document fixes them. Implementation choices
> (language, libraries, internal data structures) are left to each
> sub-agent.
>
> Anything not in this document is implementation detail.

## 0 — Mental model

The agent inside the VM is the user. It authors software, packages it,
declares it in the system manifest, and applies the manifest. The OS
state at any moment is a deterministic function of the manifest plus
the package store contents. Nothing else mutates the system.

```
agent writes recipe.null  --build-->  payload + manifest.json
                                            |
                                            v
                                      .nvpkg tarball
                                            |
                              nv-pkg install <file.nvpkg>
                                            |
                                            v
                  /var/lib/nv-store/<hash>-<name>-<version>/
                                            |
agent edits /etc/nullvoid/system.null:      |
   { packages = [ "<name>-<version>" ]; }   |
                                            |
                              nv-rebuild switch
                                            |
                                            v
            /run/current/  ->  /var/lib/nv-system/generation-N/
                                            |
                              bin/, etc/, manifest.json
                                            |
                                            v
                              $PATH = /run/current/bin
```

## 1 — Package format (`nv-pkg` ↔ everyone)

### 1.1 On-disk tarball

A package is a gzipped tar (`.nvpkg`) with **exactly** this layout at
the root:

```
manifest.json          (required)
payload/               (required, may be empty)
  bin/                 (optional — binaries exposed on $PATH)
  lib/                 (optional)
  share/               (optional)
  ...arbitrary tree...
recipe.null            (optional — original source for reproducibility)
```

- The tarball **must not contain** any other top-level entries.
- File modes inside `payload/bin/*` must include the execute bit.
- Symlinks inside the tarball are preserved; absolute symlink targets
  pointing outside the package are forbidden (validator rejects them).

### 1.2 `manifest.json`

```json
{
  "schemaVersion": 1,
  "name": "neovim-mini",
  "version": "0.1.0",
  "description": "minimal modal text editor",
  "authoredBy": "claude-code-2.1.148",
  "createdAt": "2026-05-28T15:42:00Z",
  "deps": ["bash-5.3.9", "ncurses-6.6"],
  "exposedBins": ["nvim-mini"],
  "capabilities": ["fs:read", "fs:write:cwd", "tty"],
  "sourceLanguage": "rust",
  "buildSteps": [
    "cargo build --release --target x86_64-unknown-linux-musl"
  ]
}
```

Field semantics:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `schemaVersion` | int | yes | This document defines `1`. Future versions bumped only on breaking changes. |
| `name` | string | yes | `[a-z][a-z0-9-]*`, max 64 chars. Globally identifies the package. |
| `version` | string | yes | Semver (`MAJOR.MINOR.PATCH`). |
| `description` | string | yes | One-line human summary. |
| `authoredBy` | string | yes | Identifier of the agent/tool that produced this. Free-form. |
| `createdAt` | RFC3339 string | yes | UTC timestamp. |
| `deps` | array of `"<name>-<version>"` | yes (may be empty) | Other packages this one needs at runtime. Resolution is exact-match: no version ranges. |
| `exposedBins` | array of strings | yes (may be empty) | Names under `payload/bin/` that get linked into `/run/current/bin/` when the package is in the active manifest. |
| `capabilities` | array of strings | yes (may be empty) | Declared capabilities the agent needs (see §4). Activation engine may use this for sandboxing later; for now it's just recorded. |
| `sourceLanguage` | string | optional | Hint for tooling (`"rust"`, `"python"`, `"c"`, ...). |
| `buildSteps` | array of strings | optional | Documented for reproducibility, not executed by `nv-rebuild`. |

### 1.3 Storage layout

Once installed:

```
/var/lib/nv-store/
  <storeHash>-<name>-<version>/
    manifest.json
    payload/
      bin/...
      lib/...
    recipe.null         (if shipped)
```

`storeHash` is the **SHA-256 hex** of the canonical tarball bytes,
truncated to the first 32 hex chars. Two installs of the same exact
tarball collapse to the same store path (idempotent). Different
tarballs of the same `name-version` produce **different** store
paths — collision is detected and reported as an error by `nv-pkg
install` unless `--force` is passed.

### 1.4 `nv-pkg` CLI surface

These are the commands all other components rely on:

```
nv-pkg install <file.nvpkg>            install into /var/lib/nv-store
                                       prints store path on stdout

nv-pkg resolve <name>-<version>        find the store path of an
                                       installed package
                                       prints store path on stdout,
                                       exit 1 if not found

nv-pkg list [--json]                   list installed packages

nv-pkg remove <name>-<version>         remove the store path
                                       fails if a current generation
                                       still references it

nv-pkg verify <name>-<version>         re-hash the store contents,
                                       confirm match
```

JSON output mode (`--json`) emits one JSON document per command, used
by `nv-rebuild` and by humans piping through `jq`.

`nv-pkg` does **not**:

- Talk to the network. No registry, no download. Phase 1 is local-only.
- Build packages from recipes. That is the agent's job (or a future
  helper). `nv-pkg` only consumes `.nvpkg` tarballs.

## 2 — `.null` language (Beta ↔ Gamma)

### 2.1 What `.null` is

A small declarative configuration language. **Eval-only** in Phase 1:
no compile-to-binary backend, no FFI. Evaluation of a `.null` file
produces a typed `SystemManifest` value, serialized to JSON for
`nv-rebuild` to consume.

### 2.2 Surface syntax (MVP subset)

```null
{
  hostname = "nullvoid";

  packages = [
    "neovim-mini-0.1.0"
    "task-tui-0.3.2"
    "bash-5.3.9"
  ];

  services = {
    agent = {
      exec = "/run/current/bin/claude";
      restart = "always";
    };
  };

  environment = {
    EDITOR = "nvim-mini";
    LANG = "en_US.UTF-8";
  };
}
```

Constructs in MVP:

- **Literals**: string (`"..."`), int (`42`), bool (`true`/`false`), null.
- **List**: `[ a b c ]` — whitespace-separated, same type required.
- **Attribute set**: `{ key = value; ... }` — semicolon-terminated.
- **Path**: `./relative/path` — resolved relative to the source file.
- **Reference**: bare identifier (must be in scope; Phase 1 has only
  one in-scope identifier: `pkgs`, see §2.4).
- **Field access**: `pkgs.bash` (lhs must be attrset).
- **Comment**: `# trailing only`, until end of line.

Not in MVP (deferred):
- Functions (`{ ... }: ...`)
- `let in`
- `if then else`
- Imports between `.null` files
- String interpolation

### 2.3 Type system (Phase 1)

The evaluator runs in two stages:

1. **Parse** → AST (untyped tree).
2. **Type-check + eval** in one pass, against the expected schema of
   the top-level value.

The expected schema for `system.null` is:

```
type SystemManifest = {
  hostname: String,
  packages: [String],
  services: { [String]: Service },
  environment: { [String]: String },
}

type Service = {
  exec: String,
  restart: "always" | "on-failure" | "never",
}
```

Type errors are reported with file:line:col and a short repair hint.
A `.null` file that fails type-check is rejected entirely (no partial
output).

### 2.4 The `pkgs` ambient

The single in-scope identifier in Phase 1 is `pkgs`. It is built by
the evaluator at startup by calling `nv-pkg list --json` and projecting:

```
pkgs = {
  "<name>" = "<name>-<version>";   # latest version installed
  ...
}
```

So writing `pkgs.bash` is sugar for the string `"bash-5.3.9"` if
that's the version currently in the store. This is good enough for
Phase 1 and avoids embedding a registry. Multiple versions of the
same package are addressable directly by literal string.

### 2.5 CLI surface

```
null check <file.null>              type-check, exit 0 if ok
null eval <file.null>               type-check + eval, print JSON
                                    of the resulting SystemManifest
null fmt <file.null>                canonical formatting (in-place)
null parse --json <file.null>       AST as JSON (for tooling)
```

All commands emit machine-readable diagnostics on stderr when
`--json` is set:

```json
{"level":"error","code":"TYP001","file":"system.null","line":3,"col":14,
 "message":"expected String, got Int","fix":"wrap 42 in quotes"}
```

## 3 — Activation engine (`nv-rebuild`, Gamma)

### 3.1 Surface

```
nv-rebuild check                    eval /etc/nullvoid/system.null
                                    via `null eval`, validate that
                                    every referenced package exists
                                    in the store. No mutation.

nv-rebuild build                    same as check, plus prepare a new
                                    generation directory but don't
                                    activate it.

nv-rebuild switch                   build + atomically swap
                                    /run/current to the new generation

nv-rebuild rollback                 atomically swap /run/current
                                    back to the previous generation

nv-rebuild generations              list generations, mark which is
                                    active
```

### 3.2 Generation layout

```
/var/lib/nv-system/
  generation-1/
    manifest.json       (the evaluated SystemManifest)
    bin/                (symlinks into /var/lib/nv-store/.../bin/)
    etc/
      environment       (KEY=value lines)
      services/         (one file per service)
  generation-2/
    ...
  current  ->  generation-2   (symlink)
```

`/run/current` is a symlink to `/var/lib/nv-system/current`, kept on
ramfs for fast access. The shell's `PATH` is set to
`/run/current/bin:/bin:/usr/bin` by init.

### 3.3 The switch algorithm

```
1. Read /etc/nullvoid/system.null
2. Run `null eval` → SystemManifest JSON
3. For each "<name>-<version>" in packages:
     a. `nv-pkg resolve <name>-<version>` → store path
        (fail switch if missing)
4. Pick next generation number N (max existing + 1)
5. Create /var/lib/nv-system/generation-N/ with:
     a. manifest.json = the eval result
     b. bin/<exposed> -> store-path/payload/bin/<exposed>
        for every package's exposedBins
        (conflict resolution: last package in `packages` wins,
         and a warning is emitted)
     c. etc/environment from the manifest's environment map
6. Atomically: ln -snf generation-N /var/lib/nv-system/.current.new
              mv -T /var/lib/nv-system/.current.new
                    /var/lib/nv-system/current
7. Send SIGHUP to PID 1 so services pick up the change
   (Phase 1: PID 1 is busybox sh, ignores SIGHUP — services are
    re-read manually for now; full supervision is Phase 2)
```

The atomic swap at step 6 is the only mutation that makes a generation
"the current system". Everything before it is reversible by deleting
the half-built generation directory.

### 3.4 Rollback

`nv-rebuild rollback`:

1. Read the symlink target of `/var/lib/nv-system/current` → `generation-N`
2. Find `generation-(N-1)` — fail if it doesn't exist
3. Atomic swap as in 3.3 step 6, targeting `generation-(N-1)`

Generations are never auto-deleted in Phase 1. The agent (or a future
`nv-gc` tool) decides when to prune.

## 4 — Capabilities (placeholder)

Capability strings recorded in `manifest.json` are **declared but not
enforced** in Phase 1. They are written down so we can audit what the
package *says* it needs; Phase 2 will wire them to actual sandboxing
(seccomp / landlock / cgroups).

Canonical capability names for Phase 1 — agents authoring packages
should use these strings:

```
fs:read                  read any file the user can read
fs:read:<path>           read a specific path subtree
fs:write:<path>          write a specific path subtree
fs:write:cwd             write under the working directory
net                      open arbitrary sockets
net:localhost            sockets to 127.0.0.1 only
tty                      access a controlling terminal
proc:spawn               spawn child processes
proc:exec                exec other binaries
time                     read system time
rand                     read /dev/urandom
```

Unknown capability strings are accepted (forward-compat) but logged.

## 5 — Reference end-to-end flow (the demo we want to see)

```bash
# Inside the VM, as the agent:

# 1. Author a tiny note manager in Rust
mkdir notes-tui-0.1.0/ && cd notes-tui-0.1.0/
cat > src/main.rs <<'EOF' ... EOF
cat > Cargo.toml <<'EOF' ... EOF
cargo build --release

# 2. Package it
mkdir -p payload/bin
cp target/release/notes-tui payload/bin/notes
cat > manifest.json <<'EOF'
{ "schemaVersion": 1, "name": "notes-tui", "version": "0.1.0",
  "description": "minimal notes",
  "authoredBy": "claude-code-2.1.148",
  "createdAt": "...", "deps": [], "exposedBins": ["notes"],
  "capabilities": ["fs:read:/var/notes","fs:write:/var/notes","tty"] }
EOF
tar czf notes-tui-0.1.0.nvpkg manifest.json payload/

# 3. Install into the store
nv-pkg install ./notes-tui-0.1.0.nvpkg
# /var/lib/nv-store/abc123...-notes-tui-0.1.0

# 4. Declare in the system
nvim /etc/nullvoid/system.null
# (add "notes-tui-0.1.0" to packages list)

# 5. Apply
nv-rebuild switch
# building generation 7...
# activated: /var/lib/nv-system/current -> generation-7

# 6. Use it
which notes
# /run/current/bin/notes
notes
# (the TUI written by the agent, 5 minutes ago, is the new
#  system-wide note app)
```

That sequence is the falsifiable test of Phase 1. When the agent can
do all six steps unaided, the loop is closed.

## 6 — Out of scope for Phase 1

- Multi-host package distribution (a registry, an `nv-pkg push/pull`)
- Sandboxed builds (recipes consumed by `nv-pkg`)
- Capability enforcement at runtime
- Service supervision (waiting for Phase 2's init replacement)
- Garbage collection of old generations and unreferenced store paths
- Multi-user / authentication / signing
- Cross-architecture packages

Each of these is a deliberate "later". The MVP locks the *shape* of
the system so they can be added without breaking everything else.
