{ pkgsStatic, lib }:

# `.null` CLI — Layer 3 system description language tooling.
# Pure Rust, no C deps; skills/ bundle is embedded at compile time
# via include_str!, so the source tree must include skills/ (it does;
# lib.cleanSource only strips VCS / target/).
pkgsStatic.rustPlatform.buildRustPackage {
  pname = "null";
  version = "0.1.0";
  src = lib.cleanSource ../system/null;

  # `cargoLock.lockFile` would emit one fetchurl per crate against
  # `crates.io/api/v1/crates/.../download`, which now 403s without a
  # User-Agent. `cargoHash` runs cargo inside a fixed-output derivation
  # — cargo sets its own UA, the registry serves the bytes, and the
  # vendor tree is hashed as one blob. (fetchCargoVendor is the default
  # mechanism as of nixpkgs 25.05, so no flag is needed.)
  #
  # When Cargo.lock changes, set this to `lib.fakeHash`, rebuild, and
  # paste the "got: sha256-..." line back here.
  cargoHash = "sha256-/jiL8Ez4KxCrUYhGW6T1OV2JNJrwNu+Yg1HYimcN8u4=";

  # Integration tests already pass on the host; running them under
  # pkgsStatic adds non-trivial cross-build overhead with no signal.
  doCheck = false;

  meta = with lib; {
    description = "NullVoidOS Layer 3 — .null system description language CLI";
    platforms = [ "x86_64-linux" ];
  };
}
