/* Gerado pelo omarchy-theme-set a partir de ~/.config/omarchy/themed/gtk3.css.tpl.
   Cores do tema atual nos apps GTK 3 (Evolution etc.). O Adwaita do GTK 3 tem as
   cores compiladas nas regras, então não basta redefinir as cores com nome:
   aqui vão também as regras dos widgets. O hook theme-set.d/gtk3-colors copia
   este arquivo para ~/.config/gtk-3.0/doxia-theme.css, que o gtk.css importa
   (pasta que os flatpaks enxergam). */
@define-color theme_bg_color {{ background }};
@define-color theme_fg_color {{ foreground }};
@define-color theme_base_color {{ background }};
@define-color theme_text_color {{ foreground }};
@define-color theme_selected_bg_color {{ accent }};
@define-color theme_selected_fg_color #ffffff;
@define-color theme_unfocused_bg_color {{ background }};
@define-color theme_unfocused_fg_color {{ foreground }};
@define-color theme_unfocused_base_color {{ background }};
@define-color theme_unfocused_text_color {{ foreground }};
@define-color theme_unfocused_selected_bg_color {{ selection }};
@define-color theme_unfocused_selected_fg_color {{ foreground }};
@define-color insensitive_bg_color {{ dark_background }};
@define-color insensitive_fg_color {{ mix dark_foreground background 30% }};
@define-color insensitive_base_color {{ dark_background }};
@define-color borders {{ mix background foreground 15% }};
@define-color unfocused_borders {{ mix background foreground 12% }};
@define-color accent_color {{ accent }};

/* Superfícies */
window, .background, dialog, messagedialog, assistant,
paned, box, grid, stack, scrolledwindow, viewport, frame > border,
notebook, notebook > stack, notebook > header, actionbar, statusbar, infobar,
.sidebar, placessidebar, stacksidebar, popover, .popover {
  background-color: {{ background }};
  color: {{ foreground }};
}
box, grid, stack, scrolledwindow, viewport, paned { background-color: transparent; }
.view, treeview, treeview.view, iconview, list, listview, textview text,
calendar, .content-view, webkitwebview, scrolledwindow > viewport.frame {
  background-color: {{ background }};
  color: {{ foreground }};
}
.sidebar, placessidebar, stacksidebar, .navigation-sidebar,
list.sidebar, treeview.sidebar { background-color: {{ dark_background }}; }

headerbar, .titlebar, toolbar, .toolbar, menubar, searchbar, searchbar > revealer > box,
.inline-toolbar, .primary-toolbar {
  background: {{ background }};
  color: {{ foreground }};
  border-color: {{ mix background foreground 12% }};
  box-shadow: none;
  text-shadow: none;
}
headerbar:backdrop, .titlebar:backdrop { background: {{ background }}; }

/* Seleção */
*:selected, .view:selected, treeview.view:selected, row:selected,
treeview.view:selected:focus, iconview:selected, calendar:selected,
selection, entry selection, textview text selection, label selection {
  background-color: {{ accent }};
  color: #ffffff;
}
.view:selected:backdrop, treeview.view:selected:backdrop, row:selected:backdrop {
  background-color: {{ selection }};
  color: {{ foreground }};
}
row:hover:not(:selected), treeview.view:hover:not(:selected) { background-color: {{ mix background foreground 6% }}; }
treeview.view header button {
  background: {{ dark_background }};
  color: {{ dark_foreground }};
  border-color: {{ mix background foreground 12% }};
  box-shadow: none;
}
treeview.view { -GtkTreeView-grid-line-color: {{ mix background foreground 10% }}; }

/* Campos e botões */
entry, spinbutton, spinbutton entry, combobox entry, .entry, textview.view {
  background-color: {{ dark_background }};
  color: {{ foreground }};
  border-color: {{ mix background foreground 18% }};
  box-shadow: none;
}
entry:focus, spinbutton:focus { border-color: {{ accent }}; box-shadow: inset 0 0 0 1px {{ accent }}; }
button, combobox button, menubutton button, .linked > button, spinbutton button {
  background: {{ lighter_background }};
  color: {{ foreground }};
  border-color: {{ mix background foreground 18% }};
  box-shadow: none;
  text-shadow: none;
  -gtk-icon-shadow: none;
}
button:hover { background: {{ mix lighter_background foreground 10% }}; }
button:active, button:checked { background: {{ selection }}; color: {{ foreground }}; }
button.flat, toolbar button, headerbar button.flat, .toolbar button { background: transparent; border-color: transparent; }
button.flat:hover, toolbar button:hover, .toolbar button:hover { background: {{ mix background foreground 10% }}; }
button.suggested-action, button.default { background: {{ accent }}; color: #ffffff; border-color: {{ accent }}; }
button.destructive-action { background: {{ red }}; color: #ffffff; border-color: {{ red }}; }
button:disabled, entry:disabled, label:disabled { color: {{ mix dark_foreground background 30% }}; }
button:disabled { background: {{ dark_background }}; }

check, radio { background: {{ dark_background }}; border-color: {{ mix background foreground 25% }}; color: #ffffff; }
check:checked, radio:checked { background: {{ accent }}; border-color: {{ accent }}; }
switch { background: {{ lighter_background }}; border-color: {{ mix background foreground 18% }}; }
switch:checked { background: {{ accent }}; }
switch slider { background: {{ foreground }}; border-color: transparent; }
scale trough, progressbar trough, levelbar trough { background: {{ lighter_background }}; }
scale highlight, progressbar progress, levelbar block.filled { background: {{ accent }}; }
scrollbar, scrollbar trough { background: transparent; border-color: transparent; }
scrollbar slider { background: {{ mix background foreground 30% }}; }
scrollbar slider:hover { background: {{ mix background foreground 45% }}; }

/* Menus e popovers */
menu, .menu, .context-menu, popover, popover.background, popover.menu,
menubar > menuitem > menu, combobox window menu, .csd.popup decoration {
  background-color: {{ lighter_background }};
  color: {{ foreground }};
  border-color: {{ mix background foreground 18% }};
}
menu menuitem:hover, .menu menuitem:hover, menubar > menuitem:hover, modelbutton:hover, popover modelbutton.flat:hover {
  background-color: {{ accent }};
  color: #ffffff;
}
menubar > menuitem:hover { box-shadow: none; }
separator, menu separator { background: {{ mix background foreground 15% }}; color: {{ mix background foreground 15% }}; }
tooltip, tooltip.background { background: {{ lighter_background }}; color: {{ foreground }}; border: 1px solid {{ mix background foreground 18% }}; }
tooltip * { color: {{ foreground }}; }

/* Abas, painéis e bordas */
notebook > header { border-color: {{ mix background foreground 12% }}; }
notebook > header tab:checked { box-shadow: inset 0 -2px {{ accent }}; color: {{ foreground }}; }
notebook > header tab { color: {{ dark_foreground }}; }
paned > separator { background: {{ mix background foreground 12% }}; }
frame > border, .frame, scrolledwindow.frame, scrolledwindow > undershoot { border-color: {{ mix background foreground 12% }}; }
infobar.info > revealer > box, infobar.question > revealer > box { background: {{ lighter_background }}; color: {{ foreground }}; }
infobar.warning > revealer > box { background: {{ mix background yellow 25% }}; color: {{ foreground }}; }
infobar.error > revealer > box { background: {{ mix background red 25% }}; color: {{ foreground }}; }
link, *:link, button.link { color: {{ accent }}; }
.dim-label, label.dim-label { color: {{ dark_foreground }}; }
decoration { box-shadow: none; }
