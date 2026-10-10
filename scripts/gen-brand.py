#!/usr/bin/env python3
"""Write the installer's logo and welcome image (a square maze spiral in the theme green).

Standard library only (hand-written PNG). Output: profile/airootfs/etc/calamares/branding/labyrinth/
"""
import os, struct, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "profile", "airootfs", "etc", "calamares", "branding", "labyrinth")
GREEN = (143, 217, 168, 255)
DARK = (14, 30, 22, 255)
CLEAR = (0, 0, 0, 0)

def png(path, w, h, px):
    raw = b"".join(b"\x00" + b"".join(bytes(px[y][x]) for x in range(w)) for y in range(h))
    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) \
        + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(data)

def spiral(n):
    """Cells of an n x n square spiral, as a set of (x, y) walls: a maze-like glyph."""
    cells = set()
    x = y = 0
    dx, dy = 1, 0
    seen = set()
    for _ in range(n * n):
        cells.add((x, y)); seen.add((x, y))
        nx, ny = x + dx, y + dy
        ahead2 = (x + 2 * dx, y + 2 * dy)
        if not (0 <= nx < n and 0 <= ny < n) or (nx, ny) in seen or ahead2 in seen:
            dx, dy = -dy, dx
            nx, ny = x + dx, y + dy
            if not (0 <= nx < n and 0 <= ny < n) or (nx, ny) in seen:
                break
        x, y = nx, ny
    return cells

def draw(w, h, scale, ox, oy, n=21, bg=CLEAR):
    px = [[bg] * w for _ in range(h)]
    for (cx, cy) in spiral(n):
        for yy in range(scale):
            for xx in range(scale):
                X, Y = ox + cx * scale + xx, oy + cy * scale + yy
                if 0 <= X < w and 0 <= Y < h:
                    px[Y][X] = GREEN
    return px

os.makedirs(OUT, exist_ok=True)
png(os.path.join(OUT, "logo.png"), 126, 126, draw(126, 126, 6, 0, 0))
w, h = 480, 126
png(os.path.join(OUT, "welcome.png"), w, h, draw(w, h, 6, (w - 126) // 2, 0))
print("wrote", OUT)
