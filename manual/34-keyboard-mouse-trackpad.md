# Keyboard, Mouse, Trackpad

mango lets you configure all your inputs in great detail. You can change the keyboard repeat to be supersonically fast or make the trackpad use natural scrolling. You change all of it in `~/.config/mango/input.conf`, which you can also reach via _Setup > Input_ in the Omarchy menu (`Super + Space`). Anything you set there replaces Omarchy's defaults.

Here's an example:

```ini
# Use multiple keyboard layouts and switch between them with Left Alt + Right Alt
xkb_rules_layout=us,dk
xkb_rules_options=compose:caps,shift:both_capslock_cancel,grp:alts_toggle

# Change speed of keyboard repeat
repeat_rate=40
repeat_delay=600

# Increase sensitivity for the mouse and the trackpad (-1.0 to 1.0, default: 0)
mouse_accel_speed=0.35
trackpad_accel_speed=0.35

# Use natural (inverse) scrolling on the trackpad
trackpad_natural_scrolling=1

# Use two-finger clicks for right-click instead of lower-right corner
trackpad_click_method=2

# Control the speed of your trackpad scrolling
trackpad_scroll_factor=0.3
```

Check the file with `mango -p` after editing. You can [see all the input options](https://github.com/mangowm/mango/blob/main/docs/configuration/input.md) in the mango documentation.

### Trackpad gestures

Omarchy ships with touchpad gestures already on: swipe with three fingers to move focus between windows, and with four fingers to change workspaces (up or down opens the overview). Add your own in `~/.config/mango/bindings.conf`, on a direction and finger count Omarchy doesn't use yet, like three fingers up or down for the overview:

```ini
gesturebind=none,up,3,toggleoverview
gesturebind=none,down,3,toggleoverview
```

On Dell XPS laptops with a haptic touchpad, you can also set the click strength to low, mid, or high under _Trigger > Hardware > Touchpad Haptics_.

### Typing in Chinese, Japanese, and other languages

DoxIA doesn't run an input method by default. The CapsLock compose sequences don't need one: GTK, Qt, and Chromium apps read `~/.XCompose` themselves, and keeping [fcitx5](https://fcitx-im.org/) out from between the keyboard and every app keeps the ABNT2 dead keys working.

To type Chinese, Japanese, or another language that needs an input method, install fcitx5 with an engine like `fcitx5-mozc` (Japanese) or `fcitx5-chinese-addons` (Chinese), plus `fcitx5-configtool` to add the engine to your input methods and set the key that switches between them:

```bash
omarchy pkg add fcitx5 fcitx5-mozc fcitx5-configtool
```

Then start it with your session by adding this line to `~/.config/mango/autostart.conf`:

```ini
exec_once=fcitx5 -d
```

If dead keys stop working afterwards, remove that line and log in again.

### Use ALT as SUPER

On some keyboards, it's not convenient to use the primary meta key (Windows/cmd key) as SUPER. You can change this to be ALT instead with this line in `~/.config/mango/input.conf`:

```ini
xkb_rules_options=compose:caps,shift:both_capslock_cancel,altwin:swap_alt_win
```
