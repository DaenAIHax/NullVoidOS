{ pkgsStatic, lib }:

# Gamma — activation engine. Reads /etc/nullvoid/system.null via `null
# eval`, resolves packages via `nv-pkg resolve`, materialises generations
# under /var/lib/nv-system/, atomic-swaps /run/current.
# Uses the `nix` crate for fs/signal syscalls (rename(2), kill(SIGHUP)).
pkgsStatic.rustPlatform.buildRustPackage {
  pname = "nv-rebuild";
  version = "0.1.0";
  src = lib.cleanSource ../system/nv-rebuild;

  # See null.nix for why this uses cargoHash instead of cargoLock.lockFile.
  cargoHash = "sha256-nlo0eObxoSs0w/ZxNkwMqLDtLewGGbZLqfUWOtpFc80=";

  doCheck = false;

  meta = with lib; {
    description = "NullVoidOS activation engine — builds and switches system generations";
    platforms = [ "x86_64-linux" ];
  };
}
