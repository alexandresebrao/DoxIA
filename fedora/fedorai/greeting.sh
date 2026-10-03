# FedorAI greeting for interactive bash shells. Linked from ~/.bashrc.d/fedorai.sh
# by fedora/install-branding.sh, so a git pull updates it.
[[ $- == *i* ]] || return 0

_fedorai_tips=(
  "Omarchy sem Arch: mesma vibe, menos gente perguntando 'btw'."
  "Se o Hyprland não abriu, respire. Se abriu, desconfie."
  "\`sudo dnf upgrade\`: a roleta-russa com changelog."
  "A IA tem inteligência infinita. Sua RAM, não."
  "Rode \`fastfetch\` 3 vezes ao dia. Recomendação médica do r/unixporn."
  "Fedora + IA = FedorAI. A matemática não mente."
  "Você não está usando Linux. O Linux está usando você. Ai!"
  "8 ou 80? Não, ∞."
)

printf '\n  \e[1;35m∞ Bem-vindo ao FedorAI %s (Oito Deitado)\e[0m\n' "$(rpm -E %fedora)"
printf '  \e[2mUptime: %s: mais tempo deitado que o próprio 8.\e[0m\n' "$(uptime -p | sed 's/^up //')"
printf '  \e[36m💡 %s\e[0m\n\n' "${_fedorai_tips[RANDOM % ${#_fedorai_tips[@]}]}"
unset _fedorai_tips
