#!/usr/bin/env python3
"""Infinito como um "8" tipográfico deitado (bojos redondos tipo Verdana),
em fita de Möbius limpa. Reaproveita o renderizador de infinito.py."""
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import infinito as inf  # noqa: E402
from PIL import Image, ImageDraw, ImageFont  # noqa: E402

OUT = Path(sys.argv[1])


def eight_path(r_right=1.0, r_left=1.0, k=1.55, n=3000):
    """Dois círculos + as tangentes internas que se cruzam na origem.

    k = distância do centro / raio (igual nos dois, para as tangentes passarem
    pela origem). Retorna pontos 2D igualmente espaçados ao longo da curva.
    """
    phi = math.asin(1 / k)
    pts = []

    def line(a, b, steps=200):
        for i in range(steps):
            t = i / steps
            pts.append((a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t))

    def arc(c, r, a0, a1, steps=800):
        for i in range(steps):
            a = a0 + (a1 - a0) * i / steps
            pts.append((c[0] + r * math.cos(a), c[1] + r * math.sin(a)))

    cR, cL = k * r_right, k * r_left
    LR, LL = math.sqrt(cR**2 - r_right**2), math.sqrt(cL**2 - r_left**2)
    o = (0.0, 0.0)
    pu = (LR * math.cos(phi), LR * math.sin(phi))
    pd = (LR * math.cos(phi), -LR * math.sin(phi))
    qu = (-LL * math.cos(phi), LL * math.sin(phi))
    qd = (-LL * math.cos(phi), -LL * math.sin(phi))
    # ângulo do ponto de tangência visto do centro do círculo
    aR = math.pi / 2 + phi       # pu em relação a (cR, 0)
    aL = math.pi / 2 - phi       # qu em relação a (-cL, 0)
    line(o, pu)
    arc((cR, 0), r_right, aR, -aR)            # por cima e pela direita até pd
    line(pd, o)
    line(o, qu)
    arc((-cL, 0), r_left, aL, 2 * math.pi - aL)  # por cima e pela esquerda até qd
    line(qd, o)

    # reamostra por comprimento de arco
    acc = [0.0]
    for a, b in zip(pts, pts[1:] + pts[:1]):
        acc.append(acc[-1] + math.dist(a, b))
    total = acc[-1]
    out, j = [], 0
    for i in range(n):
        s = total * i / n
        while acc[j + 1] < s:
            j += 1
        a, b = pts[j], pts[(j + 1) % len(pts)]
        t = (s - acc[j]) / max(1e-9, acc[j + 1] - acc[j])
        out.append((a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t))
    # normaliza para largura total ~2
    w = max(p[0] for p in out) - min(p[0] for p in out)
    cx = (max(p[0] for p in out) + min(p[0] for p in out)) / 2
    return [((x - cx) * 2 / w, y * 2 / w) for x, y in out]


def use_path(path):
    n = len(path)

    def curve(t, h):
        u = (t / (2 * math.pi)) % 1 * n
        i = int(u)
        f = u - i
        a, b = path[i % n], path[(i + 1) % n]
        # z separa os dois cruzamentos (s=0 em cima, s=meio embaixo)
        return (a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, h * math.cos(t))

    inf.curve = curve


def local_twists(*centers, width=0.9):
    """Fita plana (theta=pi/2) com meias-voltas rápidas só perto de cada centro."""
    def smooth(x):
        x = min(1.0, max(0.0, x))
        return x * x * (3 - 2 * x)

    def theta(t):
        th = math.pi / 2
        for c in centers:
            # sem dar a volta no módulo: o salto de meia-volta fica na emenda t=0/2π,
            # onde as fileiras não se ligam
            d = t - c
            th += math.pi * smooth(d / width + 0.5)
        return th
    return theta


def ends(path):
    """Parâmetro t das pontas esquerda e direita (onde a torção fica bonita)."""
    n = len(path)
    i_l = min(range(n), key=lambda i: path[i][0])
    i_r = max(range(n), key=lambda i: path[i][0])
    return 2 * math.pi * i_l / n, 2 * math.pi * i_r / n


VIEW = dict(width=0.13, h=0.12, tilt=-0.38, yaw=0.15)
P_SIM, P_VER = dict(), dict(r_left=0.84)
L_VER, R_VER = ends(eight_path(**P_VER))
L_SIM, R_SIM = ends(eight_path(**P_SIM))
VARIANTS = {
    # uma meia-volta (Möbius de verdade): sem dois tons, senão aparece emenda
    "A-8-verdana-uma-torcao": dict(path=P_VER, build=dict(VIEW, theta=local_twists(L_VER, width=1.8)),
                                   render=dict(two_tone=False)),
    "B-8-verdana-duas-torcoes": dict(path=P_VER, build=dict(VIEW, theta=local_twists(L_VER, R_VER, width=1.8))),
    "C-8-simetrico-duas-torcoes": dict(path=P_SIM, build=dict(VIEW, theta=local_twists(L_SIM, R_SIM, width=1.8))),
}
CLEAN = dict(two_tone=True, dark=(190, 0, 0), spec=0.3, shininess=40, amb=0.42)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    tiles = []
    for name, v in VARIANTS.items():
        use_path(eight_path(**v["path"]))
        img = inf.render(inf.build("ribbon", twists=1, **v["build"]), **dict(CLEAN, **v.get("render", {})))
        img.save(OUT / f"{name}.png")
        tiles.append((name, img))
        print(name, img.size)
    sheet(tiles)


def sheet(tiles):
    try:
        f = ImageFont.truetype("/usr/share/fonts/redhat/RedHatText-Regular.otf", 30)
    except Exception:
        f = ImageFont.load_default()
    W, rowh = 1400, 360
    sh = Image.new("RGB", (W, rowh * len(tiles)), (21, 21, 21))
    d = ImageDraw.Draw(sh)
    labels = {"A": "A. 8 Verdana, uma torção", "B": "B. 8 Verdana, duas torções", "C": "C. 8 simétrico, duas torções"}
    for i, (name, img) in enumerate(tiles):
        y = i * rowh
        big = img.copy()
        big.thumbnail((640, 300), Image.LANCZOS)
        sh.paste(big, (40, y + (rowh - big.height) // 2), big)
        d.text((740, y + 40), labels[name[0]], font=f, fill=(240, 240, 240))
        bar = Image.new("RGB", (220, 35), (5, 5, 5))
        e = img.copy()
        e.thumbnail((200, 21), Image.LANCZOS)
        bar.paste(e, (10, (35 - e.height) // 2), e)
        sh.paste(bar, (740, y + 110))
        d.text((975, y + 112), "tamanho real", font=f, fill=(138, 141, 144))
        sh.paste(bar.resize((660, 105), Image.NEAREST), (740, y + 170))
    sh.save(OUT / "amostras-8.png")


if __name__ == "__main__":
    main()
