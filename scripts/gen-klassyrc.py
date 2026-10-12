"""Build profile/airootfs/usr/share/labyrinth/klassy/klassyrc: Klassy's Opal Fruits preset plus Labyrinth's changes,
grouped as Klassy's kcfg says. Run against the Klassy source of the packaged version:

  python3 scripts/gen-klassyrc.py SRC/libbreezecommon/breezesettingsdata.kcfg SRC/kdecoration/config/presets/Opal_Fruits.klpw profile/airootfs/usr/share/labyrinth/klassy/klassyrc
"""
import re
import sys
import xml.etree.ElementTree as ET

kcfg, preset, out = sys.argv[1:4]
OVERRIDES = {
    # macOS traffic lights: small circles, always coloured, the symbol only while hovering.
    "ButtonShape": "ShapeSmallCircle",
    "ButtonIconStyle": "StyleTraditional",
    "ButtonBackgroundColorsActive": "AccentTrafficLights",
    "ButtonBackgroundColorsInactive": "AccentTrafficLights",
    "ButtonBackgroundOpacityActive": "100",
    "ButtonBackgroundOpacityInactive": "45",
    "ShowBackgroundNormallyActive": "true",
    "ShowBackgroundNormallyInactive": "true",
    "ShowCloseBackgroundNormallyActive": "true",
    "ShowCloseBackgroundNormallyInactive": "true",
    "ShowIconNormallyActive": "false",
    "ShowIconNormallyInactive": "false",
    "ShowCloseIconNormallyActive": "false",
    "ShowCloseIconNormallyInactive": "false",
    "ShowIconOnHoverActive": "true",
    "ShowCloseIconOnHoverActive": "true",
    "ButtonIconColorsActive": "TitleBarText",
    "ButtonIconColorsInactive": "TitleBarText",
    "CloseButtonIconColorActive": "AsSelected",
    "CloseButtonIconColorInactive": "AsSelected",
    "IconSize": "IconSmallMedium",
    # One header: the title bar takes the app's own colour (its toolbar), no line between them.
    "MatchTitleBarToApplicationColor": "true",
    "DrawTitleBarSeparator": "false",
    "BoldTitle": "false",
    # No border, rounded corners all round, invisible resize edges come with BorderSize=None.
    "RoundAllCornersWhenNoBorders": "true",
    "DrawBorderOnMaximizedWindows": "false",
    # The exact macOS colours (Klassy's traffic lights follow the colour scheme's darker
    # neutral/positive/negative colours otherwise), and room after the close button.
    "ButtonOverrideColorsActiveMinimize": '{"BackgroundNormal":[254,188,46],"BackgroundHover":[254,188,46],"BackgroundPress":[222,160,30]}',
    "ButtonOverrideColorsActiveMaximize": '{"BackgroundNormal":[40,200,64],"BackgroundHover":[40,200,64],"BackgroundPress":[30,165,50]}',
    "ButtonOverrideColorsActiveClose": '{"BackgroundNormal":[255,95,87],"BackgroundHover":[255,95,87],"BackgroundPress":[220,70,62]}',
    "TitleBarLeftMargin": "8",
    "TitleBarRightMargin": "8",
    "ButtonSpacingRight": "8",
    # A soft shadow like macOS's; the preset's full-strength one read as a dark band under windows.
    "ShadowSizeActive": "ShadowLarge",
    "ShadowStrengthActive": "90",
    "ShadowSizeInactive": "ShadowMedium",
    "ShadowStrengthInactive": "60",
}

ns = {"k": "http://www.kde.org/standards/kcfg/1.0"}
root = ET.parse(kcfg).getroot()
key_group = {}
for group in root.iter():
    if not group.tag.endswith("group"):
        continue
    gname = group.get("name")
    for entry in group:
        if not entry.tag.endswith("entry"):
            continue
        name = entry.get("name")
        m = re.match(r"^(.*)\$\((\w+)\)$", name)
        if m:
            base = m.group(1)
            choices = []
            for p in entry:
                if p.tag.endswith("parameter"):
                    for v in p.iter():
                        if v.tag.endswith("choice"):
                            choices.append(v.get("name"))
                        elif v.tag.endswith("value") and v.text:
                            choices.append(v.text.strip())
            for c in choices or ["Active", "Inactive"]:
                key_group[base + c] = gname
        else:
            key_group[name] = gname

values = {}
for line in open(preset, encoding="utf-8"):
    line = line.strip()
    if "=" in line and not line.startswith("["):
        k, v = line.split("=", 1)
        values[k] = v
values.update(OVERRIDES)

groups = {}
skipped = []
for k, v in values.items():
    g = key_group.get(k)
    if g is None:
        skipped.append(k)
        continue
    groups.setdefault(g, {})[k] = v

with open(out, "w", encoding="utf-8", newline="\n") as f:
    f.write("# Labyrinth's Klassy window decoration settings (generated from Klassy 6.7.3's Opal Fruits\n"
            "# preset plus Labyrinth's changes; see packages/pkgbuilds/klassy). Copied to\n"
            "# ~/.config/klassy/klassyrc by labyrinth-plasma-setup on first login.\n")
    for g in sorted(groups):
        f.write(f"\n[{g}]\n")
        for k in sorted(groups[g]):
            f.write(f"{k}={groups[g][k]}\n")
missing = [k for k in OVERRIDES if k not in key_group]
print(f"wrote {out}: {sum(len(x) for x in groups.values())} keys in {len(groups)} groups; "
      f"not Klassy keys (KWin's, handled elsewhere): {', '.join(skipped) or 'none'}; unknown overrides: {missing or 'none'}")
