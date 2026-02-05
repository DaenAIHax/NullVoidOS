nvx-where() {
  local cmd="$1"
  [[ -n "${cmd:-}" ]] || { echo "Usage: nvx-where <command>" >&2; return 2; }

  local any=0
  local p

  if p="$(command -v -- "$cmd" 2>/dev/null)"; then
    printf "HOST\tyes\t%s\n" "$p"
    any=1
  else
    printf "HOST\tno\t-\n"
  fi

  mapfile -t NVX_CONTAINERS < <(
    distrobox list --root --no-color 2>/dev/null | awk '
      NR==1 {
        for (i=1; i<=NF; i++) if ($i=="NAME") name_col=i
        next
      }
      NR>1 && name_col>0 && $name_col!="" {
        print $name_col
      }
    ' | sed '/^$/d'
  )

  local c
  for c in "${NVX_CONTAINERS[@]}"; do
    p="$(distrobox-enter --root "$c" -- sh -lc "command -v '$cmd' 2>/dev/null || true")"
    if [[ -n "$p" ]]; then
      printf "%s\tyes\t%s\n" "$c" "$p"
      any=1
    else
      printf "%s\tno\t-\n" "$c"
    fi
  done

  [[ "$any" -eq 1 ]] || {
    echo "❌ '$cmd' is not installed on the host or in any rootful distrobox container." >&2
    return 127
  }
}


command_not_found_handle() {
  nvx-where "$1" || true
  dbx-cmd "$@"
}
