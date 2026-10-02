#!/usr/bin/env python3
"""Gera os wallpapers e a imagem de unlock do tema RHEL 8 (Ootpa).

Rode de novo para regenerar: python3 make-art.py
"""
import math
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).parent
W, H = 3840, 2160

BLACK = (21, 21, 21)
BLACK_D = (10, 10, 10)
CHARCOAL = (33, 36, 39)
GRAY_7 = (60, 63, 66)
GRAY_5 = (138, 141, 144)
GRAY_2 = (210, 210, 210)
WHITE = (240, 240, 240)
RED = (238, 0, 0)
RED_D = (163, 0, 0)
RED_DD = (95, 0, 0)

FONTS = Path("/usr/share/fonts/redhat")
DISPLAY_BOLD = FONTS / "RedHatDisplay-Bold.otf"
DISPLAY = FONTS / "RedHatDisplay-Regular.otf"
TEXT = FONTS / "RedHatText-Regular.otf"
MONO = None
for cand in [
    "/usr/share/fonts/redhat/RedHatMono-Regular.otf",
    "/usr/share/fonts/fira-code-nerd-fonts/FiraCodeNerdFontMono-Regular.ttf",
    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
]:
    if Path(cand).exists():
        MONO = cand
        break


def font(path, size):
    try:
        return ImageFont.truetype(str(path), size)
    except Exception:
        return ImageFont.load_default()


def radial_gradient(w, h, cx, cy, inner, outer, radius):
    """Gradiente radial barato: calcula em baixa resolução e amplia."""
    sw, sh = w // 8, h // 8
    img = Image.new("RGB", (sw, sh))
    px = img.load()
    for y in range(sh):
        for x in range(sw):
            d = math.hypot(x * 8 - cx, y * 8 - cy) / radius
            t = min(1.0, d) ** 1.4
            px[x, y] = tuple(int(inner[i] + (outer[i] - inner[i]) * t) for i in range(3))
    return img.resize((w, h), Image.BICUBIC)


def bezier(p0, p1, p2, p3, n=40):
    pts = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        pts.append((
            u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
            u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1],
        ))
    return pts


def hat(size, band=BLACK):
    """Chapéu fedora estilizado (inspirado no Shadowman), RGBA quadrado.

    Desenhado em 4x e reduzido para bordas suaves.
    """
    ss = 4
    S = size * ss
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    def P(x, y):  # coordenadas normalizadas -1..1 -> pixels
        return (S / 2 + x * S / 2, S / 2 + y * S / 2)

    # aba: curva para cima nas pontas, levemente caída na frente
    brim = (
        bezier(P(-0.98, 0.02), P(-0.80, 0.42), P(0.70, 0.52), P(0.98, -0.04))
        + bezier(P(0.98, -0.04), P(0.80, 0.20), P(-0.70, 0.22), P(-0.98, 0.02))
    )
    d.polygon(brim, fill=RED + (255,))

    # copa: laterais inclinadas, topo com amassado central
    crown = (
        bezier(P(-0.62, 0.12), P(-0.66, -0.20), P(-0.56, -0.62), P(-0.30, -0.70))
        + bezier(P(-0.30, -0.70), P(-0.12, -0.74), P(-0.06, -0.58), P(0.04, -0.60))
        + bezier(P(0.04, -0.60), P(0.18, -0.64), P(0.40, -0.76), P(0.52, -0.56))
        + bezier(P(0.52, -0.56), P(0.64, -0.36), P(0.64, -0.10), P(0.62, 0.14))
        + bezier(P(0.62, 0.14), P(0.30, 0.26), P(-0.30, 0.24), P(-0.62, 0.12))
    )
    d.polygon(crown, fill=RED + (255,))

    # faixa preta
    band_poly = (
        bezier(P(-0.64, -0.06), P(-0.30, 0.06), P(0.30, 0.08), P(0.64, -0.06))
        + bezier(P(0.63, 0.12), P(0.30, 0.26), P(-0.30, 0.24), P(-0.63, 0.10))
    )
    d.polygon(band_poly, fill=band + (255,))

    return img.resize((size, size), Image.LANCZOS)


def red_ribbons(img, seed_shift=0.0):
    """Faixas curvas vermelhas varrendo o canto inferior direito."""
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    cx, cy = W * 1.35, H * 1.55
    for i in range(46):
        r = 1500 + i * 38
        t = i / 45
        col = tuple(int(RED_DD[k] + (RED[k] - RED_DD[k]) * (1 - abs(t - 0.35) * 1.6)) for k in range(3))
        col = tuple(max(0, min(255, c)) for c in col)
        a = int(40 + 150 * math.sin(math.pi * t) ** 2)
        d.arc([cx - r * 1.25, cy - r, cx + r * 1.25, cy + r], 180 + seed_shift, 280, fill=col + (a,), width=6)
    # faixa sólida principal
    r = 2050
    d.arc([cx - r * 1.25, cy - r, cx + r * 1.25, cy + r], 180, 280, fill=RED + (235,), width=70)
    soft = layer.filter(ImageFilter.GaussianBlur(18))
    img.alpha_composite(soft)
    img.alpha_composite(layer)


def wallpaper_ootpa():
    img = radial_gradient(W, H, W * 0.30, H * 0.40, CHARCOAL, BLACK_D, W * 0.9).convert("RGBA")
    red_ribbons(img)

    logo = hat(560)
    img.alpha_composite(logo, (300, int(H * 0.30)))

    d = ImageDraw.Draw(img)
    x = 300 + 600
    y = int(H * 0.30) + 150
    d.text((x, y), "Red Hat", font=font(DISPLAY_BOLD, 150), fill=WHITE)
    d.text((x, y + 170), "Enterprise Linux 8", font=font(DISPLAY, 150), fill=WHITE)
    d.text((x + 6, y + 380), "OOTPA  ·  4.18.0  ·  x86_64", font=font(TEXT, 52), fill=GRAY_5)
    return img.convert("RGB")


def wallpaper_console():
    """Console de login do RHEL 8: preto, barra vermelha, texto de tty."""
    img = Image.new("RGBA", (W, H), BLACK_D + (255,))
    d = ImageDraw.Draw(img)

    # malha diagonal discreta
    for k in range(-H, W, 64):
        d.line([(k, H), (k + H, 0)], fill=(255, 255, 255, 6), width=1)

    # barra lateral vermelha (anaconda)
    d.rectangle([0, 0, 520, H], fill=(24, 24, 24, 255))
    d.rectangle([520, 0, 532, H], fill=RED + (255,))
    img.alpha_composite(hat(300), (110, 140))
    d.text((110, 470), "RED HAT", font=font(DISPLAY_BOLD, 64), fill=WHITE)
    d.text((110, 545), "ENTERPRISE LINUX 8", font=font(DISPLAY, 40), fill=GRAY_2)

    mono = font(MONO or TEXT, 54)
    lines = [
        ("Red Hat Enterprise Linux 8.10 (Ootpa)", WHITE),
        ("Kernel 4.18.0-553.el8_10.x86_64 on an x86_64", GRAY_5),
        ("", None),
        ("Activate the web console with: systemctl enable --now cockpit.socket", GRAY_5),
        ("", None),
        ("alexandre login: _", WHITE),
    ]
    y = int(H * 0.36)
    for text, col in lines:
        if text:
            d.text((760, y), text, font=mono, fill=col)
        y += 84

    # cursor vermelho
    d.rectangle([760, y + 30, 1360, y + 38], fill=RED + (255,))
    return img.convert("RGB")


def svg_png(svg, height, out):
    """Rasteriza um SVG na altura pedida (ImageMagick + rsvg), fundo transparente."""
    subprocess.run(["magick", "-background", "none", "-density", "600", str(svg),
                    "-resize", f"x{height}", "-trim", "+repage", str(out)], check=True)


def unlock_logo():
    img = Image.new("RGBA", (1108, 523), (0, 0, 0, 0))
    logo = hat(460, band=BLACK_D)
    img.alpha_composite(logo, ((1108 - 460) // 2, 30))
    return img


if __name__ == "__main__":
    bg = HERE / "backgrounds"
    bg.mkdir(exist_ok=True)
    # fundo liso no cinza do tema (lighter_background); as artes antigas
    # (wallpaper_ootpa / wallpaper_console) ficam aqui caso queira voltar.
    for old in bg.glob("*.png"):
        old.unlink()
    Image.new("RGB", (W, H), CHARCOAL).save(bg / "1-rhel8-cinza.png", optimize=True)
    ootpa = wallpaper_ootpa()
    # chapéu oficial (hat.svg, sem a faixa preta) para a barra e o unlock
    svg_png(HERE / "hat.svg", 128, HERE / "emblem.png")
    tmp = HERE / ".hat-unlock.png"
    svg_png(HERE / "hat.svg", 300, tmp)
    hat_img = Image.open(tmp).convert("RGBA")
    unlock = Image.new("RGBA", (1108, 523), (0, 0, 0, 0))
    unlock.alpha_composite(hat_img, ((1108 - hat_img.width) // 2, (523 - hat_img.height) // 2 + 40))
    unlock.save(HERE / "unlock.png")
    tmp.unlink()
    ootpa.resize((960, 540), Image.LANCZOS).save(HERE / "preview.png")
    print("ok")
