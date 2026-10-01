#!/bin/sh
# Para o compartilhamento de tela "apertando" o botão "Parar de compartilhar"
# da barra flutuante do Chrome (escondida em special:screenshare): traz a
# janela pro workspace atual, foca, Tab (foca o botão) e Enter. Assim o Meet
# fica sabendo que acabou. Se não houver barra do Chrome ou o stream
# continuar, reinicia o portal, o que corta qualquer outro compartilhamento.
pattern='(está compartilhando|is sharing) '
addr=$(hyprctl -j clients | jq -r --arg p "$pattern" '.[] | select(.title | test($p)) | .address' | head -1)

if [ -n "$addr" ]; then
  ws=$(hyprctl -j activeworkspace | jq -r '.name')
  hyprctl dispatch "hl.dsp.window.move({ workspace = '$ws', follow = false, window = 'address:$addr' })" >/dev/null
  hyprctl dispatch "hl.dsp.focus({ window = 'address:$addr' })" >/dev/null
  sleep 0.3
  if [ "$(hyprctl -j activewindow | jq -r '.address')" = "$addr" ]; then
    wtype -k Tab
    sleep 0.15
    wtype -k Return
  fi
  sleep 1.5
fi

if pw-cli ls Node 2>/dev/null | grep -q 'node.name = "xdg-desktop-portal-hyprland"'; then
  systemctl --user restart xdg-desktop-portal-hyprland
fi
