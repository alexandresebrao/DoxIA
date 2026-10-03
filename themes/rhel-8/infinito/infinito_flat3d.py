#!/usr/bin/env python3
"""∞ em 3D flat: cores chapadas, lateral extrudada num vermelho mais escuro,
cruzamento com a parte da frente passando por cima (recorte e/ou sombra dura)."""
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from infinito8 import eight_path  # noqa: E402
from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont  # noqa: E402

OUT = Path(sys.argv[1])
SS = 4
FACE = (238, 0, 0)      # #ee0000
SIDE = (163, 0, 0)      # #a30000
SHADOW = (110, 0, 0)    # sombra dura no cruzamento
UNDER = (204, 0, 0)     # parte de trás, um tom abaixo


def stroke_mask(size, pts, w, closed=False):
    """Traço sem falhas: carimba discos ao longo do caminho (line() grossa
    com muitos segmentos deixa riscos finos por dentro)."""
    m = Image.new("L", size, 0)
    d = ImageDraw.Draw(m)
    q = pts + pts[:1] if closed else pts
    r = w / 2
    step = max(1.0, w / 40)
    for (x0, y0), (x1, y1) in zip(q, q[1:]):
        k = max(1, int(math.dist((x0, y0), (x1, y1)) / step))
        for i in range(k):
            x, y = x0 + (x1 - x0) * i / k, y0 + (y1 - y0) * i / k
            d.ellipse([x - r, y - r, x + r, y + r], fill=255)
    x, y = q[-1]
    d.ellipse([x - r, y - r, x + r, y + r], fill=255)
    return m


def solid(size, mask, color):
    img = Image.new("RGBA", size, color + (255,))
    img.putalpha(mask)
    return img


def extrude(mask, depth, angle=(0.55, 0.85)):
    """União do contorno deslocado passo a passo: a 'lateral' do objeto."""
    out = Image.new("L", mask.size, 0)
    steps = max(1, int(depth))
    for i in range(1, steps + 1):
        out = ImageChops.lighter(out, ImageChops.offset(mask, int(angle[0] * i), int(angle[1] * i)))
    return out


def render(path, stroke=0.22, depth=0.10, gap=0.0, two_tone=False, hard_shadow=False, size=1024):
    S = size * SS
    xs, ys = [p[0] for p in path], [p[1] for p in path]
    pad = stroke + depth + 0.1
    scale = S / (max(xs) - min(xs) + 2 * pad)
    H = int((max(ys) - min(ys) + 2 * pad) * scale)
    cy = (max(ys) + min(ys)) / 2
    P = [((x - min(xs) + pad) * scale, H / 2 - (y - cy) * scale) for x, y in path]
    size2 = (S, H)
    sw, dp = stroke * scale, depth * scale

    n = len(P)
    win = int(n * 0.09)
    top = P[-win:] + P[:win]                    # diagonal da frente
    back = P[n // 2 - win:n // 2 + win]         # diagonal de trás

    full = stroke_mask(size2, P, sw, closed=True)
    top_m = stroke_mask(size2, top, sw)
    back_m = stroke_mask(size2, back, sw)

    # disco em volta do cruzamento: só ali a parte da frente é redesenhada
    cx, cyy = P[0]
    R = sw * 1.35
    disk = Image.new("L", size2, 0)
    ImageDraw.Draw(disk).ellipse([cx - R, cyy - R, cx + R, cyy + R], fill=255)

    img = Image.new("RGBA", size2, (0, 0, 0, 0))
    # 1) lateral + face do símbolo inteiro
    if dp:
        img.alpha_composite(solid(size2, extrude(full, dp), SIDE))
    img.alpha_composite(solid(size2, full, FACE))
    if two_tone:
        under = ImageChops.multiply(back_m, disk.filter(ImageFilter.GaussianBlur(0)))
        img.alpha_composite(solid(size2, under, UNDER))

    # 2) no cruzamento, a parte da frente passa por cima
    layer = Image.new("RGBA", size2, (0, 0, 0, 0))
    if hard_shadow:
        sh = ImageChops.offset(top_m, int(sw * 0.16), int(sw * 0.24))
        sh = ImageChops.multiply(sh, back_m)
        layer.alpha_composite(solid(size2, sh, SHADOW))
    if dp:
        layer.alpha_composite(solid(size2, extrude(top_m, dp), SIDE))
    layer.alpha_composite(solid(size2, top_m, FACE))
    layer.putalpha(ImageChops.multiply(layer.getchannel("A"), disk))
    if gap:
        # recorte transparente em volta da parte da frente, só sobre a de trás
        ring = stroke_mask(size2, top, sw + 2 * gap * scale)
        if dp:
            ring = ImageChops.lighter(ring, extrude(ring, dp))
        cut = ImageChops.multiply(ImageChops.subtract(ring, ImageChops.lighter(top_m, extrude(top_m, dp) if dp else top_m)), disk)
        a = img.getchannel("A")
        img.putalpha(ImageChops.subtract(a, cut))
    img.alpha_composite(layer)

    img = img.resize((size, H // SS), Image.LANCZOS)
    return img.crop(img.getbbox())


BASE = dict(k=1.5)
VARIANTS = {
    "A-extrudado": dict(depth=0.10, gap=0.0),
    "B-extrudado-recorte": dict(depth=0.10, gap=0.05),
    "C-chapado-sombra": dict(depth=0.0, two_tone=True, hard_shadow=True),
    "D-extrudado-sombra": dict(depth=0.08, two_tone=True, hard_shadow=True),
}
LABELS = {
    "A": "A. extrudado",
    "B": "B. extrudado com recorte",
    "C": "C. chapado, sombra dura",
    "D": "D. extrudado + sombra dura",
}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    path = eight_path(**BASE)
    tiles = []
    for name, v in VARIANTS.items():
        img = render(path, **v)
        img.save(OUT / f"{name}.png")
        tiles.append((name, img))
        print(name, img.size)
    try:
        f = ImageFont.truetype("/usr/share/fonts/redhat/RedHatText-Regular.otf", 30)
    except Exception:
        f = ImageFont.load_default()
    Wd, rowh = 1400, 340
    sh = Image.new("RGB", (Wd, rowh * len(tiles)), (21, 21, 21))
    d = ImageDraw.Draw(sh)
    for i, (name, img) in enumerate(tiles):
        y = i * rowh
        big = img.copy()
        big.thumbnail((620, 280), Image.LANCZOS)
        sh.paste(big, (40, y + (rowh - big.height) // 2), big)
        d.text((740, y + 40), LABELS[name[0]], font=f, fill=(240, 240, 240))
        bar = Image.new("RGB", (220, 35), (5, 5, 5))
        e = img.copy()
        e.thumbnail((200, 21), Image.LANCZOS)
        bar.paste(e, (10, (35 - e.height) // 2), e)
        sh.paste(bar, (740, y + 110))
        d.text((975, y + 112), "tamanho real", font=f, fill=(138, 141, 144))
        sh.paste(bar.resize((660, 105), Image.NEAREST), (740, y + 170))
    sh.save(OUT / "amostras-flat.png")


if __name__ == "__main__":
    main()
