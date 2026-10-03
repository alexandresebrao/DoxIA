#!/usr/bin/env python3
"""Gera a marca do FedorAI: o ∞ do tema RHEL 8 com "FedorAI" ao lado, em Red Hat Display
(a fonte da marca do RHEL), "Fedor" em cinza-claro e "AI" em vermelho com a extrusão 3D
do ∞ (nos terminais, chapado).

Saídas (rode da raiz do repositório depois de mudar a marca):
  logo.txt, icon.txt                         terminal, 8 linhas (icon.txt com $1/$2 de cor)
  fedora/personal/omarchy/branding/about.txt tela Sobre (fastfetch), igual ao icon.txt
  fedora/personal/omarchy/branding/logo.ansi omarchy-show-logo, em cores 24 bits
  fedora/personal/omarchy/branding/screensaver.txt  versão grande, 12 linhas
  fedora/fedorai/brand.ansi                  saudação do terminal e /etc/motd, 8 linhas
  default/plymouth/fedorai/watermark.png     marca no rodapé do boot splash
  default/sddm/omarchy/brand.png             marca no rodapé da tela de login

Precisa do PIL e das fontes redhat-display-fonts.
"""
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]
FONT = "/usr/share/fonts/redhat/RedHatDisplay-Bold.otf"
INFINITY = ROOT / "themes/rhel-8/infinito.png"
LIGHT = (240, 240, 240)
RED = (238, 0, 0)
SIDE = (163, 0, 0)  # lateral 3D, a mesma do ∞

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
    direção e proporção do ∞ (themes/rhel-8/infinito/infinito_flat3d.py)."""
    out = Image.new("L", mask.size, 0)
    for i in range(1, int(depth) + 1):
        out = ImageChops.lighter(out, ImageChops.offset(mask, int(0.55 * i), int(0.85 * i)))
    return out


def brand(height, smooth):
    """∞ + FedorAI numa tela de `height` px. Devolve (RGBA colorida, máscara do vermelho).
    Na versão lisa (imagens) o AI ganha a extrusão do ∞; nos blocos do terminal fica chapado."""
    if smooth:
        # Desenha 4x maior para a extrusão sair sem serrilhado e reduz no fim.
        big, red = brand_canvas(height * 4, smooth=True)
        size = (big.width // 4, big.height // 4)
        return big.resize(size, Image.LANCZOS), red.resize(size, Image.LANCZOS)
    return brand_canvas(height, smooth=False)


def brand_canvas(height, smooth):
    font = ImageFont.truetype(FONT, int(height * 1.32))
    box = font.getbbox("FedorAI")
    cap = font.getbbox("F")
    inf = infinity(int(height * 0.78))
    fedor = font.getlength("Fedor")
    gap = int(height * 0.45)
    x_text = inf.width + gap
    # Mesma proporção do ∞: extrusão de ~42% da espessura do traço, que é ~20% da caixa-alta.
    depth = int((cap[3] - cap[1]) * 0.084) if smooth else 0
    width = x_text + int(fedor + font.getlength("AI")) + 4 + depth
    canvas_h = height + depth
    y = -box[1] + (height - (box[3] - box[1])) // 2

    img = Image.new("RGBA", (width, canvas_h), (0, 0, 0, 0))
    red = Image.new("L", (width, canvas_h), 0)
    ImageDraw.Draw(img).text((x_text, y), "Fedor", font=font, fill=LIGHT + (255,))
    ai = Image.new("L", (width, canvas_h), 0)
    ImageDraw.Draw(ai).text((x_text + fedor, y), "AI", font=font, fill=255)
    if depth:
        side = extrude(ai, depth)
        img.paste(SIDE + (255,), (0, 0), side)
        red = ImageChops.lighter(red, side)
    img.paste(RED + (255,), (0, 0), ai)
    red = ImageChops.lighter(red, ai)

    inf_y = (height - inf.height) // 2
    if smooth:
        img.alpha_composite(inf, (0, inf_y))
    else:
        solid = inf.split()[3].point(lambda v: 255 if v > 100 else 0)
        img.paste(RED + (255,), (0, inf_y), solid)
    red.paste(255, (0, inf_y), inf.split()[3].point(lambda v: 255 if v > 100 else 0))
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
    write("fedora/personal/omarchy/branding/about.txt", icon)
    ansi_light, ansi_red = "\x1b[1;38;2;240;240;240m", "\x1b[1;38;2;238;0;0m"
    write("fedora/personal/omarchy/branding/logo.ansi", render(medium, ansi_light, ansi_red, "\x1b[0m"))
    write("fedora/personal/omarchy/branding/screensaver.txt", render(blocks(12), "", ""))
    write("fedora/fedorai/brand.ansi", render(blocks(8), ansi_light, ansi_red, "\x1b[0m"))

    # Boot splash: a altura (43 px) e o arranjo ícone + nome da marca do Fedora, com a
    # marca ocupando ~3/4 da altura como lá.
    watermark, _ = brand(32, smooth=True)
    watermark = watermark.crop(watermark.getbbox())
    canvas = Image.new("RGBA", (watermark.width, 43), (0, 0, 0, 0))
    canvas.alpha_composite(watermark, (0, (43 - watermark.height) // 2))
    canvas.save(ROOT / "default/plymouth/fedorai/watermark.png", optimize=True)
    print("wrote default/plymouth/fedorai/watermark.png")

    login, _ = brand(110, smooth=True)
    login.crop(login.getbbox()).save(ROOT / "default/sddm/omarchy/brand.png", optimize=True)
    print("wrote default/sddm/omarchy/brand.png")


if __name__ == "__main__":
    main()
