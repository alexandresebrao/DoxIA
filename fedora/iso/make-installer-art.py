#!/usr/bin/env python3
"""Gera as imagens do tema DoxIA do instalador (Anaconda GTK) a partir da marca da
tela de login (default/sddm/omarchy/brand.png e infinity.png).

  fedora/iso/anaconda/sidebar-bg.png    fundo da barra lateral (boas-vindas e resumo)
  fedora/iso/anaconda/sidebar-logo.png  ∞ DoxIA deitado, de baixo para cima, no pé da barra lateral
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
    gradient(406, 1080, TOP, BOTTOM).save(OUT / "sidebar-bg.png", optimize=True)


def sidebar_logo():
    # A barra lateral tem 15% da largura da janela (154 px a 1024x768), estreita demais
    # para a marca na horizontal: ela vai girada, lida de baixo para cima, com o ∞ embaixo.
    # O CSS a prende no rodapé (background-position: 50% 100%); a margem já vem na imagem.
    brand = Image.open(SDDM / "brand.png").convert("RGBA")
    thickness, margin = 60, 56
    length = round(thickness * brand.width / brand.height)
    brand = brand.resize((length, thickness), Image.LANCZOS).transpose(Image.Transpose.ROTATE_90)
    img = Image.new("RGBA", (thickness, length + margin))
    img.alpha_composite(brand, (0, 0))
    img.save(OUT / "sidebar-logo.png", optimize=True)


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
