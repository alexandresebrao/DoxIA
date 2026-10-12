/* Gerado pelo omarchy-theme-set a partir de ~/.config/omarchy/themed/firefox-content.css.tpl.
   Nova aba e página inicial do Firefox nas cores do tema atual: o hook
   theme-set.d/firefox copia este arquivo para chrome/userContent.css de cada
   perfil. Os sites não são tocados. */
@-moz-document url("about:newtab"), url("about:home"), url("about:privatebrowsing") {
  :root:root {
    --newtab-background-color: {{ background }} !important;
    --newtab-background-color-secondary: {{ lighter_background }} !important;
    --newtab-text-primary-color: {{ foreground }} !important;
    --newtab-text-secondary-color: {{ dark_foreground }} !important;
    --newtab-primary-action-background: {{ accent }} !important;
    --newtab-element-hover-color: {{ mix lighter_background foreground 8% }} !important;
    --newtab-element-active-color: {{ mix lighter_background foreground 14% }} !important;
    --color-accent-primary: {{ accent }} !important;
  }
  body {
    background-color: {{ background }} !important;
  }
}
