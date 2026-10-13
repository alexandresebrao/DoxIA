#!/usr/bin/env python3
"""Gera o fundo, o emblema da barra, a imagem de unlock e a prévia do tema LiquidIA.

O fundo é "vidro líquido": gotas fundidas (metaballs: blur + limiar) num
degradê vermelho -> laranja, com brilho especular na borda de cima, na frente
de um entardecer com uma casa no alto de uma montanha. O ∞ é o infinito.png do DoxIA repintado no
mesmo degradê.

Rode de novo para regenerar: python3 make-art.py
"""
import math
import random
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

HERE = Path(__file__).parent
BRAND = HERE.parents[1] / "default/sddm/omarchy/brand.png"
W, H = 3840, 2160

BG = (18, 10, 10)          # background de colors.toml
RED = (255, 42, 42)        # #ff2a2a
ORANGE = (255, 154, 31)    # #ff9a1f
DEEP = (120, 8, 20)        # sombra das gotas


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t + 0.5) for i in range(3))


def gradient(size, c0, c1, diagonal=True):
    """Degradê c0 -> c1 da esquerda-baixo para a direita-cima."""
    w, h = size
    small = Image.new("RGB", (256, 256))
    px = small.load()
    for y in range(256):
        for x in range(256):
            t = (x / 255 * 0.7 + (1 - y / 255) * 0.3) if diagonal else x / 255
            px[x, y] = lerp(c0, c1, t)
    return small.resize(size, Image.BILINEAR)


def radial(size, center, radius, color, strength):
    """Brilho radial suave (alpha cai até 0 no raio)."""
    w, h = size
    m = Image.new("L", size, 0)
    d = ImageDraw.Draw(m)
    cx, cy = center
    d.ellipse([cx - radius, cy - radius, cx + radius, cy + radius], fill=int(255 * strength))
    m = m.filter(ImageFilter.GaussianBlur(radius * 0.6))
    layer = Image.new("RGBA", size, color + (0,))
    layer.putalpha(m)
    return layer


def metaballs(size, blobs, blur, threshold):
    """Gotas fundidas: elipses borradas somadas e cortadas num limiar suave."""
    m = Image.new("L", size, 0)
    d = ImageDraw.Draw(m)
    for (x, y, rx, ry) in blobs:
        d.ellipse([x - rx, y - ry, x + rx, y + ry], fill=255)
    m = m.filter(ImageFilter.GaussianBlur(blur))
    lo, hi = threshold - 6, threshold + 6
    return m.point(lambda v: 0 if v < lo else 255 if v > hi else int((v - lo) * 255 / (hi - lo)))


def ridge(xs, base, waves, peak=None):
    """Linha de cume: base + ondas senoidais, com um pico gaussiano opcional."""
    pts = []
    for x in xs:
        y = base + sum(a * math.sin(f * x / W + ph) for (a, f, ph) in waves)
        if peak:
            px, ph_, pw = peak
            y -= ph_ * math.exp(-((x - px) / pw) ** 2)
        pts.append((x, y))
    return pts


def landscape(size):
    """Entardecer: céu quente, sol baixo, três serras e uma casa no pico."""
    w, h = size
    horizon = int(h * 0.80)
    layer = Image.new("RGBA", size, (0, 0, 0, 0))

    # céu: escuro em cima, brasa no horizonte
    sky = Image.new("RGBA", (1, 256))
    for y in range(256):
        t = (y / 255) ** 1.8
        sky.putpixel((0, y), lerp(BG, (170, 58, 22), t) + (int(255 * min(1, t * 1.4)),))
    layer.alpha_composite(sky.resize((w, horizon), Image.BILINEAR))

    # sol baixo, meio escondido pelas serras
    sx, sy = int(w * 0.63), int(h * 0.74)
    layer.alpha_composite(radial(size, (sx, sy), 900, (255, 120, 40), 0.55))
    layer.alpha_composite(radial(size, (sx, sy), 380, (255, 190, 110), 0.7))
    sun = Image.new("L", size, 0)
    ImageDraw.Draw(sun).ellipse([sx - 150, sy - 150, sx + 150, sy + 150], fill=255)
    disc = Image.new("RGBA", size, (255, 214, 150, 0))
    disc.putalpha(sun.filter(ImageFilter.GaussianBlur(6)))
    layer.alpha_composite(disc)

    xs = list(range(-40, w + 60, 20))
    house_x = w * 0.80
    serras = [
        # (base, ondas, pico, cor, blur): do mais longe para o mais perto
        (h * 0.70, [(70, 11, 1.0), (40, 27, 2.1), (18, 61, 0.7)], (w * 0.40, 420, 300), (96, 34, 26), 6),
        (h * 0.80, [(80, 7, 0.4), (35, 19, 1.7), (14, 47, 2.9)], (w * 0.18, 300, 360), (58, 18, 18), 3),
        (h * 0.92, [(40, 5, 2.4), (16, 21, 0.3), (8, 70, 1.3)], (house_x, 700, 430), (26, 9, 11), 1.5),
    ]
    for base, waves, peak, color, blur in serras:
        pts = ridge(xs, base, waves, peak)
        m = Image.new("L", size, 0)
        ImageDraw.Draw(m).polygon(pts + [(w + 60, h + 10), (-40, h + 10)], fill=255)
        hill = Image.new("RGBA", size, color + (0,))
        hill.putalpha(m.filter(ImageFilter.GaussianBlur(blur)))
        layer.alpha_composite(hill)
        top = pts

    # casa no pico da serra da frente, em silhueta, com a janela acesa
    hy = min(y for (x, y) in top if abs(x - house_x) < 60) + 55
    hx = int(house_x)
    sil = Image.new("L", size, 0)
    d = ImageDraw.Draw(sil)
    d.rectangle([hx - 120, hy - 150, hx + 120, hy + 80], fill=255)                 # corpo, enterrado no cume
    d.polygon([(hx - 160, hy - 140), (hx, hy - 285), (hx + 160, hy - 140)], fill=255)  # telhado
    d.rectangle([hx + 62, hy - 270, hx + 96, hy - 190], fill=255)                  # chaminé
    house = Image.new("RGBA", size, (26, 9, 11, 0))
    house.putalpha(sil.filter(ImageFilter.GaussianBlur(1.2)))
    layer.alpha_composite(house)
    win = [hx - 70, hy - 105, hx - 20, hy - 50]
    layer.alpha_composite(radial(size, ((win[0] + win[2]) // 2, (win[1] + win[3]) // 2), 110, (255, 160, 60), 0.5))
    wm = Image.new("L", size, 0)
    ImageDraw.Draw(wm).rectangle(win, fill=255)
    lit = Image.new("RGBA", size, (255, 196, 110, 0))
    lit.putalpha(wm.filter(ImageFilter.GaussianBlur(1)))
    layer.alpha_composite(lit)
    return layer


def wallpaper():
    size = (W, H)
    img = Image.new("RGBA", size, BG + (255,))
    # brasa ao fundo, bem difusa
    img.alpha_composite(radial(size, (int(W * 0.78), int(H * 0.25)), 1300, (140, 30, 10), 0.55))
    img.alpha_composite(radial(size, (int(W * 0.15), int(H * 0.9)), 1100, (110, 0, 20), 0.5))
    img.alpha_composite(landscape(size))

    # fio de gotas atravessando a tela em diagonal
    rnd = random.Random(7)
    blobs = []
    for i in range(26):
        t = i / 25
        x = W * (0.05 + 0.9 * t) + rnd.uniform(-120, 120)
        y = H * (0.82 - 0.62 * t) + 260 * math.sin(t * 7.0) + rnd.uniform(-80, 80)
        r = rnd.uniform(90, 300) * (1.25 - abs(t - 0.5))
        blobs.append((x, y, r * rnd.uniform(0.9, 1.3), r))
    # gotas soltas
    for _ in range(9):
        r = rnd.uniform(35, 110)
        blobs.append((rnd.uniform(0, W), rnd.uniform(0, H), r, r))
    mask = metaballs(size, blobs, 70, 120)

    # corpo da gota: degradê vermelho -> laranja, escurecendo para baixo
    body = gradient(size, RED, ORANGE).convert("RGBA")
    shade = mask.filter(ImageFilter.GaussianBlur(60))
    # "volume": onde a borda está perto, escurece para DEEP
    inner = ImageChops.multiply(mask, shade)
    deep = Image.new("RGBA", size, DEEP + (255,))
    body = Image.composite(body, deep, inner.point(lambda v: min(255, int(v * 1.6))))
    body.putalpha(mask)
    # sombra suave sob o líquido
    glow = Image.new("RGBA", size, (255, 70, 20, 0))
    glow.putalpha(mask.filter(ImageFilter.GaussianBlur(120)).point(lambda v: int(v * 0.45)))
    img.alpha_composite(glow)
    img.alpha_composite(body)

    # brilho especular: a máscara deslocada para baixo, recortada da original
    shifted = ImageChops.offset(mask, 0, 38)
    rim = ImageChops.subtract(mask, shifted).filter(ImageFilter.GaussianBlur(10))
    spec = Image.new("RGBA", size, (255, 240, 225, 0))
    spec.putalpha(rim.point(lambda v: int(v * 0.85)))
    img.alpha_composite(spec)
    # reflexo interno mais largo e fraco
    shifted2 = ImageChops.offset(mask, -30, 90)
    sheen = ImageChops.subtract(mask, shifted2).filter(ImageFilter.GaussianBlur(40))
    sh = Image.new("RGBA", size, (255, 210, 160, 0))
    sh.putalpha(sheen.point(lambda v: int(v * 0.35)))
    img.alpha_composite(sh)

    # granulação leve para não dar banding
    noise = Image.effect_noise((W // 2, H // 2), 18).resize(size).point(lambda v: v // 14)
    img = ImageChops.add(img.convert("RGB"), Image.merge("RGB", (noise, noise, noise)))
    return img


def recolor_logo(logo):
    """Repinta o ∞ vermelho no degradê, mantendo o claro/escuro do 3D."""
    r, g, b, a = logo.split()
    grad = gradient(logo.size, RED, ORANGE, diagonal=False)
    light = r.point(lambda v: min(255, int(v * 255 / 238)))
    out = ImageChops.multiply(grad, Image.merge("RGB", (light, light, light))).convert("RGBA")
    out.putalpha(a)
    return out


if __name__ == "__main__":
    bg = HERE / "backgrounds"
    bg.mkdir(exist_ok=True)
    for old in bg.glob("*.png"):
        old.unlink()
    wall = wallpaper()
    wall.save(bg / "1-liquidia.png", optimize=True)

    logo = recolor_logo(Image.open(HERE / "infinito.png").convert("RGBA"))
    emblem = logo.copy()
    emblem.thumbnail((10000, 82), Image.LANCZOS)
    pad = (128 - emblem.height) // 2
    canvas = Image.new("RGBA", (emblem.width + 4, 128), (0, 0, 0, 0))
    canvas.alpha_composite(emblem, (2, pad))
    canvas.save(HERE / "emblem.png")

    unlock_logo = logo.copy()
    unlock_logo.thumbnail((10000, 260), Image.LANCZOS)
    unlock = Image.new("RGBA", (1108, 523), (0, 0, 0, 0))
    unlock.alpha_composite(unlock_logo, ((1108 - unlock_logo.width) // 2, (523 - unlock_logo.height) // 2 + 40))
    unlock.save(HERE / "unlock.png")

    # Prévia do seletor de temas: o fundo com o ∞ em cima
    preview = wall.resize((960, 540), Image.LANCZOS).convert("RGBA")
    mark = logo.copy()
    mark.thumbnail((360, 10000), Image.LANCZOS)
    preview.alpha_composite(mark, ((960 - mark.width) // 2, (540 - mark.height) // 2))
    preview.convert("RGB").save(HERE / "preview.png", optimize=True)
    print("ok")
