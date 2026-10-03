# DoxIA greeting for interactive bash shells. Linked from ~/.bashrc.d/doxia.sh
# by fedora/update.sh, so a git pull updates it.
[[ $- == *i* ]] || return 0

_doxia_tips=(
  "Omarchy sem Arch: mesma vibe, menos gente perguntando 'btw'."
  "Se o Hyprland não abriu, respire. Se abriu, desconfie."
  "\`sudo dnf upgrade\`: a roleta-russa com changelog."
  "A IA tem inteligência infinita. Sua RAM, não."
  "Rode \`fastfetch\` 3 vezes ao dia. Recomendação médica do r/unixporn."
  "8 deitado + IA = DoxIA. A matemática não mente."
  "Você não está usando Linux. O Linux está usando você. Ai!"
  "8 ou 80? Não, ∞."
)

_doxia_dir=$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")
echo
# The brand as an image where the terminal draws one, the ASCII version elsewhere
if [[ -t 1 ]] && _doxia_protocol=$("$_doxia_dir/term-graphics") &&
  _doxia_brand=$("$_doxia_dir/brand-image" "$_doxia_protocol"); then
  /usr/bin/fastfetch --logo "$_doxia_brand" --logo-type "$_doxia_protocol" \
    --logo-width 46 --logo-position top --logo-padding-left 2 --logo-padding-top 0 --structure break
else
  sed 's/^/  /' "$_doxia_dir/brand.ansi"
fi
printf '\n  \e[38;2;240;240;240mBem-vindo ao Dox\e[38;2;238;0;0mIA\e[38;2;240;240;240m %s (Oito Deitado)\e[0m\n' "$(rpm -E %fedora)"
printf '  \e[2mUptime: %s: mais tempo deitado que o próprio 8.\e[0m\n' "$(uptime -p | sed 's/^up //')"
printf '  \e[38;2;210;210;210m💡 %s\e[0m\n\n' "${_doxia_tips[RANDOM % ${#_doxia_tips[@]}]}"
unset _doxia_tips _doxia_dir _doxia_protocol _doxia_brand
