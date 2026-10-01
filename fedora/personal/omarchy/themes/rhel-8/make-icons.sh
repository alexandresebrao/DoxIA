#!/bin/bash
# Gera ~/.local/share/icons/Papirus-Tela-Red: pastas do Tela em vermelho e todo
# o resto (apps, arquivos) do Papirus-Dark, por herança. Fica no $HOME para
# nenhum update de pacote desfazer. Precisa do papirus-icon-theme-dark e git.
set -euo pipefail

name=Papirus-Tela-Red
color=${1:-red}
tela_rev=a1fffc5bfab716bd022dd228ee96fe3965cdb33d
dest="$HOME/.local/share/icons/$name"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

git -c advice.detachedHead=false clone -q --filter=blob:none https://github.com/vinceliuice/Tela-icon-theme.git "$work/src"
git -C "$work/src" checkout -q "$tela_rev"
(cd "$work/src" && bash install.sh -d "$work/out" "$color" >/dev/null)
tela="$work/out/Tela-$color"

rm -rf "$dest"
mkdir -p "$dest"
dirs=()
for dir in "$tela"/*/places; do
  sub=$(basename "$(dirname "$dir")")
  [[ $sub == symbolic ]] && continue
  mkdir -p "$dest/$sub/places"
  find -L "$dir" -maxdepth 1 \( -name 'folder*.svg' -o -name 'user-*.svg' -o -name 'inode-directory*.svg' \) \
    -exec cp -L {} "$dest/$sub/places/" \;
  dirs+=("$sub/places")
done

{
  echo "[Icon Theme]"
  echo "Name=$name"
  echo "Comment=Pastas do Tela ($color) sobre o Papirus-Dark"
  echo "Inherits=Papirus-Dark,breeze-dark,hicolor"
  echo "Directories=$(IFS=,; echo "${dirs[*]}")"
  for d in "${dirs[@]}"; do
    sub=${d%/places}
    size=${sub%@2x}
    scale=1; [[ $sub == *@2x ]] && scale=2
    echo
    echo "[$d]"
    echo "Context=Places"
    if [[ $size == scalable ]]; then
      echo "Size=64"; echo "MinSize=16"; echo "MaxSize=512"; echo "Type=Scalable"
    else
      echo "Size=$size"; echo "Type=Fixed"
    fi
    echo "Scale=$scale"
  done
} > "$dest/index.theme"

gtk-update-icon-cache -q -f "$dest" 2>/dev/null || true
