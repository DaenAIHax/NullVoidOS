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
  local GOLD='\[\e[38;2;241;250;140m\]'
  local WHITE='\[\e[38;2;248;248;242m\]'
  local GRAY='\[\e[38;2;200;200;200m\]'
  local RESET='\[\e[0m\]'

  PS1="\n${PURPLE}\u@${target}${RESET} ${GOLD}(${WHITE}\w${GOLD})${RESET}\n${GOLD}└─∅${RESET} "
}

PROMPT_COMMAND="__prompt"


alias ls='ls --color=auto'
export LS_COLORS='di=38;2;241;250;140:fi=38;2;200;200;200:ex=38;2;241;250;140:ln=38;2;189;147;249:or=38;2;255;85;85:mi=38;2;255;85;85'
