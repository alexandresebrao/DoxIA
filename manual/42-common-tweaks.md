# Common tweaks

This is a collection of common tailorings to the Omarchy setup. Know that it might occasionally be necessary for system updates to restore certain configs to their original condition. If this happens, your changes won't be lost, but put in a `.bak` file in the same directory.

If you screw something up, you can restore individual configs to their original setup via _Update > Config_ in the Omarchy menu. If you _really_ screw everything up, you can reset all configs via `omarchy-reinstall`.

### Reveal all tray icons all the time

By default, tray icons, like Dropbox, 1password, or Steam, are hidden behind the tray expander arrow, which reveals them when you hover it. If you'd like to have them exposed all the time, right-click the expander arrow to open the tray icon manager, then pin the icons you want to keep visible (you can also hide the ones you never want to see).

### Rounded window corners

Omarchy's own themes use square corners (DoxIA's LiquidIA themes come rounded already). To set the roundness yourself, uncomment this line in `~/.config/mango/looknfeel.conf` and pick a radius. Your file is loaded after the theme, so it wins:

```ini
# Round window corners.
border_radius=8
```

### Remove window gaps

On laptop displays, some people prefer not to waste any pixels on window gaps (or even a top bar, which you can toggle off with `Super + Shift + Space`). You can toggle the gaps off with `Super + Shift + Backspace`, or remove gaps and borders permanently by removing the comments in this section of `~/.config/mango/looknfeel.conf`:

```ini
# No gaps between windows or borders.
gap_inner_horizontal=0
gap_inner_vertical=0
gap_outer_horizontal=0
gap_outer_vertical=0
border_px=0
```
