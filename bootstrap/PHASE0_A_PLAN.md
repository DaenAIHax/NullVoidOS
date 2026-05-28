# Phase 0 variant (a) — operational plan

> Operational document for the next session. Delete when (a) ships
> (CHANGELOG entry replaces it).

## Goal

Run Claude Code CLI inside the QEMU VM, authenticated with the user's
existing Claude Max subscription. After boot the user lands in a
busybox shell, can inspect the VM, and types `claude` to start the
agent. Claude Code then has tool-use access **inside the VM** — it
reads/writes files in the initramfs, runs commands, can drive
`zero` and assemble the layers above.

Success criteria:

1. QEMU boots → busybox shell
2. `node --version` and `claude --version` work inside the VM
3. Host's `~/.config/claude/` mounted at `/root/.config/claude/` (9P)
4. Network up (DHCP, 10.0.2.15)
5. `claude` launches, authenticated via subscription, accepts a prompt,
   responds with a normal message

Stretch: `claude` writes a small Zero program inside the VM and runs
it via `zero run` — demonstrating that Claude Code has real tool-use
inside the agent-primary OS we're building.

## Why (a) and not (b)

User pays Claude Max subscription. Calling `api.anthropic.com` directly
with a per-token API key (variant b) means paying twice. Claude Code
inside the VM consumes the Max subscription — same dollars, real
agent loop.

Memory: `feedback_claude_subscription`.

## Substrate additions

| Component | Source | Approx size |
|---|---|---|
| Node.js musl | `pkgsMusl.nodejs_22` | ~70 MB |
| Claude Code CLI | TBD: `pkgs.claude-code` if available, else npm install in derivation | ~30 MB |
| `cacert` CA bundle | `pkgs.cacert` | ~250 KB |
| 9P mount helper | busybox `mount` (already there, kernel-side support needed) | 0 |
| `udhcpc.script` | busybox example script | <1 KB |

**Verification step before building**: `nix-env -qa claude-code` or
search nixpkgs to confirm derivation exists. If not, write derivation
that does `npm install -g @anthropic-ai/claude-code` inside a fixed
Nix sandbox with `fetchNpmDeps` for reproducibility. **This is the
biggest unknown of the plan and the first thing to verify next
session.**

Expected total initramfs size: **~120-150 MB compressed**.

## Kernel changes

Add to `bootstrap/pkgs/kernel.nix` configurePhase:

```sh
# 9P filesystem (for host ~/.config/claude/ via virtio)
scripts/config \
  --enable NET_9P \
  --enable NET_9P_VIRTIO \
  --enable 9P_FS \
  --enable 9P_FS_POSIX_ACL

# FUSE (in case we move to virtio-fs later)
# scripts/config --enable FUSE_FS --enable VIRTIO_FS
```

Rebuild kernel. Expected bzImage size delta: ~50 KB.

## Init script (variant a)

```sh
#!/bin/sh

mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true

# Mount host's ~/.config/claude/ via 9P
mkdir -p /root/.config/claude
mount -t 9p -o trans=virtio,version=9p2000.L,ro,msize=131072 \
  claudefs /root/.config/claude 2>/dev/null \
  || echo "WARN: 9P mount failed (credentials unavailable)"

# Network: DHCP + DNS
ifconfig lo up
echo "Bringing up network..."
udhcpc -i eth0 -q -t 5 -T 2 -s /etc/udhcpc/default.script 2>/dev/null
echo "nameserver 10.0.2.3" > /etc/resolv.conf
IP=$(ip -4 addr show eth0 2>/dev/null | sed -n 's/.*inet \([^ ]*\).*/\1/p')

# Banner
echo ""
echo "============================================="
echo " NullVoidOS bootstrap — Phase 0 variant (a)"
echo "============================================="
echo "kernel:   $(uname -srvm)"
echo "zero:     $(zero --version)"
echo "node:     $(node --version 2>/dev/null || echo 'missing')"
echo "claude:   $(claude --version 2>/dev/null || echo 'missing')"
echo "IP:       ${IP:-none}"
echo "creds:    $(ls /root/.config/claude/ 2>/dev/null | head -3 | tr '\n' ' ')"
echo ""
echo "Ready. Type 'claude' to start the agent."
echo "Or 'zero --help' to explore the language."
echo ""

export HOME=/root
export TERM=xterm-256color
export NODE_OPTIONS="--max-old-space-size=512"

exec /bin/sh
```

## QEMU invocation (boot-vm flake app)

Add to `bootstrap/flake.nix`:

```nix
apps.boot-vm = {
  type = "app";
  program = toString (pkgs.writeShellScript "boot-vm" ''
    KERNEL=${self.packages.${system}.kernel}/bzImage
    INITRD=${self.packages.${system}.initramfs}/initramfs.cpio.gz
    CRED_DIR="$HOME/.config/claude"
    [ -d "$CRED_DIR" ] || {
      echo "ERROR: $CRED_DIR not found. Run 'claude login' on host first." >&2
      exit 1
    }
    exec ${pkgs.qemu_kvm}/bin/qemu-system-x86_64 \
      -kernel "$KERNEL" -initrd "$INITRD" \
      -append "console=ttyS0" \
      -netdev user,id=net0 -device virtio-net-pci,netdev=net0 \
      -virtfs "local,path=$CRED_DIR,mount_tag=claudefs,security_model=mapped-xattr,readonly=on" \
      -m 1024 -nographic -no-reboot "$@"
  '');
};
```

Usage: `nix run ./bootstrap#boot-vm`.

Memory bumped to 1 GB (Node.js + Claude Code needs more than busybox).

## Edge cases & contingencies

1. **`pkgsMusl.nodejs_22` doesn't compile clean.** Node.js has C++
   deps (V8) and musl-cross builds aren't always trivial. If broken:
   try `pkgsStatic.nodejs_22`. If that fails: ship glibc in the
   initramfs (accept the inconsistency with the musl-only decision).
   Document the deviation.

2. **`pkgs.claude-code` doesn't exist or doesn't work with pkgsMusl
   node.** Write a derivation that does `npm install
   @anthropic-ai/claude-code` and bundles with esbuild into a single
   .js file the musl node can run. Fragile; pin npm version.

3. **9P mount fails inside VM.** Symptoms: `mount: 9p: invalid filesystem`
   → kernel CONFIG missing → rebuild kernel. `mount: claudefs not found`
   → tag mismatch → check `-virtfs mount_tag=claudefs` in QEMU
   invocation.

4. **`security_model=passthrough` requires root.** Use `mapped-xattr`
   (default, user-friendly) or `mapped-file` (no xattr support needed,
   stores attrs in `.virtfs_metadata` — pollutes the host dir). For
   read-only mount, `mapped-xattr` is fine.

5. **Token refresh on RO mount.** Claude Code may try to write a
   refreshed token to `~/.config/claude/`. With RO mount it'll either
   fail silently (token still valid until expiry) or error loudly.
   Contingency: bump mount to RW (`-virtfs ... readonly=off`) and
   accept that the VM can mutate the host credentials directory.
   Decide on first error.

6. **Claude Code TUI rendering on ttyS0.** Serial console is 80×24
   without true ANSI. Claude Code uses Ink (React-for-CLI), expects a
   real terminal. May render badly. Mitigations: try `TERM=xterm`
   (set above), `TERM=xterm-256color`, `TERM=screen`. If unusable,
   add `-display vnc=:0` to QEMU and connect a real terminal via
   `xterm` from host. For Phase 0 we accept some ugliness.

7. **CA bundle path for Node.js.** Node uses `NODE_EXTRA_CA_CERTS` or
   built-in trust store. Set in init: `export
   NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-bundle.crt`. Ensure cacert
   is copied into the initramfs at that path.

8. **udhcpc applet/script.** Same as (b) plan — verify
   `busybox --list | grep dhcp`. Ship `udhcpc.script` from busybox
   examples. Fallback: hardcode `10.0.2.15`.

9. **`/root` doesn't exist in initramfs.** Add `mkdir -p root/root`
   in the initramfs.nix derivation.

10. **Claude Code wants to mkdir/write in `/`.** Possible — bind-mount
    nothing yet, so `/` is the initramfs ramfs (writeable). Should be
    fine. If Claude Code wants to chdir to `~`, it'll go to `/root`
    which is writeable.

## Verification checklist

After implementing, in order:

- [ ] `nix build .#kernel` succeeds (with 9P CONFIGs)
- [ ] `nix build .#initramfs` succeeds, output 120-150 MB
- [ ] `nix run .#boot-vm` reaches banner
- [ ] Banner shows: kernel, zero version, **node version**, **claude version**, IP, credential file listing
- [ ] `mount` inside VM shows `claudefs on /root/.config/claude type 9p`
- [ ] `cat /root/.config/claude/<some-credential-file>` works
- [ ] `claude` launches without error, shows interactive prompt
- [ ] Send "hello" — Claude responds, **subscription consumed**, not API key
- [ ] Send "create a file /tmp/test.txt with content hello" — Claude
      uses tool-use to write the file inside the VM, verify with
      `cat /tmp/test.txt` in another VM shell or after `/exit`
- [ ] Send "run zero --version and tell me the output" — Claude executes
      bash, parses output, replies

## Open after (a) ships

- Stretch: `claude` writes a Zero hello-world program inside the VM,
  runs it via `zero run`, reports output. This is the actual Phase 0
  "wow" moment: the agent uses its tools to drive the language
  inside the OS we built for it.
- Move `bootstrap/PHASE0_A_PLAN.md` to the trash.
- Add `### Milestone — Phase 0 (a) complete` entry to CHANGELOG.
- Advance to Phase 1: substrate selection + first Zero capability
  wrappers (`substrate/openssl.zero`, etc.).
