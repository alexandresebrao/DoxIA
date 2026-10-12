{
  "manifest_version": 3,
  "name": "DoxIA",
  "description": "Gerado pelo omarchy-theme-set a partir de ~/.config/omarchy/themed/edge-theme.json.tpl: as cores do tema atual no Microsoft Edge. O hook theme-set.d/edge o instala como extensão de tema.",
  "version": "1.0",
  "theme": {
    "colors": {
      "frame": [{{ darker_background_rgb }}],
      "frame_inactive": [{{ dark_background_rgb }}],
      "frame_incognito": [{{ darker_background_rgb }}],
      "frame_incognito_inactive": [{{ dark_background_rgb }}],
      "toolbar": [{{ lighter_background_rgb }}],
      "toolbar_text": [{{ foreground_rgb }}],
      "toolbar_button_icon": [{{ foreground_rgb }}],
      "tab_text": [{{ bright_foreground_rgb }}],
      "tab_background_text": [{{ dark_foreground_rgb }}],
      "tab_background_text_inactive": [{{ dark_foreground_rgb }}],
      "bookmark_text": [{{ foreground_rgb }}],
      "omnibox_background": [{{ background_rgb }}],
      "omnibox_text": [{{ foreground_rgb }}],
      "ntp_background": [{{ background_rgb }}],
      "ntp_text": [{{ foreground_rgb }}],
      "ntp_link": [{{ accent_rgb }}],
      "ntp_header": [{{ accent_rgb }}]
    }
  }
}
