#!/usr/bin/env python3
"""Run shell commands on the guest's serial console and print their output.

  serial.py [--sock PATH] [--timeout S] 'command'      run one command, print output, exit with its status
  serial.py '@/path/to/script.sh'                      run the contents of a file
  serial.py --wait-login                               wait until a root prompt appears

The guest needs a shell on its serial port (the live ISO has one: see
profile/airootfs/etc/systemd/system/serial-getty@ttyS0.service.d). Each command is wrapped
with unique markers so its output and exit status can be told apart from console noise.
"""
import argparse, os, re, socket, sys, time, uuid

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sock", default="/root/vmiso/ser.sock")
    ap.add_argument("--timeout", type=float, default=60)
    ap.add_argument("--wait-login", action="store_true")
    ap.add_argument("cmd", nargs="?")
    a = ap.parse_args()
    if a.cmd and a.cmd.startswith("@"):   # @FILE: read the command from a file (no shell quoting trouble)
        a.cmd = open(a.cmd[1:], encoding="utf-8").read()
    s = socket.socket(socket.AF_UNIX)
    s.connect(a.sock)
    s.settimeout(1.0)
    buf = b""
    def pump(t):
        nonlocal buf
        end = time.time() + t
        while time.time() < end:
            try:
                d = s.recv(65536)
                if not d:
                    return
                buf += d
            except socket.timeout:
                pass
    if a.wait_login:
        s.sendall(b"\n")
        end = time.time() + a.timeout
        while time.time() < end:
            pump(2)
            if re.search(rb"[#$] ?$", buf.rstrip()[-3:] + b" ") or b"@" in buf[-200:] and buf.rstrip().endswith((b"#", b"$")):
                print("login ok"); return
            s.sendall(b"\n")
        print("no prompt"); sys.exit(1)
    tag = uuid.uuid4().hex[:8]
    s.sendall(b"\n")
    pump(0.5)
    buf = b""
    s.sendall(f"echo S-{tag}; {{ {a.cmd}\n}} 2>&1; echo E-{tag}-$?\n".encode())
    end = time.time() + a.timeout
    m = None
    while time.time() < end:
        pump(1)
        m = re.search(rb"E-" + tag.encode() + rb"-(\d+)\r?\n", buf)
        if m:
            break
    if not m:
        print(buf.decode(errors="replace")[-2000:]); print("TIMEOUT"); sys.exit(124)
    out = buf[:m.start()].decode(errors="replace")
    out = out.split("S-" + tag, 2)[-1] if ("S-" + tag) in out else out
    print(out.replace("\r", "").strip("\n"))
    sys.exit(int(m.group(1)))

main()
