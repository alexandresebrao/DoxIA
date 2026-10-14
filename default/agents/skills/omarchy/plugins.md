# DoxIA Shell: Bar, Widgets, and Idle

Read this before changing the status bar, notifications, widgets, or
idle/lock behavior.

The bar, notifications, OSD, menus, pickers, lock screen, and idle timers all
run inside one long-running process, `doxia-shell` (Rust + GTK 4, the user
unit `doxia-shell.service`; source in `$OMARCHY_PATH/fedora/doxia/rs/shell`).
Its widgets are built in: there are no shell plugins to add, clone, or edit.

```
~/.config/omarchy/shell.json             # User overrides: bar, idle
$OMARCHY_PATH/config/omarchy/shell.json  # Canonical defaults
```

doxia-shell rereads `shell.json` on save — no restart needed for layout
changes. `idle.screensaver` and `idle.lock` are seconds since user idle began.

**Commands:** `omarchy restart shell`, `omarchy refresh shell`

## Bar Layout

Use the `omarchy bar` group to place, move, and configure widgets:

```bash
omarchy bar move omarchy.clock --section right
omarchy bar put omarchy.keyboard-layout --after omarchy.clock
omarchy bar remove omarchy.keyboard-layout
omarchy bar set omarchy.clock format HH:mm
```

For layout edits beyond what the commands cover, edit the bar configuration
in `~/.config/omarchy/shell.json`; it reloads on save.

## Idle and Lock

Set `idle.screensaver` and `idle.lock` in `~/.config/omarchy/shell.json`,
in seconds since user idle began. Example: "lock after ten minutes" means
setting `idle.lock` to `600`.
