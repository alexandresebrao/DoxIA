#!/usr/bin/env python3
"""Gera a marca do DoxIA: o ∞ do tema DoxIA com "DoxIA" ao lado, em Red Hat Display
(fonte livre, SIL OFL), "Dox" em cinza-claro e "IA" com a extrusão 3D do ∞. Nas
imagens o ∞ e o IA vão no degradê vermelho -> laranja do tema LiquidIA; nos
terminais ficam chapados em vermelho e o "Dox" usa a cor de texto do terminal, para
aparecer tanto em tema escuro quanto claro.

Saídas (rode da raiz do repositório depois de mudar a marca):
  logo.txt, icon.txt                         terminal, 8 linhas (icon.txt com $1/$2 de cor)
  fedora/doxia/branding/about.txt tela Sobre (fastfetch), igual ao icon.txt
  fedora/doxia/branding/logo.ansi omarchy-show-logo, em cores 24 bits
  fedora/doxia/branding/screensaver.txt  versão grande, 12 linhas
  fedora/doxia/brand.ansi                  saudação do terminal e /etc/motd, 8 linhas
  default/plymouth/doxia/watermark.png     marca no rodapé do boot splash
  default/login/brand.png                  marca da tela de login e do bloqueio (e nos
                                             terminais com imagem, em tema escuro)
  fedora/doxia/brand-light.png               a mesma com "Dox" escuro, para tema claro

Precisa do PIL e das fontes redhat-display-fonts.
"""
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]
FONT = "/usr/share/fonts/redhat/RedHatDisplay-Bold.otf"
INFINITY = ROOT / "themes/doxia/infinito.png"
LIGHT = (240, 240, 240)
DARK = (21, 21, 21)  # "Dox" sobre fundo claro
RED = (238, 0, 0)
SIDE = (163, 0, 0)  # lateral 3D, a mesma do ∞
# Degradê do LiquidIA (themes/liquidia/make-art.py), da esquerda para a direita.
GRAD_FROM = (255, 42, 42)
GRAD_TO = (255, 154, 31)

# Quadrantes: cada célula do terminal vale 2x2 pixels (cima-esq, cima-dir, baixo-esq, baixo-dir).
QUADRANTS = {
    (0, 0, 0, 0): " ", (1, 0, 0, 0): "▘", (0, 1, 0, 0): "▝", (0, 0, 1, 0): "▖",
    (0, 0, 0, 1): "▗", (1, 1, 0, 0): "▀", (0, 0, 1, 1): "▄", (1, 0, 1, 0): "▌",
    (0, 1, 0, 1): "▐", (1, 0, 0, 1): "▚", (0, 1, 1, 0): "▞", (1, 1, 1, 0): "▛",
    (1, 1, 0, 1): "▜", (1, 0, 1, 1): "▙", (0, 1, 1, 1): "▟", (1, 1, 1, 1): "█",
}


def infinity(height):
    img = Image.open(INFINITY).convert("RGBA")
    img = img.crop(img.getbbox())
    return img.resize((round(img.width * height / img.height), height), Image.LANCZOS)


def extrude(mask, depth):
    """A lateral 3D: o contorno deslocado passo a passo para baixo e à direita, na mesma
    direção e proporção do ∞ (themes/doxia/infinito/infinito_flat3d.py)."""
    out = Image.new("L", mask.size, 0)
    for i in range(1, int(depth) + 1):
        out = ImageChops.lighter(out, ImageChops.offset(mask, int(0.55 * i), int(0.85 * i)))
    return out


def brand(height, smooth, text=LIGHT):
    """∞ + DoxIA numa tela de `height` px. Devolve (RGBA colorida, máscara do vermelho).
    Na versão lisa (imagens) o IA ganha a extrusão do ∞; nos blocos do terminal fica chapado."""
    if smooth:
        # Desenha 4x maior para a extrusão sair sem serrilhado e reduz no fim.
        big, red = brand_canvas(height * 4, smooth=True, text=text)
        size = (big.width // 4, big.height // 4)
        return big.resize(size, Image.LANCZOS), red.resize(size, Image.LANCZOS)
    return brand_canvas(height, smooth=False)


def liquid(img, mask, x0, x1):
    """Repinta o vermelho entre x0 e x1 no degradê, mantendo o claro/escuro do 3D."""
    strip = Image.new("RGB", (256, 1))
    for x in range(256):
        strip.putpixel((x, 0), tuple(round(a + (b - a) * x / 255) for a, b in zip(GRAD_FROM, GRAD_TO)))
    grad = Image.new("RGB", img.size, GRAD_FROM)
    grad.paste(GRAD_TO, (x1, 0, img.width, img.height))
    grad.paste(strip.resize((x1 - x0, img.height), Image.BILINEAR), (x0, 0))
    region = Image.new("L", img.size, 0)
    region.paste(255, (x0, 0, x1, img.height))
    mask = ImageChops.multiply(mask, region)
    light = img.getchannel("R").point(lambda v: min(255, v * 255 // RED[0]))
    tinted = ImageChops.multiply(grad, Image.merge("RGB", (light, light, light))).convert("RGBA")
    tinted.putalpha(img.getchannel("A"))
    return Image.composite(tinted, img, mask)


def brand_canvas(height, smooth, text=LIGHT):
    font = ImageFont.truetype(FONT, int(height * 1.32))
    box = font.getbbox("DoxIA")
    cap = font.getbbox("D")
    inf = infinity(int(height * 0.78))
    dox = font.getlength("Dox")
    gap = int(height * 0.45)
    x_text = inf.width + gap
    # Mesma proporção do ∞: extrusão de ~42% da espessura do traço, que é ~20% da caixa-alta.
    depth = int((cap[3] - cap[1]) * 0.084) if smooth else 0
    width = x_text + int(dox + font.getlength("IA")) + 4 + depth
    canvas_h = height + depth
    y = -box[1] + (height - (box[3] - box[1])) // 2

    img = Image.new("RGBA", (width, canvas_h), (0, 0, 0, 0))
    red = Image.new("L", (width, canvas_h), 0)
    ImageDraw.Draw(img).text((x_text, y), "Dox", font=font, fill=text + (255,))
    ia = Image.new("L", (width, canvas_h), 0)
    ImageDraw.Draw(ia).text((x_text + dox, y), "IA", font=font, fill=255)
    if depth:
        side = extrude(ia, depth)
        img.paste(SIDE + (255,), (0, 0), side)
        red = ImageChops.lighter(red, side)
    img.paste(RED + (255,), (0, 0), ia)
    red = ImageChops.lighter(red, ia)

    inf_y = (height - inf.height) // 2
    if smooth:
        img.alpha_composite(inf, (0, inf_y))
    else:
        solid = inf.split()[3].point(lambda v: 255 if v > 100 else 0)
        img.paste(RED + (255,), (0, inf_y), solid)
    red.paste(255, (0, inf_y), inf.split()[3].point(lambda v: 255 if v > 100 else 0))
    if smooth:
        # O ∞ e o IA, cada um com o degradê inteiro, como o emblema do LiquidIA.
        img = liquid(img, red, 0, inf.width)
        img = liquid(img, red, int(x_text + dox), width)
    return img, red


def blocks(rows, oversample=4, threshold=110):
    """A marca em caracteres de quadrante: lista de linhas de (caractere, é_vermelho)."""
    img, red = brand(rows * 2 * oversample, smooth=False)
    size = (img.width // oversample, img.height // oversample)
    alpha = img.split()[3].resize(size, Image.BOX).load()
    red = red.resize(size, Image.BOX).load()
    w, h = size
    lines = []
    for cy in range(0, h, 2):
        line = []
        for cx in range(0, w - 1, 2):
            cells = [(cx + dx, cy + dy) for dy in (0, 1) for dx in (0, 1)]
            char = QUADRANTS[tuple(int(alpha[c] > threshold) for c in cells)]
            line.append((char, any(red[c] > threshold for c in cells)))
        lines.append(line)
    used = max(max((i for i, (c, _) in enumerate(l) if c != " "), default=-1) for l in lines) + 1
    return [l[:used] for l in lines]


def render(lines, light, red, reset=""):
    out = []
    for line in lines:
        text, current = "", None
        for char, is_red in line:
            # Espaços não precisam de cor; troca só quando um caractere visível muda de cor.
            if char != " " and is_red != current:
                text += red if is_red else light
                current = is_red
            text += char
        out.append(text.rstrip() + reset)
    return "\n".join(out) + "\n"


def write(path, text):
    (ROOT / path).write_text(text)
    print("wrote", path)


def main():
    medium = blocks(8)
    write("logo.txt", render(medium, "", ""))
    icon = render(medium, "$1", "$2")
    write("icon.txt", icon)
    write("fedora/doxia/branding/about.txt", icon)
    # "Dox" na cor de texto padrão do terminal (39), que acompanha o tema claro ou escuro.
    ansi_light, ansi_red = "\x1b[1;39m", "\x1b[1;38;2;238;0;0m"
    write("fedora/doxia/branding/logo.ansi", render(medium, ansi_light, ansi_red, "\x1b[0m"))
    write("fedora/doxia/branding/screensaver.txt", render(blocks(12), "", ""))
    write("fedora/doxia/brand.ansi", render(blocks(8), ansi_light, ansi_red, "\x1b[0m"))

    # Boot splash: a altura (43 px) e o arranjo ícone + nome da marca do Fedora, com a
    # marca ocupando ~3/4 da altura como lá.
    watermark, _ = brand(32, smooth=True)
    watermark = watermark.crop(watermark.getbbox())
    canvas = Image.new("RGBA", (watermark.width, 43), (0, 0, 0, 0))
    canvas.alpha_composite(watermark, (0, (43 - watermark.height) // 2))
    canvas.save(ROOT / "default/plymouth/doxia/watermark.png", optimize=True)
    print("wrote default/plymouth/doxia/watermark.png")

    login, _ = brand(110, smooth=True)
    login.crop(login.getbbox()).save(ROOT / "default/login/brand.png", optimize=True)
    print("wrote default/login/brand.png")

    light, _ = brand(110, smooth=True, text=DARK)
    light.crop(light.getbbox()).save(ROOT / "fedora/doxia/brand-light.png", optimize=True)
    print("wrote fedora/doxia/brand-light.png")


if __name__ == "__main__":
    main()
