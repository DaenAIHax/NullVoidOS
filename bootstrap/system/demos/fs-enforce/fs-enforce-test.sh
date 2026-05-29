#!/bin/sh
# Traccia A slice — runtime capability enforcement: `!fs.read` end-to-end (Landlock).
#
# Run inside the booted VM (needs the Landlock-enabled kernel). Packages ONE
# probe and declares it as TWO services that differ only in their granted
# filesystem capability:
#
#   fs-granted  requires = [ !net !fs.read."/srv" !tty ]  → may read /srv
#   fs-denied   requires = [ !net !tty ]                  → may NOT read /srv
#
# Both are granted `!net` so neither is put in a netns — the ONLY variable is
# the filesystem capability, isolating what Landlock enforces. `nv-rebuild run`
# builds a Landlock ruleset (deny-by-default: a runtime baseline + the declared
# subtrees) and `restrict_self`s before execve. The probe reads a canary under
# /srv and exits 0 (read) or 7 (denied). Same binary, opposite outcome — decided
# solely by the declared `!fs.read`.

set -u

NAME="fsprobe"
VERSION="0.1.0"
BUILD_DIR="/tmp/${NAME}-build"
PKG_FILE="/tmp/${NAME}-${VERSION}.nvpkg"
CANARY="/srv/nv-canary"

step() { printf '\n=== %s ===\n' "$1"; }

step "0. plant a canary outside the runtime baseline (/srv)"
mkdir -p /srv
echo "TOP SECRET — only a service with !fs.read.\"/srv\" may read this" > "${CANARY}"
echo "  wrote ${CANARY}"

step "1. author the probe payload"
PKG_STAGING="${BUILD_DIR}/pkg-staging"
rm -rf "${BUILD_DIR}"
mkdir -p "${PKG_STAGING}/payload/bin"
cat > "${PKG_STAGING}/payload/bin/${NAME}" <<EOF
#!/bin/sh
# Try to read the canary. Under Landlock, this succeeds only if the launching
# service declared !fs.read."/srv"; otherwise open() returns EACCES.
if content=\$(cat ${CANARY} 2>/dev/null); then
  echo "fsprobe: READ ok — \${content}"
  exit 0
else
  echo "fsprobe: DENIED — cannot read ${CANARY} (Landlock)"
  exit 7
fi
EOF
chmod +x "${PKG_STAGING}/payload/bin/${NAME}"
echo "  wrote payload/bin/${NAME}"

step "2. write manifest.json + nv-pkg install"
NOW_RFC3339=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
cat > "${PKG_STAGING}/manifest.json" <<EOF
{
  "schemaVersion": 1,
  "name": "${NAME}",
  "version": "${VERSION}",
  "description": "Traccia A: reads a canary, gated by Landlock fs confinement",
  "authoredBy": "claude-code (lfs-bootstrap)",
  "createdAt": "${NOW_RFC3339}",
  "deps": [],
  "exposedBins": ["${NAME}"],
  "capabilities": ["fs:read"],
  "sourceLanguage": "sh",
  "buildSteps": []
}
EOF
( cd "${PKG_STAGING}" && tar czf "${PKG_FILE}" manifest.json payload/ )
nv-pkg install "${PKG_FILE}"

step "3. write /etc/nullvoid/system.null — one package, two services"
mkdir -p /etc/nullvoid
cat > /etc/nullvoid/system.null <<EOF
{
  hostname = "nullvoid";
  caps = [ !net !fs.read."/srv" !tty ];
  packages = [ pkgs.${NAME} ];
  services = {
    fs-granted = {
      exec = "/run/current/bin/${NAME}";
      restart = .never;
      requires = [ !net !fs.read."/srv" !tty ];
    };
    fs-denied = {
      exec = "/run/current/bin/${NAME}";
      restart = .never;
      requires = [ !net !tty ];
    };
  };
  environment = {};
}
EOF
cat /etc/nullvoid/system.null

step "4. null check + nv-rebuild switch"
null check /etc/nullvoid/system.null || { echo "FAIL: null check"; exit 1; }
nv-rebuild switch || { echo "FAIL: nv-rebuild switch"; exit 1; }

step "5. run fs-granted (expect: READ ok, exit 0)"
granted_rc=0
nv-rebuild run fs-granted || granted_rc=$?
echo "  → fs-granted exit ${granted_rc}"

step "6. run fs-denied (expect: DENIED, exit 7)"
denied_rc=0
nv-rebuild run fs-denied || denied_rc=$?
echo "  → fs-denied exit ${denied_rc}"

step "VERDICT"
if [ "${granted_rc}" -eq 0 ] && [ "${denied_rc}" -eq 7 ]; then
  echo "PASS — same binary, same package; the declared !fs.read capability is"
  echo "       the enforced set. fs-granted read the canary (exit 0),"
  echo "       fs-denied was blocked by Landlock (exit 7)."
  exit 0
else
  echo "FAIL — granted_rc=${granted_rc} (want 0), denied_rc=${denied_rc} (want 7)"
  exit 1
fi
