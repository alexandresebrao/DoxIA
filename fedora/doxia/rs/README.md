# DoxIA apps in Rust + GTK 4

DoxIA is moving off the Quickshell shell (`shell/`) one piece at a time; each
piece that moves becomes an app in this Cargo workspace, and its launcher
falls back to the shell piece while the app is not installed.

| Crate | Binary | Replaces |
| --- | --- | --- |
| `doxia-theme` | (library) | `shell/Commons` Color and Style: the look, from the current theme |
| `atualizar` | `doxia-atualizar` | the `omarchy.update` panel (`omarchy-launch-update`) |
| `painel` | `doxia-painel` | the Python/libadwaita Painel Rápido (`fedora/doxia/painel`) |
| `shell` | `doxia-shell` | Quickshell's layer-shell surfaces, one at a time: the OSD, the notifications, the polkit agent, the lock screen, the Omarchy menu, the Start menu and the pickers (clipboard, emojis, images) so far |

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
session bus, run by the user unit `fedora/doxia/doxia-shell.service`
(installed by `install-user-units`, started by mango's autostart, restarted
if it dies). It answers the Quickshell IPC calls for the targets it has taken
over, through `org.doxia.Shell.Call(target, method, args)` at
`/org/doxia/Shell/Ipc`. `bin/omarchy-shell` asks it first and goes on to
Quickshell on "Target not found." or when it is not running, so callers and
key bindings stay as they are; `DOXIA_SHELL=0` skips it (the tests set it).

```bash
busctl --user call org.doxia.Shell /org/doxia/Shell/Ipc org.doxia.Shell \
  Call ssas notifications toggleDnd 0
```

- `osd`: `show <json>` (the omarchy-osd payload), `close`, `state`, `ping`.
  Drawn like `shell/plugins/osd` on an overlay layer surface (namespace
  `doxia-osd`, no keyboard, clicks pass through). Every surface's namespace
  is `doxia-*`; mango's layer rules (`default/mango/windows.conf`, the
  themes' `mango-looknfeel.conf`) name them next to Quickshell's.
- `notifications`: the `org.freedesktop.Notifications` server and its popups,
  with the IPC of `shell/plugins/notifications` (`dndState`, `toggleDnd`,
  `setDnd`, `showHistory`, `clear`, `dismissAll`, `dismissOne`, `invokeLast`,
  `dismiss <summary>`, `ping`). Same lifetimes, do-not-disturb bypasses,
  replaces_id updates and files under `~/.local/state/omarchy/notifications`
  (popups survive a restart, the last ten are the history), so history carries
  over between the two servers. The bus name has one owner, and a Quickshell
  that ever started its notification service keeps it for as long as it runs,
  dropping what it receives once the plugin is unloaded. So `apply-bar` keeps
  `omarchy.notifications` (and `omarchy.polkit`) in `disabledPlugins` where
  doxia-shell is installed, and back out where it is not; doxia-shell adds
  them too, and if Quickshell still holds the name 10 s after start it
  restarts Quickshell once, only when `omarchy-mango-session-locked` says the
  session is unlocked (`omarchy-restart-shell` re-locks a session it takes
  for locked). The bar's DND indicator
  reads `notifications.json`, which both servers keep.

- polkit: the session's authentication agent, replacing
  `shell/plugins/polkit` (same card over a scrim, label and fingerprint mode).
  It registers with polkitd for the user's display session (logind), and again
  whenever polkitd restarts. Passwords go to polkit's setuid helper
  (`/usr/lib/polkit-1/polkit-agent-helper-1`) over its line protocol, as
  libpolkit-agent does when the helper socket is off (Fedora). Quickshell's
  agent goes the same way as its notifications (`disabledPlugins`; doxia-shell
  retries until polkitd takes the registration).

  Known limit, Quickshell's agent included: apps run by the user manager (all
  of them under uwsm) belong to no logind session. polkit 127 maps such a
  process to the display session when the subject is a pid, but not when it
  carries a pidfd, which pkexec sends, so pkexec from a GUI app finds no
  agent ("Error creating textual authentication agent"). Also, a
  CheckAuthorization with a unix-session subject aborts polkitd 127
  (assertion in push_subject).

- `lock`: the lock screen, replacing `shell/plugins/lock` (`lock`,
  `isLocked`, `status`, `preview`, `hidePreview`). ext-session-lock through
  gtk4-layer-shell's session lock API (FFI to `gtk4-session-lock.h`; the Rust
  crate has no binding), one field per monitor over the blurred wallpaper.
  PAM runs in a child, `doxia-shell --pam <service>` (`lock/pam.rs`, FFI to
  libpam; needs `pam-devel` to build), started through `/proc/self/exe` so it
  still works after an update replaced the binary: the password against
  `omarchy-lock-password`, an enrolled finger against
  `omarchy-lock-fingerprint` in parallel. Same marker
  (`$XDG_RUNTIME_DIR/omarchy-session-locked`), 5 s blank and wake on input as
  the QML service; a lock left behind by a client that died is locked again
  at startup (the stranded-lock check). Quickshell's `omarchy.lock` goes to
  `disabledPlugins` like the other two.

- `shell` (the plugin surface): `summon`, `toggle`, `hide` and `call` for
  the plugins doxia-shell has taken over, `doxia.menu`, `doxia.iniciar`,
  `doxia.clipboard`, `doxia.emojis`, `doxia.image-picker` and
  `doxia.reminders` (Quickshell's `omarchy.*` names for them still work, and
  `bin/omarchy-shell` hands `doxia.*` on to Quickshell under those); for any
  other plugin, or a call without one (`ping`,
  `listPlugins`), it answers "Target not found." and the call goes on to
  Quickshell. Both plugins stay enabled there: the menu's bar button is a
  Quickshell widget, and either menu still opens from Quickshell when
  doxia-shell is not running.

- The Omarchy menu (`menu/`), replacing `shell/plugins/menu`: the same JSONC
  files (`default/omarchy/omarchy-menu.jsonc`, then
  `~/.config/omarchy/extensions/omarchy-menu.jsonc` over it, both watched),
  routes and aliases, search with the divider between this menu's matches
  and deeper ones, the providers (apps, fonts, power profiles), the
  `when:`/`checked:` guards batched into one bash run (`menu/model.rs` is
  `MenuModel.js` with its tests), the select and input prompts of
  `omarchy-menu-select`/`-input` (selection and done files), and Delete to
  uninstall an app. The card is drawn with cairo (`card.rs`): GTK's
  `border-image` ignores `border-radius`. Not carried over: the "Launching…"
  OSD for slow apps.

- The Start menu (`iniciar/`), replacing `fedora/doxia/plugins/doxia.iniciar`:
  the Windows 98 cascade in the menu's colors (the relief is stacked over
  black, as in the QML, so it reads nearly opaque). Programs come from the
  same app list as the menu's Apps, which honors `OnlyShowIn`/`NotShowIn`;
  the QML one read the desktop entries without them, so GNOME-only entries
  such as Yelp no longer show. Each submenu opens level with the row that
  opened it, as the QML meant to.

- The pickers, sharing the menu's card and keys (`popup.rs`):
  - `emojis.rs`, replacing `shell/plugins/emojis`: the grid of the plugin's
    `emojis.json`, filtered by keyword, inserted with
    `omarchy-menu-emoji-insert`.
  - `clipboard/`, replacing `shell/plugins/clipboard`: the history in
    `~/.local/state/omarchy/clipboard-history.json` (`clipboard/model.rs` is
    `ClipboardHistory.js`) and the picker that pastes, copies or opens from
    it. doxia-shell also runs the plugin's watchers (`wl-paste --watch
    capture.sh`), so it turns `omarchy.clipboard` off in shell.json and ends
    Quickshell's watchers before starting its own; two sets would record
    every copy twice. `apply-bar` keeps it off, or back on without
    doxia-shell, like the services above.
  - `images.rs`, replacing `shell/plugins/image-picker`: the carousel of
    `omarchy-menu-images` (theme, background and boot screen switchers),
    drawn with cairo, as the `image-selector` IPC target (`open`, `preload`,
    `cancel`) as well as the plugin.
  - `reminders.rs`, replacing `shell/plugins/reminders`: the minutes, then
    the message, handed to `omarchy-reminder`.

- `background`: the desktop background (`background.rs`), replacing
  `shell/plugins/background` (`refresh`, `set`, `setInstant`, `transition`,
  `themeTransition`). One background-layer window per screen with the image
  cropped to fill it; a new one comes in through the QML's slanted band.
  `themeTransition` (`omarchy-theme-set`) also carries the theme's
  `colors.toml` and `shell.toml`: as the band starts doxia-shell reloads its
  CSS and passes them on to Quickshell's `shell applyTheme` (which does not
  watch the files), so the bar changes with the picture. Double-click picks a
  background, a right double-click a theme.

- The battery warning (`battery.rs`), replacing `shell/services/battery`:
  UPower's display device every 30 s and on every switch to or from battery;
  `omarchy-battery-low` once per discharge at 10% or less, and the profile
  saved for battery or AC (`omarchy-powerprofiles-set`) on each switch.

- `omarchy.background` and `omarchy.battery` go to `disabledPlugins` like the
  services above. Still Quickshell's, because its bar's indicators read them
  directly: `omarchy.idle` (stay awake), `omarchy.nightlight` and
  `omarchy.media`; they move with the bar.

- `doxia-shell --features` lists what the build takes over, for
  `apply-bar` (builds before 0.7.0 reject the option).

- Commands the menus start run in a scope of their own
  (`systemd-run --user --scope`, `run.rs`): doxia-shell is a service, and
  restarting it (the update does) would otherwise end everything it started.

## CSS gotchas

- GTK 4.22's `GtkPasswordEntry` has the CSS name `entry` (class `password`),
  not `passwordentry`.
- A gradient border over a translucent card: use `border-image`; a
  `border-box` background layer shows through the card's fill.

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
