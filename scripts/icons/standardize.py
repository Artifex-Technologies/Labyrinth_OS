#!/usr/bin/env python3
"""Standardize a folder of macOS-style app icons (.icns or .png) so they look like one set.

  standardize.py SRC_DIR OUT_DIR [--dark HEX] [--sheet]

For each icon:
  1. the tile is found (the opaque part) and cut out, dropping its soft shadow and any uneven margin;
  2. it is placed on a 1024 canvas at one size (82%), under one squircle mask, so every icon has the
     same size and the same corners;
  3. a dark tile (a neutral dark background) is brought to ONE dark colour and made matte: the
     gradient and gloss that differ from icon to icon are mostly taken out; the logo on it is left alone;
  4. coloured tiles (Firefox, Snagit, Notes...) are kept as they are, but get the same size and corners.

Writes OUT_DIR/<name>.png (1024) and, with --sheet, OUT_DIR/_before_after.png. Nothing in SRC_DIR is changed.
A report of what was done to each icon is printed.
"""
import argparse
import math
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

CANVAS = 1024
TILE = 840  # the tile's side on the canvas (82%)
SQUIRCLE_N = 5.0


def load(path):
    im = Image.open(path)
    if path.lower().endswith(".icns"):
        sizes = im.info.get("sizes") or []
        if sizes:
            best = max(sizes, key=lambda s: s[0] * s[1] * s[2])
            im = Image.open(path)
            im.size = (best[0], best[1])
            im.load()
    return im.convert("RGBA")


def squircle_mask(size, n=SQUIRCLE_N, ss=4):
    """An anti-aliased squircle (superellipse) filling a size x size square."""
    big = size * ss
    ys, xs = np.mgrid[0:big, 0:big]
    x = (xs + 0.5) / big * 2 - 1
    y = (ys + 0.5) / big * 2 - 1
    inside = (np.abs(x) ** n + np.abs(y) ** n) <= 1.0
    m = Image.fromarray((inside * 255).astype(np.uint8)).resize((size, size), Image.LANCZOS)
    return np.array(m).astype(np.float32) / 255.0


def lum(a):
    return 0.2126 * a[..., 0] + 0.7152 * a[..., 1] + 0.0722 * a[..., 2]


def tile_box(alpha):
    ys, xs = np.where(alpha > 200)
    return xs.min(), ys.min(), xs.max() + 1, ys.max() + 1


def base_colour(rgb, alpha):
    """The tile's own background: the middle of its left, right, top and bottom strips near the edge."""
    h, w = alpha.shape
    pts = [(int(w * 0.07), h // 2), (int(w * 0.93), h // 2), (w // 2, int(h * 0.07)), (w // 2, int(h * 0.93))]
    return np.median(np.array([rgb[y, x] for x, y in pts]), axis=0)


def standardize(im, dark_rgb, matte=0.30):
    a = np.array(im)
    alpha = a[..., 3].astype(np.float32)
    x0, y0, x1, y1 = tile_box(a[..., 3])
    # an object (a bin, a drive) is not a tile: it does not fill its box. Keep its shape, only set its size.
    filled = float((a[y0:y1, x0:x1, 3] > 200).mean())
    aspect = (x1 - x0) / (y1 - y0)
    if filled < 0.88 or not 0.92 < aspect < 1.08:
        obj = im.crop((max(0, x0 - 12), max(0, y0 - 12), min(a.shape[1], x1 + 12), min(a.shape[0], y1 + 12)))
        s = min(TILE / obj.width, TILE / obj.height)
        obj = obj.resize((max(1, int(obj.width * s)), max(1, int(obj.height * s))), Image.LANCZOS)
        out = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
        out.paste(obj, ((CANVAS - obj.width) // 2, (CANVAS - obj.height) // 2), obj)
        return out, "object", np.array([0, 0, 0])
    # cut out the tile, a little inside its edge so the new corners sit inside the old ones
    inset = int(0.012 * (x1 - x0))
    crop = im.crop((x0 + inset, y0 + inset, x1 - inset, y1 - inset)).resize((TILE, TILE), Image.LANCZOS)
    c = np.array(crop).astype(np.float32)
    rgb = c[..., :3]
    # the tile has been cut from inside its edge, so it is opaque everywhere the new mask is
    base = base_colour(rgb, c[..., 3])
    spread = float(base.max() - base.min())
    kind = "coloured"
    if spread < 14 and float(base.mean()) < 75:
        kind = "dark"
        l = lum(rgb)
        bl = float(base.mean())
        neutral = (rgb.max(axis=-1) - rgb.min(axis=-1)) < 14
        near = np.abs(l - bl) < 30
        sel = neutral & near
        std = np.array(dark_rgb, dtype=np.float32)
        # keep a little of the original gradient (matte, not dead flat)
        shift = ((l - bl) * matte)[..., None]
        # fade the change in at the edges of the selection so no halo is left round a logo
        weight = sel.astype(np.float32)
        weight = np.array(Image.fromarray((weight * 255).astype(np.uint8)).filter(__import__("PIL.ImageFilter", fromlist=["x"]).GaussianBlur(1.2))).astype(np.float32)[..., None] / 255.0
        new = std + shift
        rgb = rgb * (1 - weight) + new * weight
    out = np.zeros((CANVAS, CANVAS, 4), dtype=np.float32)
    off = (CANVAS - TILE) // 2
    mask = squircle_mask(TILE)
    out[off:off + TILE, off:off + TILE, :3] = np.clip(rgb, 0, 255)
    out[off:off + TILE, off:off + TILE, 3] = mask * 255
    return Image.fromarray(out.astype(np.uint8), "RGBA"), kind, base


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("out")
    ap.add_argument("--dark", default="1c1c22", help="the one dark tile colour (hex)")
    ap.add_argument("--sheet", action="store_true")
    a = ap.parse_args()
    dark = tuple(int(a.dark[i:i + 2], 16) for i in (0, 2, 4))
    os.makedirs(a.out, exist_ok=True)
    rows = []
    for f in sorted(os.listdir(a.src)):
        if not f.lower().endswith((".icns", ".png")):
            continue
        p = os.path.join(a.src, f)
        try:
            im = load(p)
            new, kind, base = standardize(im, dark)
        except Exception as e:  # keep going; say which one failed
            print(f"  skipped {f}: {e}")
            continue
        name = os.path.splitext(f)[0]
        name = name.rsplit("-", 1)[0] if len(name.rsplit("-", 1)[-1]) == 10 else name
        new.save(os.path.join(a.out, name + ".png"))
        rows.append((name, im, new, kind, base))
        print(f"{name[:44]:46s} {kind:9s} tile base {tuple(int(v) for v in base)}")
    if a.sheet and rows:
        cell = 144
        per = 8
        n = len(rows)
        h = ((n + per - 1) // per) * (cell * 2 + 16)
        sheet = Image.new("RGB", (per * cell, h), (58, 58, 66))
        for i, (name, before, after, kind, base) in enumerate(rows):
            r, c = divmod(i, per)
            y = r * (cell * 2 + 16)
            for k, im in enumerate((before, after)):
                t = im.resize((cell, cell), Image.LANCZOS)
                sheet.paste(t, (c * cell, y + k * cell), t)
        sheet.save(os.path.join(a.out, "_before_after.png"))
        print("sheet:", os.path.join(a.out, "_before_after.png"))


if __name__ == "__main__":
    sys.exit(main())
