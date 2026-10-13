# mango Configuration

Read this before changing keybindings, monitors, window rules, or any other
mango (window manager) configuration.

Omarchy configures mango with plain `key=value` files. `config.conf` sources
Omarchy's defaults first and then the user files, so the same option or key
set later wins:

```
~/.config/mango/
├── config.conf        # Main config (sources Omarchy defaults, then the files below)
├── bindings.conf      # Keybindings
├── monitors.conf      # Display configuration
├── input.conf         # Keyboard/mouse/touchpad settings
├── looknfeel.conf     # Appearance (gaps, borders, corners, animations)
├── autostart.conf     # Startup applications (exec_once=)
└── windows.conf       # Window rules
```

Omarchy's defaults live in `$OMARCHY_PATH/default/mango/` (read them, never edit
them). The current theme adds `~/.local/state/omarchy/current/theme/mango.conf`
(colors) and, when the theme ships one, `mango-looknfeel.conf`.

**Key behaviors:**
- After ANY change, validate with `mango -p` and fix every reported error before going on
- Apply with `omarchy restart mango` (runs `mmsg dispatch reload_config`)
- `source=` paths only expand `~`; `./file` is relative to the file that sources it
- Use `omarchy refresh mango` to reset the files to Omarchy's defaults
- mango's IPC is `mmsg`: `mmsg get all-clients`, `mmsg get all-monitors`, `mmsg dispatch <action>`

## Keybindings

Edit `~/.config/mango/bindings.conf`. Format: `bind=MODS,KEY,action,args`. The
comment line right above a bind is its description in the SUPER+K menu:

```ini
# SSH
bind=SUPER+SHIFT,R,spawn,alacritty -e ssh your-server
# File manager
bind=SUPER,F,spawn,uwsm-app -- nautilus
```

View current bindings: `omarchy menu keybindings --print`

**IMPORTANT: mango has no unbind, and binding the same key twice is a config error.**
`omarchy-mango-bindings` builds Omarchy's default binds minus every key the user
files bind again, so a user bind on a key Omarchy uses replaces it. To drop an
Omarchy bind without rebinding the key, add a comment: `# unbind=SUPER,F`.

When re-binding an existing key:

1. First check existing bindings: `omarchy menu keybindings --print`
2. Add the new bind in `~/.config/mango/bindings.conf` (no unbind needed)
3. Inform the user what the key was previously bound to

## Display/Monitors

Edit `~/.config/mango/monitors.conf`. The name is a regex, so anchor it:

```ini
monitor_rule=name:^eDP-1$,width:1920,height:1080,refresh:60,x:0,y:0,scale:1
monitor_rule=name:^HDMI-A-1$,width:2560,height:1440,refresh:144,x:1920,y:0,scale:1
```

List monitors and supported modes: `wlr-randr`

mango applies the first matching rule to a new output but the last one on
reload; Omarchy's toggles (laptop display off, clamshell, SUPER+/ scale) are
sourced before and after `monitors.conf` so they win in both cases.

## Window Rules

Window rules go in `~/.config/mango/windows.conf`. Format:
`window_rule=option:value,...,app_id:REGEX` (or `title:REGEX`).

```ini
window_rule=is_floating:1,width:1100,height:620,app_id:^org\.gnome\.Calculator$
```

Before writing rules, read the upstream reference for the installed version:
https://github.com/mangowm/mango/blob/main/docs/window-management/rules.md
and the examples in `$OMARCHY_PATH/default/mango/windows.conf`. Get an app's
`appid` and `title` from `mmsg get all-clients`.

## Night light

`omarchy toggle nightlight` (gammastep). It is not part of the mango config.
