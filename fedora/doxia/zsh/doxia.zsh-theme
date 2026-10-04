# DoxIA theme for Oh My Zsh (fedora/doxia/install-zsh links it into
# ~/.oh-my-zsh/custom/themes): the folder, the git branch, and the Node.js and
# Java in use (nvm's and SDKMAN!'s, so `nvm use` and `sdk use` show right away);
# a red ❯ below and the exit code on the right when a command fails. Theme colors,
# so the Omarchy theme recolors it; the icons are JetBrainsMono Nerd Font's.

_doxia_versions() {
  [[ -n $NVM_BIN ]] && print -n " %F{2} ${NVM_BIN:h:t}%f"
  [[ -n $JAVA_HOME && -d $JAVA_HOME ]] && print -n " %F{3} ${JAVA_HOME:A:t}%f"
}

PROMPT='%B%F{15}%~%f%b$(git_prompt_info)$(_doxia_versions)
%F{1}❯%f '
RPROMPT='%(?..%F{1}✘ %?%f)'

# Over SSH, who and where first
[[ -n $SSH_CONNECTION ]] && PROMPT="%F{8}%n@%m%f $PROMPT"

ZSH_THEME_GIT_PROMPT_PREFIX=" %F{5} "
ZSH_THEME_GIT_PROMPT_SUFFIX="%f"
ZSH_THEME_GIT_PROMPT_DIRTY=" %F{3}●"
ZSH_THEME_GIT_PROMPT_CLEAN=""
