#!/usr/bin/python3
"""Try the wizard's manual partitioning page (manual.py) on its own, without installing.

Needs root (blivet reads the disks). Nothing is written: Concluído prints the check
result, the summary and the part/btrfs lines the wizard would put in answers.ks.

    sudo python3 doxia-blivet.py [--disk NAME...] [--image NAME=FILE]

--disk limits it to these disks (default: all); --image sets up a disk image file as
a disk through a loop device.
"""

import argparse
import sys
from pathlib import Path

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, Gtk  # noqa: E402

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from manual import ManualPartitioning  # noqa: E402


def load_css():
    settings = Gtk.Settings.get_default()
    settings.set_property("gtk-theme-name", "Adwaita")
    settings.set_property("gtk-application-prefer-dark-theme", False)
    settings.set_property("gtk-font-name", "Red Hat Text 10")
    Gtk.IconTheme.get_default().prepend_search_path(str(HERE / "icons"))
    settings.set_property("gtk-icon-theme-name", "DoxIA-Installer")
    for css in (HERE.parent / "doxia-installer.css", HERE / "partitioner.css"):
        provider = Gtk.CssProvider()
        provider.load_from_path(str(css))
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_USER)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--disk", action="append", default=[], metavar="NAME")
    ap.add_argument("--image", action="append", default=[], metavar="NAME=FILE")
    args = ap.parse_args()

    load_css()
    win = Gtk.Window(title="Particionamento manual — DoxIA")
    win.set_default_size(1000, 680)
    win.get_style_context().add_class("desktop")
    stage = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, border_width=20)
    stage.get_style_context().add_class("stage")
    win.add(stage)
    frame = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    frame.get_style_context().add_class("win")
    stage.pack_start(frame, True, True, 0)
    bar = Gtk.Box()
    bar.get_style_context().add_class("titlebar98")
    bar.pack_start(Gtk.Label(label="Instalação do DoxIA", xalign=0), True, True, 0)
    frame.pack_start(bar, False, False, 0)
    body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
    body.get_style_context().add_class("win-body")
    frame.pack_start(body, True, True, 0)
    heading = Gtk.Label(label="Como o disco deve ser dividido?", xalign=0)
    heading.get_style_context().add_class("heading")
    body.pack_start(heading, False, False, 0)

    done = Gtk.Button.new_with_mnemonic("C_oncluído")
    done.get_style_context().add_class("default")
    page = ManualPartitioning(win, args.disk or None, lambda: done.set_sensitive(not page.check()))
    if args.image:
        orig_load = page.load

        def load_with_images():
            import blivet
            real = blivet.Blivet

            def with_images():
                b = real()
                for spec in args.image:
                    name, path = spec.split("=", 1)
                    b.disk_images[name] = path
                return b
            blivet.Blivet = with_images
            page.disk_names = page.disk_names or [s.split("=", 1)[0] for s in args.image]
            orig_load()
        page.load = load_with_images
    body.pack_start(page, True, True, 0)
    body.pack_start(Gtk.Separator(), False, False, 0)
    row = Gtk.Box(spacing=8, halign=Gtk.Align.END)
    cancel = Gtk.Button.new_with_mnemonic("_Cancelar")
    row.pack_start(cancel, False, False, 0)
    row.pack_start(done, False, False, 0)
    body.pack_start(row, False, False, 0)

    def on_done(_b):
        msg = page.check()
        page.error.set_text(msg)
        print("check:", msg or "ok")
        if msg:
            return
        print("\n".join(page.summary()))
        print("ignoredisk --only-use=" + ",".join(page.disks_used()))
        print(f"bootloader --boot-drive={page.boot_disk()}")
        print("\n".join(page.kickstart("SENHA" if "--encrypt" in sys.argv else None)))
        win.destroy()

    done.connect("clicked", on_done)
    cancel.connect("clicked", lambda *_: win.destroy())
    win.connect("destroy", Gtk.main_quit)
    win.show_all()
    page.load()
    Gtk.main()


if __name__ == "__main__":
    main()
