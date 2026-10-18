#!/usr/bin/env bash
# Build the Labyrinth OS logo set from assets/logo/labyrinth-source.jpg (white mark on black, any size;
# it is scaled to 1024 px first).
#   scripts/gen-logo.sh
# Needs ImageMagick (magick) and potrace. Outputs:
#   assets/logo/labyrinth-{white,silver,grey,charcoal,black}.png   1024 px, transparent background
#   assets/logo/labyrinth-{white,silver,grey,charcoal,black}.svg   traced vector
#   profile/airootfs/usr/share/icons/hicolor/<size>/apps/labyrinthos.png   (white, for dark UIs)
#   profile/airootfs/usr/share/labyrinth/logo/                    the PNG and SVG set, shipped on the OS
#   profile/airootfs/etc/calamares/branding/labyrinth/{logo,welcome}.png   installer
# Tones: white for dark backgrounds, black for light ones, silver/grey/charcoal for in-between.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
src=$root/assets/logo/labyrinth-source.jpg
out=$root/assets/logo
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
declare -A tones=([white]="#ffffff" [silver]="#c7c7cc" [grey]="#8e8e93" [charcoal]="#3a3a3c" [black]="#000000")

# Brightness becomes opacity: white lines on black turn into opaque shapes on transparency.
magick "$src" -resize 1024x1024 -colorspace Gray -level 14%,86% "$tmp/alpha.png"
for n in "${!tones[@]}"; do
  magick -size 1024x1024 "xc:${tones[$n]}" "$tmp/alpha.png" -compose CopyOpacity -composite "$out/labyrinth-$n.png"
done

# Vector: trace the shapes once, then recolour.
magick "$src" -resize 1024x1024 -colorspace Gray -threshold 50% -negate "$tmp/trace.pbm"
potrace -s -o "$tmp/trace.svg" --turdsize 20 --alphamax 0.9 --opttolerance 0.4 "$tmp/trace.pbm"
for n in "${!tones[@]}"; do
  sed -E "s/fill=\"#000000\"/fill=\"${tones[$n]}\"/" "$tmp/trace.svg" > "$out/labyrinth-$n.svg"
done

# Installed copies. The icon is called "labyrinthos", with no dash: icon lookup shortens a name like
# "labyrinth-os" to "labyrinth", which another icon theme may name differently.
shipped=$root/profile/airootfs/usr/share/labyrinth/logo
mkdir -p "$shipped"
cp "$out"/labyrinth-*.png "$out"/labyrinth-*.svg "$shipped/"
for s in 512 256 128 64 48 32; do
  d=$root/profile/airootfs/usr/share/icons/hicolor/${s}x${s}/apps
  mkdir -p "$d"
  magick "$out/labyrinth-white.png" -resize ${s}x${s} "$d/labyrinthos.png"
done
b=$root/profile/airootfs/etc/calamares/branding/labyrinth
mkdir -p "$b"
magick "$out/labyrinth-white.png" -resize 78x78 -background none -gravity center -extent 126x126 "$b/logo.png"   # dark sidebar (room around it so it is not clipped)
magick "$out/labyrinth-white.png" -resize 190x190 -background none -gravity center -extent 480x190 "$b/welcome.png"   # dark page
echo "logo set written to $out and the profile"
