echo "Sync GTK's dark mode setting with the current theme"

# A KDE leftover (gtk-application-prefer-dark-theme=true) darkened what the
# apps' CSS leaves to GTK in a light theme; the theme-set hook keeps it in step
# from now on, and this sets it once for the theme already in place.
bash "$OMARCHY_PATH/fedora/doxia/hooks/theme-set.d/gtk-dark-mode"
