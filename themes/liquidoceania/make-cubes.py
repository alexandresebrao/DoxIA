#!/usr/bin/env python3
"""Gera o fundo padrão do LiquidOceanIA: céu claro, sol, montanhas e um chão
de cubos em perspectiva com um cubo faltando a noroeste. Tudo plano, em tons
suaves de azul.

Rode de novo para regenerar: python3 make-cubes.py (precisa do ImageMagick)
"""
import subprocess
from pathlib import Path

HERE = Path(__file__).parent
W, H = 2560, 1440
# Perspective floor: camera HC cubes above the ground, looking at a horizon
# hidden behind the mountains. Rows run from Z0 (bottom of the screen) to ZF.
F, HC, YH, CX = 1300, 1.9, 520, W/2
Z0 = F*HC/(H-YH)
ROWS = 5
ZF = Z0 + ROWS
HOLE_X, HOLE_ROW = -3, 2       # missing cube: column, row counted from the back

def P(X, Z, d=0):
    return (CX + F*X/Z, YH + F*(HC+d)/Z)

def mix(a, b, t):
    a = [int(a[i:i+2], 16) for i in (1, 3, 5)]
    b = [int(b[i:i+2], 16) for i in (1, 3, 5)]
    return "#%02x%02x%02x" % tuple(round(x+(y-x)*t) for x, y in zip(a, b))

def poly(pts, fill, extra=""):
    return '<polygon points="%s" fill="%s" %s/>' % (" ".join("%.1f,%.1f" % p for p in pts), fill, extra)

out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="3840" height="2160" viewBox="0 0 {W} {H}">',
'''<defs>
<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
 <stop offset="0" stop-color="#b9d9ee"/><stop offset="0.65" stop-color="#e3f1f9"/><stop offset="1" stop-color="#eef7fb"/>
</linearGradient>
<radialGradient id="halo"><stop offset="0" stop-color="#fff6e0" stop-opacity="0.9"/><stop offset="1" stop-color="#fff6e0" stop-opacity="0"/></radialGradient>
</defs>''',
f'<rect width="{W}" height="{H}" fill="url(#sky)"/>',
'<circle cx="1720" cy="330" r="300" fill="url(#halo)"/>',
'<circle cx="1720" cy="330" r="135" fill="#fff3d9"/>',
# soft clouds
'<g fill="#f4fafd" opacity="0.85"><rect x="380" y="150" width="360" height="56" rx="28"/><rect x="470" y="112" width="190" height="56" rx="28"/>'
'<rect x="2080" y="190" width="300" height="48" rx="24"/><rect x="2160" y="158" width="150" height="48" rx="24"/></g>',
]
# mountains: far layer then near layer
far = [(0,520),(250,360),(470,470),(760,250),(1040,450),(1250,380),(1480,520),(1800,300),(2080,480),(2330,350),(2560,450),(2560,1440),(0,1440)]
near = [(0,640),(330,470),(600,600),(900,440),(1220,620),(1560,500),(1880,640),(2200,460),(2560,620),(2560,1440),(0,1440)]
out.append(poly(far, "#c9e0ef"))
out.append(poly(near, "#aecfe5"))
out.append(poly([(0,820),(400,740),(820,840),(1280,760),(1740,850),(2160,750),(2560,830),(2560,1440),(0,1440)], "#bcd8ea"))
# pit of the missing cube: its floor, the back wall facing us and the left
# wall (the hole is left of the camera, so only that side shows)
hx, hz = HOLE_X, ZF - HOLE_ROW - 1
out.append(poly([P(hx,hz,1),P(hx+1,hz,1),P(hx+1,hz+1,1),P(hx,hz+1,1)], "#b3d2e6"))
out.append(poly([P(hx,hz+1),P(hx+1,hz+1),P(hx+1,hz+1,1),P(hx,hz+1,1)], "#a2c6de"))
out.append(poly([P(hx,hz),P(hx,hz+1),P(hx,hz+1,1),P(hx,hz,1)], "#8bb3cf"))
# floor tiles, back to front, fading slightly into the haze with distance
tiles = []
for r in range(ROWS):
    z = ZF - r - 1
    t = 1 - r/(ROWS-1)
    for X in range(-12, 12):
        if (X, r) == (HOLE_X, HOLE_ROW): continue
        c = "#dcedf7" if (X + r) % 2 == 0 else "#cfe5f2"
        tiles.append(poly([P(X,z),P(X+1,z),P(X+1,z+1),P(X,z+1)], mix(c, "#e4f1f8", 0.45*t)))
out.append('<g stroke="#c4dcec" stroke-width="2" stroke-linejoin="round">' + "".join(tiles) + '</g>')
out.append('</svg>')
svg = HERE / "cubes.svg"
svg.write_text("\n".join(out))
subprocess.run(["magick", svg, HERE / "backgrounds" / "1-cubos.png"], check=True)
svg.unlink()
