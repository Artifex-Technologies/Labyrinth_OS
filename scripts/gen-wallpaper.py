#!/usr/bin/env python3
"""Generate Labyrinth's default wallpaper: a dark purple diagonal gradient with soft glows.

Standard library only, so it can run anywhere. Output is a plain PNG. Placeholder art for
the look (a dark night mood in the brand purple #3A1F5D); replace with real artwork freely.

  scripts/gen-wallpaper.py [OUT.png] [WIDTH HEIGHT]
"""
import math
import struct
import sys
import zlib

out = sys.argv[1] if len(sys.argv) > 1 else "profile/airootfs/usr/share/labyrinth/wallpapers/labyrinth-purple.png"
w = int(sys.argv[2]) if len(sys.argv) > 2 else 2560
h = int(sys.argv[3]) if len(sys.argv) > 3 else 1080

TOP, BOTTOM = (40, 22, 66), (8, 4, 15)
GLOWS = [(0.22, 0.30, 0.34, (124, 84, 214)), (0.78, 0.62, 0.28, (90, 52, 160))]  # x, y, radius (fraction of width), colour


def clamp(v):
    return 0 if v < 0 else 255 if v > 255 else int(v)


rows = []
for y in range(h):
    t = y / (h - 1)
    base = [TOP[i] + (BOTTOM[i] - TOP[i]) * t for i in range(3)]
    row = bytearray()
    for x in range(w):
        r, g, b = base
        for gx, gy, gr, col in GLOWS:
            d = math.hypot(x / w - gx, (y / h - gy) * h / w) / gr
            if d < 1.0:
                k = (1 - d) ** 2 * 0.55
                r += col[0] * k
                g += col[1] * k
                b += col[2] * k
        row += bytes((clamp(r), clamp(g), clamp(b)))
    rows.append(b"\x00" + bytes(row))
raw = b"".join(rows)


def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


import os
os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
with open(out, "wb") as f:
    f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))
print(f"wrote {out} ({w}x{h}, {os.path.getsize(out) // 1024} KB)")
