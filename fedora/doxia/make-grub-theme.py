#!/usr/bin/env python3
"""Gera as imagens e fontes do tema DoxIA do GRUB a partir da marca da tela de login
(default/login/brand.png) e do fundo do tema LiquidIA.

  default/grub/doxia/background.png       fundo líquido do LiquidIA, desfocado e escurecido
  default/grub/doxia/logo.png             ∞ DoxIA no topo do menu
  default/grub/doxia/select_c.png         fundo da entrada escolhida
  default/grub/doxia/select_w.png         barra laranja à esquerda da entrada escolhida
  default/grub/doxia/red_hat_text_*.pf2   Red Hat Text nos tamanhos do theme.txt

O theme.txt é escrito à mão. Precisa do PIL, de grub2-mkfont (grub2-tools-extra) e das
fontes redhat-text-fonts. Rode de novo para regenerar: python3 fedora/doxia/make-grub-theme.py
"""
import subprocess
from pathlib import Path

from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "default/grub/doxia"
BRAND = ROOT / "default/login/brand.png"
FONT = "/usr/share/fonts/redhat/RedHatText-Regular.otf"

WALLPAPER = ROOT / "themes/liquidia/backgrounds/1-liquidia.png"
SHADE = (18, 10, 10)     # background do LiquidIA, por cima do fundo para o menu ler bem
SELECTED = (255, 255, 255, 22)
ACCENT = (255, 90, 31, 255)  # accent do LiquidIA

# Latim básico e Latin-1/Extended-A (acentos do português), sem o resto da fonte.
FONT_RANGES = "0x20-0x7E,0xA0-0x17F,0x2013-0x2014,0x2022-0x2022"
FONT_SIZES = (14, 18)


def background(width, height):
    # Desfocado como atrás de vidro: o líquido vira só a luz ambiente do menu.
    img = Image.open(WALLPAPER).convert("RGB").resize((width, height), Image.LANCZOS)
    img = img.filter(ImageFilter.GaussianBlur(40))
    return Image.blend(img, Image.new("RGB", (width, height), SHADE), 0.6)


def write_png(img, name):
    img.save(OUT / name, optimize=True)
    print("wrote", (OUT / name).relative_to(ROOT))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    # O GRUB estica o fundo para a resolução da tela.
    write_png(background(1920, 1080), "background.png")

    # O theme.txt centraliza a marca com largura e altura fixas (420x73): ela já vai no tamanho.
    brand = Image.open(BRAND).convert("RGBA")
    width = 420
    write_png(brand.resize((width, round(brand.height * width / brand.width)), Image.LANCZOS), "logo.png")

    # Caixa da entrada escolhida: só o centro e a borda esquerda (o GRUB estica cada peça).
    write_png(Image.new("RGBA", (8, 8), SELECTED), "select_c.png")
    write_png(Image.new("RGBA", (4, 8), ACCENT), "select_w.png")

    for size in FONT_SIZES:
        name = f"red_hat_text_{size}.pf2"
        subprocess.run(["grub2-mkfont", "-s", str(size), "-r", FONT_RANGES, "-o", OUT / name, FONT], check=True)
        print("wrote", (OUT / name).relative_to(ROOT))


if __name__ == "__main__":
    main()
