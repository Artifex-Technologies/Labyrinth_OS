# Labyrinth's own programs in Rust (labyrinth-tools)

Rust ports of Labyrinth OS's Python programs in `profile/airootfs/usr/local/bin`, with the same interface: arguments, output,
files, environment variables and exit codes. Started 2026-11-10 for lightness (no Python interpreter in what runs all the time,
while games and a local model share the computer).

| Program | For | Runs | Port |
| --- | --- | --- | --- |
| `labyrinth-scan-downloads` | the malware scan of new files (Heimdall's scanner, ClamAV) | always (user service `labyrinth-scan-watch`), every 10 min, weekly | done |
| `labyrinth-quarantine` | its quarantine; Settings > Security | when a file is caught, from Settings | done |

Each module (`src/*.rs`) says what it is and where it is not 1:1. Where a script stopped with a traceback (exit code 1), its port
stops too, with exit code 1 and one line on standard error. `src/pycompat` is a copy of Yggdrasil's `packages/pycompat` (the few
Python behaviours the ports need: json.dumps' bytes, str.strip(), splitlines() ...); `src/py.rs` is how the scripts met the outside
(os.path, shutil.move and rmtree as Python 3.14 does them, subprocess, time.strftime, shlex).

## Not 1:1

- `labyrinth-scan-downloads --watch` sleeps until the kernel reports a new file, a scan is due or the 5-minute look for new places
  comes (the script woke every 2 s): a scan starts 4 s after the last new file, where the script started it 4 to 6 s after.
- Names that are not UTF-8: the watcher watches such folders as they are (the script stopped with a traceback at the first one and
  systemd restarted it, over and over); in the scanner's report such a name has its bad bytes replaced, as for any text a tool
  printed that a script could not decode (where the script stopped). `labyrinth-quarantine` stores and reads paths as UTF-8 (the
  script kept undecodable bytes with Python's surrogate escapes).
- Python's UnicodeDecodeError message gives a position inside the 8 KB piece it was decoding; here it is the position in the file
  (the same for files under 8 KB).

## Found while porting (kept as the script does it, to fix in both)

- `labyrinth-scan-downloads`: the "Malware quarantined" question with Restore / Delete is asked by a child process started as
  `labyrinth-scan-downloads --prompt ID NAME SIGNATURE`, but `main` wants five arguments after the program's name (`len(argv) >= 5`)
  and gets four, so the child runs an ordinary scan instead and the question is never shown (the file stays in quarantine; Settings
  > Security can restore it). Only the tests' LABYRINTH_SCAN_SYNC path asked. The fix is `>= 4` in both.

## Tests and parity

`cargo test`: every test of the Python scripts carried over (tests/test_scan_downloads.py 5 and tests/test_quarantine.py 11, as
`tests/scan_downloads.rs` and `tests/quarantine.rs`, running the programs as the Python tests ran the scripts), and more.

`parity/compare.py` runs each program and its script on the same made-up worlds: every tool they call is a stand-in that writes
down how it was called, every file is in a scratch folder (and, for moves across file systems, `target/parity-xdev`). Exit
codes, output, the calls and the files afterwards must be the same, byte for byte, after quarantine ids and the local time are
made alike. One case runs the real `heimdalld scan-files` (ClamAV) on ClamAV's EICAR test string.

    cargo build --release && python3 parity/compare.py [--only labyrinth-quarantine] [-v]

Results (2026-11-10): labyrinth-scan-downloads 63/63 cases the same, labyrinth-quarantine 40/40.

## Memory (the watcher, the same scratch home, 5 folders, one new file, then 20 s idle)

| | RSS | PSS | private | wake-ups idle |
| --- | --- | --- | --- | --- |
| Python (`labyrinth-scan-downloads --watch`) | 14.4 MB | 9.7 MB | 6.8 MB | every 2 s |
| Rust | 3.0 MB | 0.8 MB | 0.3 MB | none |

## The Python scripts

They stay in `profile/airootfs/usr/local/bin` (after the switch, in `packages/tools/python/` with their tests) for a rollback,
until these have run a while. The switch (the package `labyrinth-tools` installing these in place of the scripts) is on the branch
`tools-rust-switch`.
