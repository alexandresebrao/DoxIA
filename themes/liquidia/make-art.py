#!/usr/bin/env python3
"""Gera o fundo, o emblema da barra, a imagem de unlock e a prévia do tema LiquidIA.

O fundo é "vidro líquido": gotas fundidas (metaballs: blur + limiar) num
degradê vermelho -> laranja, com brilho especular na borda de cima. O ∞ é o infinito.png do DoxIA repintado no
mesmo degradê.

Rode de novo para regenerar: python3 make-art.py
"""
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


def wallpaper():
    size = (W, H)
    img = Image.new("RGBA", size, BG + (255,))
    # brasa ao fundo, bem difusa
    img.alpha_composite(radial(size, (int(W * 0.78), int(H * 0.25)), 1300, (140, 30, 10), 0.55))
    img.alpha_composite(radial(size, (int(W * 0.15), int(H * 0.9)), 1100, (110, 0, 20), 0.5))

    # fio de gotas atravessando a tela em diagonal
    rnd = random.Random(7)
    blobs = []
    for i in range(26):
        t = i / 25
        x = W * (0.05 + 0.9 * t) + rnd.uniform(-120, 120)
        y = H * (0.82 - 0.62 * t) + 260 * __import__("math").sin(t * 7.0) + rnd.uniform(-80, 80)
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
