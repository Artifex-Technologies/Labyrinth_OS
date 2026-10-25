#!/usr/bin/env python3
import sys
# Makes the fastfetch logo from the emblem: scripts/logo-to-braille.py assets/logo/labyrinth-black.png 36 > profile/airootfs/usr/share/labyrinth/fastfetch/labyrinth.txt
# (braille dots, 2x4 per character; "$1" is the colour fastfetch fills in)
from PIL import Image, ImageFilter
src, cols = sys.argv[1], int(sys.argv[2])
im0 = Image.open(src).convert("RGBA"); bg = Image.new("RGBA", im0.size, (255,255,255,255)); im = Image.alpha_composite(bg, im0).convert("L")
bbox = Image.eval(im, lambda v: 255 - v).getbbox()   # crop to the emblem
im = im.crop(bbox)
w = cols * 2
h = int(im.height * w / im.width * 0.5 / 0.5)  # braille cells are ~1:2, two dots wide, four tall
h = h - h % 4
im = im.resize((w, h), Image.LANCZOS)
px = im.load()
dots = [(0,0,0x01),(0,1,0x02),(0,2,0x04),(1,0,0x08),(1,1,0x10),(1,2,0x20),(0,3,0x40),(1,3,0x80)]
for y in range(0, h, 4):
    line = ""
    for x in range(0, w, 2):
        b = 0
        for dx, dy, bit in dots:
            if px[x+dx, y+dy] < 128: b |= bit
        line += chr(0x2800 + b) if b else " "
    print("$1" + line.rstrip() if line.strip() else "")
