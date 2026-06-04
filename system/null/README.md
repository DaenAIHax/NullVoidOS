# null — NullVoidOS system description language (Phase 1 MVP)

`null` is the CLI for the `.null` declarative configuration language.
A `.null` file evaluates to a typed `SystemManifest` JSON document that
`nv-rebuild` consumes to build a new system generation.

## Quick syntax

```null
{
  hostname = "nullvoid";

  packages = [
    "bash-5.3.9"
    pkgs.neovim-mini        # sugar for "neovim-mini-<installed-version>"
  ];

  services = {
    agent = {
      exec = "/run/current/bin/claude";
      restart = "always";   # "always" | "on-failure" | "never"
    };
  };

  environment = {
    EDITOR = "nvim-mini";
    LANG  = "en_US.UTF-8";
  };
}
```

Comments: `# anything until end of line`. No inline block comments.

Literals: `"string"`, `42` (i64), `true`/`false`, `null`.

**Not in Phase 1:** functions, `let in`, `if then else`, imports,
string interpolation. Parser will tell you so with a clear error.

## Commands

```
null check  <file.null>       # parse + type-check; exit 0 if OK
null eval   <file.null>       # type-check + print SystemManifest JSON
null fmt    <file.null>       # canonical in-place format (idempotent)
null parse --json <file.null> # emit AST as JSON for tooling
```

Add `--json` to any command to get machine-readable stderr diagnostics:

```json
{"level":"error","code":"TYP001","file":"system.null","line":3,"col":14,
 "message":"expected String, got Int","fix":"wrap 42 in quotes"}
```

## pkgs ambient

`pkgs` is built at startup by calling `nv-pkg list --json`. If `nv-pkg`
is not on PATH a warning is emitted and any `pkgs.*` reference is a
type error. Use literal strings (`"bash-5.3.9"`) in that case.

## Build

Requires Rust stable (edition 2021). If cargo is not on PATH:

```bash
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo build --release
```

For a static musl binary (needs the musl target + toolchain):

```bash
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#musl.dev nixpkgs#musl -c \
  cargo build --release --target x86_64-unknown-linux-musl
```

Binary lands at `target/release/null` (or
`target/x86_64-unknown-linux-musl/release/null`).

## Tests

```bash
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo test
```

## Gotchas

- The top-level value **must** be an attrset conforming to `SystemManifest`.
  Any other top-level type is a type error.
- List items must all be the same type (`[String]`). Mixed lists are
  rejected at type-check time.
- `services = {}` and `environment = {}` are valid empty maps.
- Duplicate attribute keys in the same attrset are an error.
