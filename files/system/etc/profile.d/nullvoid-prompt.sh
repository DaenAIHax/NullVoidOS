__prompt() {
  local target

  # nome container se siamo in container, altrimenti hostname host
  if [ -f /run/.containerenv ]; then
    target="$(sed -n 's/^name="\([^"]*\)"/\1/p' /run/.containerenv)"
    [ -z "$target" ] && target="\h"
  else
    target="\h"
  fi

  local PURPLE='\[\e[38;2;189;147;249m\]'
  local GOLD='\[\e[38;2;255;215;0m\]'
  local WHITE='\[\e[38;2;248;248;242m\]'
  local GRAY='\[\e[38;2;200;200;200m\]'
  local RESET='\[\e[0m\]'

  local path_colored
  path_colored="$(printf '%s' "$PWD" | sed "s|/|${GOLD}/${WHITE}|g")"

  PS1="\n${PURPLE}\u@${target}${RESET} ${GOLD}(${WHITE}\w${GOLD})${RESET}\n${GOLD}└─∅${RESET} "
}

PROMPT_COMMAND="__prompt"


alias ls='ls --color=auto'
export LS_COLORS='di=38;2;241;250;140:fi=38;2;200;200;200:ex=38;2;241;250;140:ln=38;2;189;147;249:or=38;2;255;85;85:mi=38;2;255;85;85'

# ---- NVX automatic command lookup & exec ----
command_not_found_handle() {
  local cmd="$1"; shift || true

  # evita loop
  [[ "${NVX_CNF_GUARD:-}" == "1" ]] && return 127
  export NVX_CNF_GUARD=1

  # se nvx non esiste, fallback normale
  command -v nvx-where >/dev/null 2>&1 || return 127

  # cerca il comando nei container
  local hits
  hits="$(nvx-where "$cmd" 2>/dev/null | awk -F'\t' 'NR>1 && $2=="yes"{print $1}')"

  [[ -n "$hits" ]] || return 127

  # se un solo container → esegui subito
  local count
  count="$(printf "%s\n" "$hits" | wc -l | tr -d ' ')"

  if [[ "$count" -eq 1 ]]; then
    local box="$hits"

    if distrobox list --root --no-color 2>/dev/null | grep -qE "[|][[:space:]]*$box[[:space:]]*[|]"; then
      distrobox-enter --root "$box" -- "$cmd" "$@" </dev/null
    else
      distrobox-enter "$box" -- "$cmd" "$@" </dev/null
    fi
    return $?
  fi

  # più container → chiedi
  echo "Command '$cmd' found in multiple containers:"
  local i=1
  printf "%s\n" "$hits" | while read -r b; do
    printf "%d) %s\n" "$i" "$b"
    i=$((i+1))
  done

  printf "#? "
  read -r choice
  [[ "$choice" =~ ^[0-9]+$ ]] || return 127

  local box
  box="$(printf "%s\n" "$hits" | sed -n "${choice}p")"
  [[ -n "$box" ]] || return 127

  if distrobox list --root --no-color 2>/dev/null | grep -qE "[|][[:space:]]*$box[[:space:]]*[|]"; then
    distrobox-enter --root "$box" -- "$cmd" "$@" </dev/null
  else
    distrobox-enter "$box" -- "$cmd" "$@" </dev/null
  fi

  return $?
}
