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

    # PCI + VirtIO (QEMU paravirt block / net / console).
    scripts/config \
      --enable PCI \
      --enable VIRTIO \
      --enable VIRTIO_PCI \
      --enable VIRTIO_BLK \
      --enable VIRTIO_NET \
      --enable VIRTIO_CONSOLE

    # Networking (the agent backend needs HTTP out).
    scripts/config \
      --enable NET \
      --enable INET \
      --enable PACKET \
      --enable UNIX \
      --enable NETDEVICES \
      --enable ETHERNET

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
