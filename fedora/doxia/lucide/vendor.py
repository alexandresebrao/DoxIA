#!/usr/bin/env python3
"""Copia do lucide-static só os ícones que o map.tsv usa.

Uso (só quando for atualizar o Lucide; a instalação não roda isto):
  python3 -m venv /tmp/v && /tmp/v/bin/pip install picosvg
  /tmp/v/bin/python fedora/doxia/lucide/vendor.py <pasta do pacote lucide-static>

Gera, ao lado deste arquivo:
  svg/<id>.svg       o ícone em traço (stroke="currentColor"); make-theme pinta
  symbolic/<id>.svg  o mesmo em contorno preenchido, que o GTK recolore
  lucide.ttf e codepoints.json (a fonte, para a barra e os painéis), LICENSE e VERSION

<id> é o nome Lucide, ou "pasta+ícone": a pasta com o ícone pequeno dentro.
Também passa os extra/src/*.svg (ícones de fora do Lucide) para contorno.
"""
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
package = Path(sys.argv[1]).resolve()
icons = package / "icons"
picosvg = Path(sys.executable).with_name("picosvg")

STROKE = "1.75"
# Ícone interno das pastas compostas: 9px centrado no corpo da pasta.
INNER = 'transform="translate(7.5 8.5) scale(0.375)" stroke-width="4"'


def body(name):
    text = (icons / f"{name}.svg").read_text()
    return re.search(r"<svg[^>]*>(.*)</svg>", text, re.S).group(1).strip()


def stroke_svg(ident):
    parts = ident.split("+")
    inner = "".join(f"\n  <g {INNER}>\n    {body(p)}\n  </g>" for p in parts[1:])
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" '
        f'fill="none" stroke="currentColor" stroke-width="{STROKE}" stroke-linecap="round" stroke-linejoin="round">\n'
        f"  {body(parts[0])}{inner}\n</svg>\n"
    )


ids = set()
for line in (here / "map.tsv").read_text().splitlines():
    cols = line.split("\t")
    if len(cols) >= 3 and not line.startswith("#"):
        ids.add(cols[2])

for sub in ("svg", "symbolic"):
    shutil.rmtree(here / sub, ignore_errors=True)
    (here / sub).mkdir()

for ident in sorted(ids):
    src = here / "svg" / f"{ident}.svg"
    src.write_text(stroke_svg(ident))
    out = subprocess.run([str(picosvg), str(src)], check=True, capture_output=True, text=True).stdout
    # Cor padrão dos simbólicos do Adwaita; o GTK troca pela do texto.
    out = out.replace('fill="currentColor"', 'fill="#2e3436"').replace("<defs/>", "")
    out = re.sub(r"<svg[^>]*>", lambda m: re.sub(r'\s(class|width|height)="[^"]*"', "", m.group(0)), out, count=1)
    (here / "symbolic" / f"{ident}.svg").write_text(out)

# Ícones de fora do Lucide no mesmo estilo (traço 2 em grade de 24), como logos
# do Tabler Icons (MIT, extra/LICENSE.tabler): extra/src/<id>.svg em traço vira
# extra/<id>.svg em contorno, que o make-shell-font desenha na fonte.
for src in sorted((here / "extra" / "src").glob("*.svg")):
    out = subprocess.run([str(picosvg), str(src)], check=True, capture_output=True, text=True).stdout
    (here / "extra" / src.name).write_text(out.replace("<defs/>", ""))

shutil.copy(package / "font" / "lucide.ttf", here / "lucide.ttf")
shutil.copy(package / "font" / "codepoints.json", here / "codepoints.json")
shutil.copy(package / "LICENSE", here / "LICENSE")
version = json.loads((package / "package.json").read_text())["version"]
(here / "VERSION").write_text(f"lucide-static {version}\n")
print(f"{len(ids)} ícones do lucide-static {version}")
