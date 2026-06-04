{ lib, runCommand, writeText, closureInfo
, cpio, pigz
, pkgsStatic
, zerolang
, nullLang
, nullang
, nv-pkg
, nv-rebuild
, claude-code
, cacert
, bash
, dropbear
, e2fsprogs
, devSubstrate
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
    # devpts so anything that opens a PTY (dropbear sessions, `script`,
    # tmux-style multiplexers, …) finds /dev/pts/N slaves. devtmpfs above
    # only creates /dev/ptmx; the slave nodes need devpts mounted on
    # /dev/pts. Without this, any PTY-allocating program fails with
    # ENOENT on the slave open and the operation looks unrelated.
    mkdir -p /dev/pts
    mount -t devpts devpts /dev/pts 2>/dev/null || true

    # Silence late kernel info-level messages so they don't disrupt the prompt.
    dmesg -n 1 2>/dev/null || true

    # Persistent /var on the qcow2 attached to virtio-blk (/dev/vda).
    # Probe by trying to mount, not by trusting `blkid`. blkid was
    # found to false-negative on existing qcow2 images here (no udev
    # in the initramfs, and blkid wanted a cache under /run before
    # /run had been created), pushing init into the mkfs branch on
    # every boot — and then `mkfs.ext4` (without -F) would block on
    # its interactive "Proceed anyway?" prompt forever. Mount-first
    # probes actual usability and only formats if mount truly fails.
    VAR_OK=no
    if [ -b /dev/vda ]; then
      if mount -t ext4 /dev/vda /var 2>/dev/null; then
        VAR_OK=yes
      else
        echo "Formatting /dev/vda as ext4 (first boot)..."
        # -F skips the safety prompt. We only get here because the
        # mount above just failed, so there is nothing valid to
        # preserve on /dev/vda anyway.
        if mkfs.ext4 -F -q -L nv-var /dev/vda 2>/dev/null \
           && mount -t ext4 /dev/vda /var 2>/dev/null; then
          VAR_OK=yes
        fi
      fi
      if [ "$VAR_OK" = yes ]; then
        mkdir -p /var/lib /var/log /var/tmp /var/lib/nv-store \
                 /var/lib/nv-system /var/lib/dropbear /var/lib/nv-config
      fi
    fi

    # Phase 1 system layout (CONTRACTS §3.2). /etc/nullvoid/system.null
    # is where the agent declares system state; live it on /var so it
    # survives reboots. Bootstrap an empty generation-0 so /run/current
    # resolves to a real directory before the agent ever runs
    # `nv-rebuild switch` (the first real generation is then generation-1).
    if [ "$VAR_OK" = yes ]; then
      [ -e /etc/nullvoid ] || ln -s /var/lib/nv-config /etc/nullvoid
      if [ ! -e /var/lib/nv-system/current ]; then
        mkdir -p /var/lib/nv-system/generation-0/bin
        ln -snf generation-0 /var/lib/nv-system/current
      fi
    fi
    mkdir -p /run
    ln -snf /var/lib/nv-system/current /run/current

    # ---- Headless capability-verification mode (no secrets) ---------------
    # Triggered by `nvtest` on the kernel cmdline (flake app
    # `verify-capabilities`). PID 1 brings up DHCP (the net slice checks for
    # a default route in the host netns) and runs the Traccia A enforcement
    # tests, then powers off — WITHOUT reaching the ~/.claude / ~/.ssh 9P
    # mounts below. This is the secret-free twin of selfhost-bootstrap.sh: a
    # skeptic reproduces "the declared capability set IS the enforced set" in
    # a single `nix run .#verify-capabilities`.
    if grep -qw nvtest /proc/cmdline 2>/dev/null; then
      export HOME=/root
      export TERM=xterm-256color
      export PATH=/run/current/bin:/bin
      # The seccomp slice compiles its probes with `cc`; ship the alias.
      [ -e /bin/cc ] || ln -sf gcc /bin/cc 2>/dev/null || true
      ifconfig lo up 2>/dev/null
      udhcpc -i eth0 -q -t 5 -T 2 -s /etc/udhcpc/default.script >/dev/null 2>&1
      [ -s /etc/resolv.conf ] || echo "nameserver 10.0.2.3" > /etc/resolv.conf
      echo ""
      echo "=== NullVoidOS headless capability-enforcement harness (nvtest) ==="
      sh /usr/src/nv-tests/run-all.sh
      echo ""
      echo "[nvtest complete — powering off]"
      poweroff -f
      # poweroff -f shouldn't return; loop so PID 1 never dies and panics.
      while true; do sleep 1; done
    fi

    # 9P share: host's ~/.claude/ mounted READ-ONLY at /mnt/claude-ro.
    # THREAT-MODEL (DESIGN.md "Trust model & sandboxing"): the share is
    # readonly=on at the hypervisor (the -virtfs line in flake.nix), so a
    # god-inside agent CANNOT write back into the host's Claude config — no
    # MCP/hook injection that would later run on the HOST. The agent's own
    # Claude home is guest-local on /var: at each boot we copy ONLY the
    # credentials out of the RO share (so a fresh host `claude login` is
    # picked up), while sessions/history/config live on /var and never
    # touch the host. Closes sharp edge (2) of the perimeter-as-jail model.
    CREDS_OK=no
    mkdir -p /mnt/claude-ro
    if mount -t 9p -o trans=virtio,version=9p2000.L,ro,msize=131072 \
         claudefs /mnt/claude-ro 2>/dev/null; then
      if [ "$VAR_OK" = yes ]; then
        CLAUDE_DIR=/var/lib/nv-claude
        CLAUDE_CFG=/var/lib/nv-claude.json
      else
        CLAUDE_DIR=/root/.claude.local
        CLAUDE_CFG=/root/.claude.json.local
      fi
      mkdir -p "$CLAUDE_DIR"
      # Refresh credentials from the RO host share every boot (the host is
      # the source of truth for login; the guest then refreshes its own
      # copy in place during the session, going nowhere near the host).
      if [ -f /mnt/claude-ro/.credentials.json ]; then
        cp /mnt/claude-ro/.credentials.json "$CLAUDE_DIR/.credentials.json"
        CREDS_OK=yes
      fi
      # ~/.claude.json (project trust + MCP + model prefs) lives in $HOME,
      # not in .claude/. Seed it guest-local once from the share's latest
      # backup; thereafter it persists on /var.
      if [ ! -f "$CLAUDE_CFG" ]; then
        LATEST_CFG=$(ls -1t /mnt/claude-ro/backups/.claude.json.backup.* \
                       2>/dev/null | head -1)
        [ -n "$LATEST_CFG" ] && cp "$LATEST_CFG" "$CLAUDE_CFG"
      fi
      umount /mnt/claude-ro 2>/dev/null || true
      # Expose the guest-local copies at the paths Claude Code expects.
      ln -snf "$CLAUDE_DIR" /root/.claude
      ln -snf "$CLAUDE_CFG" /root/.claude.json
    else
      CREDS_OK="no (9P mount failed)"
    fi

    # Outbox — the agent's RW return channel to the host (DESIGN.md "separate
    # writable scratch"). A dedicated host dir, RW, kept separate from the RO
    # credentials share: the agent drops artifacts (diffs/patches/files) in
    # /root/outbox and the host reads them byte-exact, instead of lossy
    # console copy-paste. Plain scratch, inspected as data by the host gate.
    OUTBOX_OK=no
    mkdir -p /root/outbox
    if mount -t 9p -o trans=virtio,version=9p2000.L,msize=131072 \
         outbox /root/outbox 2>/dev/null; then
      OUTBOX_OK=yes
    fi

    # Loopback + DHCP on eth0 (QEMU user networking, gateway 10.0.2.2).
    ifconfig lo up 2>/dev/null
    udhcpc -i eth0 -q -t 5 -T 2 -s /etc/udhcpc/default.script \
      >/dev/null 2>&1
    [ -s /etc/resolv.conf ] || echo "nameserver 10.0.2.3" > /etc/resolv.conf
    IP=$(ip -4 addr show eth0 2>/dev/null | sed -n 's/.*inet \([^ ]*\).*/\1/p')

    # SSH: mount host's ~/.ssh/ via 9P, grab the first .pub key as
    # authorized_keys for root, generate host keys on first boot,
    # start dropbear. Port 22 inside, forwarded to host:2222 by QEMU.
    SSH_OK=no
    mkdir -p /root/.ssh
    chmod 700 /root/.ssh
    if mount -t 9p -o trans=virtio,version=9p2000.L,ro,msize=131072 \
         sshfs /mnt 2>/dev/null; then
      PUB=$(ls /mnt/*.pub 2>/dev/null | head -1)
      if [ -n "$PUB" ]; then
        cp "$PUB" /root/.ssh/authorized_keys
        chmod 600 /root/.ssh/authorized_keys
      fi
      umount /mnt 2>/dev/null
    fi
    if [ "$VAR_OK" = yes ] && [ -s /root/.ssh/authorized_keys ]; then
      # Host key generation (persisted in /var so reboots don't reset
      # them and trigger SSH "host key changed" warnings on host).
      for kt in rsa ecdsa ed25519; do
        kp=/var/lib/dropbear/dropbear_''${kt}_host_key
        if [ ! -f "$kp" ]; then
          dropbearkey -t "$kt" -f "$kp" >/dev/null 2>&1
        fi
      done
      dropbear -E -R -p 22 -r /var/lib/dropbear/dropbear_rsa_host_key \
        -r /var/lib/dropbear/dropbear_ecdsa_host_key \
        -r /var/lib/dropbear/dropbear_ed25519_host_key 2>/dev/null &
      sleep 0.5
      pidof dropbear >/dev/null 2>&1 && SSH_OK=yes
    fi

    # Node.js TLS — claude-code's bundled node looks here for the CA bundle.
    export NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-bundle.crt
    export SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt
    export HOME=/root
    export TERM=xterm-256color
    # /run/current/bin first so a `nv-rebuild switch` immediately shadows
    # /bin equivalents. Empty on first boot (generation-0 bin/ is empty)
    # so this is a no-op until the agent activates a real generation.
    export PATH=/run/current/bin:/bin

    # Egress is general slirp NAT for now (DESIGN.md "Trust model"): the
    # allow-list proxy over guestfwd dropped the guest's TLS flow, so the
    # egress edge stays open on this single-user laptop alpha. Keep the
    # non-essential-traffic switch off as a courtesy (smaller footprint),
    # but it is NOT a containment boundary.
    export CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1

    echo ""
    echo "================================================="
    echo " NullVoidOS bootstrap -- Phase 0 (a) lab edition"
    echo "================================================="
    echo "kernel:   $(uname -srvm)"
    echo "hostname: $(hostname)"
    echo "IP:       ''${IP:-none}"
    echo "zero:     $(zero --version 2>/dev/null || echo missing)"
    echo "claude:   $(claude --version 2>/dev/null || echo missing)"
    echo "null:     $(null --version 2>/dev/null || echo missing)"
    echo "nullang:  $(nullang --version 2>/dev/null || echo missing)"
    echo "nv-pkg:   $(nv-pkg --version 2>/dev/null || echo missing)"
    echo "nv-rbld:  $(nv-rebuild --version 2>/dev/null || echo missing)"
    echo "python:   $(python3 --version 2>/dev/null || echo missing)"
    echo "rustc:    $(rustc --version 2>/dev/null | awk '{print $1,$2}' || echo missing)"
    echo "node:     $(node --version 2>/dev/null || echo missing)"
    echo "gcc:      $(gcc --version 2>/dev/null | head -1 | awk '{print $1,$NF}')"
    echo "creds:    $CREDS_OK"
    echo "/var:     $VAR_OK ($(df -h /var 2>/dev/null | awk 'NR==2 {print $4" free"}'))"
    echo "ssh:      $SSH_OK ($([ "$SSH_OK" = yes ] && echo 'host:2222 -> guest:22' || echo 'disabled'))"
    echo "egress:   general NAT (egress perimeter not yet enforced)"
    echo "creds:    RO from host, guest-local copy on /var (host not writable)"
    echo "outbox:   $OUTBOX_OK  (write artifacts to /root/outbox -> host reads them)"
    echo ""
    echo "Type 'claude' to start the agent."
    echo "From host:  ssh -p 2222 root@localhost  (multi-shell)"
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

  # Headless capability-enforcement runner. Baked into the image and invoked
  # by `init` only when `nvtest` is on the kernel cmdline (flake app
  # `verify-capabilities`). Runs the three Traccia A slices in sequence and
  # prints ONE machine-readable verdict line the host harness greps. Each
  # child test exits 0 on PASS / 1 on FAIL, so we key off the exit code.
  capTestRunner = writeText "nv-tests-run-all" ''
    #!/bin/sh
    set -u
    T=/usr/src/nv-tests
    fail=0
    summary=""
    for n in net fs procrand; do
      printf '\n\n###################### TEST: %s ######################\n' "$n"
      if sh "$T/$n-enforce-test.sh"; then
        summary="$summary $n=PASS"
      else
        summary="$summary $n=FAIL"
        fail=1
      fi
    done
    echo ""
    echo "=================================================================="
    if [ "$fail" -eq 0 ]; then
      echo "NVTEST-VERDICT: PASS (3/3) —$summary"
    else
      echo "NVTEST-VERDICT: FAIL —$summary"
    fi
    echo "=================================================================="
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
    rootPaths = [ claude-code bash dropbear e2fsprogs ] ++ devSubstrate;
  };

  # Minimal /etc/passwd + /etc/group so dropbear and other tools that
  # look up "root" via getpwnam() succeed. Without this, dropbear logs
  # "user root has invalid shell, rejected" and refuses connections.
  passwdFile = writeText "passwd" ''
    root:x:0:0:root:/root:/bin/bash
    nobody:x:65534:65534:nobody:/var/empty:/bin/false
  '';
  groupFile = writeText "group" ''
    root:x:0:
    nobody:x:65534:
  '';
  shadowFile = writeText "shadow" ''
    root::0:0:99999:7:::
    nobody:!:0:0:99999:7:::
  '';
  # /etc/shells — the system's whitelist of valid login shells. Looked
  # up via getusershell(3) by dropbear (and any other service that
  # validates the shell column in /etc/passwd). When /etc/shells is
  # missing, glibc returns a hardcoded default of `{/bin/sh, /bin/csh}`,
  # which does not include `/bin/bash`. Dropbear then rejects every
  # login with "user 'root' has invalid shell, rejected" — and since
  # /etc/passwd points root at /bin/bash (Phase 0 (a) ships bash for
  # claude-code tool-use), SSH from the host is permanently broken
  # without this file.
  shellsFile = writeText "shells" ''
    /bin/sh
    /bin/bash
  '';

  # Nullang's own source, shipped read-only at /usr/src/nullang (Via B of the
  # self-improvement loop). The in-VM agent copies it to a writable dir, adds a
  # builtin within BUILTINS_CONTRACT.md's boundary, `cargo build`s, and swaps
  # /bin/nullang — no GitHub access from the VM, the source travels inside the
  # versioned image ("the knowledge is already in the depot"). `target/` is
  # excluded; everything else (Cargo.toml/lock, src, tests, the contract .md)
  # ships so `cargo build` works offline after the first dependency fetch.
  nullangSrc = lib.cleanSourceWith {
    src = ../system/nullang;
    filter = path: type:
      baseNameOf (toString path) != "target" && lib.cleanSourceFilter path type;
  };

  # Compute /bin symlinks for the developer substrate. Each package
  # contributes whatever it exports in its $out/bin (typically the
  # `pname`-matching binary, but sometimes a whole family — e.g.
  # `gcc`, `g++`, `cc`, `cpp`; `coreutils` brings ~100 GNU tools that
  # override the busybox symlinks for proper GNU semantics).
  devSubstrateBinPaths = lib.concatMapStringsSep " "
    (p: "${p}/bin") devSubstrate;
in
runCommand "nullvoid-initramfs" {
  nativeBuildInputs = [ cpio pigz ];

  meta = with lib; {
    description = "NullVoidOS Phase 0 initramfs (variant a): busybox + zero + claude-code closure";
    platforms = [ "x86_64-linux" ];
  };
} ''
  mkdir -p root/{bin,dev,proc,sys,tmp,root,var,mnt,etc/ssl/certs,etc/udhcpc,etc/dropbear,nix/store,usr/bin}

  cp ${pkgsStatic.busybox}/bin/busybox root/bin/

  # Auto-symlink every applet busybox was compiled with. Avoids the
  # bug where common commands (whoami, date, dmesg, ...) "weren't there
  # because we forgot to add them to a list".
  for applet in $(${pkgsStatic.busybox}/bin/busybox --list); do
    [ -e "root/bin/$applet" ] || ln -s busybox "root/bin/$applet"
  done

  cp ${zerolang}/bin/zero root/bin/

  # Phase 1 system tooling: copied as standalone musl-static binaries.
  # No /nix/store dependency, no closure shipping — they just work.
  cp ${nullLang}/bin/null root/bin/
  cp ${nullang}/bin/nullang root/bin/
  # Ship Nullang's source so the agent can extend it (Via B). Read-only here
  # (initramfs); the agent copies it to /var to edit + build.
  mkdir -p root/usr/src
  cp -r ${nullangSrc} root/usr/src/nullang
  cp ${nv-pkg}/bin/nv-pkg root/bin/
  cp ${nv-rebuild}/bin/nv-rebuild root/bin/

  # Traccia A capability-enforcement tests, baked in so the headless
  # `verify-capabilities` harness runs them with NO host mounts. `init`
  # invokes run-all.sh only when `nvtest` is on the kernel cmdline.
  mkdir -p root/usr/src/nv-tests
  cp ${../system/demos/net-enforce/net-enforce-test.sh}           root/usr/src/nv-tests/net-enforce-test.sh
  cp ${../system/demos/fs-enforce/fs-enforce-test.sh}             root/usr/src/nv-tests/fs-enforce-test.sh
  cp ${../system/demos/procrand-enforce/procrand-enforce-test.sh} root/usr/src/nv-tests/procrand-enforce-test.sh
  cp ${capTestRunner} root/usr/src/nv-tests/run-all.sh
  chmod +x root/usr/src/nv-tests/*.sh

  # Ship the agent runtime closure (claude-code + bash) into /nix/store.
  for p in $(cat ${agentClosure}/store-paths); do
    cp -a "$p" "root/nix/store/$(basename "$p")"
  done

  # Symlink the wrappers into /bin and /usr/bin so they're on PATH.
  ln -s ${claude-code}/bin/claude root/bin/claude
  ln -s ${bash}/bin/bash root/bin/bash
  ln -s /bin/env root/usr/bin/env

  # SSH + filesystem tools. Some of these overlap with busybox applets
  # already symlinked above (blkid, ssh) — force-override so we get the
  # GNU/dropbear versions, which understand more flags and produce more
  # detailed output than the busybox subset.
  for src in ${dropbear}/bin/dropbear ${dropbear}/bin/dropbearkey \
             ${e2fsprogs}/bin/mkfs.ext4 ${e2fsprogs}/bin/blkid \
             ${e2fsprogs}/bin/e2fsck; do
    name=$(basename "$src")
    rm -f "root/bin/$name"
    ln -s "$src" "root/bin/$name"
  done
  rm -f root/bin/ssh
  ln -s ${dropbear}/bin/dbclient root/bin/ssh

  # Developer substrate: symlink everything exported by each package.
  # GNU coreutils overrides busybox applets where they overlap
  # (cp, mv, ls, ...), giving the agent proper GNU semantics.
  for bindir in ${devSubstrateBinPaths}; do
    [ -d "$bindir" ] || continue
    for bin in "$bindir"/*; do
      name=$(basename "$bin")
      rm -f "root/bin/$name"
      ln -s "$bin" "root/bin/$name"
    done
  done

  # CA bundle for Node.js TLS (Claude Code calls api.anthropic.com).
  cp ${cacert}/etc/ssl/certs/ca-bundle.crt root/etc/ssl/certs/ca-bundle.crt

  # /etc/passwd, group, shadow so getpwnam("root") returns something
  # and dropbear accepts the login.
  cp ${passwdFile} root/etc/passwd
  cp ${groupFile}  root/etc/group
  cp ${shadowFile} root/etc/shadow
  cp ${shellsFile} root/etc/shells
  chmod 600 root/etc/shadow

  cp ${udhcpcScript} root/etc/udhcpc/default.script
  chmod +x root/etc/udhcpc/default.script

  cp ${init} root/init
  chmod +x root/init

  mkdir -p $out
  # pigz = parallel gzip: same gzip format (the kernel's RD_GZIP decompresses
  # it identically), but compresses the ~1.1 GB image across all build cores
  # instead of pinning one. `-p $NIX_BUILD_CORES` respects Nix's allocation.
  (cd root && find . -print0 | cpio --null -H newc -o 2>/dev/null \
     | pigz -9 -p ''${NIX_BUILD_CORES:-$(nproc)} > $out/initramfs.cpio.gz)

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
