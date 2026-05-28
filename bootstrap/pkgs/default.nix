{ pkgs }:
let
  self = {
    zerolang = pkgs.callPackage ./zerolang.nix { };
    kernel = pkgs.callPackage ./kernel.nix { };
    initramfs = pkgs.callPackage ./initramfs.nix {
      inherit (self) zerolang;
    };
  };
in
self
