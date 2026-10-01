#!/usr/bin/env python3
"""Gera os wallpapers e a imagem de unlock do tema Xi Gundam.

Rode de novo para regenerar: python3 make-art.py
"""
import math
import random
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).parent
W, H = 3840, 2160

NAVY_TOP = (7, 9, 16)
NAVY_BOT = (22, 30, 50)
WHITE = (232, 236, 242)
GUNMETAL = (44, 52, 74)
GUNMETAL_D = (26, 32, 48)
RED = (230, 57, 70)
YELLOW = (242, 194, 48)
GREEN = (95, 211, 141)
CYAN = (76, 201, 240)
PINK = (224, 90, 184)

FONT_BOLD = "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf"
FONT_MONO = None
for cand in [
    "/usr/share/fonts/fira-code-nerd-fonts/FiraCodeNerdFontMono-Regular.ttf",
    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
]:
    if Path(cand).exists():
        FONT_MONO = cand
        break


def font(path, size):
    try:
        return ImageFont.truetype(path, size)
    except Exception:
        return ImageFont.load_default()


def vertical_gradient(w, h, top, bot):
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        t = y / (h - 1)
        c = tuple(int(top[i] + (bot[i] - top[i]) * t) for i in range(3))
        for x in range(w):
            px[x, y] = c
    return img


def glow(base, layer, radius):
    """Compõe uma camada RGBA com brilho (blur) por baixo."""
    blurred = layer.filter(ImageFilter.GaussianBlur(radius))
    base.alpha_composite(blurred)
    base.alpha_composite(layer)


def hud_grid(draw, color, step=120):
    for x in range(0, W, step):
        draw.line([(x, 0), (x, H)], fill=color, width=1)
    for y in range(0, H, step):
        draw.line([(0, y), (W, y)], fill=color, width=1)


def city_lights(img, seed=105):
    rnd = random.Random(seed)
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    horizon = int(H * 0.82)
    # silhueta de prédios
    x = 0
    while x < W:
        bw = rnd.randint(40, 160)
        bh = rnd.randint(30, 260)
        d.rectangle([x, horizon - bh, x + bw, H], fill=(9, 12, 20, 255))
        for _ in range(bw * bh // 900):
            wx = rnd.randint(x + 4, x + bw - 6)
            wy = rnd.randint(horizon - bh + 6, H - 6)
            c = rnd.choice([YELLOW, (255, 220, 160), CYAN, WHITE, (255, 170, 90)])
            a = rnd.randint(90, 220)
            d.rectangle([wx, wy, wx + 3, wy + 3], fill=c + (a,))
        x += bw + rnd.randint(0, 20)
    glow(img, layer, 6)


def xi_emblem(cx, cy, s, layer):
    """Ξ estilizado com V-fin por cima."""
    d = ImageDraw.Draw(layer)
    bar_w = int(s * 1.0)
    bar_h = int(s * 0.12)
    gap = int(s * 0.30)
    for i, (wf, col) in enumerate([(1.0, WHITE), (0.66, RED), (1.0, WHITE)]):
        bw = int(bar_w * wf)
        y = cy + (i - 1) * gap
        d.polygon(
            [
                (cx - bw // 2 + bar_h, y - bar_h // 2),
                (cx + bw // 2, y - bar_h // 2),
                (cx + bw // 2 - bar_h, y + bar_h // 2),
                (cx - bw // 2, y + bar_h // 2),
            ],
            fill=col + (255,),
        )
    # V-fin (antena) amarela
    top = cy - gap - int(s * 0.25)
    span = int(s * 0.95)
    thick = int(s * 0.07)
    d.polygon(
        [
            (cx, top + int(s * 0.12)),
            (cx - span // 2, top - int(s * 0.38)),
            (cx - span // 2 + thick * 2, top - int(s * 0.38)),
            (cx, top + int(s * 0.12) - thick * 2),
            (cx + span // 2 - thick * 2, top - int(s * 0.38)),
            (cx + span // 2, top - int(s * 0.38)),
        ],
        fill=YELLOW + (255,),
    )
    # câmera verde
    d.ellipse(
        [cx - thick, top + int(s * 0.04) - thick // 2, cx + thick, top + int(s * 0.04) + thick // 2],
        fill=GREEN + (255,),
    )


def wallpaper_night():
    img = vertical_gradient(W, H, NAVY_TOP, NAVY_BOT).convert("RGBA")

    grid = Image.new("RGBA", img.size, (0, 0, 0, 0))
    hud_grid(ImageDraw.Draw(grid), (76, 201, 240, 14))
    img.alpha_composite(grid)

    # rastro do beam saber cortando o céu
    beam = Image.new("RGBA", img.size, (0, 0, 0, 0))
    bd = ImageDraw.Draw(beam)
    bd.line([(int(W * 0.02), int(H * 0.52)), (int(W * 0.36), int(H * 0.03))], fill=PINK + (200,), width=6)
    bd.line([(int(W * 0.02), int(H * 0.52)), (int(W * 0.36), int(H * 0.03))], fill=(255, 220, 245, 255), width=2)
    glow(img, beam, 28)

    city_lights(img)

    emb = Image.new("RGBA", img.size, (0, 0, 0, 0))
    xi_emblem(W // 2, int(H * 0.45), 560, emb)
    glow(img, emb, 22)

    d = ImageDraw.Draw(img)
    title = font(FONT_BOLD, 96)
    sub = font(FONT_MONO or FONT_BOLD, 40)
    t = "RX-105  Ξ GUNDAM"
    tw = d.textlength(t, font=title)
    d.text((W / 2 - tw / 2, H * 0.62), t, font=title, fill=WHITE)
    s = "MAFTY NAVUE ERIN  //  MINOVSKY FLIGHT SYSTEM"
    sw = d.textlength(s, font=sub)
    d.text((W / 2 - sw / 2, H * 0.62 + 130), s, font=sub, fill=(107, 117, 144))

    # cantos HUD
    for (x, y, dx, dy) in [(80, 80, 1, 1), (W - 80, 80, -1, 1), (80, H - 80, 1, -1), (W - 80, H - 80, -1, -1)]:
        d.line([(x, y), (x + 160 * dx, y)], fill=CYAN, width=4)
        d.line([(x, y), (x, y + 160 * dy)], fill=CYAN, width=4)
    return img.convert("RGB")


def wallpaper_armor():
    """Placas de armadura angulares: branco, gunmetal, vermelho e amarelo."""
    img = Image.new("RGBA", (W, H), GUNMETAL_D + (255,))
    d = ImageDraw.Draw(img)

    plates = [
        ([(0, 0), (1500, 0), (1150, 900), (0, 1250)], GUNMETAL),
        ([(1500, 0), (3840, 0), (3840, 700), (2300, 1050), (1150, 900)], WHITE),
        ([(0, 1250), (1150, 900), (1550, 1500), (900, 2160), (0, 2160)], WHITE),
        ([(1150, 900), (2300, 1050), (2700, 1700), (1550, 1500)], GUNMETAL),
        ([(2300, 1050), (3840, 700), (3840, 2160), (2700, 1700)], (36, 43, 62)),
        ([(1550, 1500), (2700, 1700), (3000, 2160), (900, 2160)], (36, 43, 62)),
    ]
    for poly, col in plates:
        d.polygon(poly, fill=col + (255,))
    # linhas de painel
    for poly, _ in plates:
        d.line(poly + [poly[0]], fill=(12, 15, 24, 255), width=14)

    # acentos vermelho e amarelo
    d.polygon([(1150, 900), (2300, 1050), (2200, 1130), (1210, 1000)], fill=RED + (255,))
    d.polygon([(2700, 1700), (3840, 1450), (3840, 1540), (2760, 1790)], fill=YELLOW + (255,))
    d.polygon([(1700, 120), (2100, 120), (2050, 200), (1650, 200)], fill=RED + (255,))
    d.polygon([(200, 1900), (700, 1900), (650, 1960), (150, 1960)], fill=YELLOW + (255,))

    # vents (fendas) na placa branca
    for i in range(6):
        y = 300 + i * 70
        d.polygon([(2700, y), (3500, y - 160), (3500, y - 130), (2700, y + 30)], fill=(150, 158, 175, 255))

    # sensor verde com brilho
    eye = Image.new("RGBA", img.size, (0, 0, 0, 0))
    ImageDraw.Draw(eye).polygon([(1820, 1180), (2080, 1215), (2060, 1255), (1800, 1220)], fill=GREEN + (255,))
    glow(img, eye, 30)

    # marcações técnicas
    mono = font(FONT_MONO or FONT_BOLD, 44)
    d.text((2480, 820), "RX-105", font=mono, fill=(60, 68, 90))
    d.text((180, 1480), "Ξ  XI", font=font(FONT_BOLD, 120), fill=(200, 205, 214))
    d.text((3000, 1960), "ANAHEIM ELECTRONICS", font=mono, fill=(110, 120, 145))
    return img.convert("RGB")


def unlock_logo():
    s = 523
    img = Image.new("RGBA", (1108, s), (0, 0, 0, 0))
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    xi_emblem(1108 // 2, int(s * 0.62), 300, layer)
    glow(img, layer, 10)
    return img


if __name__ == "__main__":
    bg = HERE / "backgrounds"
    bg.mkdir(exist_ok=True)
    night = wallpaper_night()
    night.save(bg / "1-xi-night.png", optimize=True)
    wallpaper_armor().save(bg / "2-xi-armor.png", optimize=True)
    unlock_logo().save(HERE / "unlock.png")
    night.resize((960, 540), Image.LANCZOS).save(HERE / "preview.png")
    print("ok")
