---
name: omarchy
description: >
  REQUIRED for end-user customization of Linux desktop, window manager, or system config.
  Use when editing ~/.config/mango/, ~/.config/omarchy/,
  ~/.config/alacritty/, ~/.config/foot/, ~/.config/kitty/, or ~/.config/ghostty/.
  Triggers: mango, window rules, animations, keybindings, monitors, gaps, borders,
  blur, opacity, omarchy-shell, bar, terminal config, themes, background,
  night light, idle, lock screen, screenshots, reminders, layer rules, workspace
  settings, display config, and user-facing omarchy commands. Excludes Omarchy
  source development through `omarchy dev link` workflows.
---

# Omarchy Skill

Manage [Omarchy](https://omarchy.org/) Linux systems - a beautiful, modern, opinionated Linux distribution. DoxIA runs it on Fedora with the mango compositor.

This skill is for end-user customization on installed systems.
It is not for contributing to Omarchy source code.

## When This Skill MUST Be Used

**ALWAYS invoke this skill for end-user requests involving ANY of these:**

- Editing ANY file in `~/.config/mango/` (window rules, animations, keybindings, monitors, etc.)
- Editing `~/.config/omarchy/shell.json` (status bar layout, widgets)
- Editing terminal configs (alacritty, foot, kitty, ghostty)
- Editing ANY file in `~/.config/omarchy/`
- Window behavior, animations, opacity, blur, gaps, borders
- Layer rules, workspace settings, display/monitor configuration
- Themes, backgrounds, fonts, appearance changes
- User-facing `omarchy` commands (`omarchy theme ...`, `omarchy refresh ...`, `omarchy restart ...`, etc.)
- Screenshots, screen recording, reminders, night light, idle behavior, lock screen

**If you're about to edit a config file in ~/.config/ on this system, STOP and use this skill first.**

**Do NOT use this skill for Omarchy development tasks** (editing the Omarchy source tree, creating migrations, or running `omarchy dev ...` workflows).

## Topic Guides

Deeper instructions for common areas live next to this file. Read the
matching guide before starting:

- [`mango.md`](mango.md) - keybindings, monitors, window rules, and other mango config
- [`plugins.md`](plugins.md) - the DoxIA shell: bar layout, widgets, idle behavior
- [`theming.md`](theming.md) - themes, backgrounds, and fonts
- [`hooks.md`](hooks.md) - automation hooks that run on system events
- [`capture.md`](capture.md) - screenshots, screen recordings, OCR text capture, and file sharing
- [`contributing.md`](contributing.md) - reporting Omarchy bugs and submitting fixes upstream

## Critical Safety Rules

For privileged commands, follow the Privilege Escalation rules below: `sudo` when a terminal is available for the password prompt, `pkexec` when it is not. Do not wrap commands that already manage privilege elevation themselves.

**For end-user customization tasks, NEVER modify anything in `/usr/share/omarchy/`** - but READING is safe and encouraged.

This directory is owned by the omarchy package. Any local changes will be
overwritten on the next `omarchy update`.

```
/usr/share/omarchy/     # READ-ONLY - NEVER EDIT (reading is OK)
├── bin/                    # Command source (packaged binaries are on PATH)
├── config/                 # Default config templates
├── themes/                 # Stock themes
├── default/                # System defaults
├── shell/                  # Omarchy shell source and defaults
├── migrations/             # Update migrations
└── install/                # Installation scripts
```

**Reading `/usr/share/omarchy/` is SAFE and useful** - do it freely to:
- Understand how omarchy commands work: `omarchy theme set --help` or `cat $(which omarchy-theme-set)`
- See default configs before customizing: `cat "$OMARCHY_PATH/config/omarchy/shell.json"`
- Check stock theme files to copy for customization
- Reference default mango settings: `cat /usr/share/omarchy/default/mango/*.conf`

**Always use these safe locations instead:**
- `~/.config/` - User configuration (safe to edit)
- `~/.config/omarchy/themes/<custom-name>/` - Custom themes
- `~/.config/omarchy/hooks/` - Custom automation hooks

If the request is to develop Omarchy itself, this skill is out of scope. Follow repository development instructions instead of this skill.

## Privilege Escalation

For an interactive script or command run in a visible terminal, use `sudo` for
privileged work. Omarchy may grant passwordless `sudo` access to particular
commands, and the terminal is the appropriate place to request a password
when one is needed.

Use `pkexec` only when the caller cannot interact with a terminal or cannot
enter a password there, such as a command launched by an agent or a graphical
background process. Do not replace `sudo` with `pkexec` merely because a
command changes system state.

## System Architecture

Omarchy is built on:

| Component | Purpose | Config Location |
|-----------|---------|-----------------|
| **Fedora** | Base OS | `/etc/`, `~/.config/` |
| **mango** | Wayland compositor/WM | `~/.config/mango/` |
| **DoxIA shell** | Status bar + notifications (doxia-shell) | `~/.config/omarchy/shell.json` |
| **Launcher/menus** | doxia-shell menu | `~/.config/omarchy/extensions/omarchy-menu.jsonc` |
| **Alacritty/Foot/Kitty/Ghostty** | Terminals | `~/.config/<terminal>/` |
| **Omarchy OSD** | On-screen display | doxia-shell |

## Command Discovery

Omarchy ships a single `omarchy` CLI that dispatches to all `omarchy-*` binaries via `omarchy <group> <action>`. Always prefer this form — it is self-documenting and stable. The underlying `omarchy-*` binaries still exist on `PATH` and remain safe to read for source.

```bash
# List every documented command and its summary (--all includes hidden commands)
omarchy commands

# Show the commands inside a group
omarchy theme --help
omarchy refresh --help
omarchy restart --help

# Show help for a specific command (does not execute it)
omarchy theme set --help

# Machine-readable listing (binary, route, summary, args, aliases)
omarchy commands --json

# Read a command's source to understand it
cat $(which omarchy-theme-set)
```

### Command Groups

Run `omarchy --help` for the full list. The most common groups:

| Group | Purpose | Example |
|-------|---------|---------|
| `omarchy refresh` | Reset config to defaults (backs up first) | `omarchy refresh shell` |
| `omarchy restart` | Restart a service/app | `omarchy restart shell` |
| `omarchy toggle` | Toggle feature on/off | `omarchy toggle nightlight` |
| `omarchy theme` | Theme management | `omarchy theme set <name>` |
| `omarchy bar` | Bar layout and widgets | `omarchy bar move omarchy.clock --section right` |
| `omarchy hook` | Install automation hooks | `omarchy hook install theme-set <script>` |
| `omarchy install` | Install optional software / packages | `omarchy install docker dbs` |
| `omarchy launch` | Launch apps | `omarchy launch browser` |
| `omarchy capture` | Screenshots and recordings | `omarchy capture screenshot` |
| `omarchy reminder` | Desktop notification reminders | `omarchy reminder 15 "Pickup Jack"` |
| `omarchy pkg` | Package management | `omarchy pkg add <pkg>` |
| `omarchy setup` | Interactive setup wizards | `omarchy setup security fingerprint` |
| `omarchy update` | System updates | `omarchy update` |

## Configuration Locations

mango config lives in `~/.config/mango/` — see [`mango.md`](mango.md).
The DoxIA shell (bar, notifications, idle) is configured in
`~/.config/omarchy/shell.json` — see [`plugins.md`](plugins.md).

### Terminals

```
~/.config/alacritty/alacritty.toml
~/.config/foot/foot.ini
~/.config/kitty/kitty.conf
~/.config/ghostty/config
```

**Command:** `omarchy restart terminal`

### Other Configs

| App | Location |
|-----|----------|
| btop | `~/.config/btop/btop.conf` |
| fastfetch | `/etc/fastfetch/config.jsonc` default; `~/.config/fastfetch/config.jsonc` user override |
| lazygit | `~/.config/lazygit/config.yml` |
| starship | `~/.config/starship.toml` |
| git | `~/.config/git/config` |

## Safe Customization Patterns

### Edit User Config Directly

For simple changes, edit files in `~/.config/`:

```bash
# 1. Read current config
cat ~/.config/mango/bindings.conf

# 2. Backup before changes
cp ~/.config/mango/bindings.conf ~/.config/mango/bindings.conf.bak.$(date +%s)

# 3. Make changes with Edit tool

# 4. Apply changes
# - mango: MUST validate with `mango -p`, then apply with `omarchy restart mango`
# - DoxIA shell: shell.json reloads on save
# - Menus/launcher: ~/.config/omarchy/extensions/omarchy-menu.jsonc hot-reloads on save
# - Terminals: apply with `omarchy restart terminal` (reloads running terminals; foot picks changes up in new windows)
```

### Reset to Defaults -- ALWAYS SEEK USER CONFIRMATION BEFORE RUNNING

When customizations go wrong:

```bash
# Reset specific config (creates backup automatically)
omarchy refresh shell
omarchy refresh mango

# The refresh command:
# 1. Backs up current config with timestamp
# 2. Copies default from $OMARCHY_PATH/config/
# 3. Restarts the component where the refresh needs it (e.g. `refresh shell`)
```

## System Commands

```bash
omarchy update                  # Full system update
omarchy version                 # Show Omarchy version
omarchy debug --no-sudo --print # Debug info (ALWAYS use these flags)
omarchy system lock             # Lock screen
omarchy system shutdown         # Shutdown
omarchy system reboot           # Reboot
```

**IMPORTANT:** Always run `omarchy debug` with `--no-sudo --print` flags to avoid interactive sudo prompts that will hang the terminal.

## Troubleshooting

```bash
# Get debug information (ALWAYS use these flags to avoid interactive prompts)
omarchy debug --no-sudo --print

# Reset specific config to defaults
omarchy refresh <app>

# Refresh specific config file
# config-file path is relative to ~/.config/
# eg. `omarchy refresh config mango/config.conf` will refresh ~/.config/mango/config.conf
omarchy refresh config <config-file>

# Full reinstall of configs (nuclear option)
omarchy reinstall
```

## Decision Framework

When user requests system changes:

1. **Is it a stock omarchy command?** Use it directly
2. **Is it a config edit?** Edit in `~/.config/`, never `/usr/share/omarchy/`
3. **Is it a theme customization?** Follow [`theming.md`](theming.md); create a NEW custom theme directory
4. **Is it automation?** Follow [`hooks.md`](hooks.md); use `omarchy hook install` and the hook `.d` directories
5. **Is it a package install?** Use `omarchy pkg add <pkgs...>` (or `omarchy pkg aur add <pkgs...>` for AUR-only packages)
6. **Is it the bar or idle?** Follow [`plugins.md`](plugins.md); use `omarchy bar` or edit shell.json, never the packaged code
7. **Unsure if command exists?** Run `omarchy commands` (or `omarchy <group> --help` for one group)

### Reminder Requests

When the user asks to set a reminder, use `omarchy reminder <minutes> [message]` directly. Convert natural language durations to minutes and title-case short reminder labels when appropriate.

```bash
omarchy reminder 15 "Pickup Jack"
omarchy reminder 60 "Check laundry"
omarchy reminder show
omarchy reminder clear
```

## Out of Scope

This skill intentionally does not cover Omarchy source development. Do not use this skill for:
- Editing files in `/usr/share/omarchy/` (`bin/`, `config/`, `default/`, `shell/`, `themes/`, `migrations/`, etc.)
- Creating or editing migrations
- Running `omarchy dev ...` commands

## Example Requests

- "Change my theme to catppuccin" -> `omarchy theme set catppuccin`
- "Add a keybinding for Super+E to open file manager" -> Check existing bindings first, then add a `bind=` in `~/.config/mango/bindings.conf` (it replaces an Omarchy bind on the same key)
- "Configure my external monitor" -> Edit `~/.config/mango/monitors.conf`
- "Make the window gaps smaller" -> Edit `~/.config/mango/looknfeel.conf`
- "Turn on night light" -> `omarchy toggle nightlight`
- "Set a reminder to pickup jack in 15 minutes" -> `omarchy reminder 15 "Pickup Jack"`
- "Show my reminders" -> `omarchy reminder show`
- "Clear all reminders" -> `omarchy reminder clear`
- "Customize the catppuccin theme colors" -> Overlay: put an edited `colors.toml` in `~/.config/omarchy/themes/catppuccin/`, then re-apply the theme (see `theming.md`)
- "Run a script every time I change themes" -> Install it with `omarchy hook install theme-set <script>`
- "Change how workspace labels are rendered" -> Clone `omarchy.workspaces`, which switches the bar to `<username>.workspaces`, then edit the clone
- "Lock after ten minutes" -> Set `idle.lock` to `600` in `~/.config/omarchy/shell.json`
- "Reset shell/bar to defaults" -> `omarchy refresh shell`
- "Record my screen" -> `omarchy screenrecord --fullscreen`, then `omarchy screenrecord --stop-recording` (see `capture.md`)
- "Report this bug to Omarchy" -> Gather diagnostics and a capture of the problem, then file it (see `contributing.md`)
