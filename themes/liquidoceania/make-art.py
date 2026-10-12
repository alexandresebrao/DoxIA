#!/usr/bin/env python3
"""Gera o fundo, o emblema da barra, a imagem de unlock e a prévia do tema LiquidOceanIA.

O mesmo vidro líquido do LiquidIA (themes/liquidia/make-art.py) em tom de
oceano: céu claro e enevoado, bancos de neblina, uma faixa de água de azul a
ciano com brilho na borda de cima e um painel de vidro fosco por cima. O ∞ é
o infinito.png do DoxIA repintado no mesmo degradê.

Rode de novo para regenerar: python3 make-art.py
"""
import math
import random
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

HERE = Path(__file__).parent
W, H = 3840, 2160

SKY_TOP = (238, 243, 247)     # névoa clara
SKY_BOTTOM = (208, 220, 231)  # névoa mais densa embaixo
BLUE = (31, 107, 255)         # #1f6bff
CYAN = (47, 212, 255)         # #2fd4ff
DEEP = (16, 52, 110)          # fundo da água
NAVY = (27, 42, 61)           # foreground de colors.toml, para a sombra


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t + 0.5) for i in range(3))


def gradient(size, c0, c1, diagonal=True):
    small = Image.new("RGB", (256, 256))
    px = small.load()
    for y in range(256):
        for x in range(256):
            t = (x / 255 * 0.7 + (1 - y / 255) * 0.3) if diagonal else x / 255
            px[x, y] = lerp(c0, c1, t)
    return small.resize(size, Image.BILINEAR)


def vertical(size, top, bottom):
    small = Image.new("RGB", (1, 256))
    for y in range(256):
        small.putpixel((0, y), lerp(top, bottom, y / 255))
    return small.resize(size, Image.BILINEAR)


def soft(size, center, rx, ry, color, strength):
    m = Image.new("L", size, 0)
    cx, cy = center
    ImageDraw.Draw(m).ellipse([cx - rx, cy - ry, cx + rx, cy + ry], fill=int(255 * strength))
    m = m.filter(ImageFilter.GaussianBlur(min(rx, ry) * 0.7))
    layer = Image.new("RGBA", size, color + (0,))
    layer.putalpha(m)
    return layer


def metaballs(size, blobs, blur, threshold):
    m = Image.new("L", size, 0)
    d = ImageDraw.Draw(m)
    for (x, y, rx, ry) in blobs:
        d.ellipse([x - rx, y - ry, x + rx, y + ry], fill=255)
    m = m.filter(ImageFilter.GaussianBlur(blur))
    lo, hi = threshold - 6, threshold + 6
    return m.point(lambda v: 0 if v < lo else 255 if v > hi else int((v - lo) * 255 / (hi - lo)))


def wallpaper():
    size = (W, H)
    img = vertical(size, SKY_TOP, SKY_BOTTOM).convert("RGBA")
    # bancos de neblina: manchas brancas largas e bem difusas
    rnd = random.Random(3)
    for _ in range(7):
        img.alpha_composite(soft(size, (rnd.uniform(0, W), rnd.uniform(H * 0.2, H)),
                                 rnd.uniform(700, 1400), rnd.uniform(160, 320), (255, 255, 255), 0.55))
    img.alpha_composite(soft(size, (int(W * 0.8), int(H * 0.2)), 1100, 700, (190, 225, 245), 0.35))

    # a faixa de água, mais calma que a do LiquidIA (ondas mais longas)
    rnd = random.Random(7)
    blobs = []
    for i in range(30):
        t = i / 29
        x = W * (0.02 + 0.96 * t) + rnd.uniform(-80, 80)
        y = H * (0.78 - 0.5 * t) + 200 * math.sin(t * 5.0) + rnd.uniform(-50, 50)
        r = rnd.uniform(110, 260) * (1.2 - abs(t - 0.5) * 0.8)
        blobs.append((x, y, r * rnd.uniform(1.0, 1.4), r))
    for _ in range(7):
        r = rnd.uniform(30, 90)
        blobs.append((rnd.uniform(0, W), rnd.uniform(0, H), r, r))
    mask = metaballs(size, blobs, 80, 120)

    body = gradient(size, BLUE, CYAN).convert("RGBA")
    shade = mask.filter(ImageFilter.GaussianBlur(70))
    inner = ImageChops.multiply(mask, shade)
    deep = Image.new("RGBA", size, DEEP + (255,))
    body = Image.composite(body, deep, inner.point(lambda v: min(255, int(v * 1.7))))
    # água mais translúcida que o líquido do LiquidIA: deixa a névoa aparecer
    body.putalpha(mask.point(lambda v: int(v * 0.88)))
    shadow = Image.new("RGBA", size, NAVY + (0,))
    shadow.putalpha(ImageChops.offset(mask, 0, 60).filter(ImageFilter.GaussianBlur(90)).point(lambda v: int(v * 0.22)))
    img.alpha_composite(shadow)
    img.alpha_composite(body)

    shifted = ImageChops.offset(mask, 0, 40)
    rim = ImageChops.subtract(mask, shifted).filter(ImageFilter.GaussianBlur(10))
    spec = Image.new("RGBA", size, (255, 255, 255, 0))
    spec.putalpha(rim.point(lambda v: int(v * 0.95)))
    img.alpha_composite(spec)
    shifted2 = ImageChops.offset(mask, -30, 100)
    sheen = ImageChops.subtract(mask, shifted2).filter(ImageFilter.GaussianBlur(45))
    sh = Image.new("RGBA", size, (225, 245, 255, 0))
    sh.putalpha(sheen.point(lambda v: int(v * 0.45)))
    img.alpha_composite(sh)

    # neblina por cima de tudo, mais densa embaixo: deixa o quadro sereno
    fog = Image.new("RGBA", size, (240, 245, 249, 0))
    fog.putalpha(vertical(size, (40, 40, 40), (120, 120, 120)).convert("L"))
    img.alpha_composite(fog)

    # painel de vidro fosco
    px, py, pw, ph, pr = int(W * 0.56), int(H * 0.46), 1180, 760, 90
    panel_mask = Image.new("L", size, 0)
    ImageDraw.Draw(panel_mask).rounded_rectangle([px, py, px + pw, py + ph], pr, fill=255)
    frosted = img.filter(ImageFilter.GaussianBlur(55))
    frosted.alpha_composite(Image.new("RGBA", size, (255, 255, 255, 70)))
    img = Image.composite(frosted, img, panel_mask)
    edge = Image.new("L", size, 0)
    ed = ImageDraw.Draw(edge)
    ed.rounded_rectangle([px, py, px + pw, py + ph], pr, outline=200, width=3)
    ed.rounded_rectangle([px + 4, py + 4, px + pw - 4, py + ph // 2], pr - 4, outline=90, width=2)
    stroke = Image.new("RGBA", size, (255, 255, 255, 0))
    stroke.putalpha(edge.filter(ImageFilter.GaussianBlur(1.2)))
    img.alpha_composite(stroke)

    noise = Image.effect_noise((W // 2, H // 2), 18).resize(size).point(lambda v: v // 16)
    return ImageChops.subtract(img.convert("RGB"), Image.merge("RGB", (noise, noise, noise)))


def recolor_logo(logo):
    r, g, b, a = logo.split()
    grad = gradient(logo.size, BLUE, CYAN, diagonal=False)
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
    wall.save(bg / "1-liquidoceania.png", optimize=True)

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

    preview = wall.resize((960, 540), Image.LANCZOS).convert("RGBA")
    mark = logo.copy()
    mark.thumbnail((360, 10000), Image.LANCZOS)
    preview.alpha_composite(mark, ((960 - mark.width) // 2, (540 - mark.height) // 2))
    preview.convert("RGB").save(HERE / "preview.png", optimize=True)
    print("ok")
