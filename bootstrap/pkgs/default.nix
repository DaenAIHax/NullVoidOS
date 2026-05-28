{ pkgs }:
let
  self = {
    zerolang = pkgs.callPackage ./zerolang.nix { };
    kernel = pkgs.callPackage ./kernel.nix { };
    initramfs = pkgs.callPackage ./initramfs.nix {
      inherit (self) zerolang;
      # `claude-code` is unfree — only resolvable because the flake
      # imports nixpkgs with `config.allowUnfree = true`.
      inherit (pkgs) claude-code cacert bash dropbear e2fsprogs;
      # Developer substrate for the "AI builds software on demand" thesis.
      # The agent inside the VM uses these to assemble miniature replacements
      # of the host's installed apps (note-takers, file managers, vector
      # stores, ...).
      devSubstrate = with pkgs; [
        python313
        rustc
        cargo
        nodejs_22
        gcc
        gnumake
        git
        curl
        jq
        ripgrep
        fd
        neovim
        sqlite
        coreutils
      ];
    };
  };
in
self
