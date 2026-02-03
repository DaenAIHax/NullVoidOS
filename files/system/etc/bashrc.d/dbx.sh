# dbx helper functions

dbx-where() {
  local c="$1"
  echo "HOST:    $(command -v "$c" >/dev/null 2>&1 && echo yes || echo no)"
  echo -n "Kali:    "
  distrobox-enter --root Kali -- sh -lc "command -v '$c' >/dev/null 2>&1 && echo yes || echo no"
  echo -n "sift-lab:"
  distrobox-enter --root sift-lab -- sh -lc "command -v '$c' >/dev/null 2>&1 && echo yes || echo no"
}

command_not_found_handle() { dbx-cmd "$@"; }
