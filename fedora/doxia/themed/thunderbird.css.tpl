/* Gerado pelo omarchy-theme-set a partir de ~/.config/omarchy/themed/thunderbird.css.tpl.
   Cores do tema atual no Thunderbird: o hook theme-set.d/thunderbird copia este
   arquivo para chrome/userChrome.css de cada perfil. Só as variáveis que a
   interface do Thunderbird usa; o corpo dos e-mails fica como o remetente mandou.
   Tudo !important: regras de uma folha de usuário perdem para as do Thunderbird. */
:root:root {
  --layout-background-0: {{ background }} !important;
  --layout-background-1: {{ mix background lighter_background 50% }} !important;
  --layout-background-2: {{ lighter_background }} !important;
  --layout-background-3: {{ mix lighter_background foreground 10% }} !important;
  --layout-background-4: {{ mix lighter_background foreground 20% }} !important;
  --layout-color-0: {{ bright_foreground }} !important;
  --layout-color-1: {{ foreground }} !important;
  --layout-color-2: {{ dark_foreground }} !important;
  --layout-color-3: {{ mix dark_foreground background 30% }} !important;
  --layout-border-0: {{ mix background foreground 12% }} !important;
  --layout-border-1: {{ mix background foreground 22% }} !important;
  --layout-border-2: {{ mix background foreground 32% }} !important;

  --selected-item-color: {{ accent }} !important;
  --selected-item-text-color: {{ bright_foreground }} !important;
  --color-primary-default: {{ accent }} !important;
  --color-primary-hover: {{ mix accent bright_foreground 15% }} !important;
  --color-primary-pressed: {{ mix accent background 20% }} !important;
  --color-primary-soft: {{ mix background accent 20% }} !important;
  --tabline-color: {{ accent }} !important;
  --focus-outline-color: {{ accent }} !important;
  --new-folder-color: {{ accent }} !important;

  --color-surface-deep: {{ darker_background }} !important;
  --color-surface-lower: {{ dark_background }} !important;
  --color-surface-base: {{ background }} !important;
  --color-surface-subtle: {{ mix background lighter_background 50% }} !important;
  --color-surface-raised: {{ lighter_background }} !important;
  --color-surface-border: {{ mix background foreground 15% }} !important;
  --color-surface-border-intense: {{ mix background foreground 22% }} !important;
  --color-text-base: {{ foreground }} !important;
  --color-text-secondary: {{ dark_foreground }} !important;
  --color-text-muted: {{ mix dark_foreground background 30% }} !important;
  --color-text-brand: {{ accent }} !important;
  --color-text-highlight: {{ accent }} !important;
  --search-bar-border-color: {{ mix background foreground 22% }} !important;

  --spaces-bg-image: linear-gradient(180deg, {{ accent }}22, transparent 40%) !important;
}

/* Janela de contas (Account Hub) no estilo shadcn em vez de Material: rótulo
   fixo acima do campo em vez de flutuante na borda, campos baixos com canto de
   6 px e anel de foco suave, botões sólidos ou com contorno neutro, títulos em
   semibold, borda fina na janela e sem o brilho colorido por trás. */
:root:root {
  --button-border-radius: 6px !important;
  --hub-input-border-radius: 6px !important;
}

.account-hub-dialog {
  border-radius: 12px !important;
  box-shadow: 0 16px 40px -12px rgba(0, 0, 0, 0.6) !important;

  &::after {
    display: none !important;
  }
}

.account-hub-view:not([hidden]) {
  border-color: {{ mix background foreground 15% }} !important;
  background-color: {{ background }} !important;
  background-image: none !important;
}

#brandingHeader .branding-header-title,
.account-hub-first-run#brandingHeader .branding-header-title {
  font-weight: 600 !important;
  letter-spacing: -0.03em !important;
}

header .sub-view-title {
  font-size: 1.5rem !important;
  font-weight: 600 !important;
  letter-spacing: -0.02em !important;
}

header .sub-view-sub-header {
  color: {{ dark_foreground }} !important;
  font-weight: 400 !important;
}

.input-control:has(label.animated) {
  padding-block-start: 26px !important;
}

form {
  label.animated,
  .input-field:focus + label.animated,
  .input-field:not(:placeholder-shown) + label.animated {
    inset-block-start: 0 !important;
    transform: none !important;
    margin: 0 !important;
    padding: 0 !important;
    background: transparent !important;
    color: {{ foreground }} !important;
    font-size: 0.95rem !important;
    font-weight: 500 !important;
  }

  .input-field,
  .input-textarea,
  .menulist {
    padding-block: 8px !important;
    border: 1px solid {{ mix background foreground 15% }} !important;
    border-radius: 6px !important;
    background-color: {{ dark_background }} !important;
    font-size: 1rem !important;
    font-weight: 400 !important;
  }

  .input-field,
  .input-textarea {
    padding-inline-start: 12px !important;

    &:focus {
      border-color: {{ accent }} !important;
      box-shadow: 0 0 0 3px {{ accent }}40 !important;
    }
  }
}

.account-hub-thundermail-header button {
  border-color: {{ mix background foreground 15% }} !important;
  border-radius: 6px !important;
  color: {{ foreground }} !important;
  font-size: 1rem !important;

  &:hover {
    background-color: {{ lighter_background }} !important;
  }
}

.hub-footer {
  & hr {
    background: {{ mix background foreground 12% }} !important;
  }

  & .footer-button {
    border-radius: 6px !important;
    font-size: 1rem !important;
    padding: 8px 20px !important;
  }
}

.button.secondary-button {
  box-shadow: none !important;
}

::selection {
  background-color: {{ accent }} !important;
  color: {{ bright_foreground }} !important;
}
