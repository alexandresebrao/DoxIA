-- Painel Rápido (fedora/doxia/painel): janela flutuante centralizada, SUPER+ALT+P abre na ação favorita.
o.window("^org\\.doxia\\.Painel$", { tag = "+floating-window" })
o.bind("SUPER + ALT + P", "Painel Rápido", "doxia-painel --favorito")
