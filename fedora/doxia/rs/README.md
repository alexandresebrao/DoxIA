# DoxIA apps in Rust + GTK 4

DoxIA is moving off the Quickshell shell (`shell/`) one piece at a time; each
piece that moves becomes an app in this Cargo workspace, and its launcher
falls back to the shell piece while the app is not installed.

| Crate | Binary | Replaces |
| --- | --- | --- |
| `doxia-theme` | (library) | `shell/Commons` Color and Style: the look, from the current theme |
| `atualizar` | `doxia-atualizar` | the `omarchy.update` panel (`omarchy-launch-update`) |

The apps run as their own processes, so the update reloading or restarting
the shell no longer closes them: that is what started this, when the update
window closed and flickered during the "Configuração do DoxIA" step.

## Look

Each app adds the class `doxia` to its windows and passes its stylesheet to
`doxia_theme::Styler`, which puts `doxia-theme/src/base.css` (the shell's
buttons, fields, switches, dropdowns) before it and the color utilities
after, filling in the `$NAMES` from the theme's `colors.toml`, its
`shell.toml` (`[controls]`, `[font]`, `[spacing]`) and mango's
`border_radius`. `Styler::reload` picks up a theme switch.

## Atualizar DoxIA

The update window: same views, texts and actions as the panel. The work stays
in `bin/` (`omarchy-update-run`, `omarchy-update-pref`,
`omarchy-update-check`); the window follows the JSON files described in
`atualizar/src/model.rs`. `--prefs` opens on the preferences, and a second
launch raises the open window.

## Shipping

```bash
sudo dnf install cargo gtk4-devel
fedora/doxia/rs/package          # tests, builds, packs; prints the archive and its sha256
gh release create doxia-rs-<version> fedora/doxia/rs/dist/doxia-rs-<version>-x86_64.tar.gz \
  --target fedora --latest=false
```

Then set `version` and `sha256` in `fedora/doxia/install-rs`, which
`fedora/install-system.sh` and `fedora/update.sh` run. Bump `version` in the
workspace `Cargo.toml` for every release: the archive name carries it.
