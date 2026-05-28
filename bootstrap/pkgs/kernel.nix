{ stdenv, fetchurl, lib
, bc, bison, flex, elfutils, openssl, perl, pkg-config, cpio, xz, kmod
}:

stdenv.mkDerivation rec {
  pname = "nullvoid-kernel";
  version = "6.6.141";

  src = fetchurl {
    url = "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${version}.tar.xz";
    sha256 = "3bc2652eb62ab90a90a8b0aa156b91d276f92eb84702971df62fe1a3f1eb7fe1";
  };

  nativeBuildInputs = [
    bc bison flex elfutils openssl perl pkg-config cpio xz kmod
  ];

  enableParallelBuilding = true;

  # Minimal x86_64 kernel: tinyconfig base + just enough to boot in QEMU,
  # mount an external initramfs, expose ttyS0 serial console, run ELF
  # binaries (busybox + zero), speak VirtIO, and reach the network for
  # the agent backend's HTTP calls.
  configurePhase = ''
    runHook preConfigure

    # /usr/bin/env doesn't exist in the Nix build sandbox; rewrite the
    # shebangs in scripts/ to point at the sandbox's bash.
    patchShebangs scripts

    # Start from the absolute minimum the kernel ships.
    make ARCH=x86_64 tinyconfig

    # Promote to 64-bit and turn on kernel logging.
    scripts/config \
      --enable 64BIT \
      --enable X86_64 \
      --enable PRINTK \
      --enable EARLY_PRINTK \
      --disable LOCALVERSION_AUTO

    # Initramfs + ELF/script execution.
    scripts/config \
      --enable BLK_DEV_INITRD \
      --enable RD_GZIP \
      --enable BINFMT_ELF \
      --enable BINFMT_SCRIPT

    # Pseudo-filesystems userland needs.
    scripts/config \
      --enable PROC_FS \
      --enable SYSFS \
      --enable TMPFS \
      --enable DEVTMPFS \
      --enable DEVTMPFS_MOUNT

    # Serial console so QEMU -nographic shows boot + login.
    scripts/config \
      --enable TTY \
      --enable SERIAL_8250 \
      --enable SERIAL_8250_CONSOLE \
      --enable UNIX98_PTYS

    # PCI + VirtIO (QEMU paravirt block / net / console / 9p).
    # PCI_MSI is required by modern virtio devices to actually attach —
    # without it the host announces virtio-net / virtio-9p but the
    # kernel never binds them (symptom: "no channels available").
    #
    # VIRTIO_MENU is the parent Kconfig gate for the virtio transport
    # drivers (VIRTIO_PCI, VIRTIO_BLK, VIRTIO_CONSOLE). Without it,
    # `scripts/config --enable VIRTIO_PCI` is silently dropped by
    # olddefconfig because the symbol isn't visible — symptom: the kernel
    # boots, sees the virtio PCI devices, but never binds a driver.
    scripts/config \
      --enable PCI \
      --enable PCI_MSI \
      --enable VIRTIO \
      --enable VIRTIO_MENU \
      --enable VIRTIO_PCI \
      --enable VIRTIO_BLK \
      --enable VIRTIO_NET \
      --enable VIRTIO_CONSOLE

    # Userspace runtime requirements. tinyconfig switches off almost
    # everything beyond "boot a static binary"; modern glibc + Node.js
    # need real pthread (FUTEX), libuv I/O primitives (eventfd, signalfd,
    # timerfd, epoll, inotify), and at least voluntary preemption.
    # Without FUTEX, every pthread_mutex aborts with
    # "futex facility returned an unexpected error code".
    scripts/config \
      --enable FUTEX \
      --enable EVENTFD \
      --enable SIGNALFD \
      --enable TIMERFD \
      --enable EPOLL \
      --enable INOTIFY_USER \
      --enable AIO \
      --enable POSIX_MQUEUE \
      --enable PREEMPT_VOLUNTARY

    # Networking (the agent backend needs HTTP out).
    scripts/config \
      --enable NET \
      --enable INET \
      --enable PACKET \
      --enable UNIX \
      --enable NETDEVICES \
      --enable ETHERNET

    # Block layer + ext4 for the qcow2 /var persistent disk. tinyconfig
    # explicitly disables CONFIG_BLOCK (no real disk needed to boot from
    # initramfs), which silently masks every block driver and filesystem
    # we would --enable: VIRTIO_BLK, EXT4_FS, JBD2 etc. become "not
    # visible" so olddefconfig drops them.
    scripts/config \
      --enable BLOCK \
      --enable BLK_DEV \
      --enable EXT4_FS \
      --enable EXT4_USE_FOR_EXT2

    # 9P filesystem over VirtIO — host-shared directories. Phase 0 (a)
    # uses this to mount the host's `~/.config/claude/` into the VM at
    # `/root/.config/claude/`, so Claude Code reuses the Max subscription
    # credentials without copying them into the image.
    scripts/config \
      --enable NET_9P \
      --enable NET_9P_VIRTIO \
      --enable 9P_FS \
      --enable 9P_FS_POSIX_ACL

    # ACPI — so `poweroff` from inside the VM actually powers it off
    # (without ACPI the kernel halts the CPU but QEMU stays alive).
    scripts/config \
      --enable ACPI \
      --enable ACPI_BUTTON \
      --enable ACPI_PROCESSOR \
      --enable PNP \
      --enable PNPACPI

    # Default hostname before init takes over.
    scripts/config --set-str DEFAULT_HOSTNAME nullvoid

    # Reconcile: propagate implied options, pick defaults for any newly
    # visible config keys, fail loudly if the result is inconsistent.
    make ARCH=x86_64 olddefconfig

    runHook postConfigure
  '';

  buildPhase = ''
    runHook preBuild
    make ARCH=x86_64 -j$NIX_BUILD_CORES bzImage
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp arch/x86/boot/bzImage $out/bzImage
    cp .config $out/config
    runHook postInstall
  '';

  meta = with lib; {
    description = "Minimal Linux ${version} kernel for NullVoidOS bootstrap (x86_64, VirtIO, serial console)";
    homepage = "https://www.kernel.org";
    license = licenses.gpl2Only;
    platforms = [ "x86_64-linux" ];
  };
}
