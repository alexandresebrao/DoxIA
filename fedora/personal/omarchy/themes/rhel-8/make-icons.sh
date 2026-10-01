#!/bin/bash
# Gera ~/.local/share/icons/Papirus-Dark-Red: herda o Papirus-Dark e só troca
# as pastas azuis pelas vermelhas (vermelho Red Hat). Fica no $HOME porque
# mexer nos links de /usr/share/icons/Papirus seria desfeito a cada update.
set -euo pipefail

color=${1:-red}
src=/usr/share/icons
dest="$HOME/.local/share/icons/Papirus-Dark-Red"

rm -rf "$dest"
mkdir -p "$dest"
sed -e 's/^Name=.*/Name=Papirus-Dark-Red/' \
    -e 's/^Inherits=.*/Inherits=Papirus-Dark,breeze-dark,hicolor/' \
    "$src/Papirus-Dark/index.theme" > "$dest/index.theme"

for base in Papirus Papirus-Dark; do
  for dir in "$src/$base"/*/places; do
    size=$(basename "$(dirname "$dir")")
    mkdir -p "$dest/$size/places"
    find "$dir" -maxdepth 1 -type l -lname '*-blue*' | while read -r link; do
      target=$(readlink "$link")
      red="${target/-blue/-$color}"
      [[ -e $dir/$red ]] || continue
      ln -sfn "$dir/$red" "$dest/$size/places/$(basename "$link")"
    done
  done
done

gtk-update-icon-cache -q -f "$dest" 2>/dev/null || true
