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

# O Desktop do Tela é um monitor claro, bem diferente das outras pastas. Aqui
# ele vira a pasta normal com um monitor branco, como Downloads e Documentos.
monitor='m24 28h16c1.108 0 2 0.892 2 2v9c0 1.108-0.892 2-2 2h-6v2h3c0.554 0 1 0.446 1 1v1h-14v-1c0-0.554 0.446-1 1-1h3v-2h-6c-1.108 0-2-0.892-2-2v-9c0-1.108 0.892-2 2-2z'
for sub in scalable scalable@2x; do
  places="$dest/$sub/places"
  [[ -f $places/folder-download.svg ]] || continue
  for icon in folder-desktop user-desktop; do
    perl -0pe 's{(<path class="ColorScheme-Background" d=")[^"]*(")}{${1}'"$monitor"'${2}}' \
      "$places/folder-download.svg" > "$places/$icon.svg"
  done
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
