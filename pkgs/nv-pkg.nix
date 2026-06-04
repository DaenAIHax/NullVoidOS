{ pkgsStatic, lib }:

# Alpha — package manager. Consumes .nvpkg tarballs (gzip + tar),
# computes sha256 of the canonical bytes, stores under /var/lib/nv-store.
# Pure Rust (anyhow, clap, flate2, serde, sha2, tar, walkdir, tempfile).
pkgsStatic.rustPlatform.buildRustPackage {
  pname = "nv-pkg";
  version = "0.1.0";
  src = lib.cleanSource ../system/nv-pkg;

  # See null.nix for why this uses cargoHash instead of cargoLock.lockFile.
  cargoHash = "sha256-botWIHQrgvlVLRoaRSnUeH2JsHPobUmtbenkjoUfAiE=";

  doCheck = false;

  meta = with lib; {
    description = "NullVoidOS package manager (local-only, no network)";
    platforms = [ "x86_64-linux" ];
  };
}
