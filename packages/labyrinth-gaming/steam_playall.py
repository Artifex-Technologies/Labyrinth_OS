#!/usr/bin/env python3
"""Turn on "Enable Steam Play for all other titles" in Steam's config.vdf.

Steam stores that setting as a default compatibility tool: CompatToolMapping
entry "0". This inserts one if missing and never changes an existing choice.
Text-level edit (no full VDF parser), so it is deliberately conservative: if
the file does not look the way we expect it refuses and changes nothing.

Usage: steam_playall.py CONFIG_VDF [TOOL_NAME]   (Steam must NOT be running)
Exit: 0 changed or already set, 1 refused / nothing safe to do, 2 usage.
"""
import re
import shutil
import sys
import time

DEFAULT_TOOL = "proton_experimental"


def apply(text: str, tool: str = DEFAULT_TOOL):
    """Return (new_text, status). status: 'already', 'added', or 'refused:<why>'."""
    m = re.search(r'^([ \t]*)"CompatToolMapping"[ \t]*\r?\n[ \t]*\{[ \t]*\r?\n', text, re.M)
    if not m:
        return text, "refused:no CompatToolMapping block (run Steam once, then retry)"
    indent = m.group(1)
    # Find the matching closing brace of the block.
    depth, i = 1, m.end()
    while i < len(text) and depth:
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
        i += 1
    if depth:
        return text, "refused:unbalanced braces"
    block = text[m.end():i - 1]
    # A top-level key "0" inside the block (not nested deeper).
    d = 0
    for line in block.splitlines():
        s = line.strip()
        if s == "{":
            d += 1
        elif s == "}":
            d -= 1
        elif d == 0 and re.match(r'^"0"\s*$', s):
            return text, "already"
    nl = "\r\n" if "\r\n" in text else "\n"
    t = indent + "\t"
    entry = (
        f'{t}"0"{nl}{t}{{{nl}'
        f'{t}\t"name"\t\t"{tool}"{nl}'
        f'{t}\t"config"\t\t""{nl}'
        f'{t}\t"priority"\t\t"75"{nl}'
        f"{t}}}{nl}"
    )
    return text[:m.end()] + entry + text[m.end():], "added"


def main(argv):
    if len(argv) not in (2, 3):
        print(__doc__)
        return 2
    path = argv[1]
    tool = argv[2] if len(argv) == 3 else DEFAULT_TOOL
    with open(path, encoding="utf-8") as f:
        text = f.read()
    new, status = apply(text, tool)
    if status.startswith("refused"):
        print(status)
        return 1
    if status == "added":
        shutil.copy2(path, f"{path}.labyrinth-{int(time.time())}.bak")
        with open(path, "w", encoding="utf-8", newline="") as f:
            f.write(new)
    print(status)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
