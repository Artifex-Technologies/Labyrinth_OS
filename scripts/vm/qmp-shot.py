#!/usr/bin/env python3
"""Screenshot the VM's screen over QMP and save a PNG (no window, no host focus involved).

  qmp-shot.py OUT.png                  take a screenshot
  qmp-shot.py OUT.png --key ctrl-t     send keys first (QEMU key names, e.g. meta_l-ret), wait, then shoot
  qmp-shot.py OUT.png --type "text"    type text first
  qmp-shot.py OUT.png --click X Y      left-click at screen pixel X,Y (needs the usb-tablet device), wait, then shoot

QEMU's screendump writes PPM; this converts it to PNG with the standard library only.
"""
import json
import os
import socket
import struct
import sys
import time
import zlib

SOCK = os.environ.get("QMP_SOCK", "/root/vm/qmp.sock")


def qmp(sock_file, cmd, **args):
    sock_file.write(json.dumps({"execute": cmd, "arguments": args}) + "\n")
    sock_file.flush()
    while True:
        line = sock_file.readline()
        if not line:
            raise RuntimeError("QMP closed")
        msg = json.loads(line)
        if "return" in msg or "error" in msg:
            if "error" in msg:
                raise RuntimeError(msg["error"])
            return msg["return"]


def connect():
    s = socket.socket(socket.AF_UNIX)
    s.connect(SOCK)
    f = s.makefile("rw")
    f.readline()  # greeting
    qmp(f, "qmp_capabilities")
    return f


KEYMAP = {" ": "spc", "\n": "ret", "/": "slash", ".": "dot", "-": "minus", "_": "shift-minus", ",": "comma", ":": "shift-semicolon",
          "+": "shift-equal", "=": "equal", "*": "shift-8", "(": "shift-9", ")": "shift-0", "%": "shift-5", "^": "shift-6", "!": "shift-1"}


def send_keys(f, keys):
    parts = keys.split("-")
    qmp(f, "send-key", keys=[{"type": "qcode", "data": p} for p in parts])


def type_text(f, text):
    for ch in text:
        key = KEYMAP.get(ch) or (ch.lower() if ch.isalnum() else None)
        if key is None:
            continue
        if ch.isupper():
            key = "shift-" + key
        send_keys(f, key)
        time.sleep(0.05)


def ppm_to_png(ppm, png):
    with open(ppm, "rb") as fh:
        data = fh.read()
    parts = data.split(None, 4)
    w, h = int(parts[1]), int(parts[2])
    pixels = data[len(data) - w * h * 3:]
    raw = b"".join(b"\x00" + pixels[y * w * 3:(y + 1) * w * 3] for y in range(h))

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    with open(png, "wb") as out:
        out.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                  + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))
    return w, h


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    out = argv[0]
    f = connect()
    if "--key" in argv:
        send_keys(f, argv[argv.index("--key") + 1])
        time.sleep(1.5)
    if "--type" in argv:
        type_text(f, argv[argv.index("--type") + 1])
        time.sleep(1.0)
    if "--click" in argv:
        i = argv.index("--click")
        x, y = int(argv[i + 1]), int(argv[i + 2])
        # Absolute pointer axes run 0..32767 across the guest screen; find its size from a first dump.
        tmp = out + ".size.ppm"
        qmp(f, "screendump", filename=tmp)
        time.sleep(0.8)
        with open(tmp, "rb") as fh:
            head = fh.read(32).split(None, 3)
        os.remove(tmp)
        w, h = int(head[1]), int(head[2])
        ax, ay = round(x * 32767 / (w - 1)), round(y * 32767 / (h - 1))
        qmp(f, "input-send-event", events=[{"type": "abs", "data": {"axis": "x", "value": ax}}, {"type": "abs", "data": {"axis": "y", "value": ay}}])
        time.sleep(0.4)
        qmp(f, "input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
        time.sleep(0.1)
        qmp(f, "input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
        time.sleep(2.0)
    ppm = out + ".ppm"
    qmp(f, "screendump", filename=ppm)
    for _ in range(40):
        if os.path.exists(ppm) and os.path.getsize(ppm) > 100:
            time.sleep(0.2)
            break
        time.sleep(0.25)
    w, h = ppm_to_png(ppm, out)
    os.remove(ppm)
    print(f"saved {out} ({w}x{h})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
