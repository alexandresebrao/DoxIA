# DoxIA: nvm and SDKMAN! in every bash (linked as ~/.bashrc.d/dev-tools.sh by
# fedora/doxia/install-dev-tools).

export NVM_DIR="$HOME/.config/nvm"
[[ -s $NVM_DIR/nvm.sh ]] && . "$NVM_DIR/nvm.sh"
[[ -s $NVM_DIR/bash_completion ]] && . "$NVM_DIR/bash_completion"

export SDKMAN_DIR="$HOME/.sdkman"
[[ -s $SDKMAN_DIR/bin/sdkman-init.sh ]] && . "$SDKMAN_DIR/bin/sdkman-init.sh"
