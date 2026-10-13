# DoxIA apps in Rust + GTK 4

DoxIA is moving off the Quickshell shell (`shell/`) one piece at a time; each
piece that moves becomes an app in this Cargo workspace, and its launcher
falls back to the shell piece while the app is not installed.

| Crate | Binary | Replaces |
| --- | --- | --- |
| `doxia-theme` | (library) | `shell/Commons` Color and Style: the look, from the current theme |
| `atualizar` | `doxia-atualizar` | the `omarchy.update` panel (`omarchy-launch-update`) |
| `painel` | `doxia-painel` | the Python/libadwaita Painel Rápido (`fedora/doxia/painel`) |
| `shell` | `doxia-shell` | Quickshell's layer-shell surfaces, one at a time: the OSD so far |

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

## Painel Rápido

Same `painel.json`, credentials, recent pipelines, action types and shortcuts
as the Python app (see the `/painel` skill), in the shell's look.
`fedora/doxia/painel` stays the entry point (`doxia-painel` on the PATH, the
launcher, SUPER+ALT+P): it execs `/usr/local/bin/doxia-painel` when installed
and runs itself otherwise (`DOXIA_PAINEL_PY=1` forces it). HTTP goes through
`ureq` (rustls).

## doxia-shell

The long-running half: a GTK application that owns `org.doxia.Shell` on the
session bus, started by mango's autostart (`default/mango/autostart.conf`)
when installed. Each surface is a GApplication action, so scripts call it
with busctl and fall back to Quickshell when it is not running:

```bash
busctl --user call org.doxia.Shell /org/doxia/Shell org.gtk.Actions \
  Activate 'sava{sv}' osd 1 s '{"icon":"volume","value":"40"}' 0
```

- `osd`: the omarchy-osd payload (`bin/omarchy-osd`), drawn like
  `shell/plugins/osd` on an overlay layer surface (namespace `omarchy-osd`, no
  keyboard, clicks pass through). `osd-close` hides it.

## Font

`base.css` names "DoxIA Shell" (the shell font with Lucide icons) on every
widget, with no `monospace` in the list: omarchy-font-set's fontconfig rule
puts the terminal font first in any list that names `monospace`, and its
icons are the plain Nerd Font ones.

## VMware note

With GTK's GL renderer on vmwgfx, an animated revealer overlaid on the window
stalled the compositor's frames (screencopy hung until the app closed): the
toasts appear without a transition. Check new animations there with a
screenshot, or with `GSK_RENDERER=cairo` to compare.

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
