# DoxIA's part of ~/.zshrc, sourced after Oh My Zsh (fedora/doxia/install-zsh
# writes that line), so a git pull updates it: the PATH Fedora's ~/.bashrc sets,
# nvm and SDKMAN!, ~/.zshrc.d and the DoxIA greeting, the same as bash's.

typeset -U path
path=(~/.local/bin ~/bin $path)

source ~/.local/share/omarchy/fedora/doxia/dev-tools.sh

for rc in ~/.zshrc.d/*.zsh(N); do
  source "$rc"
done
unset rc

source ~/.local/share/omarchy/fedora/doxia/greeting.sh
