# Monitors

Omarchy is designed around a 2x-capable retina-class display. This is what you need to get those nice, crisp programmer fonts. It's what almost all new premium laptops with high-resolution screens are optimized for. It's what you'd want to run on a 27" 5K [Apple Studio Display](https://www.apple.com/studio-display/)/[ProArt PA27JCV](https://www.asus.com/us/displays-desktops/monitors/proart/proart-display-5k-pa27jcv/)/[Samsung S9](https://www.samsung.com/us/computing/monitors/5k/27-viewfinity-s9-5k-monitor-with-thunderbolt-4-matte-display-and-smart-features-ls27c900panxza/)/[Kuycon G27P](https://kuycon.us/monitors/G27P/) or 32" 6K [Apple XDR](https://www.apple.com/pro-display-xdr/)/[ProArt PA32QCV](https://www.asus.com/displays-desktops/monitors/proart/proart-display-6k-pa32qcv/)/[Kuycon G32P](https://kuycon.us/monitors/G32P/).

In DoxIA, mango starts every monitor at 1x, which suits a 1080p or 1440p display. For a high-resolution screen, set the scale in `~/.config/mango/monitors.conf` (via _Setup > Monitors_ in the Omarchy menu). List your outputs and their modes with `wlr-randr`, then add a rule. For example, 2x on a 5K display or 1.6x on a 27" or 32" 4K:

```ini
monitor_rule=name:^DP-1$,width:3840,height:2160,refresh:60,x:0,y:0,scale:1.6
```

The name is a regular expression, so anchor it with `^` and `$` for an exact match. Wayland apps follow the new scale right away; apps running through Xwayland may need a restart.

You can also quickly step through the major monitor scaling ratios (1x, 1.25x, 1.6x, 2x, 3x, 4x) using `Super + /` to go higher and `Super + Alt + /` to go lower. If you have the default configuration, these changes will also persist past reboot.

### Making text bigger or smaller

Monitor scaling changes the size of everything. If all you want is bigger or smaller _text_, there's a single knob for that:

```
omarchy display text size 14
```

That takes a pixel size between 9 and 20, and moves the Omarchy shell, GTK applications, and your terminal together, so the whole desktop stays in proportion. Run it without an argument to see where you're at, and `omarchy display text size reset` to go back to the default. Foot is the one straggler: it has no way to reload its config, so running terminals keep their old size until you open a new one.

### Extending and mirroring laptop displays

When you connect an external screen to your laptop, the display is automatically extended. But you can change that to mirroring instead using _Trigger > Hardware_ in the Omarchy menu or `Super + Ctrl + Alt + Delete`. This is especially helpful if that external screen is a projector, and you want to show something while working.

When you're extending, closing the lid on the laptop will automatically turn off the internal screen. Opening the lid will turn it back on. You can also control this manually using _Trigger > Hardware_ in the Omarchy menu or `Super + Ctrl + Delete`.

### Arranging multiple screens

mango works great with multiple screens. Lay them out with `x:` and `y:` positions in `~/.config/mango/monitors.conf` as `monitor_rule=` lines — the file ships with commented examples for pinning a specific monitor to a resolution, position, and rotation. Read more in [the mango monitor documentation](https://github.com/mangowm/mango/tree/main/docs). `wlr-randr` shows every connected output with its modes and current position.

### Controlling brightness

Monitor brightness is controlled by the dedicated function keys for brightness up/down. If you hold down shift while pressing these, you'll go to maximum or minimum brightness. The keys control the display you're focused on, so external monitors that speak DDC/CI are adjusted the same way as the laptop screen.

### Apple Displays

If you're using an Apple display, the regular keyboard brightness keys will also automatically work, if you're focused on the Apple display. This is done through the `asdcontrol` command.

Note that if you're using an Apple 6K XDR display, you may see a phantom screen in your `wlr-randr` listing. You can turn this off with something like `monitor_rule=name:^DP-2$,disable:1` via _Setup > Monitors_.

On Intel machines, you should be connecting to Apple displays using a regular Thunderbolt cable. On other machines without Thunderbolt, you'll typically have to use a [DP + USB-A -> USB-C cable](https://www.amazon.com/dp/B0BNX7MS6N) to make it work.
