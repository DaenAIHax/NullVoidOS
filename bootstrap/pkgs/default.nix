{ pkgs }:
let
  self = {
    zerolang = pkgs.callPackage ./zerolang.nix { };
    kernel = pkgs.callPackage ./kernel.nix { };
    initramfs = pkgs.callPackage ./initramfs.nix {
      inherit (self) zerolang;
      # `claude-code` is unfree — only resolvable because the flake
      # imports nixpkgs with `config.allowUnfree = true`.
      inherit (pkgs) claude-code cacert bash;
    };
  };
in
self
