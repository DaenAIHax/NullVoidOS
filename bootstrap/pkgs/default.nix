{ pkgs }:
{
  zerolang = pkgs.callPackage ./zerolang.nix { };
  kernel = pkgs.callPackage ./kernel.nix { };
}
