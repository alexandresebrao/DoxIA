#!/usr/bin/env python3
"""Amostras de símbolo do infinito 3D retorcido, vermelho Red Hat, com sombra.

Renderizador simples: superfície paramétrica em volta de uma lemniscata,
iluminação Lambert + especular, ordenação por profundidade e polígonos no PIL
em 4x (reduzido no fim para suavizar).
"""
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

OUT = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
RED = (238, 0, 0)
SS = 4


def norm(v):
    l = math.sqrt(sum(c * c for c in v)) or 1
    return tuple(c / l for c in v)


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def curve(t, h):
    d = 1 + math.sin(t) ** 2
    return (math.cos(t) / d * 1.0, math.sin(t) * math.cos(t) / d, h * math.sin(t))


def frame(t, h):
    e = 1e-4
    p0, p1 = curve(t - e, h), curve(t + e, h)
    T = norm(tuple(b - a for a, b in zip(p0, p1)))
    up = (0, 0, 1)
    N = norm(tuple(u - dot(up, T) * c for u, c in zip(up, T)))
    B = cross(T, N)
    return curve(t, h), T, N, B


def rot_x(p, a):
    x, y, z = p
    return (x, y * math.cos(a) - z * math.sin(a), y * math.sin(a) + z * math.cos(a))


def rot_y(p, a):
    x, y, z = p
    return (x * math.cos(a) + z * math.sin(a), y, -x * math.sin(a) + z * math.cos(a))


def build(kind, n=520, m=28, h=0.16, width=0.16, radius=0.085, twists=1, tilt=-0.55, yaw=0.0, phase=0.0, theta=None):
    """Lista de faces: (pontos 3D já girados, normal)."""
    faces = []
    view = lambda p: rot_y(rot_x(p, tilt), yaw)
    if kind == "ribbon":
        rows = []
        for i in range(n + 1):
            t = 2 * math.pi * i / n
            c, T, N, B = frame(t, h)
            th = theta(t) if theta else twists * t / 2 + phase  # meias-voltas ao longo da volta
            d = tuple(math.cos(th) * a + math.sin(th) * b for a, b in zip(N, B))
            rows.append([view(tuple(cc + s * width * dd for cc, dd in zip(c, d))) for s in (-1, 1)])
        for i in range(n):
            a, b = rows[i]
            c2, d2 = rows[i + 1]
            quad = [a, b, d2, c2]
            nrm = norm(cross(tuple(q - p for p, q in zip(a, b)), tuple(q - p for p, q in zip(a, c2))))
            faces.append((quad, nrm, 0))
    else:  # tube / rope
        rings = []
        for i in range(n + 1):
            t = 2 * math.pi * i / n
            c, T, N, B = frame(t, h)
            ring = []
            for j in range(m):
                ph = 2 * math.pi * j / m
                d = tuple(math.cos(ph) * a + math.sin(ph) * b for a, b in zip(N, B))
                ring.append((view(tuple(cc + radius * dd for cc, dd in zip(c, d))), view(d), ph))
            rings.append(ring)
        for i in range(n):
            t = 2 * math.pi * i / n
            for j in range(m):
                p00, n00, ph = rings[i][j]
                p01 = rings[i][(j + 1) % m][0]
                p10 = rings[i + 1][j][0]
                p11 = rings[i + 1][(j + 1) % m][0]
                stripe = 0
                if kind == "rope":
                    # faixas em espiral: dão o aspecto de corda torcida
                    stripe = 1 if math.sin(3 * ph + twists * 9 * t) > 0 else 0
                faces.append(([p00, p01, p11, p10], n00, stripe))
    return faces


def render(faces, size=1024, light=(-0.45, 0.55, 0.75), base=RED, dark=(150, 0, 0),
           spec=0.55, shininess=28, shadow=True, two_tone=False, amb=0.28):
    S = size * SS
    xs = [p[0] for f in faces for p in f[0]]
    ys = [p[1] for f in faces for p in f[0]]
    span = max(max(xs) - min(xs), (max(ys) - min(ys)) * 1.0)
    scale = S * 0.80 / span
    cx, cy = (max(xs) + min(xs)) / 2, (max(ys) + min(ys)) / 2
    H = int(S * (max(ys) - min(ys)) / span * 0.80 + S * 0.24)
    proj = lambda p: (S / 2 + (p[0] - cx) * scale, H / 2 - (p[1] - cy) * scale)

    L = norm(light)
    V = (0, 0, 1)
    Hv = norm(tuple(a + b for a, b in zip(L, V)))
    img = Image.new("RGBA", (S, H), (0, 0, 0, 0))
    mask = Image.new("L", (S, H), 0)
    d, md = ImageDraw.Draw(img), ImageDraw.Draw(mask)

    for pts, nrm, stripe in sorted(faces, key=lambda f: sum(p[2] for p in f[0])):
        back = nrm[2] < 0
        n = tuple(-c for c in nrm) if back else nrm
        diff = max(0.0, dot(n, L))
        sp = max(0.0, dot(n, Hv)) ** shininess * spec
        col = dark if (stripe or (two_tone and back)) else base

        c = tuple(min(255, int(col[k] * (amb + 0.85 * diff) + 255 * sp)) for k in range(3))
        poly = [proj(p) for p in pts]
        d.polygon(poly, fill=c + (255,))
        md.polygon(poly, fill=255)

    out = Image.new("RGBA", (S, H), (0, 0, 0, 0))
    if shadow:
        sh = mask.filter(ImageFilter.GaussianBlur(S * 0.018))
        sh_img = Image.new("RGBA", (S, H), (0, 0, 0, 0))
        sh_img.putalpha(sh.point(lambda v: int(v * 0.6)))
        out.alpha_composite(sh_img, (int(S * 0.018), int(S * 0.03)))
    out.alpha_composite(img)
    out = out.resize((size, H // SS), Image.LANCZOS)
    return out.crop(out.getbbox())


VARIANTS = {
    "1-fita-mobius": dict(kind="ribbon", twists=1, width=0.17, tilt=-0.6, yaw=0.25, render=dict(two_tone=True)),
    "2-fita-torcao-dupla": dict(kind="ribbon", twists=2, width=0.15, tilt=-0.5, yaw=-0.2, render=dict(two_tone=True)),
    "3-tubo-brilhante": dict(kind="tube", radius=0.085, tilt=-0.55, yaw=0.15, render=dict(spec=0.8, shininess=40)),
    "4-corda-retorcida": dict(kind="rope", radius=0.09, twists=2, tilt=-0.55, yaw=0.15, render=dict(spec=0.45)),
}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    tiles = []
    for name, v in VARIANTS.items():
        kw = {k: x for k, x in v.items() if k != "render"}
        img = render(build(**kw), **v.get("render", {}))
        img.save(OUT / f"{name}.png")
        tiles.append((name, img))
        print(name, img.size)
    sheet(tiles)


def sheet(tiles):
    """Folha de amostras: grande sobre o fundo do tema + tamanho real na barra."""
    from PIL import ImageFont
    try:
        f = ImageFont.truetype("/usr/share/fonts/redhat/RedHatText-Regular.otf", 30)
    except Exception:
        f = ImageFont.load_default()
    W, rowh = 1400, 360
    sh = Image.new("RGB", (W, rowh * len(tiles)), (21, 21, 21))
    d = ImageDraw.Draw(sh)
    for i, (name, img) in enumerate(tiles):
        y = i * rowh
        big = img.copy()
        big.thumbnail((640, 300), Image.LANCZOS)
        sh.paste(big, (40, y + (rowh - big.height) // 2), big)
        d.text((740, y + 40), name.split("-", 1)[0] + ". " + name.split("-", 1)[1].replace("-", " "),
               font=f, fill=(240, 240, 240))
        # barra real: 35 px de altura, emblema com ~21 px; e a mesma barra em 3x
        bar = Image.new("RGB", (220, 35), (5, 5, 5))
        e = img.copy()
        e.thumbnail((200, 21), Image.LANCZOS)
        bar.paste(e, (10, (35 - e.height) // 2), e)
        sh.paste(bar, (740, y + 110))
        d.text((975, y + 112), "tamanho real", font=f, fill=(138, 141, 144))
        bar3 = bar.resize((660, 105), Image.NEAREST)
        sh.paste(bar3, (740, y + 170))
    sh.save(OUT / "amostras.png")
    print(OUT / "amostras.png")


if __name__ == "__main__":
    main()
