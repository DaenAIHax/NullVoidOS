{ pkgs }:
let
  self = {
    zerolang = pkgs.callPackage ./zerolang.nix { };
    kernel = pkgs.callPackage ./kernel.nix { };

    # Phase 1 system tooling — three Rust crates compiled to static-musl
    # binaries that get dropped into the initramfs /bin. The attr name
    # `nullLang` avoids the bare-`null` Nix keyword clash; the binary on
    # disk is still called `null` (the `.null` CLI per SPEC).
    nullLang   = pkgs.callPackage ./null.nix { };
    nv-pkg     = pkgs.callPackage ./nv-pkg.nix { };
    nv-rebuild = pkgs.callPackage ./nv-rebuild.nix { };

    initramfs = pkgs.callPackage ./initramfs.nix {
      inherit (self) zerolang nullLang nv-pkg nv-rebuild;
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
