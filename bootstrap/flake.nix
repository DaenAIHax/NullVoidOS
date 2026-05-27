{
  description = "NullVoidOS lfs-bootstrap — Phase 0 cross-compile environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Cross-compile target: x86_64 Linux with musl libc.
        # Matches ZeroLang's linux-musl-x64 target so binaries we build
        # and binaries Zero emits live in the same ABI.
        pkgsMusl = import nixpkgs {
          inherit system;
          crossSystem = {
            config = "x86_64-unknown-linux-musl";
          };
        };
      in {
        devShells.default = pkgs.mkShell {
          name = "nullvoidos-bootstrap";

          packages = with pkgs; [
            # Linux kernel build toolchain
            gcc
            gnumake
            bc
            bison
            flex
            pkg-config
            elfutils
            openssl
            perl
            cpio
            xz
            ncurses

            # Image building
            qemu_kvm

            # Agent runtime host deps (Node.js for Claude Code CLI)
            nodejs_22

            # General tooling
            git
            jq
            curl
            file
            tree
          ];

          shellHook = ''
            echo "NullVoidOS bootstrap dev shell"
            echo "  Cross target:  x86_64-linux-musl"
            echo "  Kernel toolchain: $(gcc --version | head -1)"
            echo "  QEMU: $(qemu-system-x86_64 --version | head -1)"
            echo ""
            echo "Static musl busybox available at:"
            echo "  ${pkgsMusl.busybox}/bin/busybox"
            echo ""
            echo "Next: download Linux LTS source into kernel/, build with"
            echo "  make defconfig && make -j$(nproc)"

            export BUSYBOX_MUSL=${pkgsMusl.busybox}/bin/busybox
          '';
        };

        # Cross-compiled userspace artifacts exposed as packages.
        # Build with: nix build .#busybox-musl
        packages = {
          busybox-musl = pkgsMusl.busybox;
        };
      });
}
