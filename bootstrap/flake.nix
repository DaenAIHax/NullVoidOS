{
  description = "NullVoidOS lfs-bootstrap — Phase 0 cross-compile environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # `claude-code` in nixpkgs carries the `unfree` meta flag, so the
        # default `legacyPackages` view filters it out. Re-importing with
        # `config.allowUnfree = true` exposes it. Phase 0 variant (a)
        # depends on it directly (we ship its full closure into the
        # initramfs as a CAS-of-convenience for Claude Code).
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };

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
        # Usage:  nix run ./bootstrap       (uses default)
        #     or  nix run ./bootstrap#boot-vm
        apps = let
          bootVm = {
            type = "app";
            program = toString (pkgs.writeShellScript "nullvoid-boot-vm" ''
              set -eu

              # Phase 0 (a) ships Claude Code inside the VM and reuses
              # the host's Max-subscription credentials over a read-only
              # 9P share. Fail fast if the host hasn't logged in yet —
              # otherwise `claude` inside the VM would just sit at the
              # login prompt with no way to complete it.
              CRED_DIR="''${HOME}/.claude"
              if [ ! -f "$CRED_DIR/.credentials.json" ]; then
                cat >&2 <<EOF
              ERROR: $CRED_DIR/.credentials.json not found.

              The boot-vm app shares your host's Claude credentials into
              the VM over 9P, so the agent reuses your Max subscription.
              Run \`claude login\` on the host first, then retry.
              EOF
                exit 1
              fi

              echo ""
              echo "======================================================"
              echo " Booting NullVoidOS Phase 0 (a) in QEMU"
              echo " Credentials (RO 9P share): $CRED_DIR"
              echo " Exit the VM:  type 'poweroff -f', or Ctrl-A then x"
              echo "======================================================"
              echo ""

              # `-cpu max` exposes the modern x86-64 instruction set
              # (AVX2, BMI2, FMA, ...) that nixpkgs glibc and the
              # bundled Node.js are compiled to require — without it
              # `claude --version` aborts with "Illegal instruction"
              # inside the VM, because the default `qemu64` CPU is
              # roughly Athlon 64-era.
              exec ${pkgs.qemu_kvm}/bin/qemu-system-x86_64 \
                -kernel ${customPkgs.kernel}/bzImage \
                -initrd ${customPkgs.initramfs}/initramfs.cpio.gz \
                -append "console=ttyS0 quiet" \
                -cpu max \
                -netdev user,id=net0 \
                -device virtio-net-pci,netdev=net0 \
                -virtfs "local,path=$CRED_DIR,mount_tag=claudefs,security_model=mapped-xattr,readonly=on" \
                -nographic \
                -no-reboot \
                -m 1024 \
                "$@"
            '');
          };
        in {
          boot-vm = bootVm;
          default = bootVm;
        };
      });
}
