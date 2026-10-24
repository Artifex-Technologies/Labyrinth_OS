#!/usr/bin/env python3
"""Makes the pictures of Labyrinth's boot screen (Plymouth theme "labyrinth-rain"): falling columns of characters in Labyrinth
purple, Matrix-style, and the logo in the middle. Plymouth can only move pictures, so each column is drawn once here: a strip of
characters, faint at the top, bright at the bottom where its head is. The theme's script moves them down the screen.

    python3 scripts/gen-plymouth-rain.py [OUT_DIR]     (default: the theme in profile/airootfs)
"""
import os
import random
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont, ImageOps

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "profile", "airootfs", "usr", "share", "plymouth", "themes", "labyrinth-rain")
FONT = "/usr/share/fonts/noto/NotoSansMono-Regular.ttf"
LOGO = os.path.join(HERE, "..", "profile", "airootfs", "usr", "share", "labyrinth", "logo", "labyrinth-white.svg")
CELL_W, CELL_H, SIZE = 18, 24, 19      # one character's place in a column, and its size
STRIPS = 16                            # different columns to choose from
PURPLE = (124, 84, 214)                # Labyrinth's accent, #7c54d6
BRIGHT = (165, 110, 255)               # the brighter violet near each column's head
GLYPHS = "0123456789ABCDEFGHJKLMNPQRSTUVWXYZ<>=+*:.|ΞΣΛΠΦΨΩ"

random.seed(7)  # the same pictures every time the script runs
os.makedirs(OUT, exist_ok=True)
font = ImageFont.truetype(FONT, SIZE)


def glyph(ch, color):
    """One character, mirrored like the film's, on a transparent cell."""
    g = Image.new("RGBA", (CELL_W, CELL_H), (0, 0, 0, 0))
    d = ImageDraw.Draw(g)
    w = d.textlength(ch, font=font)
    d.text(((CELL_W - w) / 2, 1), ch, font=font, fill=color)
    return ImageOps.mirror(g) if random.random() < 0.6 else g


def strip(n):
    """A column of n characters: the head (bottom) almost white, then bright purple, fading out towards the top, with a soft glow
    around the bright part."""
    from PIL import ImageFilter
    img = Image.new("RGBA", (CELL_W, CELL_H * n), (0, 0, 0, 0))
    for i in range(n):
        k = i / (n - 1)  # 0 at the top, 1 at the head
        if i == n - 1:
            color = (240, 232, 255, 255)
        elif i >= n - 4:
            color = (190, 150, 255, 250)
        else:
            # from Labyrinth purple at the top to a brighter violet near the head
            mix = k ** 1.2
            rgb = tuple(int(PURPLE[j] + (BRIGHT[j] - PURPLE[j]) * mix) for j in range(3))
            color = rgb + (int(40 + 200 * k ** 1.4),)
        img.alpha_composite(glyph(random.choice(GLYPHS), color), (0, i * CELL_H))
    glow = img.filter(ImageFilter.GaussianBlur(4))
    glow.putalpha(glow.getchannel("A").point(lambda a: int(a * 0.8)))
    out = Image.new("RGBA", img.size, (0, 0, 0, 0))
    out.alpha_composite(glow)
    out.alpha_composite(img)
    return out


for s in range(STRIPS):
    strip(random.randint(10, 34)).save(os.path.join(OUT, f"strip-{s}.png"))

# the logo, with a soft dark halo so it reads over the rain
size = 200
png = subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), "--keep-aspect-ratio", LOGO], capture_output=True, check=True).stdout
logo = Image.open(__import__("io").BytesIO(png)).convert("RGBA")
pad = 70
out = Image.new("RGBA", (logo.width + 2 * pad, logo.height + 2 * pad), (0, 0, 0, 0))
halo = Image.new("L", out.size, 0)
ImageDraw.Draw(halo).ellipse((pad // 2, pad // 2, out.width - pad // 2, out.height - pad // 2), fill=215)
from PIL import ImageFilter
halo = halo.filter(ImageFilter.GaussianBlur(pad / 2.5))
out.alpha_composite(Image.merge("RGBA", (Image.new("L", out.size, 6), Image.new("L", out.size, 4), Image.new("L", out.size, 12), halo)))
out.alpha_composite(logo, (pad, pad))
out.save(os.path.join(OUT, "logo.png"))
print(f"wrote {STRIPS} columns and the logo to {os.path.normpath(OUT)}")

# a still for Settings (Appearance > Boot screen): one moment of the rain, with the logo
random.seed(3)
W, H = 640, 360
prev = Image.new("RGBA", (W, H), (5, 3, 10, 255))
strips = [Image.open(os.path.join(OUT, f"strip-{i}.png")) for i in range(STRIPS)]
for c in range(W // CELL_W + 1):
    s = random.choice(strips)
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    layer.paste(s, (c * CELL_W, random.randint(-s.height, H)))
    prev.alpha_composite(layer)
small = Image.open(os.path.join(OUT, "logo.png"))
small = small.resize((small.width * 2 // 3, small.height * 2 // 3))
prev.alpha_composite(small, ((W - small.width) // 2, (H - small.height) // 2))
prev.convert("RGB").save(os.path.join(OUT, "preview.png"))
