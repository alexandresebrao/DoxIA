#!/usr/bin/env python3
"""Gera o fundo, o emblema da barra, a imagem de unlock e a prévia do tema DoxIA.

O ∞ vem de infinito.png (gerado por infinito/infinito_flat3d.py, variante D) e a
marca da prévia, da tela de login (default/sddm/omarchy/brand.png).

Rode de novo para regenerar: python3 make-art.py
"""
from pathlib import Path

from PIL import Image

HERE = Path(__file__).parent
BRAND = HERE.parents[1] / "default/sddm/omarchy/brand.png"
W, H = 3840, 2160

CHARCOAL = (33, 36, 39)  # lighter_background de colors.toml


if __name__ == "__main__":
    bg = HERE / "backgrounds"
    bg.mkdir(exist_ok=True)
    # fundo liso no cinza do tema (lighter_background)
    for old in bg.glob("*.png"):
        old.unlink()
    Image.new("RGB", (W, H), CHARCOAL).save(bg / "1-doxia-cinza.png", optimize=True)

    logo = Image.open(HERE / "infinito.png").convert("RGBA")
    # Na barra o desenho fica um pouco maior que os ícones vizinhos (11 px ->
    # ~13 px dos 21 px que a barra dá ao emblem.png): 82 px de uma tela de 128.
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

    # Prévia do seletor de temas: a marca ∞ DoxIA no fundo do tema
    preview = Image.new("RGBA", (960, 540), CHARCOAL + (255,))
    brand = Image.open(BRAND).convert("RGBA")
    brand.thumbnail((560, 10000), Image.LANCZOS)
    preview.alpha_composite(brand, ((960 - brand.width) // 2, (540 - brand.height) // 2))
    preview.convert("RGB").save(HERE / "preview.png", optimize=True)
    print("ok")
