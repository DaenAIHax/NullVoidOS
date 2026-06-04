#!/bin/sh
# Traccia A slice — runtime capability enforcement: `!proc.spawn` and `!rand`
# via seccomp-bpf. Run inside the booted VM (the kernel already ships SECCOMP).
#
# Two compiled probes, one package, four services that differ only in one
# granted capability each:
#
#   rand-granted   requires = [ !net !rand !tty ]         → getrandom() works  → 0
#   rand-denied    requires = [ !net !tty ]               → getrandom() EPERM  → 7
#   spawn-granted  requires = [ !net !proc.spawn !tty ]   → fork() works       → 0
#   spawn-denied   requires = [ !net !tty ]               → fork() EPERM       → 7
#
# All four are granted `!net` so none is put in a netns — the only variable is
# the seccomp filter `nv-rebuild run` installs (in the child, via pre_exec)
# from the descriptor's `requires`.
#
# NOTE: `!proc.exec` is NOT enforced by this slice. Stateless cBPF cannot allow
# only the launch execve while blocking later ones — that needs seccomp
# USER_NOTIF or a ptrace supervisor. Denying `!proc.spawn` already blocks the
# fork+exec helper pattern, which is the practical case.

set -u

NAME="probes"
VERSION="0.1.0"
BUILD_DIR="/tmp/${NAME}-build"
PKG_FILE="/tmp/${NAME}-${VERSION}.nvpkg"

step() { printf '\n=== %s ===\n' "$1"; }

step "1. author + compile the two probes (cc inside the VM)"
SRC="${BUILD_DIR}/src"
PKG_STAGING="${BUILD_DIR}/pkg-staging"
rm -rf "${BUILD_DIR}"
mkdir -p "${SRC}" "${PKG_STAGING}/payload/bin"

cat > "${SRC}/randprobe.c" <<'EOF'
#include <sys/syscall.h>
#include <unistd.h>
/* Read 16 bytes of kernel randomness via the raw getrandom(2) syscall.
   Under seccomp without !rand, this returns -1/EPERM. */
int main(void) {
    char b[16];
    long r = syscall(SYS_getrandom, b, sizeof b, 0);
    return (r == (long)sizeof b) ? 0 : 7;
}
EOF

cat > "${SRC}/spawnprobe.c" <<'EOF'
#include <unistd.h>
#include <sys/wait.h>
/* fork() a child. glibc implements fork via clone(2); under seccomp without
   !proc.spawn that syscall returns -1/EPERM, so fork() fails. */
int main(void) {
    pid_t p = fork();
    if (p < 0) return 7;
    if (p == 0) _exit(0);
    int st;
    waitpid(p, &st, 0);
    return 0;
}
EOF

cc -O2 -o "${PKG_STAGING}/payload/bin/randprobe" "${SRC}/randprobe.c"
cc -O2 -o "${PKG_STAGING}/payload/bin/spawnprobe" "${SRC}/spawnprobe.c"
echo "  compiled randprobe + spawnprobe"

step "2. manifest.json + nv-pkg install"
NOW_RFC3339=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
cat > "${PKG_STAGING}/manifest.json" <<EOF
{
  "schemaVersion": 1,
  "name": "${NAME}",
  "version": "${VERSION}",
  "description": "Traccia A: getrandom + fork probes, gated by seccomp",
  "authoredBy": "claude-code (lfs-bootstrap)",
  "createdAt": "${NOW_RFC3339}",
  "deps": [],
  "exposedBins": ["randprobe", "spawnprobe"],
  "capabilities": ["rand", "proc:spawn"],
  "sourceLanguage": "c",
  "buildSteps": ["cc -O2"]
}
EOF
( cd "${PKG_STAGING}" && tar czf "${PKG_FILE}" manifest.json payload/ )
nv-pkg install "${PKG_FILE}"

step "3. write /etc/nullvoid/system.null — one package, four services"
mkdir -p /etc/nullvoid
cat > /etc/nullvoid/system.null <<EOF
{
  hostname = "nullvoid";
  caps = [ !net !rand !proc.spawn !tty ];
  packages = [ pkgs.${NAME} ];
  services = {
    rand-granted  = { exec = "/run/current/bin/randprobe";  restart = .never; requires = [ !net !rand !tty ]; };
    rand-denied   = { exec = "/run/current/bin/randprobe";  restart = .never; requires = [ !net !tty ]; };
    spawn-granted = { exec = "/run/current/bin/spawnprobe"; restart = .never; requires = [ !net !proc.spawn !tty ]; };
    spawn-denied  = { exec = "/run/current/bin/spawnprobe"; restart = .never; requires = [ !net !tty ]; };
  };
  environment = {};
}
EOF
cat /etc/nullvoid/system.null

step "4. null check + nv-rebuild switch"
null check /etc/nullvoid/system.null || { echo "FAIL: null check"; exit 1; }
nv-rebuild switch || { echo "FAIL: nv-rebuild switch"; exit 1; }

step "5. run all four services"
rc_rg=0; nv-rebuild run rand-granted  || rc_rg=$?; echo "  → rand-granted  exit ${rc_rg}"
rc_rd=0; nv-rebuild run rand-denied   || rc_rd=$?; echo "  → rand-denied   exit ${rc_rd}"
rc_sg=0; nv-rebuild run spawn-granted || rc_sg=$?; echo "  → spawn-granted exit ${rc_sg}"
rc_sd=0; nv-rebuild run spawn-denied  || rc_sd=$?; echo "  → spawn-denied  exit ${rc_sd}"

step "VERDICT"
if [ "${rc_rg}" -eq 0 ] && [ "${rc_rd}" -eq 7 ] && [ "${rc_sg}" -eq 0 ] && [ "${rc_sd}" -eq 7 ]; then
  echo "PASS — seccomp enforces the declared set: getrandom and fork succeed only"
  echo "       for services that declared !rand / !proc.spawn; the others get EPERM."
  exit 0
else
  echo "FAIL — rand g/d=${rc_rg}/${rc_rd} (want 0/7), spawn g/d=${rc_sg}/${rc_sd} (want 0/7)"
  exit 1
fi
