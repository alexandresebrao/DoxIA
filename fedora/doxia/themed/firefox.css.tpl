/* Gerado pelo omarchy-theme-set a partir de ~/.config/omarchy/themed/firefox.css.tpl.
   Cores do tema atual na interface do Firefox: o hook theme-set.d/firefox copia
   este arquivo para chrome/userChrome.css de cada perfil (o firefox-content.css,
   da nova aba, vai para userContent.css). Só as variáveis do tema padrão do
   Firefox; as páginas ficam como os sites desenham.
   Tudo !important: regras de uma folha de usuário perdem para as do Firefox. */
:root:root {
  /* Faixa das abas (moldura da janela). */
  --toolbox-bgcolor: {{ darker_background }} !important;
  --toolbox-bgcolor-inactive: {{ dark_background }} !important;
  --toolbox-textcolor: {{ foreground }} !important;
  --toolbox-textcolor-inactive: {{ dark_foreground }} !important;
  --lwt-accent-color: {{ darker_background }} !important;
  --lwt-accent-color-inactive: {{ dark_background }} !important;
  --lwt-text-color: {{ foreground }} !important;

  /* Barra de endereços e de favoritos. */
  --toolbar-bgcolor: {{ lighter_background }} !important;
  --toolbar-color: {{ foreground }} !important;
  --toolbar-field-background-color: {{ background }} !important;
  --toolbar-field-color: {{ foreground }} !important;
  --toolbar-field-border-color: transparent !important;
  --toolbar-field-focus-background-color: {{ lighter_background }} !important;
  --toolbar-field-focus-color: {{ bright_foreground }} !important;
  --toolbar-field-focus-border-color: {{ accent }} !important;
  --toolbarbutton-icon-fill: {{ foreground }} !important;
  --toolbarbutton-icon-fill-attention: {{ accent }} !important;
  --toolbarbutton-hover-background: {{ mix lighter_background foreground 10% }} !important;
  --toolbarbutton-active-background: {{ mix lighter_background foreground 18% }} !important;
  --chrome-content-separator-color: {{ mix background foreground 12% }} !important;

  /* Abas. */
  --tab-selected-bgcolor: {{ lighter_background }} !important;
  --tab-selected-textcolor: {{ bright_foreground }} !important;
  --tab-hover-background-color: {{ mix darker_background foreground 8% }} !important;
  --tab-loading-fill: {{ accent }} !important;
  --tab-attention-icon-color: {{ accent }} !important;

  /* Painéis, menus e a lista de sugestões da barra de endereços. */
  --arrowpanel-background: {{ lighter_background }} !important;
  --arrowpanel-color: {{ foreground }} !important;
  --arrowpanel-border-color: {{ mix background foreground 15% }} !important;
  --panel-separator-color: {{ mix background foreground 12% }} !important;
  --urlbarView-highlight-background: {{ accent }} !important;
  --urlbarView-highlight-color: {{ lighter_background }} !important;
  --urlbarView-hover-background: {{ mix lighter_background foreground 8% }} !important;
  --sidebar-background-color: {{ background }} !important;
  --sidebar-text-color: {{ foreground }} !important;
  --sidebar-border-color: {{ mix background foreground 12% }} !important;

  /* Destaque: foco, botões principais, links e caixas marcadas. */
  --focus-outline-color: {{ accent }} !important;
  --color-accent-primary: {{ accent }} !important;
  --color-accent-primary-hover: {{ mix accent bright_foreground 15% }} !important;
  --color-accent-primary-active: {{ mix accent background 20% }} !important;
  --button-primary-bgcolor: {{ accent }} !important;
  --button-primary-hover-bgcolor: {{ mix accent bright_foreground 15% }} !important;
  --button-primary-active-bgcolor: {{ mix accent background 20% }} !important;
  --button-primary-color: {{ lighter_background }} !important;
  --link-color: {{ accent }} !important;
}

/* Abas em pílula, como as janelas do tema: canto arredondado e sem sombra. */
.tab-background {
  border-radius: 8px !important;
}
.tab-background:is([selected], [multiselected]) {
  box-shadow: 0 0 0 1px {{ mix background foreground 12% }} !important;
}
