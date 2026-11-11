# Labyrinth's own programs in Rust (labyrinth-tools)

Rust ports of Labyrinth OS's Python programs in `profile/airootfs/usr/local/bin`, with the same interface: arguments, output,
files, environment variables and exit codes. Started 2026-11-10 for lightness (no Python interpreter in what runs all the time,
while games and a local model share the computer).

| Program | For | Runs | Port |
| --- | --- | --- | --- |
| `labyrinth-scan-downloads` | the malware scan of new files (Heimdall's scanner, ClamAV) | always (user service `labyrinth-scan-watch`), every 10 min, weekly | done |
| `labyrinth-quarantine` | its quarantine; Settings > Security | when a file is caught, from Settings | done |
| `labyrinth-appnet-ask` | the ask mode of Settings > Apps > Network: the questions | always (root service `labyrinth-appnet-ask`) | done |
| `labyrinth-appnet-ask-agent` | the ask mode's notifications | always (user service `labyrinth-appnet-ask-agent`) | done |
| `labyrinth-appnet` | per-app network rules (nftables) | at boot and login, from Settings (pkexec) | done |
| `labyrinth-launch` | starts apps in their own scope and slice | every app started from the dock, start menu, Settings | done |
| `labyrinth-apps` | Labyrinth's optional software (Agora's component installer, the first login) | on demand | done |
| `labyrinth-selfcheck`, `ipconfig`, `steam_playall.py`, `labyrinth-first-components` | | on demand | still Python (next) |

Each module (`src/*.rs`) says what it is and where it is not 1:1. Where a script stopped with a traceback (exit code 1), its port
stops too, with exit code 1 and one line on standard error. `src/pycompat` is a copy of Yggdrasil's `packages/pycompat` (the few
Python behaviours the ports need: json.dumps' bytes, str.strip(), splitlines() ...); `src/py.rs` is how the scripts met the outside
(os.path, shutil.move and rmtree as Python 3.14 does them, subprocess, time.strftime, shlex); `src/pyre.rs` gives the regex crate
Python's own \w, \d and \s.

## Not 1:1

- `labyrinth-scan-downloads --watch` sleeps until the kernel reports a new file, a scan is due or the 5-minute look for new places
  comes (the script woke every 2 s): a scan starts 4 s after the last new file, where the script started it 4 to 6 s after.
- Names that are not UTF-8: the watcher watches such folders as they are (the script stopped with a traceback at the first one and
  systemd restarted it, over and over); in the scanner's report such a name has its bad bytes replaced, as for any text a tool
  printed that a script could not decode (where the script stopped). `labyrinth-quarantine` stores and reads paths as UTF-8 (the
  script kept undecodable bytes with Python's surrogate escapes).
- Python's UnicodeDecodeError message gives a position inside the 8 KB piece it was decoding; here it is the position in the file
  (the same for files under 8 KB).
- `labyrinth-appnet-ask` served everything from one thread (asyncio); here each connection has a thread and one lock keeps the
  questions and answers in the script's order. A subscriber that stops reading for 5 s is dropped (the script would have stopped
  asking everyone until it read again). Where a connection's handler stopped on an error, the connection is closed as the script
  closed it; the message on standard error is one line (asyncio printed a traceback). A request line in UTF-16 or UTF-32, or with
  NaN or Infinity in it, is skipped (Python's json read those). A socket path longer than the kernel allows is refused with
  another message.
- `labyrinth-appnet-ask-agent` asks the C library for an address's host name with getnameinfo() (the script used gethostbyaddr(),
  which also resolved a destination that was a name rather than an address); a line from the service that is not UTF-8 is skipped
  (it stopped the script).
- `labyrinth-apps` writes each "==>" line as its step starts (the script's output was block-buffered when not a terminal, so in
  Agora or a log those lines came after the steps' own output); for a components.json that is not JSON the parser's own words after
  "cannot read FILE: " differ; component ids that are not text are refused (exit code 1).
- JSON files with numbers beyond 64 bits keep them as floating point (Python kept the exact integer).

## Found while porting, fixed in both (2026-11-11)

- `labyrinth-scan-downloads`: the "Malware quarantined" question with Restore / Delete is asked by a child process started as
  `labyrinth-scan-downloads --prompt ID NAME SIGNATURE`, but `main` wanted five arguments after the program's name and got four,
  so the child ran an ordinary scan and the question was never shown. Now four (Python and Rust, with a test in each).
- `labyrinth-launch` started the app's slice and asked for the rules for block, ethernet and wifi but not for ask, so an app in
  ask mode could start unlimited until the rules were next loaded. Ask is included now (Python and Rust, with a test in each).

## Tests and parity

`cargo test`: every test of the Python scripts carried over, and more:

| Python test | Rust |
| --- | --- |
| tests/test_scan_downloads.py (5) | tests/scan_downloads.rs |
| tests/test_quarantine.py (11) | tests/quarantine.rs |
| tests/test_labyrinth_appnet.py (6) | src/appnet/tests.rs |
| tests/test_labyrinth_appnet_ask.py (15: rules 4, parse 1, engine 8, agent 1, service 1) | src/appnet/tests.rs, src/appnet_ask/tests.rs, src/ask_agent.rs |
| tests/test_labyrinth_launch.py (5) | src/launch/tests.rs |
| tests/test_labyrinth_apps.py (16 of 22; the 6 Generator tests are of scripts/gen-packages.py, which stays Python) | src/apps/tests.rs, tests/apps.rs |

`parity/compare.py` runs each program and its script on the same made-up worlds: every tool they call is a stand-in that writes
down how it was called, every file is in a scratch folder (and, for moves across file systems, `target/parity-xdev`), `set` runs
as root of a user namespace of its own (`unshare -r`), the services talk to a scripted client or service. Exit codes, output,
the calls and the files afterwards must be the same, byte for byte, after quarantine ids and times are made alike. One case runs
the real `heimdalld scan-files` (ClamAV) on ClamAV's EICAR test string.

    cargo build --release && python3 parity/compare.py [--only labyrinth-quarantine] [--case TEXT] [-v]

Results (2026-11-10):

| Program | Cases the same |
| --- | --- |
| labyrinth-scan-downloads | 63/63 |
| labyrinth-quarantine | 40/40 |
| labyrinth-appnet | 41/41 |
| labyrinth-appnet-ask | 4/4 (one is a 47-line conversation: questions, answers, lists, forgets, the cooldown, bad requests) |
| labyrinth-appnet-ask-agent | 3/3 |
| labyrinth-launch | 34/34 |
| labyrinth-apps | 32/32 (the builds run in a copy, through stand-ins for pacman, yay, makepkg, sudo) |

## Memory of what runs all the time (the same scratch world for both, after some work, idle)

| | RSS | PSS | private |
| --- | --- | --- | --- |
| `labyrinth-scan-downloads --watch`, Python | 14.4 MB | 9.7 MB | 6.8 MB (wakes every 2 s) |
| the same, Rust | 3.0 MB | 0.8 MB | 0.3 MB (no wake-ups while idle) |
| `labyrinth-appnet-ask`, Python | 22.2 MB | 14.2 MB | 12.0 MB |
| the same, Rust | 4.4 MB | 2.2 MB | 0.8 MB |
| `labyrinth-appnet-ask-agent`, Python | 15.4 MB | 8.6 MB | 6.7 MB |
| the same, Rust | 3.9 MB | 0.8 MB | 0.3 MB |

## The Python scripts

They stay in `profile/airootfs/usr/local/bin` (after the switch, in `packages/tools/python/` with their tests) for a rollback,
until these have run a while. The switch (the package `labyrinth-tools` installing these in place of the scripts) is on the branch
`tools-rust-switch`.
