#!/usr/bin/env python3
"""Gera as imagens e fontes do tema DoxIA do GRUB a partir da marca da tela de login
(default/sddm/omarchy/brand.png), no mesmo carvão do login e do instalador.

  default/grub/doxia/background.png       fundo em degradê
  default/grub/doxia/logo.png             ∞ DoxIA no topo do menu
  default/grub/doxia/select_c.png         fundo da entrada escolhida
  default/grub/doxia/select_w.png         barra vermelha à esquerda da entrada escolhida
  default/grub/doxia/red_hat_text_*.pf2   Red Hat Text nos tamanhos do theme.txt

O theme.txt é escrito à mão. Precisa do PIL, de grub2-mkfont (grub2-tools-extra) e das
fontes redhat-text-fonts. Rode de novo para regenerar: python3 fedora/doxia/make-grub-theme.py
"""
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "default/grub/doxia"
BRAND = ROOT / "default/sddm/omarchy/brand.png"
FONT = "/usr/share/fonts/redhat/RedHatText-Regular.otf"

TOP = (16, 17, 18)       # #101112, o mesmo carvão da tela de login
BOTTOM = (38, 41, 45)    # #26292d
SELECTED = (255, 255, 255, 22)
RED = (238, 0, 0, 255)

# Latim básico e Latin-1/Extended-A (acentos do português), sem o resto da fonte.
FONT_RANGES = "0x20-0x7E,0xA0-0x17F,0x2013-0x2014,0x2022-0x2022"
FONT_SIZES = (14, 18)


def gradient(width, height, top, bottom):
    img = Image.new("RGB", (width, height))
    draw = ImageDraw.Draw(img)
    for y in range(height):
        t = y / max(1, height - 1)
        draw.line([(0, y), (width, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)))
    return img


def write_png(img, name):
    img.save(OUT / name, optimize=True)
    print("wrote", (OUT / name).relative_to(ROOT))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    # O GRUB estica o fundo para a resolução da tela.
    write_png(gradient(1920, 1080, TOP, BOTTOM), "background.png")

    # O theme.txt centraliza a marca com largura e altura fixas (420x73): ela já vai no tamanho.
    brand = Image.open(BRAND).convert("RGBA")
    width = 420
    write_png(brand.resize((width, round(brand.height * width / brand.width)), Image.LANCZOS), "logo.png")

    # Caixa da entrada escolhida: só o centro e a borda esquerda (o GRUB estica cada peça).
    write_png(Image.new("RGBA", (8, 8), SELECTED), "select_c.png")
    write_png(Image.new("RGBA", (4, 8), RED), "select_w.png")

    for size in FONT_SIZES:
        name = f"red_hat_text_{size}.pf2"
        subprocess.run(["grub2-mkfont", "-s", str(size), "-r", FONT_RANGES, "-o", OUT / name, FONT], check=True)
        print("wrote", (OUT / name).relative_to(ROOT))


if __name__ == "__main__":
    main()
