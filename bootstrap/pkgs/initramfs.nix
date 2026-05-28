{ lib, runCommand, writeText, closureInfo
, cpio, gzip
, pkgsStatic
, zerolang
, claude-code
, cacert
, bash
}:

let
  # udhcpc dispatcher. Busybox's stock `default.script` hardcodes its
  # own `/nix/store/.../bin/busybox` path, which is meaningless inside
  # the initramfs filesystem. This minimal replacement uses the
  # busybox applets via `$PATH` (we symlink them all under /bin).
  udhcpcScript = writeText "udhcpc-default.script" ''
    #!/bin/sh
    RESOLV_CONF="/etc/resolv.conf"
    case "$1" in
      bound|renew)
        ifconfig "$interface" $ip netmask $subnet \
          ''${broadcast:+broadcast $broadcast} \
          ''${mtu:+mtu $mtu}
        if [ -n "$router" ]; then
          ip -4 route flush exact 0.0.0.0/0 dev "$interface" 2>/dev/null
          ip -4 route add default via "$router" dev "$interface"
        fi
        R=""
        [ -n "$domain" ] && R="domain $domain"
        for dnsip in $dns; do
          R="$R
    nameserver $dnsip"
        done
        printf '%s\n' "$R" > "$RESOLV_CONF"
        ;;
      deconfig)
        ip link set "$interface" up
        ip -4 addr flush dev "$interface"
        ip -4 route flush dev "$interface"
        ;;
      leasefail|nak)
        echo "udhcpc: $1: $message" >&2
        ;;
    esac
    exit 0
  '';

  init = writeText "init" ''
    #!/bin/sh

    mount -t proc proc /proc 2>/dev/null
    mount -t sysfs sysfs /sys 2>/dev/null
    mount -t devtmpfs devtmpfs /dev 2>/dev/null || true

    # Silence late kernel info-level messages so they don't disrupt the prompt.
    dmesg -n 1 2>/dev/null || true

    # 9P share: host's ~/.claude/ → /root/.claude/ (RW).
    # Carries the Max-subscription credentials so `claude` inside the
    # VM authenticates without our owning an API key. Mounted RW so
    # claude-code can refresh the OAuth tokens in-place (Phase 0 plan
    # contingency 5 — the VM may mutate the host directory).
    mkdir -p /root/.claude
    if mount -t 9p -o trans=virtio,version=9p2000.L,msize=131072 \
         claudefs /root/.claude 2>/dev/null; then
      CREDS_OK=yes
    else
      CREDS_OK="no (9P mount failed)"
    fi

    # ~/.claude.json (Claude Code's config file with project trust + MCP
    # servers + model prefs) lives in $HOME, not in $HOME/.claude/, so
    # the 9P share above doesn't carry it. Without it `claude` aborts
    # before reading the credentials. Seed it from the most recent
    # backup in .claude/backups/, which is what Claude Code suggests in
    # its own error message.
    if [ "$CREDS_OK" = yes ] && [ ! -f /root/.claude.json ]; then
      LATEST_CFG=$(ls -1t /root/.claude/backups/.claude.json.backup.* \
                     2>/dev/null | head -1)
      if [ -n "$LATEST_CFG" ]; then
        cp "$LATEST_CFG" /root/.claude.json
        echo "seeded /root/.claude.json from $LATEST_CFG"
      fi
    fi

    # Loopback + DHCP on eth0 (QEMU user networking, gateway 10.0.2.2).
    ifconfig lo up 2>/dev/null
    udhcpc -i eth0 -q -t 5 -T 2 -s /etc/udhcpc/default.script \
      >/dev/null 2>&1
    [ -s /etc/resolv.conf ] || echo "nameserver 10.0.2.3" > /etc/resolv.conf
    IP=$(ip -4 addr show eth0 2>/dev/null | sed -n 's/.*inet \([^ ]*\).*/\1/p')

    # Node.js TLS — claude-code's bundled node looks here for the CA bundle.
    export NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-bundle.crt
    export SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt
    export HOME=/root
    export TERM=xterm-256color
    export PATH=/bin

    echo ""
    echo "================================================="
    echo " NullVoidOS bootstrap — Phase 0 variant (a)"
    echo "================================================="
    echo "kernel:   $(uname -srvm)"
    echo "hostname: $(hostname)"
    echo "zero:     $(zero --version 2>/dev/null || echo missing)"
    echo "claude:   $(claude --version 2>/dev/null || echo missing)"
    echo "IP:       ''${IP:-none}"
    echo "creds:    $CREDS_OK"
    if [ "$CREDS_OK" = "yes" ]; then
      echo "          $(ls /root/.claude 2>/dev/null | head -3 | tr '\n' ' ')"
    fi
    echo ""
    echo "Type 'claude' to start the agent."
    echo "Or 'zero --help' to explore the language."
    echo "Quit: type 'poweroff -f', or Ctrl-A x from host."
    echo ""

    # Respawn the shell instead of exec'ing it — otherwise `exit` (or any
    # accidental death) kills PID 1 and the kernel panics. `setsid
    # cttyhack` gives the shell a controlling tty so Ctrl-C only
    # interrupts the foreground program (claude, zero, ...) instead of
    # leaving the serial in raw mode after the TUI catches SIGINT and
    # forgets to restore canonical mode.
    while true; do
      setsid cttyhack /bin/sh
      echo ""
      echo "[shell exited — respawning in 2s, or Ctrl-A x to quit QEMU]"
      sleep 2
    done
  '';

  # Whole transitive closure of claude-code + bash (~30+5 paths,
  # ~360 MB uncompressed). We ship it under /nix/store inside the
  # initramfs. claude-code's wrapper binary patches PATH/LD_LIBRARY_PATH
  # with absolute /nix/store paths, so all the dynamic deps resolve
  # verbatim from there. bash is needed for tool-use: claude-code
  # invokes `bash -c "<cmd>"` for the Bash tool, and busybox ash at
  # /bin/sh is not a substitute (Claude calls `/bin/bash` or the
  # `bash` on PATH explicitly).
  #
  # This is a Phase 0 shortcut and a documented deviation from the
  # "musl-only" decision (Phase 0 contingency 1). DESIGN.md maps
  # /nix/store onto the CAS substrate role; we use the real thing
  # here, deferring the LFS-style CAS to Phase 1+.
  agentClosure = closureInfo {
    rootPaths = [ claude-code bash ];
  };
in
runCommand "nullvoid-initramfs" {
  nativeBuildInputs = [ cpio gzip ];

  meta = with lib; {
    description = "NullVoidOS Phase 0 initramfs (variant a): busybox + zero + claude-code closure";
    platforms = [ "x86_64-linux" ];
  };
} ''
  mkdir -p root/{bin,dev,proc,sys,tmp,root,etc/ssl/certs,etc/udhcpc,nix/store,usr/bin}

  cp ${pkgsStatic.busybox}/bin/busybox root/bin/

  # Auto-symlink every applet busybox was compiled with. Avoids the
  # bug where common commands (whoami, date, dmesg, ...) "weren't there
  # because we forgot to add them to a list".
  for applet in $(${pkgsStatic.busybox}/bin/busybox --list); do
    [ -e "root/bin/$applet" ] || ln -s busybox "root/bin/$applet"
  done

  cp ${zerolang}/bin/zero root/bin/

  # Ship the agent runtime closure (claude-code + bash) into /nix/store.
  for p in $(cat ${agentClosure}/store-paths); do
    cp -a "$p" "root/nix/store/$(basename "$p")"
  done

  # Symlink the wrappers into /bin and /usr/bin so they're on PATH.
  ln -s ${claude-code}/bin/claude root/bin/claude
  ln -s ${bash}/bin/bash root/bin/bash
  ln -s /bin/env root/usr/bin/env

  # CA bundle for Node.js TLS (Claude Code calls api.anthropic.com).
  cp ${cacert}/etc/ssl/certs/ca-bundle.crt root/etc/ssl/certs/ca-bundle.crt

  cp ${udhcpcScript} root/etc/udhcpc/default.script
  chmod +x root/etc/udhcpc/default.script

  cp ${init} root/init
  chmod +x root/init

  mkdir -p $out
  (cd root && find . -print0 | cpio --null -H newc -o 2>/dev/null \
     | gzip -9 > $out/initramfs.cpio.gz)

  echo ""
  echo "=== initramfs built ==="
  ls -lh $out/initramfs.cpio.gz
  echo ""
  echo "=== /bin (first 40) ==="
  ls root/bin | head -40
  echo ""
  echo "=== /nix/store top-level count ==="
  ls root/nix/store | wc -l
''
