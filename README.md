# DoxIA

> **DoxIA is an unofficial Fedora port of [Omarchy](https://github.com/basecamp/omarchy)**,
> maintained by [@alexandresebrao](https://github.com/alexandresebrao). It is not
> affiliated with or supported by Basecamp. Report issues here, not upstream.

Omarchy normally ships as an Arch Linux distribution. DoxIA (Dox + IA, and the
∞ in the logo is an 8 lying down) runs the Omarchy Hyprland + Quickshell desktop on **Fedora**,
installed alongside an existing desktop such as KDE Plasma without touching it. The UI
is translated to **Brazilian Portuguese**.

Commands, paths and config files keep their Omarchy names (`omarchy-*`,
`~/.local/share/omarchy`, `~/.config/omarchy`) so upstream changes keep merging cleanly.

### What changes from upstream

- `dnf` instead of `pacman` in the package helpers (`omarchy-pkg-*`) and update scripts
- Flathub pickers (`omarchy-pkg-flatpak-install` / `-remove`) take the place of the AUR
- Hyprland, Quickshell, uwsm and gpu-screen-recorder come from COPR
- `tuned-ppd` with a small `powerprofilesctl` shim for the power menu
- Lock screen PAM config adapted to Fedora
- Menus, panels, notifications and OSD messages in pt-BR
- DoxIA branding: login session name, login, About and screensaver logos, terminal greeting and MOTD

### Install

```bash
git clone https://github.com/alexandresebrao/ChapeuOS.git ~/.local/share/omarchy
sudo bash ~/.local/share/omarchy/fedora/install-system.sh
bash ~/.local/share/omarchy/fedora/install-user.sh
```

Then log out and pick **DoxIA (Hyprland uwsm)** on the login screen.

To bring an existing install up to date (git pull, then theme, branding, icon font,
menu, MOTD and login screen) without touching the top bar layout or its plugins, run
`bash ~/.local/share/omarchy/fedora/update.sh`.
`Super + K` shows the keybindings and `Super + Space` opens the Omarchy menu.

### Installer ISO

`fedora/iso/build.sh` builds a DoxIA installer: Fedora's netinstall (Anaconda) rebuilt
with lorax under the DoxIA name, with the generic logos instead of Fedora's and an
installer theme in the login screen's colors. Its kickstart sets pt-BR, the Brazilian
keyboard and São Paulo time, leaves the disk, network and user account to the
installer, and clones this repository at the commit the ISO was built from to run
the install scripts above (without the personal setup). Installing needs internet.

```bash
sudo dnf install lorax lorax-templates-generic
sudo bash ~/.local/share/omarchy/fedora/iso/build.sh ~/doxia-iso
```

Create a user account in the installer: the desktop is set up for that user.
`fedora/iso/make-installer-art.py` regenerates the installer images.

### Personal setup

`fedora/personal/` holds my own Omarchy customizations. `install-user.sh` installs
them through `fedora/install-personal.sh`, which can also be re-run on its own:

- **alexandre.fortivpn**: openfortivpn profiles and a connect toggle in the bar (installs
  `openfortivpn` and a polkit helper; credentials stay in `/etc/openfortivpn`)
- **alexandre.javaservers**: start/stop Tomcat 8.5 and JBoss 4.0.2 with the Oracle JDK 8 from SDKMAN
- **alexandre.media**: MPRIS now-playing widget with album art and playback controls
- Bar layout (`shell.json`), default agent and Hyprland window rules

After changing plugins or the bar, run `fedora/sync-personal.sh` and commit.

The user script backs up any existing config it replaces (`*.bak-omarchy-<date>`)
and leaves KDE's autostart, environment and Chromium settings alone.

---

# Omarchy

Omarchy is a beautiful, modern & opinionated Linux distribution by DHH.

Read more at [omarchy.org](https://omarchy.org).

## The Omarchy Manual

The manual lives in [`manual/`](manual/), which is its authoritative source. It's
mirrored to [learn.omacom.io](https://learn.omacom.io/2/the-omarchy-manual), where
its screenshots are also hosted.

- [Welcome to Omarchy!](manual/01-welcome-to-omarchy.md)

**The Basics**

- [Getting Started](manual/02-getting-started.md)
- [Coming From Mac or Windows](manual/03-coming-from-mac-or-windows.md)
- [Navigation](manual/04-navigation.md)
- [The top bar](manual/05-the-top-bar.md)
- [Themes](manual/06-themes.md)
- [Hotkeys](manual/07-hotkeys.md)
- [Unified Clipboard & History](manual/08-unified-clipboard-history.md)
- [Reminders](manual/09-reminders.md)
- [Notices](manual/10-notices.md)
- [Text Extraction & Dictation](manual/11-text-extraction-dictation.md)
- [Screenshots & Recording](manual/12-screenshots-recording.md)
- [Toggles, idle & screensaver](manual/13-toggles-idle-screensaver.md)
- [Omarchy CLI](manual/14-omarchy-cli.md)

**The Applications**

- [Terminal](manual/15-terminal.md)
- [Neovim](manual/16-neovim.md)
- [AI](manual/17-ai.md)
- [Development Tools](manual/18-development-tools.md)
- [Shell Tools](manual/19-shell-tools.md)
- [Shell Functions](manual/20-shell-functions.md)
- [TUIs](manual/21-tuis.md)
- [GUIs](manual/22-guis.md)
- [Browsers](manual/23-browsers.md)
- [Commercial apps/services](manual/24-commercial-apps-services.md)
- [Web Apps](manual/25-web-apps.md)
- [Gaming](manual/26-gaming.md)
- [Filling out PDFs](manual/27-filling-out-pdfs.md)
- [Windows VM](manual/28-windows-vm.md)
- [Other Packages](manual/29-other-packages.md)

**Configuration**

- [Updates](manual/30-updates.md)
- [Dotfiles](manual/31-dotfiles.md)
- [Shell plugins](manual/32-shell-plugins.md)
- [Monitors](manual/33-monitors.md)
- [Keyboard, Mouse, Trackpad](manual/34-keyboard-mouse-trackpad.md)
- [Networking](manual/35-networking.md)
- [System sleep](manual/36-system-sleep.md)
- [Hardware authentication](manual/37-hardware-authentication.md)
- [Fonts](manual/38-fonts.md)
- [Backgrounds](manual/39-backgrounds.md)
- [Prompt](manual/40-prompt.md)
- [Branding](manual/41-branding.md)
- [Common tweaks](manual/42-common-tweaks.md)
- [Making your own theme](manual/43-making-your-own-theme.md)

**The Rest**

- [Mac support](manual/44-mac-support.md)
- [Troubleshooting](manual/45-troubleshooting.md)
- [FAQ](manual/46-faq.md)
- [System snapshots](manual/47-system-snapshots.md)
- [Security](manual/48-security.md)
- [Omarchy on...](manual/49-omarchy-on.md)
- [Dual Boot Install](manual/50-dual-boot-install.md)
- [Unattended Installs](manual/51-unattended-installs.md)

## License

Omarchy is released under the [MIT License](https://opensource.org/licenses/MIT).
