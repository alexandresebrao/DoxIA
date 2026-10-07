"""Manual partitioning for the DoxIA installer: blivet-gui inside the wizard's window.

blivet-gui's installer mode (the one Anaconda embeds as its Blivet-GUI spoke) is put
in a Gtk.Box with the wizard's look: disks as a horizontal strip of cards instead of
blivet-gui's side list, and its logical view as a thin bar over the partition table.
The person creates, deletes and resizes partitions and picks mount points; nothing
touches the disks until apply(), which the wizard calls once Instalar is clicked
(from %pre, before Anaconda scans the storage). kickstart() then turns the mount
points into part/btrfs lines for answers.ks: what was marked to be formatted,
Anaconda formats (a Btrfs / gets the root and home subvolumes, like autopart),
the rest is used as it is (--noformat), like a Windows EFI partition.
"""

import re
from pathlib import Path

import gi
gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk, Pango  # noqa: E402

UEFI = Path("/sys/firmware/efi").exists()
BTRFS_LABEL = "doxia"
# Partitions Anaconda formats encrypted when the wizard's Encrypt box is checked
UNENCRYPTED = {"/boot", "/boot/efi"}


class ManualPartitioning(Gtk.Box):
    """The Partitioning page body. load() reads the disks in the background."""

    def __init__(self, window, disk_names, on_change):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.window = window
        self.disk_names = disk_names
        self.on_change = on_change
        self.storage = None
        self.bg = None
        self.loading = False
        self._rebuild_pending = False
        self._syncing = False
        self._groups = []  # [(tab button, items box, [(row index, item button)])]

        # blivet-gui calls these on the window it is given (Anaconda dims itself)
        for name in ("lightbox_on", "lightbox_off"):
            if not hasattr(window, name):
                setattr(window, name, lambda: None)

        # Tabs for blivet-gui's groups (Discos, Volumes Btrfs, LVM...), and under them
        # the devices of the chosen one
        strip = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        strip.get_style_context().add_class("disk-strip")
        self.tabs = Gtk.Box(spacing=2)
        self.tabs.get_style_context().add_class("disk-tabs")
        strip.pack_start(self.tabs, False, False, 0)
        self.items = Gtk.Stack()
        self.items.get_style_context().add_class("disk-items")
        self.items.set_transition_type(Gtk.StackTransitionType.NONE)
        strip.pack_start(self.items, False, False, 0)
        self.pack_start(strip, False, False, 0)

        self.holder = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.pack_start(self.holder, True, True, 0)
        self.status = Gtk.Label(label="Lendo os discos...", xalign=0.5, yalign=0.5)
        self.status.get_style_context().add_class("dim")
        self.holder.pack_start(self.status, True, True, 0)

        row = Gtk.Box(spacing=8)
        row.get_style_context().add_class("manual-actions")
        self.label_actions = Gtk.Label(label="Nenhuma alteração pendente", xalign=0)
        self.label_actions.get_style_context().add_class("dim")
        self.label_actions.set_ellipsize(Pango.EllipsizeMode.END)
        row.pack_start(self.label_actions, True, True, 0)
        self.undo = Gtk.Button.new_with_mnemonic("_Desfazer")
        self.reset = Gtk.Button.new_with_mnemonic("_Redefinir tudo")
        for b in (self.undo, self.reset):
            b.set_sensitive(False)
            row.pack_start(b, False, False, 0)
        self.pack_start(row, False, False, 2)

        self.error = Gtk.Label(xalign=0, wrap=True)
        self.error.get_style_context().add_class("error")
        self.pack_start(self.error, False, False, 0)

    # What blivet-gui's installer mode asks of Anaconda's spoke

    @property
    def main_window(self):
        return self.window

    def activate_action_buttons(self, activate):
        self.undo.set_sensitive(activate)
        self.reset.set_sensitive(activate)

    _back_already_clicked = False

    # Loading

    @property
    def ready(self):
        return self.bg is not None

    def load(self):
        """Scan the disks in a thread (blivet takes a few seconds), then embed blivet-gui."""
        if self.ready or self.loading:
            return
        self.loading = True

        def worker():
            try:
                import blivet
                storage = blivet.Blivet()
                if self.disk_names:
                    storage.exclusive_disks = list(self.disk_names)
                storage.reset()
            except Exception as e:  # noqa: BLE001 - shown to the person, details in the log
                GLib.idle_add(self._failed, e)
                return
            GLib.idle_add(self._embed, storage)

        import threading
        threading.Thread(target=worker, daemon=True).start()

    def _failed(self, error):
        self.loading = False
        self.status.set_text(f"Não foi possível ler os discos: {error}")
        self.status.get_style_context().add_class("error")
        print(f"manual partitioning: {error!r}", flush=True)

    def _embed(self, storage):
        from blivetgui.osinstall import BlivetGUIAnaconda, BlivetGUIAnacondaClient

        page = self

        class Embedded(BlivetGUIAnaconda):
            def _handle_user_change(self):
                super()._handle_user_change()
                GLib.idle_add(page.on_change)

        self.storage = storage
        self.holder.remove(self.status)
        client = BlivetGUIAnacondaClient()
        client.initialize(storage)
        self.bg = Embedded(client, self, self.holder)
        b = self.bg.builder
        self.undo.connect("clicked", self._undo)
        self.reset.connect("clicked", self._reset)

        # The side list is replaced by the strip of cards; it stays as the model
        # and selection blivet-gui works with.
        paned = b.get_object("paned")
        paned.get_child1().set_no_show_all(True)
        paned.get_child1().hide()
        # Logical view as a bar over the table, no Physical view tab
        notebook = b.get_object("notebook_views")
        notebook.set_show_tabs(False)
        notebook.set_show_border(False)
        image = b.get_object("image_window")
        image.set_min_content_height(46)
        image.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.NEVER)

        # Its own count of pending actions is ours (label_actions); the button left
        # without its label would only leave a gap under the table
        b.get_object("button_actions").set_no_show_all(True)
        b.get_object("button_actions").hide()

        # initialize() ends with main_window.show_all(): on the wizard's window that
        # would show every hidden widget of every page, so it only reaches blivet-gui's
        self.window.show_all = self.holder.show_all
        try:
            self.bg.initialize()
        finally:
            del self.window.show_all
        self.disks_view = self.bg.list_devices.disks_view
        store = self.bg.list_devices.device_list
        for signal in ("row-inserted", "row-deleted", "row-changed"):
            store.connect(signal, lambda *_: self._queue_rebuild())
        self.disks_view.get_selection().connect("changed", lambda *_: self._sync_strip())
        self.holder.show_all()
        self._rebuild_strip()
        self.loading = False
        GLib.idle_add(self._first_select)

    def _first_select(self):
        self.bg.ui_refresh(None)
        self._sync_strip()
        self.on_change()

    def _undo(self, button):
        self.bg.actions_undo(button)
        self.on_change()

    def _reset(self, button):
        self.bg.clear_actions(button)
        self.on_change()

    # Disk strip

    def _queue_rebuild(self):
        if not self._rebuild_pending:
            self._rebuild_pending = True
            GLib.idle_add(self._rebuild_strip)

    def _rebuild_strip(self):
        self._rebuild_pending = False
        for box in (self.tabs, self.items):
            for child in box.get_children():
                box.remove(child)
        self._groups = []
        group = None
        for index, row in enumerate(self.bg.list_devices.device_list):
            device, markup = row[0], row[2]
            if device is None:  # header row: "Discos", "Volumes Btrfs", "LVM"...
                title = re.sub(r"<[^>]+>", "", markup)
                tab = Gtk.ToggleButton(label=title)
                tab.get_style_context().add_class("disk-tab")
                items = Gtk.Box(spacing=4)
                group = (tab, items, [])
                tab.connect("toggled", self._on_tab, group)
                self.tabs.pack_start(tab, False, False, 0)
                self.items.add_named(items, str(len(self._groups)))
                self._groups.append(group)
                continue
            if group is None:
                continue
            first, _, rest = markup.partition("\n")
            item = Gtk.ToggleButton()
            item.get_style_context().add_class("disk-item")
            text = Gtk.Label(xalign=0)
            model = re.sub(r"<[^>]+>", "", rest).strip()
            text.set_markup(f"<b>{first}</b>  {GLib.markup_escape_text(str(device.size))}"
                            + (f"  <span alpha='60%'>{GLib.markup_escape_text(model)}</span>" if model else ""))
            item.add(text)
            item.connect("toggled", self._on_item, index)
            group[1].pack_start(item, False, False, 0)
            group[2].append((index, item))
        self.tabs.show_all()
        self.items.show_all()
        self._sync_strip()
        return False

    def _on_tab(self, tab, group):
        if self._syncing:
            return
        if not tab.get_active() or not group[2]:
            self._sync_strip()
            return
        self.disks_view.set_cursor(Gtk.TreePath(group[2][0][0]))

    def _on_item(self, item, index):
        if self._syncing:
            return
        if not item.get_active():  # stays down until another one is chosen
            self._sync_strip()
            return
        self.disks_view.set_cursor(Gtk.TreePath(index))

    def _sync_strip(self):
        """Tabs and items follow blivet-gui's (hidden) device list selection."""
        model, it = self.disks_view.get_selection().get_selected()
        selected = model.get_path(it).get_indices()[0] if it else -1
        self._syncing = True
        for n, (tab, _items, members) in enumerate(self._groups):
            mine = any(index == selected for index, _ in members)
            tab.set_active(mine)
            if mine:
                self.items.set_visible_child_name(str(n))
            for index, item in members:
                item.set_active(index == selected)
        self._syncing = False

    # The plan: mount points as Anaconda will get them

    def _new_formats(self):
        from blivet.deviceaction import ActionCreateFormat
        return {a.device.id for a in self.storage.devicetree.actions.find() if isinstance(a, ActionCreateFormat)}

    def mounts(self):
        """[(mountpoint, device, formatted)] for every mount point, swap and BIOS boot partition."""
        if not self.ready:
            return []
        new = self._new_formats()
        seen, rows = set(), []
        for mnt, dev in sorted(self.storage.mountpoints.items()):
            rows.append((mnt, dev, dev.id in new or not dev.exists))
            seen.add(dev.id)
        # blivet only sees the disks in exclusive_disks, so every swap and BIOS boot partition is ours
        for dev in self.storage.devices:
            if dev.id not in seen and dev.format.type in ("swap", "biosboot"):
                rows.append((dev.format.type, dev, dev.id in new or not dev.exists))
        return rows

    def check(self):
        """Why the layout can't be installed, or "" when it can."""
        if not self.ready:
            return "Aguarde a leitura dos discos." if self.loading else ""
        rows = self.mounts()
        mnts = {m: (d, f) for m, d, f in rows}
        if "/" not in mnts:
            return "Escolha onde montar / (a raiz do sistema): selecione uma partição e defina o ponto de montagem."
        for mnt, dev, formatted in rows:
            kind = dev.type
            if "luks" in kind or dev.format.type == "luks" or any(p.format.type == "luks" for p in dev.ancestors if p is not dev):
                return (f"{mnt}: a criptografia é escolhida na tela anterior (Criptografar o disco), "
                        "não aqui. Crie a partição sem criptografia.")
            if kind in ("lvmlv", "lvmvg", "mdarray"):
                return f"{mnt}: LVM e RAID ainda não são suportados pelo instalador do DoxIA."
            if kind in ("btrfs volume", "btrfs subvolume"):
                volume = dev if kind == "btrfs volume" else dev.volume
                if volume.exists:
                    return (f"{mnt}: reaproveitar um volume Btrfs que já existe ainda não é suportado. "
                            "Formate a partição como Btrfs ou use outra.")
                if len(volume.parents) != 1 or volume.parents[0].type != "partition":
                    return f"{mnt}: o volume Btrfs precisa estar numa única partição."
            elif kind != "partition":
                return f"{mnt}: escolha uma partição (não {kind})."
        if UEFI:
            if "/boot/efi" not in mnts:
                return ("Monte a partição de sistema EFI em /boot/efi (a do Windows pode ser reaproveitada, "
                        "sem formatar) ou crie uma de 600 MiB.")
            if mnts["/boot/efi"][0].format.type not in ("efi", "vfat"):
                return "/boot/efi precisa ser uma partição de sistema EFI (FAT)."
        else:
            disk = self._disk_of(mnts["/"][0])
            if disk is not None and getattr(disk.format, "label_type", "") == "gpt" and "biosboot" not in mnts:
                return "Este computador inicia em modo BIOS: crie uma partição BIOS boot de 1 MiB no disco do sistema."
        return ""

    def summary(self):
        """One line per mount point for the Ready page."""
        out = []
        for mnt, dev, formatted in self.mounts():
            part = self._partition_of(dev)
            what = "formatar" if formatted else "manter"
            fs = dev.format.name if dev.format.type else ""
            out.append(f"{mnt}  →  {part.name if part else dev.name} ({fs}, {part.size if part else dev.size}, {what})")
        return out

    def disks_used(self):
        names = []
        for _mnt, dev, _f in self.mounts():
            disk = self._disk_of(dev)
            if disk is not None and disk.name not in names:
                names.append(disk.name)
        return names

    def boot_disk(self):
        mnts = {m: d for m, d, _f in self.mounts()}
        disk = self._disk_of(mnts.get("/boot/efi") or mnts.get("/boot") or mnts["/"])
        return disk.name if disk is not None else None

    @staticmethod
    def _partition_of(dev):
        if dev.type == "btrfs subvolume":
            dev = dev.volume
        if dev.type == "btrfs volume":
            dev = dev.parents[0]
        return dev if dev.type == "partition" else None

    def _disk_of(self, dev):
        part = self._partition_of(dev)
        return part.disk if part is not None else None

    # Writing it

    def apply(self):
        """Write the partition changes to disk (create, delete, resize) and free the disks for
        Anaconda. Formats scheduled here are written too; Anaconda formats them again with
        what kickstart() says. Runs in a thread: no GTK here."""
        self._plan = [(m, d, f) for m, d, f in self.mounts()]
        self.storage.do_it()
        self.storage.devicetree.teardown_all()

    def kickstart(self, passphrase=None):
        """part/btrfs lines for the mount points, after apply() (partition names are final)."""
        from shlex import quote
        rows = getattr(self, "_plan", None) or self.mounts()
        crypt = f" --encrypted --passphrase={quote(passphrase)}" if passphrase else ""
        lines, volumes = [], {}
        mnts = {m for m, _d, _f in rows}
        for mnt, dev, formatted in rows:
            if dev.type in ("btrfs volume", "btrfs subvolume"):
                volume = dev if dev.type == "btrfs volume" else dev.volume
                if volume.id not in volumes:
                    tag = f"btrfs.{len(volumes) + 1:02d}"
                    label = BTRFS_LABEL if not volumes else f"{BTRFS_LABEL}{len(volumes) + 1}"
                    volumes[volume.id] = label
                    lines.append(f"part {tag} --onpart={volume.parents[0].name}{crypt}")
                    lines.append(f"btrfs none --label={label} {tag}")
                label = volumes[volume.id]
                if dev.type == "btrfs volume":
                    # The whole volume on /: root and home subvolumes, like autopart
                    if mnt == "/":
                        lines.append(f"btrfs / --subvol --name=root LABEL={label}")
                        if "/home" not in mnts:
                            lines.append(f"btrfs /home --subvol --name=home LABEL={label}")
                    else:
                        name = mnt.strip("/").replace("/", "_") or "root"
                        lines.append(f"btrfs {mnt} --subvol --name={name} LABEL={label}")
                else:
                    lines.append(f"btrfs {mnt} --subvol --name={dev.name.split('/')[-1]} LABEL={label}")
                continue
            target = "swap" if mnt == "swap" else "biosboot" if mnt == "biosboot" else mnt
            opts = f"--onpart={dev.name}"
            if mnt == "biosboot":
                opts += " --fstype=biosboot"
            elif formatted:
                fstype = "efi" if mnt == "/boot/efi" else dev.format.type
                opts += f" --fstype={fstype}"
                if mnt not in UNENCRYPTED:
                    opts += crypt
            else:
                opts += " --noformat"
            lines.append(f"part {target} {opts}")
        return lines

