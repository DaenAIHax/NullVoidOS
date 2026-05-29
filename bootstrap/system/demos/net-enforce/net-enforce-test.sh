#!/bin/sh
# Traccia A slice — runtime capability enforcement: `!net` end-to-end.
#
# Run this inside the booted bootstrap VM (it needs root for `unshare -n`,
# which PID 1 lineage has). It packages ONE probe binary and declares it as
# TWO services that differ only in their granted capabilities:
#
#   net-granted  requires = [ !net !tty ]   → stays in the host netns
#   net-denied   requires = [ !tty ]        → launched in a fresh, empty netns
#
# Same binary, same package. The ONLY difference is the declared capability
# set. `nv-rebuild run <service>` confines each process accordingly, and the
# probe reports whether it can see a network. This is the falsifiable test
# that the *granted* capability set IS the *enforced* set.
#
# Why /proc/net/dev (not /sys/class/net or `ip`): procfs `net` is per-netns
# (it resolves through /proc/self/net to the reader's network namespace), so
# it reflects an `unshare -n` isolation correctly without remounting anything
# and without needing the network to actually be up — it is offline-safe.

set -u

NAME="netprobe"
VERSION="0.1.0"
BUILD_DIR="/tmp/${NAME}-build"
PKG_FILE="/tmp/${NAME}-${VERSION}.nvpkg"

step() { printf '\n=== %s ===\n' "$1"; }

step "1. author the probe payload"
PKG_STAGING="${BUILD_DIR}/pkg-staging"
rm -rf "${BUILD_DIR}"
mkdir -p "${PKG_STAGING}/payload/bin"

cat > "${PKG_STAGING}/payload/bin/${NAME}" <<'EOF'
#!/bin/sh
# "Has a network" == there is a DEFAULT ROUTE in the CURRENT netns.
# /proc/net/route is per-netns. The host netns has a default route via eth0
# (DHCP, gateway 10.0.2.2 under QEMU user-net); a fresh `unshare -n` netns has
# none. We test the route, NOT the mere presence of an interface: a fresh
# netns auto-gets `lo` AND `sit0` (the IPv6-in-IPv4 tunnel device), so
# "any non-lo interface" wrongly reads as "reachable" — the route is the
# semantically correct, offline-safe signal of off-link connectivity.
# Route table line with Destination 00000000 is the default route.
if awk 'NR>1 && $2 == "00000000" { found = 1 } END { exit !found }' /proc/net/route; then
  gw=$(awk 'NR>1 && $2 == "00000000" { print $1; exit }' /proc/net/route)
  echo "netprobe: network REACHABLE — default route via ${gw}"
  exit 0
else
  echo "netprobe: NO network — no default route (isolated namespace)"
  exit 7
fi
EOF
chmod +x "${PKG_STAGING}/payload/bin/${NAME}"
echo "  wrote payload/bin/${NAME}"

step "2. write manifest.json (CONTRACTS §1.2)"
NOW_RFC3339=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
cat > "${PKG_STAGING}/manifest.json" <<EOF
{
  "schemaVersion": 1,
  "name": "${NAME}",
  "version": "${VERSION}",
  "description": "Traccia A: probes network reachability of the current netns",
  "authoredBy": "claude-code (lfs-bootstrap)",
  "createdAt": "${NOW_RFC3339}",
  "deps": [],
  "exposedBins": ["${NAME}"],
  "capabilities": ["net"],
  "sourceLanguage": "sh",
  "buildSteps": []
}
EOF

step "3. tar czf ${PKG_FILE} + nv-pkg install"
( cd "${PKG_STAGING}" && tar czf "${PKG_FILE}" manifest.json payload/ )
nv-pkg install "${PKG_FILE}"

step "4. write /etc/nullvoid/system.null — one package, two services"
mkdir -p /etc/nullvoid
cat > /etc/nullvoid/system.null <<EOF
{
  hostname = "nullvoid";
  caps = [ !net !tty ];
  packages = [ pkgs.${NAME} ];
  services = {
    net-granted = {
      exec = "/run/current/bin/${NAME}";
      restart = .never;
      requires = [ !net !tty ];
    };
    net-denied = {
      exec = "/run/current/bin/${NAME}";
      restart = .never;
      requires = [ !tty ];
    };
  };
  environment = {};
}
EOF
cat /etc/nullvoid/system.null

step "5. null check + nv-rebuild switch"
null check /etc/nullvoid/system.null || { echo "FAIL: null check"; exit 1; }
nv-rebuild check || { echo "FAIL: nv-rebuild check"; exit 1; }
nv-rebuild switch || { echo "FAIL: nv-rebuild switch"; exit 1; }

step "6. inspect the materialised descriptors (requires is persisted)"
GEN=$(readlink -f /run/current)
echo "  active generation: ${GEN}"
for s in net-granted net-denied; do
  echo "  --- etc/services/${s} ---"
  cat "${GEN}/etc/services/${s}"
done

step "7. run net-granted (expect: REACHABLE, exit 0)"
granted_rc=0
nv-rebuild run net-granted || granted_rc=$?
echo "  → net-granted exit ${granted_rc}"

step "8. run net-denied (expect: isolated, exit 7)"
denied_rc=0
nv-rebuild run net-denied || denied_rc=$?
echo "  → net-denied exit ${denied_rc}"

step "VERDICT"
if [ "${granted_rc}" -eq 0 ] && [ "${denied_rc}" -eq 7 ]; then
  echo "PASS — same binary, same package; the declared capability set is the"
  echo "       enforced set. net-granted reached the network (exit 0),"
  echo "       net-denied was confined to an empty netns (exit 7)."
  exit 0
else
  echo "FAIL — granted_rc=${granted_rc} (want 0), denied_rc=${denied_rc} (want 7)"
  exit 1
fi
