{ pkgsStatic, lib }:

# Nullang CLI — Layer 3 native construction language (SPEC re-lock
# 2026-05-28). One language, two modes: declaration mode is the `.null`
# v2 profile (eval-only, reads system.null); construction mode lowers
# the full surface to C and shells out to `cc` → ELF.
#
# Pure Rust (clap, serde, serde_json), no C deps in the compiler itself.
# Construction mode invokes `cc` at *runtime* inside the VM — that's the
# devSubstrate gcc, not a build-time dependency here.
#
# Shipped ALONGSIDE the `null` binary (see null.nix), not replacing it:
# the Phase 1 loop still evaluates system.null via `null`. Retiring
# `null` waits until nullang declaration mode is verified to read
# system.null identically and nv-rebuild is pointed at it.
pkgsStatic.rustPlatform.buildRustPackage {
  pname = "nullang";
  version = "0.1.0";

  # The agent builds in-tree, so a `target/` directory exists and would
  # otherwise be copied into the store (impure, huge, hash churns on
  # every local rebuild). cleanSource only strips VCS metadata, so we
  # compose an explicit `target/` exclusion on top of it. We also drop
  # `*.md` (SPEC, BUILTINS_CONTRACT, README): docs are not compiler input,
  # and editing them must NOT churn the source hash and force a full
  # nullang + initramfs rebuild (which is exactly what bit us once).
  src = lib.cleanSourceWith {
    src = ../system/nullang;
    filter = path: type:
      let base = baseNameOf (toString path);
      in base != "target"
         && !(lib.hasSuffix ".md" base)
         && lib.cleanSourceFilter path type;
  };

  # See null.nix for why this uses cargoHash instead of cargoLock.lockFile.
  # When Cargo.lock changes, set to lib.fakeHash, rebuild, paste the
  # "got: sha256-..." line back here.
  cargoHash = "sha256-ZH//AvI/0IiQGvjTmhHfaBLyZAtenSsA38keNbLVYws=";

  # Integration tests already pass on the host (14 green as of the
  # re-lock); running them under pkgsStatic adds cross-build overhead
  # with no signal.
  doCheck = false;

  meta = with lib; {
    description = "NullVoidOS Layer 3 — Nullang native construction language CLI";
    platforms = [ "x86_64-linux" ];
  };
}
