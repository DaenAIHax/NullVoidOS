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
              # the host's Max-subscription credentials over a 9P share
              # (RW — see THREAT-MODEL note at the claudefs -virtfs line
              # below). Fail fast if the host hasn't logged in yet —
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

              # Persistent /var on the host: a qcow2 disk under
              # ~/.cache/nullvoid/var.qcow2 mounted in the VM as /var.
              # Auto-created (8 GB sparse) on first run. Whatever the
              # agent builds inside the VM (compiled binaries, packages,
              # generations, history) survives reboots.
              VAR_DIR="''${XDG_CACHE_HOME:-$HOME/.cache}/nullvoid"
              VAR_QCOW2="$VAR_DIR/var.qcow2"
              mkdir -p "$VAR_DIR"
              if [ ! -f "$VAR_QCOW2" ]; then
                echo "  provisioning $VAR_QCOW2 (8 GB sparse)"
                ${pkgs.qemu_kvm}/bin/qemu-img create -q -f qcow2 \
                  "$VAR_QCOW2" 8G
              fi

              # Host SSH public key forwarded into the VM via a RO 9P
              # share. The init script copies it into the agent's
              # authorized_keys so `ssh -p 2222 root@localhost` works
              # from another terminal once the VM is up. If you have
              # no SSH key, dropbear inside the VM still starts but
              # logins via key are impossible — generate one with
              # `ssh-keygen` first.
              SSH_PUB_DIR=""
              for cand in "$HOME/.ssh/id_ed25519.pub" "$HOME/.ssh/id_rsa.pub"; do
                if [ -f "$cand" ]; then
                  SSH_PUB_DIR="$(dirname "$cand")"
                  break
                fi
              done
              SSH_VIRTFS=""
              if [ -n "$SSH_PUB_DIR" ]; then
                SSH_VIRTFS="-virtfs local,path=$SSH_PUB_DIR,mount_tag=sshfs,security_model=mapped-xattr,readonly=on"
              fi

              echo ""
              echo "======================================================"
              echo " Booting NullVoidOS Phase 0 (a) in QEMU"
              echo " Credentials (RW 9P share): $CRED_DIR"
              echo " Persistent /var:           $VAR_QCOW2"
              if [ -n "$SSH_PUB_DIR" ]; then
                echo " SSH key share (RO):        $SSH_PUB_DIR"
                echo " SSH from host:             ssh -p 2222 root@localhost"
              else
                echo " SSH:                       disabled (no key found in ~/.ssh/)"
              fi
              echo " Exit the VM:  type 'poweroff -f', or Ctrl-A then x"
              echo "======================================================"
              echo ""

              # Pick KVM if /dev/kvm is usable, fall back to TCG with
              # -cpu max. KVM is ~10-100x faster than TCG for Node.js
              # / Claude Code (TCG has to interpret every AVX2 op).
              # Either way we expose modern x86-64 (AVX2, BMI2, FMA),
              # because the nixpkgs glibc + bundled Node abort with
              # "Illegal instruction" on the default `qemu64` CPU.
              #
              # THREAT-MODEL (DESIGN.md "Trust model & sandboxing"): the
              # qemu invocation below has two known sharp edges, accepted
              # for single-user alpha but the first things to tighten
              # before any untrusted / multi-tenant use:
              #   (1) -netdev user gives the guest general NAT egress, not
              #       a single whitelisted hole — replace with an egress
              #       proxy scoped to the model endpoint.
              #   (2) the claudefs -virtfs share of ~/.claude is RW, so a
              #       god-inside agent can read host credentials AND write
              #       back into the host's Claude config (inject MCP
              #       servers/hooks that later run on the HOST). The
              #       perimeter-as-jail model needs a CLEAN perimeter —
              #       narrow this to RO creds + a separate writable scratch.
              if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
                ACCEL_ARGS="-accel kvm -cpu host"
                echo "  accel: KVM (-cpu host)"
              else
                ACCEL_ARGS="-accel tcg -cpu max"
                echo "  accel: TCG (-cpu max) — slow, /dev/kvm not usable"
              fi

              exec ${pkgs.qemu_kvm}/bin/qemu-system-x86_64 \
                -kernel ${customPkgs.kernel}/bzImage \
                -initrd ${customPkgs.initramfs}/initramfs.cpio.gz \
                -append "console=ttyS0 quiet" \
                $ACCEL_ARGS \
                -netdev user,id=net0,hostfwd=tcp::2222-:22 \
                -device virtio-net-pci,netdev=net0 \
                -drive "file=$VAR_QCOW2,if=virtio,format=qcow2,cache=writeback" \
                -virtfs "local,path=$CRED_DIR,mount_tag=claudefs,security_model=mapped-xattr" \
                $SSH_VIRTFS \
                -nographic \
                -no-reboot \
                -m 8192 \
                "$@"
            '');
          };
        in {
          boot-vm = bootVm;
          default = bootVm;
        };
      });
}
