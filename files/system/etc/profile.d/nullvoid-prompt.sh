# ---------------- PROMPT ----------------

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
  local RESET='\[\e[0m\]'

# --- Python venv prefix [venv] ---
  local venv=""
  if [[ -n "${VIRTUAL_ENV:-}" ]]; then
    local venv_name
    venv_name="$(basename -- "$VIRTUAL_ENV")"
    venv="${GOLD}[${WHITE}${venv_name}${GOLD}]${RESET} ${PURPLE}"
  fi

  # --- Path con / gialli (NO sed) ---
  local path_colored=""
  local p="${PWD}"

  # home abbreviata stile \w
  if [[ -n "${HOME:-}" && "$p" == "$HOME"* ]]; then
    p="~${p#$HOME}"
  fi

  # costruzione: ogni "/" GOLD, il resto WHITE
  # gestiamo caso root "/" e percorsi normali
  if [[ "$p" == "/" ]]; then
    path_colored="${GOLD}/${WHITE}"
  else
    local rest="$p"
    # se inizia con "/" o "~/" mettiamo il primo separatore
    if [[ "$rest" == /* ]]; then
      path_colored="${GOLD}/${WHITE}"
      rest="${rest#/}"
    elif [[ "$rest" == "~/" ]]; then
      path_colored="${WHITE}~${GOLD}/${WHITE}"
      rest="${rest#~/}"
    elif [[ "$rest" == "~" ]]; then
      path_colored="${WHITE}~"
      rest=""
    fi

    IFS='/' read -r -a parts <<< "$rest"
    local i
    for (( i=0; i<${#parts[@]}; i++ )); do
      [[ -z "${parts[i]}" ]] && continue
      path_colored+="${WHITE}${parts[i]}"
      # aggiungi "/" tra segmenti
      if (( i < ${#parts[@]}-1 )); then
        path_colored+="${GOLD}/${WHITE}"
      fi
    done
  fi

  # RESET all'inizio per evitare "sbavature" colori da output/hook precedenti
  PS1="${RESET}\n${PURPLE}${venv}\u@${target}${RESET} ${GOLD}(${path_colored}${GOLD})${RESET}\n${GOLD}\u2514\u2500\u2205${RESET} "
}

# Non sopprimere altri PROMPT_COMMAND gi� presenti: chain + __prompt per ultimo
__NVX_PRE_PROMPT_COMMAND="${PROMPT_COMMAND:-}"

__nvx_prompt_wrapper() {
  local ec=$?

  if [[ -n "${__NVX_PRE_PROMPT_COMMAND:-}" ]]; then
    eval "$__NVX_PRE_PROMPT_COMMAND"
  fi

  __prompt
  return $ec
}

PROMPT_COMMAND="__nvx_prompt_wrapper"


# ---------------- ALIAS / COLORS ----------------

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

  # se un solo container \u2192 esegui subito
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

  # pi� container \u2192 chiedi
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
