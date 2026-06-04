#!/bin/sh
# Phase 1 stretch test — build a real Rust ELF inside the VM, package it,
# declare it via the `pkgs.<name>` ambient, switch, and run.
#
# Run this inside the booted bootstrap VM (Phase 0 (a) lab edition).
# It writes everything to /tmp/hello-rust-build/ and leaves the running
# system on generation-N after switching.
#
# What this exercises that the original Phase 1 demo did not:
#   - A real ELF binary, not a bash-script payload
#   - The full Cargo/rustc toolchain from the dev substrate
#   - The `pkgs.hello-rust` ambient projection (CONTRACTS §5.4)
#     instead of the literal "hello-rust-0.1.0" string

set -eu

NAME="hello-rust"
VERSION="0.1.0"
BUILD_DIR="/tmp/${NAME}-build"
PKG_FILE="/tmp/${NAME}-${VERSION}.nvpkg"

step() { printf '\n=== %s ===\n' "$1"; }

step "1. write source tree to ${BUILD_DIR}"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}/src"

cat > "${BUILD_DIR}/Cargo.toml" <<'EOF'
[package]
name = "hello-rust"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "hello-rust"
path = "src/main.rs"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
EOF

cat > "${BUILD_DIR}/src/main.rs" <<'EOF'
use std::env;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let path = env::var("PATH").unwrap_or_else(|_| "<unset>".to_string());
    let argv0 = env::args().next().unwrap_or_else(|| "<unknown>".to_string());

    println!("hello-rust: ELF binary built from Rust, running on NullVoidOS");
    println!("  pid:       {}", process::id());
    println!("  argv[0]:   {}", argv0);
    println!("  unix_ts:   {}", now);
    println!("  path_head: {}", path.split(':').next().unwrap_or(""));
}
EOF
echo "  wrote Cargo.toml + src/main.rs"

step "2. cargo build --release"
cd "${BUILD_DIR}"
# CARGO_HOME on /var so the registry cache survives reboots (and the
# initramfs root isn't writable past a certain budget). /var is the
# persistent ext4 disk.
export CARGO_HOME="/var/lib/cargo"
mkdir -p "${CARGO_HOME}"
cargo build --release --offline 2>&1 | tail -5 || cargo build --release 2>&1 | tail -5
BIN="${BUILD_DIR}/target/release/${NAME}"
[ -x "${BIN}" ] || { echo "build failed: no binary at ${BIN}"; exit 1; }
file "${BIN}" 2>/dev/null || ls -lh "${BIN}"

step "3. assemble .nvpkg layout"
PKG_STAGING="${BUILD_DIR}/pkg-staging"
rm -rf "${PKG_STAGING}"
mkdir -p "${PKG_STAGING}/payload/bin"
cp "${BIN}" "${PKG_STAGING}/payload/bin/${NAME}"
chmod +x "${PKG_STAGING}/payload/bin/${NAME}"

# CONTRACTS §1.2 manifest.json
NOW_RFC3339=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
cat > "${PKG_STAGING}/manifest.json" <<EOF
{
  "schemaVersion": 1,
  "name": "${NAME}",
  "version": "${VERSION}",
  "description": "Phase 1 stretch test: real Rust ELF, exercises pkgs ambient",
  "authoredBy": "claude-code (background, lfs-bootstrap)",
  "createdAt": "${NOW_RFC3339}",
  "deps": [],
  "exposedBins": ["${NAME}"],
  "capabilities": ["tty", "time", "fs:read"],
  "sourceLanguage": "rust",
  "buildSteps": ["cargo build --release"]
}
EOF
ls -lh "${PKG_STAGING}"
ls -lh "${PKG_STAGING}/payload/bin"

step "4. tar czf ${PKG_FILE}"
( cd "${PKG_STAGING}" && tar czf "${PKG_FILE}" manifest.json payload/ )
ls -lh "${PKG_FILE}"

step "5. nv-pkg install"
nv-pkg install "${PKG_FILE}"

step "6. nv-pkg list (--json) to confirm registration"
nv-pkg list --json 2>/dev/null || nv-pkg list

step "7. write /etc/nullvoid/system.null (uses pkgs.hello-rust ambient)"
# CONTRACTS §3.2 path: /etc/nullvoid -> /var/lib/nv-config symlink in init.
mkdir -p /etc/nullvoid
cat > /etc/nullvoid/system.null <<EOF
{
  hostname = "nullvoid";
  caps = [ !tty !time ];
  packages = [ pkgs.${NAME} ];
  services = {};
  environment = {};
}
EOF
cat /etc/nullvoid/system.null

step "8. null check + null eval"
null check /etc/nullvoid/system.null
null eval /etc/nullvoid/system.null

step "9. nv-rebuild check + switch"
nv-rebuild check
nv-rebuild switch

step "10. verify activation"
echo "  /run/current target:"
readlink -f /run/current || ls -la /run/current
echo "  which ${NAME}:"
which "${NAME}" || true
echo "  generation listing:"
nv-rebuild generations

step "11. run ${NAME}"
"${NAME}"

echo ""
echo "=== stretch test passed ==="
