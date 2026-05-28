# Phase 0 variant (b) — operational plan

> Operational document for the next session. Delete this file when (b)
> ships (CHANGELOG entry "completed Phase 0 (b)" replaces it).

## Goal

Replace the `exec /bin/sh` tail of `/init` (variant d) with a real
agent loop. After boot, the user should see:

```
=============================================
 NullVoidOS bootstrap — Phase 0 variant (b)
=============================================
kernel:   Linux 6.6.141 #1 ... x86_64
zero:     zero 0.1.4

Bringing up network... 10.0.2.15
Agent: claude-opus-4-7 via Anthropic API
Type prompt, Enter. Ctrl-D to exit.

> hello
[thinking...]

Hello! I'm running inside your minimal Linux VM. The kernel is 1.5 MB
and the rootfs is an initramfs in RAM. What would you like me to do?

>
```

Success criteria: agent answers a trivial prompt end-to-end inside the
VM, no host-side proxy.

## Substrate additions

| Package | From | Approx size (static, stripped) |
|---|---|---|
| `curl` | `pkgsStatic.curl` | ~3-4 MB |
| `jq` | `pkgsStatic.jq` | ~1-2 MB |
| `cacert` CA bundle | `pkgs.cacert.unbundled` or `/etc/ssl/certs/ca-bundle.crt` | ~250 KB |
| `udhcpc.script` | busybox example script (`examples/udhcp/simple.script`) | <1 KB |

Verify first: `pkgsStatic.busybox` has `udhcpc` applet compiled in
(check with `busybox --list | grep dhcp` after boot, or inspect the
nixpkgs busybox config). If missing, fallback: configure eth0
manually — QEMU's default user network is `10.0.2.0/24`, gateway
`10.0.2.2`, DNS `10.0.2.3`.

Expected total initramfs size: ~12-15 MB compressed (currently 1.2 MB
in d).

## Kernel — no changes needed

CONFIG already has: `NET`, `INET`, `PACKET`, `UNIX`, `NETDEVICES`,
`ETHERNET`, `VIRTIO_NET`. Re-verify after first boot attempt that
`ip link` shows `eth0`.

If networking misbehaves, candidate additions: `CONFIG_E1000` (in case
QEMU falls back to e1000 instead of virtio-net), `CONFIG_DNS_RESOLVER`.

## Init script structure

```sh
#!/bin/sh

mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true

# Pull API key from kernel cmdline (security: visible in /proc/cmdline,
# acceptable for alpha; harden later with virtio-fs or kernel keyring).
ANTHROPIC_API_KEY=$(sed -n 's/.*ANTHROPIC_API_KEY=\([^ ]*\).*/\1/p' /proc/cmdline)

# Network up
ifconfig lo up
echo "Bringing up network..."
udhcpc -i eth0 -q -t 5 -T 2 -s /etc/udhcpc/default.script 2>/dev/null \
  || ifconfig eth0 10.0.2.15 netmask 255.255.255.0 up
echo "nameserver 10.0.2.3" > /etc/resolv.conf
IP=$(ip -4 addr show eth0 | sed -n 's/.*inet \([^ ]*\).*/\1/p')
echo "  IP: $IP"

# Banner + agent loop (see below)
. /etc/agent-loop.sh
```

Agent loop (separate file for readability):

```sh
echo ""
echo "============================================="
echo " NullVoidOS bootstrap — Phase 0 variant (b)"
echo "============================================="
echo "kernel:   $(uname -srvm)"
echo "zero:     $(zero --version)"
echo ""
echo "Agent: claude-opus-4-7 via Anthropic API"
echo "Type prompt, Enter. Ctrl-D to exit."
echo ""

while printf '> ' && IFS= read -r prompt; do
  [ -z "$prompt" ] && continue

  body=$(jq -n --arg p "$prompt" '{
    model: "claude-opus-4-7",
    max_tokens: 1024,
    messages: [{role: "user", content: $p}]
  }')

  echo "[thinking...]"
  resp=$(curl -sS https://api.anthropic.com/v1/messages \
    -H "x-api-key: $ANTHROPIC_API_KEY" \
    -H "anthropic-version: 2023-06-01" \
    -H "content-type: application/json" \
    --cacert /etc/ssl/certs/ca-bundle.crt \
    -d "$body")

  text=$(echo "$resp" | jq -r '.content[0].text // .error.message // "?"')
  echo ""
  echo "$text"
  echo ""
done

echo "Goodbye."
```

Note: prompt is passed to `jq` via `--arg` (safe escaping). Never
shell-interpolate user input into JSON.

## QEMU invocation changes

Variant (d):
```sh
qemu-system-x86_64 -kernel bzImage -initrd initramfs.cpio.gz \
  -append "console=ttyS0" -nographic -no-reboot -m 256
```

Variant (b) needs network + API key:
```sh
qemu-system-x86_64 -kernel bzImage -initrd initramfs.cpio.gz \
  -append "console=ttyS0 ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY" \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0 \
  -nographic -no-reboot -m 512
```

Recommend wrapping this in a flake `apps.boot-vm`:
```nix
apps.boot-vm = {
  type = "app";
  program = "${pkgs.writeShellScript "boot-vm" ''
    : "''${ANTHROPIC_API_KEY:?ANTHROPIC_API_KEY env var required}"
    KERNEL=${self.packages.${system}.kernel}/bzImage
    INITRD=${self.packages.${system}.initramfs}/initramfs.cpio.gz
    exec ${pkgs.qemu_kvm}/bin/qemu-system-x86_64 \
      -kernel "$KERNEL" -initrd "$INITRD" \
      -append "console=ttyS0 ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY" \
      -netdev user,id=net0 -device virtio-net-pci,netdev=net0 \
      -nographic -no-reboot -m 512 "$@"
  ''}";
};
```

Usage: `ANTHROPIC_API_KEY=sk-... nix run ./bootstrap#boot-vm`.

## Edge cases & contingencies

1. **`udhcpc` applet missing from pkgsStatic.busybox.** Verify with
   `busybox --list`. If missing: rebuild busybox with `enableStatic`
   override or use a separately-built `udhcpc`. Fallback: hardcode
   `ifconfig eth0 10.0.2.15` (QEMU's default DHCP assignment).

2. **`udhcpc.script` missing.** Busybox ships `examples/udhcp/simple.script`
   in its source. Easiest: write a 10-line shell script that reads
   env vars set by udhcpc (`ip`, `subnet`, `router`, `dns`) and runs
   ifconfig + writes /etc/resolv.conf. Place at `/etc/udhcpc/default.script`.

3. **CA bundle path mismatch.** Curl looks at `/etc/ssl/certs/ca-bundle.crt`
   by default on many distros, `/etc/ssl/certs/ca-certificates.crt` on
   others. Pass `--cacert` explicitly to avoid ambiguity.

4. **TLS handshake failure.** curl prints "SSL: no alternative certificate
   subject names match". Means CA bundle isn't being read. Verify path
   inside the running VM: `ls -l /etc/ssl/certs/`.

5. **JSON escaping breakage.** If a user prompt contains quotes,
   backslashes, newlines — `jq --arg` handles it correctly. Do NOT
   build the JSON body with `printf` or here-doc + shell interpolation.

6. **API key visible in /proc/cmdline.** Phase 0 alpha — accept it.
   Document explicitly. Harden in Phase 2 via virtio-fs or kernel
   keyring.

7. **`tty: can't access tty` warning from busybox sh.** Polish at the
   same time: `exec setsid cttyhack /bin/sh` (verify cttyhack applet
   exists in pkgsStatic.busybox first).

## Verification checklist

After implementing:

- [ ] `nix build .#initramfs` succeeds, output ~12-15 MB
- [ ] `nix run .#boot-vm` (with `ANTHROPIC_API_KEY` set) reaches banner
- [ ] DHCP completes within ~5 s, IP printed
- [ ] `> hello` round-trip returns a Claude response
- [ ] `> what's the kernel version` returns "Linux 6.6.141 ..."
  (proves the agent can reason about its environment)
- [ ] Ctrl-D exits cleanly with "Goodbye."
- [ ] Total cpio.gz size < 20 MB

## When done

- CHANGELOG entry: "Phase 0 (b): agent loop alive — Claude responds
  to prompts inside the VM via Anthropic API."
- Delete this file (`PHASE0_B_PLAN.md`).
- Phase 0 complete; advance to Phase 1 (substrate selection + Zero
  capability wrappers).
