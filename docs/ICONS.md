# Icons and icon packs

**The Labyrinth icon pack** (`/usr/share/icons/Labyrinth`, plus your own additions in
`~/.local/share/icons/Labyrinth`) is the pack Labyrinth ships and uses by default: MacTahoe in dark
(it inherits from it), plus Labyrinth's own icons for apps (`apps/scalable`, added with
`labyrinth-icon`), and the Minos icon. It is a normal XDG icon theme, so it shows up in KDE's icon
settings and can be swapped for any other pack.

## What follows the pack, and what does not

- KDE and Qt apps: the pack chosen in KDE (`[Icons] Theme` in `kdeglobals`).
- The top bar, dock and Fylgja (the panel), and Minos: the chosen pack first, then **hicolor**, then
  the Labyrinth pack and what it stands on, so an icon the pack lacks is filled in and nothing is ever
  missing. (With the Labyrinth pack chosen the order is just Labyrinth, MacTahoe, hicolor, Breeze,
  Adwaita. Choosing MacTahoe, Breeze or Adwaita directly counts as another pack.) Minos reads the pack when it starts; the panel notices a change within a few seconds.
- Settings keeps its own look and does not follow the pack.

## The Minos icon

Packs will not have a Minos icon for a long while, so `labyrinth-minos-icon` decides, at login, when
the accent or style changes, and when the panel sees a new pack:

| Pack | Minos shows |
| --- | --- |
| Labyrinth | the logo on a rounded macOS-style tile, lines in the accent colour, dark or light to match the style |
| another pack with an icon called `minos` | the pack's own |
| another pack without one | **that pack's own file-manager icon** (`system-file-manager`, then `file-manager`, `org.kde.dolphin`), so it looks like part of the pack |
| a pack with no file-manager icon either | the bare logo (dark or light, accent-coloured lines) |

The last two are put in the user's own copy of `hicolor`
(`~/.local/share/icons/hicolor/scalable/apps/minos.svg`, or a 256 px PNG), which is why the order above
puts hicolor right after the chosen pack. Nothing is written into a pack's own folder.

The artwork is in `/usr/share/labyrinth/minos-icon/`: `dark.svg` and `light.svg` (the bare logos, which
are also the system's default Minos icon, `hicolor/scalable/apps/minos.svg`) and `tile-dark.svg` /
`tile-light.svg`. `@LINE@` marks the lines, which take the accent colour. To use the bare logo on the
Labyrinth pack too: `echo plain > ~/.config/labyrinth/minos-icon-shape`.

## Choosing and installing a pack (Settings)

**Settings > Appearance > Icons** lists every icon pack on the system and in `~/.local/share/icons`, with a few sample icons
each. "Use" applies it to the whole desktop (KDE, the panel and Minos follow). "Install an icon pack" takes a folder, an
archive (.tar.gz, .tar.xz, .tar.bz2, .zip) or an https link and puts the pack in your own icons folder, so no password is
needed; "Remove" deletes a pack you installed (never a system one).

## Adding another pack by hand

Install it as a normal icon theme and choose it in KDE. To give it a Minos icon of its own, add
`minos.svg` to its `apps/scalable` folder (or `apps/<size>/minos.png`).

## An icon pack of app logos (not macOS-looking)

`labyrinth-brand-pack [--shape glyph|rounded|circle|hexagon|squircle] [--style brand|dark|purple]` builds
an icon pack called "Labyrinth Brands" in `~/.local/share/icons` from the apps installed here, using
about 90 logos from [Simple Icons](https://github.com/simple-icons/simple-icons) (CC0; the logos stay
their owners' trademarks) in `/usr/share/labyrinth/brand-icons/`. Minos, Settings, Calculator and the trash (empty and full) get
Labyrinth's own glyphs in the same form, so nothing in the set looks out of place. The default,
`glyph` + `brand`, is the bare logo in its brand colour with no tile, the way the Simple Icons page
shows them. `rounded` is a softly rounded square, `circle` a round tile, `hexagon` echoes Labyrinth's mark,
and `squircle` is the macOS / iOS tile. Styles: `brand` (the brand colour: a tile with a white logo, or
the logo itself for `glyph`), `dark` (a dark tile with the logo in its colour), `purple` (Labyrinth purple
for every app). Choose the pack in KDE's icon settings; the panel and Minos follow. Icons the pack does
not have (folders, files, system icons) still come from the icon packs behind it.

The brand pack's own glyphs now also cover the trash (empty and full), folders (including the special
ones: Documents, Downloads, Music and so on), home, drives and a generic document, so Minos shows them in
the same style when the pack is chosen. Other file types (PDF, images, audio, archives) still come from
the icon packs behind it until they get glyphs too.

## Plan: the Artifex icon pack

Decided with the user (2026-10-13), not built yet:

- One icon pack for all of Artifex's systems (Labyrinth OS and Chymaera OS), named **Artifex**. It will
  replace the MacTahoe base (about 195 MB installed) and the Labyrinth per-app PNGs (27 MB), and Breeze
  will not be used as a base.
- A macOS-style set of our own, a blend of the MacTahoe icons and the user's icons folder, but with a
  matte finish, in three versions: **purple**, **dark** and **light**. The colours of the dark version
  (and its blacks and greys) are standardised, so the same shade is used throughout instead of the
  slightly different ones the sources have now.
- The app-logo pack above stays a second, non-macOS choice (`labyrinth-brand-pack`); the Artifex pack is
  what the logos and glyphs sit on.

### Standardizing the macOS-style icons we collect (`scripts/icons/standardize.py`)

The icons found online disagree with each other: the tile fills 80 to 83% of the canvas, corners and
rims differ, and the "dark" tiles use at least five different greys (22, 33, 37, 46 and 0 on a 0 to 255
scale) with different amounts of gloss. `standardize.py SRC OUT --sheet` puts every icon on one 1024
canvas at one tile size (82%) under one squircle mask, brings every neutral dark tile to one dark colour
(default `#1c1c22`, `--dark` changes it) and takes most of the gradient out so it reads matte, leaving the
logo alone. Coloured tiles keep their colour and get the same size and corners; objects that are not tiles
(a bin, a drive) keep their shape. Nothing in SRC is touched. `_before_after.png` in OUT shows each icon
before and after, in alternating rows. Run on the 43 icons in the user's folder: 30 dark, 8 coloured, 2
objects (the first pass; the dark colour and the matte strength are to be chosen).

### The trash (`scripts/icons/make_trash.py`)

The empty bin is the standardized white matte bin (the one beside Vivaldi in the before/after sheet);
the full bin is the same picture with crumpled paper filling the opening and heaped above the rim, two
facets with a hint of print (blue, orange, green) so it reads as paper at dock size. The lip of the bin
covers the lower part of the paper, so it sits in the bin. Same size and position for both, so the dock
does not jump when it fills. `make_trash.py BIN.png OUT` writes `user-trash.png` and
`user-trash-full.png`; both are in `Labyrinth/apps/scalable`, which the panel and Minos read first. The
crumpled look is deliberately simple (facets, not a photo) to match an icon pack; more detail is a matter
of more facets.
