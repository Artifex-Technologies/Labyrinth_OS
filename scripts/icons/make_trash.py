#!/usr/bin/env python3
"""Make the trash icons from the standardized bin: user-trash.png (empty) and user-trash-full.png.

  make_trash.py BIN.png OUT_DIR [--size 512]

BIN.png is the bin as standardize.py wrote it (1024 canvas). The full bin is the same picture with crumpled paper
of crumpled paper heaped in the opening: they show over the grey inside and above the rim, and the front lip
of the bin covers their lower part, so it reads as paper IN the bin and not on it. The empty one is the
bin unchanged. Both have the same size and position, so the dock does not jump when it fills.
"""
import argparse
import math
import os
import random

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

# where the opening of the bin is, in the 1024 canvas (measured on the standardized picture)
CX, RIM_TOP, RX, RY = 511.0, 103.0, 328.0, 80.0
SS = 2  # draw at twice the size and shrink, for smooth edges


def lower_lip(x):
    """The y of the front lip of the opening at x (the lower half of its ellipse)."""
    t = 1 - ((x - CX) / RX) ** 2
    return RIM_TOP + RY + RY * math.sqrt(max(0.0, t))


def crumple(draw, c, r, seed, tint=None):
    """A crumpled ball of paper: a ragged outline cut into irregular facets of slightly different light
    greys, lighter towards the top, so it catches the light in pieces. `tint` colours some facets (a
    printed page showing through)."""
    rnd = random.Random(seed)
    n = 11
    pts = []
    for i in range(n):
        ang = 2 * math.pi * i / n + rnd.uniform(-0.2, 0.2)
        rad = r * rnd.uniform(0.70, 1.15)
        pts.append((c[0] + rad * math.cos(ang), c[1] + rad * math.sin(ang) * 0.86))
    # two inner points: the fan of facets is split between them, so the facets are not spokes of a wheel
    inner = [(c[0] + rnd.uniform(-0.25, 0.1) * r, c[1] + rnd.uniform(-0.3, 0.1) * r), (c[0] + rnd.uniform(-0.1, 0.3) * r, c[1] + rnd.uniform(-0.1, 0.35) * r)]
    k = 0
    for i in range(n):
        p1, p2 = pts[i], pts[(i + 1) % n]
        hub = inner[0] if i < n // 2 + 1 else inner[1]
        cy = (p1[1] + p2[1] + hub[1]) / 3
        top = (c[1] - cy) / r  # higher up: lighter
        v = int(236 + 14 * top + rnd.uniform(-10, 10))
        v = max(206, min(254, v))
        col = (v, v, min(255, v + 4))
        if tint and i in tint:
            col = tint[i]
        poly = [(hub[0] * SS, hub[1] * SS), (p1[0] * SS, p1[1] * SS), (p2[0] * SS, p2[1] * SS)]
        draw.polygon(poly, fill=col + (255,), outline=(182, 187, 198, 255))
        k += 1
    # a few folds across the middle
    for _ in range(3):
        a1, a2 = pts[rnd.randrange(n)], pts[rnd.randrange(n)]
        draw.line([(a1[0] * SS, a1[1] * SS), (inner[0][0] * SS, inner[0][1] * SS), (a2[0] * SS, a2[1] * SS)], fill=(190, 195, 206, 255), width=2 * SS)


def mound(draw, seed=31):
    """The pile that fills the opening from rim to rim, darker towards the sides and into the bin, so
    there is paper all the way round and the heap on top has something to sit on."""
    rnd = random.Random(seed)
    n = 20
    cy = RIM_TOP + RY
    pts = []
    for i in range(n):
        ang = 2 * math.pi * i / n
        pts.append((CX + (RX - 4) * math.cos(ang), cy + (RY - 2) * math.sin(ang)))
    # a base first, so no part of the opening is left empty between the facets
    draw.polygon([(x * SS, y * SS) for x, y in pts], fill=(206, 209, 218, 255))
    hubs = [(CX - 150, cy - 18), (CX + 140, cy - 12), (CX, cy + 6)]
    for i in range(n):
        p1, p2 = pts[i], pts[(i + 1) % n]
        hub = min(hubs, key=lambda h: (h[0] - (p1[0] + p2[0]) / 2) ** 2)
        mx, my = (p1[0] + p2[0] + hub[0]) / 3, (p1[1] + p2[1] + hub[1]) / 3
        edge = min(1.0, abs(mx - CX) / RX)
        v = int(226 + 12 * (cy - my) / RY - 24 * edge ** 2 + rnd.uniform(-8, 8))
        v = max(180, min(246, v))
        draw.polygon([(hub[0] * SS, hub[1] * SS), (p1[0] * SS, p1[1] * SS), (p2[0] * SS, p2[1] * SS)], fill=(v, v, min(255, v + 4), 255), outline=(176, 181, 192, 255))


def full_from(bin_img):
    base = bin_img.convert("RGBA")
    layer = Image.new("RGBA", (1024 * SS, 1024 * SS), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    # back to front: the pile that fills the opening, the small ones at the sides, then the heap on top
    mound(d)
    crumple(d, (326, 150), 60, 3)
    crumple(d, (700, 148), 62, 5)
    crumple(d, (408, 116), 88, 11, tint={2: (112, 160, 228), 3: (240, 158, 92)})
    crumple(d, (618, 120), 86, 19, tint={6: (120, 196, 140)})
    crumple(d, (512, 84), 94, 23)
    layer = layer.resize((1024, 1024), Image.LANCZOS)
    arr = np.array(layer).astype(np.float32)
    ys, xs = np.mgrid[0:1024, 0:1024]
    lip = np.vectorize(lower_lip)(np.arange(1024)).astype(np.float32)  # per column
    inside = (ys < lip[None, :]) & (xs > CX - RX + 6) & (xs < CX + RX - 6)
    # the lip's shadow: the paper darkens a little as it goes down into the bin
    depth = np.clip((ys - (lip[None, :] - 70)) / 70.0, 0, 1)
    arr[..., :3] *= (1 - 0.30 * depth)[..., None]
    arr[..., 3] *= inside
    papers = Image.fromarray(arr.astype(np.uint8), "RGBA")
    out = Image.alpha_composite(base, papers)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("bin")
    ap.add_argument("out")
    ap.add_argument("--size", type=int, default=512)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    empty = Image.open(a.bin).convert("RGBA")
    full = full_from(empty)
    for name, im in (("user-trash", empty), ("user-trash-full", full)):
        im.resize((a.size, a.size), Image.LANCZOS).save(os.path.join(a.out, name + ".png"))
    print("wrote user-trash.png and user-trash-full.png to", a.out)


if __name__ == "__main__":
    main()
