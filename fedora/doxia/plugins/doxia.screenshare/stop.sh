#!/bin/sh
# Para o compartilhamento de tela "apertando" o botão "Parar de compartilhar"
# da barra flutuante do Chrome (escondida na tag especial do mango): traz a
# janela pra tag atual, foca, Tab (foca o botão) e Enter. Assim o Meet fica
# sabendo que acabou. Se não houver barra do Chrome ou o stream continuar,
# reinicia o portal, o que corta qualquer outro compartilhamento.
pattern='(está compartilhando|is sharing) '
id=$(mmsg get all-clients | jq -r --arg p "$pattern" 'first(.clients[] | select(.title | test($p)) | .id) // empty')

if [ -n "$id" ]; then
  tag=$(mmsg get all-monitors | jq -r 'first(.monitors[] | select(.active == true) | .active_tags[0]) // 1')
  mmsg dispatch "tagsilent,$tag" "client,$id" >/dev/null
  mmsg dispatch focusid "client,$id" >/dev/null
  sleep 0.3
  if [ "$(mmsg get focusing-client | jq -r '.id')" = "$id" ]; then
    wtype -k Tab
    sleep 0.15
    wtype -k Return
  fi
  sleep 1.5
fi

if pw-cli ls Node 2>/dev/null | grep -q 'node.name = "xdg-desktop-portal-wlr"'; then
  systemctl --user restart xdg-desktop-portal-wlr
fi
