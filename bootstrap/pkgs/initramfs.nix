{ lib, runCommand, writeText, cpio, gzip, pkgsStatic, zerolang }:

let
  init = writeText "init" ''
    #!/bin/sh

    mount -t proc proc /proc 2>/dev/null
    mount -t sysfs sysfs /sys 2>/dev/null
    mount -t devtmpfs devtmpfs /dev 2>/dev/null || true

    # Silence late kernel info-level messages so they don't disrupt the prompt.
    dmesg -n 1 2>/dev/null || true

    echo ""
    echo "============================================="
    echo " NullVoidOS bootstrap — Phase 0 variant (d)"
    echo "============================================="
    echo ""
    echo "kernel:   $(uname -srvm)"
    echo "hostname: $(hostname)"
    echo "zero:     $(zero --version)"
    echo ""
    echo "Pipeline alive. Dropping to busybox sh."
    echo "Try: zero --help    |    Ctrl-A X to quit QEMU"
    echo ""

    exec /bin/sh
  '';
in
runCommand "nullvoid-initramfs" {
  nativeBuildInputs = [ cpio gzip ];

  meta = with lib; {
    description = "NullVoidOS Phase 0 initramfs (variant d): busybox + zero + sh init";
    platforms = [ "x86_64-linux" ];
  };
} ''
  mkdir -p root/{bin,dev,proc,sys,tmp,root,etc}

  cp ${pkgsStatic.busybox}/bin/busybox root/bin/

  # Auto-symlink every applet busybox was compiled with. Avoids the
  # bug where common commands (whoami, date, dmesg, ...) "weren't there
  # because we forgot to add them to a list".
  for applet in $(${pkgsStatic.busybox}/bin/busybox --list); do
    [ -e "root/bin/$applet" ] || ln -s busybox "root/bin/$applet"
  done

  cp ${zerolang}/bin/zero root/bin/

  cp ${init} root/init
  chmod +x root/init

  mkdir -p $out
  (cd root && find . -print0 | cpio --null -H newc -o 2>/dev/null \
     | gzip -9 > $out/initramfs.cpio.gz)

  echo ""
  echo "=== initramfs built ==="
  ls -lh $out/initramfs.cpio.gz
  echo ""
  echo "=== cpio table of contents ==="
  zcat $out/initramfs.cpio.gz | cpio -t 2>/dev/null | head -40
''
