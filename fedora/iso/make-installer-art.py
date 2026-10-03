#!/usr/bin/env python3
"""Gera as imagens do tema DoxIA do instalador (Anaconda GTK) a partir da marca da
tela de login (default/sddm/omarchy/brand.png e infinity.png).

  fedora/iso/anaconda/sidebar-bg.png    fundo da barra lateral (boas-vindas e resumo)
  fedora/iso/anaconda/sidebar-logo.png  ∞ DoxIA no topo da barra lateral
  fedora/iso/anaconda/topbar-bg.png     faixa do topo de cada tela de configuração

Rode de novo para regenerar: python3 fedora/iso/make-installer-art.py
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
SDDM = ROOT / "default/sddm/omarchy"
OUT = Path(__file__).parent / "anaconda"

TOP = (16, 17, 18)       # #101112, o mesmo carvão da tela de login
BOTTOM = (38, 41, 45)    # #26292d
RED = (238, 0, 0)


def gradient(width, height, top, bottom):
    img = Image.new("RGB", (width, height))
    draw = ImageDraw.Draw(img)
    for y in range(height):
        t = y / max(1, height - 1)
        draw.line([(0, y), (width, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)))
    return img


def sidebar_bg():
    img = gradient(406, 1080, TOP, BOTTOM).convert("RGBA")
    # O ∞ bem apagado, cortado pela borda de baixo, como na tela de login.
    inf = Image.open(SDDM / "infinity.png").convert("RGBA")
    inf = inf.resize((520, round(520 * inf.height / inf.width)), Image.LANCZOS)
    alpha = inf.getchannel("A").point(lambda a: round(a * 0.07))
    inf.putalpha(alpha)
    img.alpha_composite(inf, (-60, 1080 - round(inf.height * 0.62)))
    img.convert("RGB").save(OUT / "sidebar-bg.png", optimize=True)


def sidebar_logo():
    brand = Image.open(SDDM / "brand.png").convert("RGBA")
    width = 170
    brand.resize((width, round(width * brand.height / brand.width)), Image.LANCZOS) \
        .save(OUT / "sidebar-logo.png", optimize=True)


def topbar_bg():
    # Repetido pelo Anaconda; a linha vermelha embaixo vem do CSS (border-bottom).
    gradient(1040, 132, (26, 28, 31), TOP).save(OUT / "topbar-bg.png", optimize=True)


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    sidebar_bg()
    sidebar_logo()
    topbar_bg()
    for name in ("sidebar-bg.png", "sidebar-logo.png", "topbar-bg.png"):
        print("wrote", (OUT / name).relative_to(ROOT))
