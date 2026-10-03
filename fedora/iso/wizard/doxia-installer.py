#!/usr/bin/python3
"""DoxIA installer: a Windows 98 style setup wizard in front of Anaconda.

doxia.ks starts it from %pre (through doxia-installer-start). It asks for the
language, disk, account and computer name, writes them to answers.ks for the
kickstart to %include, and then stays up while Anaconda installs in cmdline
mode: it follows the installation over Anaconda's D-Bus and shows it as the
Win98 block progress bar. The last %post waits for the Restart button.

  DOXIA_DEMO=1 python3 doxia-installer.py   # try it without installing anything
"""
import json
import os
import re
import shlex
import socket
import subprocess
import threading
import time
import unicodedata
from pathlib import Path

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, Gio, GLib, Gtk, Pango  # noqa: E402

HERE = Path(__file__).resolve().parent
DEMO = bool(os.environ.get("DOXIA_DEMO"))
STATE = Path(os.environ.get("DOXIA_STATE", "/tmp/doxia"))
SYSROOT = Path("/mnt/sysroot")
BUS_ADDRESS = Path("/run/anaconda/bus.address")
TASK_IFACE = "org.fedoraproject.Anaconda.Task"
VERSION = os.environ.get("DOXIA_VERSION", "44")

# name shown, then its regions: (region, locale, keyboard index, time zone)
LANGUAGES = [
    ("Português", "Portuguese", [
        ("Português (Brasil)", "pt_BR.UTF-8", 0, "America/Sao_Paulo"),
        ("Português (Portugal)", "pt_PT.UTF-8", 1, "Europe/Lisbon"),
    ]),
    ("English", "English", [
        ("English (United States)", "en_US.UTF-8", 2, "America/New_York"),
        ("English (United Kingdom)", "en_GB.UTF-8", 4, "Europe/London"),
    ]),
    ("Español", "Spanish", [
        ("Español (España)", "es_ES.UTF-8", 5, "Europe/Madrid"),
        ("Español (México)", "es_MX.UTF-8", 6, "America/Mexico_City"),
        ("Español (Argentina)", "es_AR.UTF-8", 6, "America/Argentina/Buenos_Aires"),
    ]),
    ("Français", "French", [
        ("Français (France)", "fr_FR.UTF-8", 7, "Europe/Paris"),
        ("Français (Canada)", "fr_CA.UTF-8", 8, "America/Toronto"),
    ]),
    ("Deutsch", "German", [
        ("Deutsch (Deutschland)", "de_DE.UTF-8", 9, "Europe/Berlin"),
        ("Deutsch (Österreich)", "de_AT.UTF-8", 9, "Europe/Vienna"),
    ]),
]

# shown, console keymap, X layout, X variant
KEYBOARDS = [
    ("Português (Brasil, ABNT2)", "br-abnt2", "br", ""),
    ("Português (Portugal)", "pt-latin1", "pt", ""),
    ("Inglês (EUA)", "us", "us", ""),
    ("Inglês (EUA, internacional com teclas mortas)", "us-acentos", "us", "intl"),
    ("Inglês (Reino Unido)", "uk", "gb", ""),
    ("Espanhol (Espanha)", "es", "es", ""),
    ("Espanhol (América Latina)", "la-latin1", "latam", ""),
    ("Francês (França)", "fr", "fr", ""),
    ("Francês (Canadá)", "cf", "ca", ""),
    ("Alemão", "de", "de", ""),
]

STEPS = [
    "Preparando a instalação",
    "Coletando informações",
    "Copiando arquivos do DoxIA",
    "Reiniciando o computador",
    "Configurando o DoxIA",
]

TIPS = [
    "Pressione Super + Espaço para abrir qualquer aplicativo no DoxIA.",
    "Super + Enter abre o terminal.",
    "Super + W fecha a janela em foco.",
    "O DoxIA se atualiza pelo menu: Super + Alt + Espaço.",
    "Você pode trocar o tema inteiro do sistema pelo menu de estilo.",
    "Arraste janelas segurando Super e o botão esquerdo do mouse.",
]

MIN_DISK = 20 * 1000 ** 3
RESERVED_USERS = {
    "root", "bin", "daemon", "adm", "lp", "sync", "shutdown", "halt", "mail",
    "operator", "games", "ftp", "nobody", "dbus", "systemd-network", "sshd",
}


def human_size(n):
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if n < 1000 or unit == "TB":
            text = f"{n:.0f}" if unit in ("B", "KB") or n >= 100 else f"{n:.1f}".removesuffix(".0")
            return f"{text.replace('.', ',')} {unit}"
        n /= 1000


def list_disks():
    """Disks Anaconda can install to: not the boot media, not read-only, not zram."""
    if DEMO:
        return [
            {"name": "nvme0n1", "model": "Samsung SSD 980", "size": 500107862016, "parts": 3},
            {"name": "sda", "model": "Kingston A400", "size": 240057409536, "parts": 1},
            {"name": "sdb", "model": "Pendrive", "size": 8004304896, "parts": 0},
        ]
    out = subprocess.run(
        ["lsblk", "-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,RO,LABEL,MOUNTPOINTS,PKNAME"],
        capture_output=True, text=True, check=False).stdout
    devices = json.loads(out or '{"blockdevices": []}')["blockdevices"]
    disks = []
    for dev in devices:
        if dev.get("type") != "disk" or dev.get("ro") or dev["name"].startswith(("zram", "loop", "sr")):
            continue
        children = dev.get("children") or []
        media = [dev] + children
        # the USB stick or disk the installer booted from
        if any((d.get("label") or "").startswith("DoxIA") for d in media) or any(
                m and m.startswith("/run/install") for d in media for m in (d.get("mountpoints") or [])):
            continue
        disks.append({
            "name": dev["name"],
            "model": (dev.get("model") or "Disco").strip(),
            "size": int(dev.get("size") or 0),
            "parts": len(children),
        })
    return disks


def suggest_username(full_name):
    first = (full_name.strip().split() or [""])[0]
    ascii_name = unicodedata.normalize("NFKD", first).encode("ascii", "ignore").decode()
    return re.sub(r"[^a-z0-9_-]", "", ascii_name.lower())[:32]


def internet_ok():
    if DEMO:
        return True
    try:
        socket.create_connection(("mirrors.fedoraproject.org", 443), timeout=6).close()
        return True
    except OSError:
        return False


def run_quiet(*argv):
    if DEMO:
        return
    subprocess.run(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)


def nmcli_rows(*args):
    """nmcli -t output as lists of fields (it escapes ':' inside a field as '\\:')."""
    out = subprocess.run(["nmcli", "-t", *args], capture_output=True, text=True, check=False).stdout
    return [[f.replace("\x00", ":") for f in line.replace("\\:", "\x00").split(":")]
            for line in out.splitlines() if line]


DEMO_JOINED = []


def network_state():
    """(how we are connected or None, whether there is a Wi-Fi card)."""
    if DEMO:
        return (f"Conectado à rede sem fio “{DEMO_JOINED[-1]}”." if DEMO_JOINED else None), True
    connected, wifi = None, False
    for device, kind, state, connection in nmcli_rows("-f", "DEVICE,TYPE,STATE,CONNECTION", "device"):
        if kind == "wifi":
            wifi = True
        if state == "connected" and kind in ("ethernet", "wifi"):
            connected = connected or (f"Conectado à rede sem fio “{connection}”." if kind == "wifi"
                                      else f"Conectado por cabo ({device}).")
    return connected, wifi


def wifi_networks(rescan):
    """Visible networks, strongest first: (ssid, signal %, security, in use)."""
    if DEMO:
        return [("Casa", 82, "WPA2", False), ("Vizinho_5G", 54, "WPA2 WPA3", False),
                ("Café Livre", 31, "", False)]
    rows = nmcli_rows("-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list",
                      "--rescan", "yes" if rescan else "auto")
    best = {}
    for in_use, ssid, signal, security in rows:
        if not ssid:
            continue  # hidden networks
        net = (ssid, int(signal or 0), "" if security in ("", "--") else security, in_use == "*")
        if ssid not in best or net[1] > best[ssid][1]:
            best[ssid] = net
    return sorted(best.values(), key=lambda n: -n[1])


def wifi_connect(ssid, password):
    """Connect with NetworkManager; returns an error message or None."""
    if DEMO:
        time.sleep(1.5)
        if password == "errada":
            return "Senha incorreta."
        DEMO_JOINED.append(ssid)
        return None
    argv = ["nmcli", "--wait", "40", "device", "wifi", "connect", ssid]
    if password:
        argv += ["password", password]
    result = subprocess.run(argv, capture_output=True, text=True, check=False)
    if result.returncode == 0:
        return None
    err = (result.stderr or result.stdout).strip().splitlines()
    return err[-1].removeprefix("Error: ") if err else "Não foi possível conectar."


def signal_words(percent):
    return "Excelente" if percent >= 75 else "Bom" if percent >= 50 else "Regular" if percent >= 30 else "Fraco"


def bevel_label(text, css=None, xalign=0.0, wrap=True):
    lbl = Gtk.Label(label=text, xalign=xalign)
    lbl.set_line_wrap(wrap)
    lbl.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
    if css:
        for c in css.split():
            lbl.get_style_context().add_class(c)
    return lbl


class Wizard(Gtk.Window):
    def __init__(self):
        super().__init__(title="Instalação do DoxIA")
        self.set_default_size(1024, 768)
        self.connect("delete-event", lambda *_: True)

        self.lang = LANGUAGES[0][2][0]
        self.keyboard = 0
        self.disks = list_disks()
        self.disk = None
        self.installing = False
        self.started_at = None
        self.fraction = 0.0
        self.phase_floor = 0.0
        self.task_steps = {}
        self.bus = None
        self.current = None

        root = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        root.get_style_context().add_class("desktop")
        self.add(root)
        root.pack_start(self._sidebar(), False, False, 0)

        stage = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        stage.get_style_context().add_class("stage")
        root.pack_start(stage, True, True, 0)

        frame = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        frame.get_style_context().add_class("win")
        frame.set_halign(Gtk.Align.CENTER)
        frame.set_valign(Gtk.Align.CENTER)
        frame.set_size_request(700, 560)
        frame.set_margin_start(28)
        frame.set_margin_end(28)
        stage.pack_start(frame, True, True, 0)

        frame.pack_start(self._titlebar(), False, False, 0)
        body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        body.get_style_context().add_class("win-body")
        frame.pack_start(body, True, True, 0)

        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.NONE)
        body.pack_start(self.stack, True, True, 0)
        body.pack_start(Gtk.Separator(), False, False, 0)

        buttons = Gtk.Box(spacing=8)
        buttons.get_style_context().add_class("buttons")
        buttons.set_halign(Gtk.Align.END)
        self.back = Gtk.Button.new_with_mnemonic("< _Voltar")
        self.next = Gtk.Button.new_with_mnemonic("_Avançar >")
        self.next.get_style_context().add_class("default")
        self.cancel = Gtk.Button.new_with_mnemonic("_Cancelar")
        for b in (self.back, self.next, self.cancel):
            b.set_size_request(92, -1)
            buttons.pack_start(b, False, False, 0)
        body.pack_start(buttons, False, False, 0)
        self.back.connect("clicked", self.on_back)
        self.next.connect("clicked", self.on_next)
        self.cancel.connect("clicked", self.on_cancel)

        self.pages = ["welcome", "network", "disk", "user", "ready", "copy", "done", "failed"]
        self.stack.add_named(self._page_welcome(), "welcome")
        self.stack.add_named(self._page_network(), "network")
        self.stack.add_named(self._page_disk(), "disk")
        self.stack.add_named(self._page_user(), "user")
        self.stack.add_named(self._page_ready(), "ready")
        self.stack.add_named(self._page_copy(), "copy")
        self.stack.add_named(self._page_done(), "done")
        self.stack.add_named(self._page_failed(), "failed")
        self.show_all()
        self.enc_box.hide()
        self.go("welcome")

    # Chrome

    def _sidebar(self):
        side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        side.get_style_context().add_class("sidebar")
        side.set_size_request(262, -1)
        # the ISO gets a copy next to this file; from the repository, use the login screen's
        brand = next((p for p in (HERE / "brand.png", HERE.parents[2] / "default/sddm/omarchy/brand.png")
                      if p.exists()), None)
        if brand:
            from gi.repository import GdkPixbuf
            pix = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(brand), 172, -1, True)
            logo = Gtk.Image.new_from_pixbuf(pix)
        else:
            logo = bevel_label("DoxIA", "brand")
        logo.set_halign(Gtk.Align.START)
        logo.get_style_context().add_class("logo")
        side.pack_start(logo, False, False, 0)

        self.step_labels = []
        steps = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        steps.get_style_context().add_class("steps")
        for text in STEPS:
            row = Gtk.Box(spacing=8)
            mark = Gtk.Label(label="•", xalign=0.5)
            mark.set_size_request(14, -1)
            lbl = Gtk.Label(label=text, xalign=0)
            row.pack_start(mark, False, False, 0)
            row.pack_start(lbl, False, False, 0)
            steps.pack_start(row, False, False, 0)
            self.step_labels.append((row, mark))
        side.pack_start(steps, False, False, 0)

        bottom = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        bottom.get_style_context().add_class("eta")
        bottom.pack_start(Gtk.Separator(), False, False, 0)
        bottom.pack_start(Gtk.Label(label="Tempo restante estimado:", xalign=0), False, False, 0)
        self.eta = Gtk.Label(label="30 a 45 minutos", xalign=0)
        self.eta.get_style_context().add_class("eta-value")
        bottom.pack_start(self.eta, False, False, 0)
        side.pack_end(bottom, False, False, 0)
        return side

    def _titlebar(self):
        bar = Gtk.Box(spacing=4)
        bar.get_style_context().add_class("titlebar98")
        title = Gtk.Label(label="Instalação do DoxIA", xalign=0)
        bar.pack_start(title, True, True, 0)
        for glyph, handler in (("?", self.on_help), ("×", self.on_cancel)):
            b = Gtk.Button(label=glyph)
            b.get_style_context().add_class("caption")
            b.set_can_focus(False)
            b.connect("clicked", handler)
            bar.pack_start(b, False, False, 0)
        self.close_button = b
        return bar

    def set_step(self, current):
        for i, (row, mark) in enumerate(self.step_labels):
            ctx = row.get_style_context()
            for c in ("done", "current", "todo"):
                ctx.remove_class(c)
            if i < current:
                mark.set_text("✓")
                ctx.add_class("done")
            elif i == current:
                mark.set_text("►")
                ctx.add_class("current")
            else:
                mark.set_text("•")
                ctx.add_class("todo")

    def page(self, title, intro):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        box.get_style_context().add_class("page")
        box.pack_start(bevel_label(title, "heading"), False, False, 0)
        if intro:
            box.pack_start(bevel_label(intro), False, False, 0)
        return box

    # Pages

    def _page_welcome(self):
        box = self.page(
            f"Bem-vindo à Instalação do DoxIA {VERSION}",
            "Este assistente vai instalar o DoxIA no seu computador. A instalação leva de "
            "30 a 45 minutos e precisa de internet.\n\nEscolha o idioma e o teclado e clique em Avançar.")

        lists = Gtk.Box(spacing=8, homogeneous=True)
        self.lang_store = Gtk.ListStore(str, str, int)
        for i, (native, english, _) in enumerate(LANGUAGES):
            self.lang_store.append([native, english, i])
        self.lang_view = Gtk.TreeView(model=self.lang_store, headers_visible=False)
        for col, cls in ((0, None), (1, "dim")):
            r = Gtk.CellRendererText()
            if cls:
                r.set_property("foreground-rgba", Gdk.RGBA(0.62, 0.63, 0.66, 1))
            self.lang_view.append_column(Gtk.TreeViewColumn("", r, text=col))
        self.region_store = Gtk.ListStore(str, int)
        self.region_view = Gtk.TreeView(model=self.region_store, headers_visible=False)
        self.region_view.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        for view in (self.lang_view, self.region_view):
            sw = Gtk.ScrolledWindow()
            sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
            sw.get_style_context().add_class("well")
            sw.set_size_request(-1, 190)
            sw.add(view)
            lists.pack_start(sw, True, True, 0)
        box.pack_start(lists, False, False, 0)

        kb = Gtk.Box(spacing=10)
        kb.pack_start(Gtk.Label(label="Teclado:"), False, False, 0)
        self.kb_combo = Gtk.ComboBoxText()
        for k in KEYBOARDS:
            self.kb_combo.append_text(k[0])
        self.kb_combo.set_active(0)
        self.kb_combo.connect("changed", self.on_keyboard)
        kb.pack_start(self.kb_combo, True, True, 0)
        box.pack_start(kb, False, False, 0)
        box.pack_start(bevel_label("Teste o teclado aqui:", "dim"), False, False, 0)
        box.pack_start(Gtk.Entry(placeholder_text="ç á ã é ê ó õ ú @ / ?"), False, False, 0)

        self.lang_view.get_selection().connect("changed", self.on_language)
        self.region_view.get_selection().connect("changed", self.on_region)
        self.lang_view.get_selection().select_path(Gtk.TreePath(0))
        return box

    def _page_network(self):
        box = self.page("Conexão com a internet",
                        "O DoxIA baixa os pacotes durante a instalação. Conecte o cabo de rede ou "
                        "escolha uma rede sem fio.")
        self.net_state = bevel_label("Verificando a conexão...", "warn")
        box.pack_start(self.net_state, False, False, 0)

        self.wifi_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self.wifi_store = Gtk.ListStore(str, str, str, str)
        self.wifi_view = Gtk.TreeView(model=self.wifi_store)
        for title, col, expand in (("Rede sem fio", 0, True), ("Sinal", 1, False), ("Segurança", 2, False)):
            c = Gtk.TreeViewColumn(title, Gtk.CellRendererText(), text=col)
            c.set_expand(expand)
            c.set_min_width(110)
            self.wifi_view.append_column(c)
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.get_style_context().add_class("well")
        sw.set_size_request(-1, 150)
        sw.add(self.wifi_view)
        self.wifi_box.pack_start(sw, False, False, 0)

        row = Gtk.Box(spacing=10)
        self.wifi_pass_label = Gtk.Label(label="Senha da rede:")
        self.wifi_pass = Gtk.Entry(visibility=False, hexpand=True)
        self.wifi_pass.connect("activate", lambda *_: self.on_wifi_connect())
        self.wifi_join = Gtk.Button.new_with_mnemonic("C_onectar")
        self.wifi_join.connect("clicked", lambda *_: self.on_wifi_connect())
        self.wifi_rescan = Gtk.Button.new_with_mnemonic("Atuali_zar lista")
        self.wifi_rescan.connect("clicked", lambda *_: self.refresh_wifi(rescan=True))
        for w in (self.wifi_pass_label, self.wifi_pass, self.wifi_join, self.wifi_rescan):
            row.pack_start(w, w is self.wifi_pass, w is self.wifi_pass, 0)
        self.wifi_box.pack_start(row, False, False, 0)
        self.wifi_msg = bevel_label("", "dim")
        self.wifi_box.pack_start(self.wifi_msg, False, False, 0)
        box.pack_start(self.wifi_box, False, False, 0)
        self.wifi_view.get_selection().connect("changed", lambda *_: self.on_wifi_selected())
        self._online = False
        self._wifi_nets = []
        return box

    def _page_disk(self):
        box = self.page("Onde instalar o DoxIA?", "Escolha o disco de destino.")
        self.disk_store = Gtk.ListStore(str, str, str, int)
        for i, d in enumerate(self.disks):
            content = "vazio" if not d["parts"] else f"{d['parts']} partiç{'ão' if d['parts'] == 1 else 'ões'}"
            self.disk_store.append([f"{d['name']}  {d['model']}", human_size(d["size"]), content, i])
        self.disk_view = Gtk.TreeView(model=self.disk_store)
        for title, col, expand in (("Disco", 0, True), ("Tamanho", 1, False), ("Conteúdo", 2, False)):
            c = Gtk.TreeViewColumn(title, Gtk.CellRendererText(), text=col)
            c.set_expand(expand)
            c.set_min_width(110)
            self.disk_view.append_column(c)
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.get_style_context().add_class("well")
        sw.set_size_request(-1, 130)
        sw.add(self.disk_view)
        box.pack_start(sw, False, False, 0)
        self.disk_view.get_selection().connect("changed", lambda *_: self.validate())
        if self.disks:
            self.disk_view.get_selection().select_path(Gtk.TreePath(0))
        else:
            box.pack_start(bevel_label("Nenhum disco foi encontrado neste computador.", "error"), False, False, 0)

        box.pack_start(bevel_label(
            "O DoxIA vai usar o disco inteiro. Tudo o que estiver nele será apagado.", "warn"), False, False, 0)

        self.encrypt = Gtk.CheckButton.new_with_mnemonic("_Criptografar o disco (pede uma senha a cada inicialização)")
        self.encrypt.connect("toggled", self.on_encrypt)
        box.pack_start(self.encrypt, False, False, 0)
        self.enc_box = Gtk.Grid(column_spacing=10, row_spacing=8)
        self.enc_box.set_margin_start(26)
        self.enc_pass = Gtk.Entry(visibility=False, hexpand=True)
        self.enc_confirm = Gtk.Entry(visibility=False, hexpand=True)
        for row, (text, entry) in enumerate((("Senha do disco:", self.enc_pass), ("Confirmar:", self.enc_confirm))):
            self.enc_box.attach(Gtk.Label(label=text, xalign=1), 0, row, 1, 1)
            self.enc_box.attach(entry, 1, row, 1, 1)
            entry.connect("changed", lambda *_: self.validate())
        box.pack_start(self.enc_box, False, False, 0)
        self.disk_error = bevel_label("", "error")
        box.pack_start(self.disk_error, False, False, 0)
        return box

    def _page_user(self):
        box = self.page("Identificação",
                        "Digite seu nome e uma senha. Você será o administrador deste computador.")
        grid = Gtk.Grid(column_spacing=10, row_spacing=10)
        grid.set_margin_top(6)
        self.full_name = Gtk.Entry(hexpand=True)
        self.username = Gtk.Entry(hexpand=True)
        self.hostname = Gtk.Entry(hexpand=True, text="doxia")
        self.password = Gtk.Entry(hexpand=True, visibility=False)
        self.confirm = Gtk.Entry(hexpand=True, visibility=False)
        fields = (("Seu nome:", self.full_name), ("Nome de usuário:", self.username),
                  ("Nome do computador:", self.hostname), ("Senha:", self.password),
                  ("Confirmar senha:", self.confirm))
        for row, (text, entry) in enumerate(fields):
            lbl = Gtk.Label(label=text, xalign=1)
            lbl.set_size_request(150, -1)
            grid.attach(lbl, 0, row, 1, 1)
            grid.attach(entry, 1, row, 1, 1)
            entry.connect("changed", lambda *_: self.validate())
            entry.set_activates_default(True)
        self._username_touched = False
        self.full_name.connect("changed", self.on_full_name)
        self.username.connect("key-press-event", self.on_username_typed)
        box.pack_start(grid, False, False, 0)
        self.user_error = bevel_label("", "error")
        box.pack_start(self.user_error, False, False, 0)
        return box

    def _page_ready(self):
        box = self.page("Pronto para instalar",
                        "O DoxIA será instalado com estas configurações. Clique em Instalar para "
                        "começar; o disco escolhido será apagado.")
        group = Gtk.Frame(label=" Configurações escolhidas ")
        group.get_style_context().add_class("group")
        self.summary = Gtk.Grid(column_spacing=12, row_spacing=6)
        self.summary.set_margin_start(12)
        self.summary.set_margin_end(12)
        self.summary.set_margin_top(10)
        self.summary.set_margin_bottom(12)
        group.add(self.summary)
        box.pack_start(group, False, False, 0)
        self.net_status = bevel_label("", "dim")
        box.pack_start(self.net_status, False, False, 0)
        self.net_retry = Gtk.Button.new_with_mnemonic("_Testar a conexão de novo")
        self.net_retry.set_halign(Gtk.Align.START)
        self.net_retry.connect("clicked", lambda *_: self.check_internet())
        box.pack_start(self.net_retry, False, False, 0)
        return box

    def _page_copy(self):
        box = self.page("Copiando arquivos do DoxIA...",
                        "Isto pode levar alguns minutos. Você pode tomar um café.")
        self.copy_status = bevel_label("Preparando o disco...")
        box.pack_start(self.copy_status, False, False, 0)
        self.copy_detail = bevel_label(" ", "dim", wrap=False)
        self.copy_detail.set_ellipsize(Pango.EllipsizeMode.MIDDLE)
        box.pack_start(self.copy_detail, False, False, 0)
        self.bar = Gtk.ProgressBar()
        self.bar.get_style_context().add_class("blocks")
        box.pack_start(self.bar, False, False, 0)
        self.percent = bevel_label("0% concluído", "dim")
        box.pack_start(self.percent, False, False, 0)
        tip = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        tip.get_style_context().add_class("tip")
        tip.pack_start(bevel_label("Você sabia?", "tip-title"), False, False, 0)
        self.tip = bevel_label(TIPS[0])
        tip.pack_start(self.tip, False, False, 0)
        box.pack_start(tip, False, False, 6)
        return box

    def _page_done(self):
        box = self.page("Concluindo a instalação",
                        "O DoxIA foi instalado. Remova o pendrive ou o DVD e clique em Reiniciar.\n\n"
                        "Depois de reiniciar, o DoxIA vai terminar de se configurar e abrir a tela de login.")
        return box

    def _page_failed(self):
        box = self.page("A instalação não pôde ser concluída", None)
        self.fail_text = bevel_label("")
        box.pack_start(self.fail_text, False, False, 0)
        box.pack_start(bevel_label(
            "Os detalhes estão no console do instalador: pressione Ctrl+Alt+F1 para vê-lo e "
            "Ctrl+Alt+F6 para voltar a esta tela. Nada foi instalado se o erro aconteceu antes "
            "da cópia dos arquivos.", "dim"), False, False, 0)
        return box

    # Navigation

    def go(self, name):
        self.stack.set_visible_child_name(name)
        self.current = name
        step = {"welcome": 0, "network": 1, "disk": 1, "user": 1, "ready": 1, "copy": 2, "done": 3, "failed": 2}[name]
        self.set_step(step)
        self.back.set_sensitive(name in ("network", "disk", "user", "ready"))
        self.back.set_visible(name not in ("done", "failed"))
        self.next.set_visible(name not in ("copy",))
        self.cancel.set_visible(name not in ("done", "failed"))
        self.cancel.set_sensitive(name != "copy")
        self.close_button.set_sensitive(name not in ("copy", "done"))
        label = {"ready": "_Instalar", "done": "_Reiniciar", "failed": "_Desligar"}.get(name, "_Avançar >")
        self.next.set_label(label)
        self.next.set_use_underline(True)
        if name == "ready":
            self.fill_summary()
            self.check_internet()
        if name == "user":
            GLib.idle_add(self.full_name.grab_focus)
        if name == "network":
            self.check_network()
            self.refresh_wifi(rescan=False)
            if not getattr(self, "_net_timer", None):
                self._net_timer = GLib.timeout_add_seconds(5, self.check_network)
        self.validate()
        self.next.grab_default() if self.next.get_can_default() else None

    def on_back(self, _button):
        i = self.pages.index(self.current)
        self.go(self.pages[i - 1])

    def on_next(self, _button):
        if self.current == "welcome":
            self.go("network")
        elif self.current == "network":
            self.go("disk")
        elif self.current == "disk":
            self.go("user")
        elif self.current == "user":
            self.go("ready")
        elif self.current == "ready":
            self.start_install()
        elif self.current == "done":
            self.next.set_sensitive(False)
            self.next.set_label("Reiniciando...")
            (STATE / "reboot").touch()
            if DEMO:
                Gtk.main_quit()
        elif self.current == "failed":
            run_quiet("systemctl", "poweroff")
            if DEMO:
                Gtk.main_quit()

    def on_cancel(self, _button):
        if self.installing:
            return
        dlg = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.WARNING,
                                buttons=Gtk.ButtonsType.NONE,
                                text="Sair da Instalação do DoxIA?")
        dlg.format_secondary_text("O DoxIA ainda não foi instalado e nada foi alterado no disco. "
                                  "O computador será desligado.")
        dlg.add_button("_Não", Gtk.ResponseType.NO)
        dlg.add_button("_Sim, desligar", Gtk.ResponseType.YES)
        dlg.set_default_response(Gtk.ResponseType.NO)
        response = dlg.run()
        dlg.destroy()
        if response == Gtk.ResponseType.YES:
            run_quiet("systemctl", "poweroff")
            if DEMO:
                Gtk.main_quit()

    def on_help(self, _button):
        dlg = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.INFO,
                                buttons=Gtk.ButtonsType.OK, text=f"DoxIA {VERSION}")
        dlg.format_secondary_text(
            "Siga as telas e clique em Avançar. A instalação apaga o disco escolhido e precisa de "
            "internet para baixar os pacotes.\n\nCtrl+Alt+F1 mostra o console do instalador.")
        dlg.run()
        dlg.destroy()

    # Welcome

    def on_language(self, selection):
        model, it = selection.get_selected()
        if not it:
            return
        regions = LANGUAGES[model[it][2]][2]
        self.region_store.clear()
        for i, r in enumerate(regions):
            self.region_store.append([r[0], i])
        self._regions = regions
        self.region_view.get_selection().select_path(Gtk.TreePath(0))

    def on_region(self, selection):
        model, it = selection.get_selected()
        if not it:
            return
        self.lang = self._regions[model[it][1]]
        self.kb_combo.set_active(self.lang[2])

    def on_keyboard(self, combo):
        self.keyboard = max(0, combo.get_active())
        _, _, layout, variant = KEYBOARDS[self.keyboard]
        # gnome-kiosk follows localed, so the entries here type with the chosen layout
        run_quiet("localectl", "--no-convert", "set-x11-keymap", layout, "", variant, "")

    # Network

    def check_network(self):
        """Runs every 5 s while the network page is up: cable plugged, Wi-Fi joined..."""
        if self.current != "network":
            self._net_timer = None
            return False
        if getattr(self, "_checking", False):
            return True
        self._checking = True

        def worker():
            how, wifi = network_state()
            online = internet_ok() if how else False
            GLib.idle_add(done, how, wifi, online)

        def done(how, wifi, online):
            self._checking = False
            self._online = online
            self.wifi_box.set_visible(wifi)
            ctx = self.net_state.get_style_context()
            ctx.remove_class("error")
            if online:
                self.net_state.set_text((how or "Conectado.") + " A internet está funcionando.")
            elif how:
                self.net_state.set_text(how + " Mas a internet não responde.")
                ctx.add_class("error")
            elif wifi:
                self.net_state.set_text("Sem conexão. Escolha uma rede sem fio abaixo ou conecte o cabo.")
            else:
                self.net_state.set_text("Sem conexão e nenhuma placa de rede sem fio encontrada. Conecte o cabo de rede.")
                ctx.add_class("error")
            self.validate()

        threading.Thread(target=worker, daemon=True).start()
        return True

    def refresh_wifi(self, rescan):
        self.wifi_rescan.set_sensitive(False)
        if rescan:
            self.wifi_msg.get_style_context().remove_class("error")
            self.wifi_msg.set_text("Procurando redes sem fio...")

        def worker():
            nets = wifi_networks(rescan)
            GLib.idle_add(done, nets)

        def done(nets):
            selected = self.selected_wifi()
            self._wifi_nets = nets
            self.wifi_store.clear()
            for ssid, signal, security, in_use in nets:
                self.wifi_store.append([("● " if in_use else "") + ssid, signal_words(signal),
                                        security.replace(" ", ", ") or "Aberta", ssid])
            for i, net in enumerate(nets):
                if selected and net[0] == selected[0]:
                    self.wifi_view.get_selection().select_path(Gtk.TreePath(i))
            self.wifi_rescan.set_sensitive(True)
            if not nets:
                self.wifi_msg.set_text("Nenhuma rede sem fio encontrada. Clique em Atualizar lista.")
            elif rescan:
                self.wifi_msg.set_text("")
            self.on_wifi_selected()

        threading.Thread(target=worker, daemon=True).start()

    def selected_wifi(self):
        model, it = self.wifi_view.get_selection().get_selected()
        if not it:
            return None
        ssid = model[it][3]
        return next((n for n in self._wifi_nets if n[0] == ssid), None)

    def on_wifi_selected(self):
        net = self.selected_wifi()
        secure = bool(net and net[2])
        self.wifi_pass.set_sensitive(secure)
        self.wifi_pass_label.set_sensitive(secure)
        self.wifi_join.set_sensitive(net is not None)
        if secure:
            self.wifi_pass.grab_focus()

    def on_wifi_connect(self):
        net = self.selected_wifi()
        if not net:
            return
        if net[2] and not self.wifi_pass.get_text():
            self.wifi_msg.set_text("Digite a senha da rede.")
            return
        self.wifi_join.set_sensitive(False)
        self.wifi_msg.get_style_context().remove_class("error")
        self.wifi_msg.set_text(f"Conectando a “{net[0]}”...")

        def worker():
            err = wifi_connect(net[0], self.wifi_pass.get_text() if net[2] else "")
            GLib.idle_add(done, err)

        def done(err):
            self.wifi_join.set_sensitive(True)
            ctx = self.wifi_msg.get_style_context()
            if err:
                self.wifi_msg.set_text(f"Não foi possível conectar a “{net[0]}”: {err}")
                ctx.add_class("error")
            else:
                self.wifi_msg.set_text(f"Conectado a “{net[0]}”.")
                self.refresh_wifi(rescan=False)
            self.check_network()

        threading.Thread(target=worker, daemon=True).start()

    # Disk

    def on_encrypt(self, check):
        self.enc_box.set_visible(check.get_active())
        self.validate()

    # User

    def on_full_name(self, entry):
        if not self._username_touched:
            self.username.set_text(suggest_username(entry.get_text()))

    def on_username_typed(self, *_):
        self._username_touched = True
        return False

    # Validation

    def validate(self):
        ok, msg = True, ""
        if self.current == "network":
            ok = self._online
        elif self.current == "disk":
            model, it = self.disk_view.get_selection().get_selected()
            self.disk = self.disks[model[it][3]] if it else None
            if not self.disk:
                ok, msg = False, "Escolha um disco."
            elif self.disk["size"] < MIN_DISK:
                ok, msg = False, f"Este disco é pequeno demais: o DoxIA precisa de pelo menos {human_size(MIN_DISK)}."
            elif self.encrypt.get_active():
                p, c = self.enc_pass.get_text(), self.enc_confirm.get_text()
                if len(p) < 8:
                    ok, msg = False, "A senha do disco precisa ter pelo menos 8 caracteres." if p else ""
                elif p != c:
                    ok, msg = False, "As senhas do disco não conferem." if c else ""
            self.disk_error.set_text(msg)
        elif self.current == "user":
            name, user = self.full_name.get_text().strip(), self.username.get_text()
            host, pw, cf = self.hostname.get_text(), self.password.get_text(), self.confirm.get_text()
            if not name:
                ok = False
            elif not re.fullmatch(r"[a-z_][a-z0-9_-]{0,31}", user):
                ok, msg = False, "O nome de usuário usa só letras minúsculas, números, - e _, e começa com uma letra."
            elif user in RESERVED_USERS:
                ok, msg = False, f"“{user}” é reservado pelo sistema. Escolha outro nome de usuário."
            elif not re.fullmatch(r"[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?", host):
                ok, msg = False, "O nome do computador usa só letras, números e hífen (sem espaços)."
            elif len(pw) < 6:
                ok, msg = False, "A senha precisa ter pelo menos 6 caracteres." if pw else ""
            elif pw != cf:
                ok, msg = False, "As senhas não conferem." if cf else ""
            self.user_error.set_text(msg)
        elif self.current == "ready":
            ok = getattr(self, "_net_ok", False)
        self.next.set_sensitive(ok)
        return ok

    # Ready

    def fill_summary(self):
        for child in self.summary.get_children():
            self.summary.remove(child)
        d = self.disk
        disk = f"{d['model']} ({human_size(d['size'])}) — inteiro, Btrfs"
        if self.encrypt.get_active():
            disk += ", criptografado"
        rows = (("Idioma:", self.lang[0]), ("Teclado:", KEYBOARDS[self.keyboard][0]),
                ("Fuso horário:", self.lang[3]), ("Disco:", disk),
                ("Usuário:", f"{self.username.get_text()} (administrador)"),
                ("Computador:", self.hostname.get_text()))
        for i, (k, v) in enumerate(rows):
            self.summary.attach(Gtk.Label(label=k, xalign=1), 0, i, 1, 1)
            self.summary.attach(Gtk.Label(label=v, xalign=0, selectable=False), 1, i, 1, 1)
        self.summary.show_all()

    def check_internet(self):
        self._net_ok = False
        self.validate()
        self.net_status.set_text("Verificando a conexão com a internet...")
        self.net_retry.hide()

        def worker():
            ok = internet_ok()
            GLib.idle_add(done, ok)

        def done(ok):
            self._net_ok = ok
            ctx = self.net_status.get_style_context()
            ctx.remove_class("error")
            if ok:
                self.net_status.set_text("Conexão com a internet: OK.")
            else:
                self.net_status.set_text("Sem conexão com a internet. Conecte o cabo de rede e teste de novo: "
                                         "os pacotes do DoxIA são baixados durante a instalação.")
                ctx.add_class("error")
                self.net_retry.show()
            self.validate()

        threading.Thread(target=worker, daemon=True).start()

    # Install

    def answers(self):
        q = shlex.quote
        d, (_, vc, layout, variant) = self.disk, KEYBOARDS[self.keyboard]
        xlayout = f"{layout} ({variant})" if variant else layout
        lines = [
            "# Written by the DoxIA installer wizard",
            f"lang {self.lang[1]}",
            f"keyboard --vckeymap={vc} --xlayouts={q(xlayout)}",
            f"timezone {self.lang[3]} --utc",
            f"network --hostname={self.hostname.get_text()}",
            f"ignoredisk --only-use={d['name']}",
            "zerombr",
            f"clearpart --all --initlabel --drives={d['name']}",
            f"bootloader --boot-drive={d['name']}",
        ]
        part = "autopart --type=btrfs"
        if self.encrypt.get_active():
            part += f" --encrypted --passphrase={q(self.enc_pass.get_text())}"
        lines += [
            part,
            "rootpw --lock",
            f"user --name={self.username.get_text()} --gecos={q(self.full_name.get_text().strip())} "
            f"--groups=wheel --password={q(self.password.get_text())} --plaintext",
        ]
        return "\n".join(lines) + "\n"

    def start_install(self):
        STATE.mkdir(parents=True, exist_ok=True)
        tmp = STATE / "answers.ks.tmp"
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w") as f:
            f.write(self.answers())
        tmp.rename(STATE / "answers.ks")  # doxia-installer-start waits for this file

        self.installing = True
        self.started_at = time.time()
        self.go("copy")
        self.set_progress(0.01)
        GLib.timeout_add_seconds(15, self.next_tip)
        GLib.timeout_add(1000, self.tick)
        if DEMO:
            threading.Thread(target=self.demo_feed, daemon=True).start()
        else:
            GLib.timeout_add(500, self.connect_bus)

    def connect_bus(self):
        """Anaconda's bus already exists in %pre; keep trying until it answers."""
        try:
            address = BUS_ADDRESS.read_text().strip()
            self.bus = Gio.DBusConnection.new_for_address_sync(
                address,
                Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
                None, None)
        except (OSError, GLib.Error):
            return True
        self.bus.signal_subscribe(None, TASK_IFACE, None, None, None, Gio.DBusSignalFlags.NONE, self.on_signal)
        return False

    def on_signal(self, _conn, _sender, path, _iface, signal, params):
        if signal == "ProgressChanged":
            step, message = params.unpack()
            self.on_progress(path, step, message)
        elif signal == "Failed" and not (STATE / "installed").exists():
            self.fail("O instalador encontrou um erro: " + (self.last_message or "erro desconhecido") + ".")

    last_message = ""

    def on_progress(self, path, step, message):
        """Map Anaconda's messages to one bar: download 4-40%, packages 40-80%, setup 80-99%."""
        if not message:
            return
        self.last_message = message
        downloaded = re.search(r"\((\d+)%\)", message)
        installed = re.search(r"\((\d+)/(\d+)\)\s*$", message)
        if downloaded:
            self.set_status("Baixando os pacotes", message)
            self.set_progress(0.04 + 0.36 * int(downloaded.group(1)) / 100)
        elif installed:
            n, total = int(installed.group(1)), int(installed.group(2))
            name = message[:installed.start()].split()[-1] if message[:installed.start()].split() else ""
            self.set_status(f"Instalando pacotes ({n} de {total}):", name)
            self.set_progress(0.40 + 0.40 * n / max(1, total))
        elif self.fraction >= 0.79:
            self.set_status("Configurando o sistema", message)
            self.set_progress(max(self.fraction, 0.80))
        else:
            self.set_status(message, " ")

    def set_status(self, status, detail):
        self.copy_status.set_text(status)
        self.copy_detail.set_text(detail or " ")

    def set_progress(self, fraction):
        self.fraction = max(self.fraction, min(fraction, 1.0))
        self.bar.set_fraction(self.fraction)
        self.percent.set_text(f"{int(self.fraction * 100)}% concluído")

    def tick(self):
        if not self.installing:
            return False
        if (STATE / "installed").exists():
            self.finished()
            return False
        log = SYSROOT / "root/doxia-install.log"
        if not DEMO and log.exists():
            # DoxIA's own %post: clone and run the install scripts (the last ~20%)
            steps = [ln[4:].strip() for ln in log.read_text(errors="replace").splitlines() if ln.startswith("==> ")]
            self.set_status("Configurando o DoxIA", steps[-1] if steps else "Baixando o DoxIA")
            self.set_progress(0.82 + 0.17 * (1 - 0.92 ** len(steps)))
        elif self.fraction >= 0.80:
            self.set_progress(self.fraction + (0.99 - self.fraction) * 0.002)
        elif self.fraction < 0.04:
            self.set_progress(self.fraction + 0.0005)
        self.update_eta()
        return True

    def update_eta(self):
        elapsed = time.time() - self.started_at
        if self.fraction < 0.08 or elapsed < 60:
            self.eta.set_text("30 a 45 minutos")
            return
        remaining = elapsed * (1 - self.fraction) / self.fraction
        minutes = max(1, round(remaining / 60))
        self.eta.set_text("1 minuto" if minutes == 1 else f"{minutes} minutos")

    def next_tip(self):
        if self.current != "copy":
            return False
        i = (TIPS.index(self.tip.get_text()) + 1) % len(TIPS)
        self.tip.set_text(TIPS[i])
        return True

    def finished(self):
        self.installing = False
        self.set_progress(1.0)
        self.eta.set_text("5 minutos")
        self.go("done")

    def fail(self, text):
        if not self.installing:
            return
        self.installing = False
        self.fail_text.set_text(text)
        self.eta.set_text("—")
        self.go("failed")

    # Demo: replay a plausible installation in ~40 s

    def demo_feed(self):
        def emit(step, msg):
            GLib.idle_add(self.on_progress, "/demo", step, msg)
        emit(1, "Configurando o armazenamento")
        time.sleep(2)
        for p in range(0, 101, 5):
            emit(5, f"Baixando 1430 RPMs, {p * 9.1:.1f} MB / 910.0 MB ({p}%) done.")
            time.sleep(0.3)
        for n in range(1, 1431, 37):
            emit(6, f"Instalando hyprland-0.56.0-1.fc44.x86_64 ({n}/1430)")
            time.sleep(0.15)
        emit(9, "Instalando o carregador de inicialização")
        time.sleep(2)
        emit(20, "Executando scripts de pós-instalação")
        time.sleep(4)
        STATE.mkdir(parents=True, exist_ok=True)
        (STATE / "installed").touch()


def main():
    settings = Gtk.Settings.get_default()
    settings.set_property("gtk-theme-name", "Adwaita")
    settings.set_property("gtk-application-prefer-dark-theme", False)
    settings.set_property("gtk-font-name", "Red Hat Text 10")
    provider = Gtk.CssProvider()
    provider.load_from_path(str(HERE / "doxia-installer.css"))
    Gtk.StyleContext.add_provider_for_screen(
        Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_USER)
    if DEMO:
        (STATE / "installed").unlink(missing_ok=True)
    win = Wizard()
    win.connect("destroy", Gtk.main_quit)
    if not DEMO:
        win.fullscreen()
    Gtk.main()


if __name__ == "__main__":
    main()
