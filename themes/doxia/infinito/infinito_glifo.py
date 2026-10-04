#!/usr/bin/env python3
"""Gera infinito-glifo.svg: o ∞ da variante D em uma cor só (para a fonte de
ícones do menu), com recorte no cruzamento mostrando o que passa por cima.

Precisa do shapely. Uso: python3 infinito_glifo.py saida.svg [previa.png]
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
sys.argv += ["/dev/null"] if len(sys.argv) < 2 else []
from infinito8 import eight_path  # noqa: E402
from shapely.geometry import LineString, Point, Polygon  # noqa: E402
from shapely.geometry.polygon import orient  # noqa: E402
from shapely.ops import unary_union  # noqa: E402

STROKE = 0.22   # mesma espessura da variante D
GAP = 0.06      # recorte em volta da diagonal da frente

path = eight_path(k=1.5)
n = len(path)
win = int(n * 0.09)
r = STROKE / 2

full = LineString(path + path[:1]).buffer(r, quad_segs=32)
top_line = LineString(path[-win:] + path[:win])
top = top_line.buffer(r, quad_segs=32)
cross = Point(path[0]).buffer(STROKE * 1.35, quad_segs=32)

cut = top_line.buffer(r + GAP, quad_segs=32).intersection(cross)
shape = unary_union([full.difference(cut), top.intersection(cross)])
shape = shape.simplify(0.0015)

polys = list(shape.geoms) if shape.geom_type == "MultiPolygon" else [shape]
minx, miny, maxx, maxy = shape.bounds
W, H = maxx - minx, maxy - miny


def ring_d(coords):
    pts = [(x - minx, maxy - y) for x, y in coords]  # SVG: y para baixo
    return "M" + " L".join(f"{x * 1000:.1f} {y * 1000:.1f}" for x, y in pts[:-1]) + " Z"


d = []
for p in polys:
    p = orient(Polygon(p.exterior, p.interiors), sign=1.0)
    d.append(ring_d(p.exterior.coords))
    d += [ring_d(i.coords) for i in p.interiors]

out = Path(sys.argv[1])
out.write_text(
    f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W * 1000:.1f} {H * 1000:.1f}">'
    f'<path fill-rule="nonzero" d="{" ".join(d)}"/></svg>\n'
)
print(out, f"{len(polys)} partes")

if len(sys.argv) > 2 and sys.argv[2] != "/dev/null":
    from PIL import Image, ImageDraw
    s = 400
    img = Image.new("L", (int(W * s) + 20, int(H * s) + 20), 0)
    dr = ImageDraw.Draw(img)
    for p in polys:
        dr.polygon([((x - minx) * s + 10, (maxy - y) * s + 10) for x, y in p.exterior.coords], fill=255)
        for i in p.interiors:
            dr.polygon([((x - minx) * s + 10, (maxy - y) * s + 10) for x, y in i.coords], fill=0)
    img.save(sys.argv[2])
