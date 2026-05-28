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

        # Static-musl busybox for the initramfs. pkgsStatic on Linux gives
        # musl + full static linking, so the cpio doesn't need to ship
        # ld-musl as a runtime dependency. Same ABI as ZeroLang's
        # linux-musl-x64 target.
        busyboxStatic = pkgs.pkgsStatic.busybox;

        customPkgs = import ./pkgs { inherit pkgs; };
      in {
        devShells.default = pkgs.mkShell {
          name = "nullvoidos-bootstrap";

          packages = (with pkgs; [
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
          ]) ++ [
            customPkgs.zerolang
          ];

          shellHook = ''
            echo "NullVoidOS bootstrap dev shell"
            echo "  Cross target:  x86_64-linux-musl (static)"
            echo "  Kernel toolchain: $(gcc --version | head -1)"
            echo "  QEMU: $(qemu-system-x86_64 --version | head -1)"
            echo "  zero:  $(zero --version 2>/dev/null || echo 'not available')"
            echo ""
            echo "Static musl busybox available at:"
            echo "  ${busyboxStatic}/bin/busybox"
            echo ""
            echo "Next: download Linux LTS source into kernel/, build with"
            echo "  make defconfig && make -j$(nproc)"

            export BUSYBOX_MUSL=${busyboxStatic}/bin/busybox
          '';
        };

        # Cross-compiled userspace artifacts exposed as packages.
        # Build with: nix build .#busybox-musl
        packages = customPkgs // {
          busybox-musl = busyboxStatic;
        };

        # Convenience runner for the Phase 0 (d) bootable VM.
        # Usage: nix run ./bootstrap#boot-vm
        apps.boot-vm = {
          type = "app";
          program = toString (pkgs.writeShellScript "nullvoid-boot-vm" ''
            echo ""
            echo "======================================================"
            echo " Booting NullVoidOS Phase 0 (d) in QEMU"
            echo " Exit the VM:  press Ctrl-A   then   x"
            echo "======================================================"
            echo ""
            exec ${pkgs.qemu_kvm}/bin/qemu-system-x86_64 \
              -kernel ${customPkgs.kernel}/bzImage \
              -initrd ${customPkgs.initramfs}/initramfs.cpio.gz \
              -append "console=ttyS0" \
              -nographic \
              -no-reboot \
              -m 256 \
              "$@"
          '');
        };
      });
}
