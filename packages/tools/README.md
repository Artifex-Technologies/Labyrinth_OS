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
| `labyrinth-selfcheck` | is the desktop in a sane state (after an upgrade) | on demand | done |
| `ipconfig` | Windows' `ipconfig` from `ip` and NetworkManager | on demand | done |
| `steam_playall` (was `steam_playall.py`) | Steam Play for all titles in Steam's config.vdf | `labyrinth-gaming setup` | done |
| `labyrinth-first-components` | the optional parts chosen in the installer, at the first login | the session's autostart | done |
| `labyrinth-agent` (the script is in /usr/local/sbin) | the gate: the root actions the engineering agent may take alone, each checked (sudo lets its account run this and nothing else as root) | on demand, through `claude-sudo` | done, not switched (the switch: branch `agent-gate-rust`) |

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
- `labyrinth-selfcheck`, `ipconfig`: a tool's output that is not UTF-8 has its bad bytes replaced (the scripts stopped); JSON from
  hyprctl, ip or Heimdall's config with NaN, Infinity or a lone surrogate counts as not JSON (Python read it); kernel module
  folders whose names are not UTF-8 have their bad bytes replaced.
- `steam_playall`: a path that is not UTF-8 has its bad bytes replaced.
- `labyrinth-selfcheck` and `steam_playall` match their scripts' regular expressions with code written out for them (no
  regular-expression engine: each program is a third of the size); the parity cases try them on lines written oddly.

- `labyrinth-agent`: Ctrl+C while a program it started is running: the script killed that program a quarter of a second later
  (pacman too, in the middle of a transaction) and stopped with a traceback; the port leaves the Ctrl+C to the program (it gets
  it from the terminal as before) and exits with its code. Its arguments are bytes (the script decoded them in the locale's
  encoding, which under sudo is UTF-8 too).

## Found while porting, fixed in both (2026-11-11)

- `labyrinth-scan-downloads`: the "Malware quarantined" question with Restore / Delete is asked by a child process started as
  `labyrinth-scan-downloads --prompt ID NAME SIGNATURE`, but `main` wanted five arguments after the program's name and got four,
  so the child ran an ordinary scan and the question was never shown. Now four (Python and Rust, with a test in each).
- `labyrinth-launch` started the app's slice and asked for the rules for block, ethernet and wifi but not for ask, so an app in
  ask mode could start unlimited until the rules were next loaded. Ask is included now (Python and Rust, with a test in each).

- `labyrinth-selfcheck`: a warning before a failure left the exit status at 2 (a warning) instead of 1; a failure is 1 now wherever
  it comes. And kernel versions were sorted as text, so with a leftover module folder of 7.9 while 7.10 runs it said a newer kernel
  was installed; they are sorted as versions now (numbers as numbers).
- `steam_playall`: the script read config.vdf with universal newlines, so a file with \r\n line ends was written back with \n
  everywhere (its own \r\n handling never saw one). It reads the line ends as they are now.
- `ipconfig`: a second gateway or DNS server was printed two columns to the right of the first one's value; it lines up under it
  now, as in Windows.

A test for each, in Python (tests/test_labyrinth_selfcheck.py, tests/test_steam_playall.py, tests/test_ipconfig.py) and in Rust.

## labyrinth-agent: holes found while porting, closed in both (2026-11-11)

The gate is what stands between the agent's account and root, so its port went further than the others (below: parity with a
check of every call against an allow-list of its own, random command lines, a fuzz target). These holes were in the script; each
is closed in the script and the port alike, with a test in each (tests/test_labyrinth_agent.py, src/agent/tests.rs).

1. **The journal could be wiped as root.** journalctl's `-n`/`--lines` take the next word as their value only when it is a count,
   but the gate passed any word after them as that value: `labyrinth-agent logs -n --vacuum-time=1s` ran
   `journalctl --no-pager -n --vacuum-time=1s` as root, which vacuums the journal, the record Heimdall reads. `--rotate`,
   `--flush`, `--setup-keys`, `--relinquish-var`, `--directory` and `--cursor-file=PATH` (a file written as root) got through the
   same way. Checked on journalctl itself (systemd 262): `journalctl -n --version` prints the version. Now the word after `-n` is
   its value only when it is a count (`5`, `+5`, `all`), as the word after `-b` already was only when it is a boot; any other word
   is checked on its own.
2. **deploy read any file as root and made it readable to all.** `deploy /etc/shadow /usr/share/yggdrasil/x` copied it with
   mode 0644; the same for my ssh keys (the agent's account cannot enter my home folder), Limbo's keys, sudo's recordings,
   Sarina's private state. The source is now opened by a child process that is the account sudo ran the gate for (SUDO_UID,
   SUDO_GID and its groups), never through a link at its end: what that account cannot read is refused.
3. **deploy could replace programs root runs.** `/usr/bin/yggdrasil-*` is a deploy place, but yggdrasil-spell-global.service runs
   `/usr/bin/yggdrasil-spell` as root and yggdrasil-gametime-catch-up.service `/usr/bin/yggdrasil-gametime`; with
   `service restart yggdrasil-spell-global` that was root for any program. Now a program named in a system unit or its drop-ins,
   a pacman hook, a udev rule, a polkit action or sudoers.d is refused ("root runs it"). And three places root uses are not deploy
   places any more: the graphics card unlock's support table (/usr/share/yggdrasil/gpu-unlock; docs/GPU_UNLOCK.md says never
   through claude-sudo), the optional apps' build recipes (/usr/share/labyrinth/pkgbuilds, built and installed as root) and the
   boot menu theme (/usr/share/labyrinth/grub-evodevo, whose install.sh runs as root).
4. **The agent could keep my window open, and stop its watcher.** `service stop labyrinth-agent-window-close.timer` (or
   `restart`, which starts the countdown again) was allowed, a "labyrinth-" unit: the window's units are refused now. And
   Heimdall, which watches the agent, could be stopped or disabled by it (`service disable-now heimdall`): stop, disable,
   disable-now and user-disable of heimdall units are mine now (start, restart, reload, enable stay the agent's).
5. **pkg-add installed remote-access tools as dependencies.** Only the names given were checked, and `pkg-add sshfs` brings
   openssh. What pacman would install (`pacman -S --print`) is checked now. The list also has the names of Heimdall's catalogue
   it lacked (headscale, netbird, frpc, frps, bore, tor, cloudflared-bin) and the remote-desktop and web-shell servers of the
   official repositories (gnome-remote-desktop, krfb, krdp, ttyd, cockpit, tinyssh).
6. **limbo doctor/status read any file as root.** Their manifest was not checked (`limbo status /etc/shadow`; Limbo's error names
   the first field of a JSON file it cannot use); it is held to the same folders as up and down now.
7. Smaller ones:
   - The patterns were matched with Python's `$`, which also matches before a final newline: `yggdrasil-x\n` was a unit, a
     package name and a deploy place. Every pattern is matched whole now.
   - `rollback dry-run N` took any Unicode digit (str.isdigit: ², ١٢); ASCII digits now (labyrinth-rollback refused the others).
   - deploy looked for links in the two folders above the file only, so a link higher up was followed (the cases write through
     /usr/share/yggdrasil/a -> /etc/yggdrasil); every folder on the way is now opened without following a link. A path with `.`
     or a trailing `/` is not clean.
   - A deploy that stopped half way left NAME.agent-tmp, and every later deploy to NAME stopped on it; it is removed on failure
     and replaced when found.
   - The caller's umask reached deploy's folders (sudo keeps the stricter one: 077 made them root-only); the gate's umask is 022.
   - A refusal whose arguments came to more than 128 KiB could not be written to the journal (logger could not be started: the
     script stopped with a traceback, nothing logged); the journal line is cut at 4000 bytes now.
   - The programs it starts got the environment sudo left it; they get a fixed one now: PATH=/usr/bin, HOME=/root, sudo's own
     variables (heimdalld names the window's actor from SUDO_USER) and a terminal type that names no file. Arch's sudoers sets
     secure_path and env_reset, so there the caller's PATH, LD_*, SYSTEMD_* or LIMBO_* (limbo reads LIMBO_GO_BINARY) did not
     get through; the gate no longer depends on that (in the cases, the script ran a pacman from the caller's PATH).

Left as they are, for me to decide:
- pkg-install stays a root path by design (a package of ours can hold anything; the name is read from the file before pacman
  installs it, so a file swapped in between is what gets installed).
- deploy to /etc/yggdrasil/*.json|conf writes the fan, pump, power-limit and tuning settings that root's helpers apply; Settings
  asks for my password (polkit, auth_admin_keep) to change them, deploy does not.
- /var/lib/limbo/build-agent does not exist yet; whoever can write there decides what `limbo up` starts as root.

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
| tests/test_labyrinth_selfcheck.py (3, and 2 for the fixes) | src/selfcheck.rs |
| tests/test_ipconfig.py (2) | src/ipconfig.rs |
| tests/test_steam_playall.py (7, and 1 for the fix) | src/playall.rs |
| tests/test_install_system.py's 2 of labyrinth-first-components | src/first_components.rs |
| tests/test_labyrinth_agent.py (6, and 13 for the holes) | src/agent/tests.rs (with 60,000 random command lines against src/agent/audit.rs), tests/agent.rs |

`parity/compare.py` runs each program and its script on the same made-up worlds: every tool they call is a stand-in that writes
down how it was called, every file is in a scratch folder (and, for moves across file systems, `target/parity-xdev`), `set` runs
as root of a user namespace of its own (`unshare -r`), the services talk to a scripted client or service. Exit codes, output,
the calls and the files afterwards must be the same, byte for byte, after quarantine ids and times are made alike. One case runs
the real `heimdalld scan-files` (ClamAV) on ClamAV's EICAR test string.

The programs that read fixed places of the system (`labyrinth-selfcheck`, `ipconfig`, `labyrinth-first-components`) run in a world
of their own: a user and mount namespace (`unshare -r -m -u`, no root needed) where the world's files and folders are seen at
/var/log, /usr/lib/modules, /etc/greetd, /etc/heimdall, /proc, /sys/class/net, /etc/resolv.conf ..., /usr/bin has the world's
additions and removals laid over it (overlayfs), and the host name is parity-host. `labyrinth-first-components` waits 15 s and up
to 120 s; `parity/fastclock.c` (built with cc, loaded with LD_PRELOAD) makes those waits pass at once on its own clocks.
`--show` prints what each script did.

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
| labyrinth-selfcheck | 74/74 (2026-11-11) |
| ipconfig | 60/60 (2026-11-11; the addresses are documentation ones) |
| steam_playall | 32/32 (2026-11-11; a made-up config.vdf, Steam never started) |
| labyrinth-first-components | 24/24 (2026-11-11; stand-ins for nmcli, notify-send, labyrinth-apps) |
| labyrinth-agent | 60/60 (396 command lines) and 400/400 random command lines (2026-11-11), every call within the cases' allow-list |

All 467 cases and the 400 random ones are the same. Run against the scripts as they were before the fixes, the cases that show the
fixes differ (and only those, with the cases built on them); for labyrinth-agent nearly every case differs, because every program
it starts now gets a clean environment, and the allow-list check finds each hole above in the old script.

labyrinth-agent's cases (parity/cases_agent.py) run as root of a user and mount namespace whose uids 1000-2999 are this account's
subordinate ids (/etc/subuid), so deploy's caller, uid 2001, is another account. /etc, /usr/bin, /usr/share, /usr/lib and
/usr/local are overlays: the stand-ins for pacman, systemctl, journalctl, logger, heimdalld, limbo, ufw and labyrinth-rollback are
where the gate looks for the real ones, and every place deploy writes to or the gate searches is a folder of the case's own (no
file of this computer shows through, none is changed). Each stand-in writes down its arguments and its whole environment. Beside
the comparison, each run is checked against an allow-list written a third time in the cases (it models journalctl's option
parsing, optional values included, and Go's flags for heimdalld): no stand-in called outside it, no environment but the clean one,
no file written outside deploy's places. LABYRINTH_AGENT_RANDOM sets the number of random command lines (default 400).

### The gate's fuzz target

`fuzz/` (cargo-fuzz's layout, not part of the package or of `cargo test`): fuzz_targets/agent_args.rs gives any command line
(words split at 0 bytes) to `decide` on a made-up system and panics when anything it would do as root is outside
src/agent/audit.rs's allow-list (written apart from the checks; it models journalctl's and heimdalld's option parsing). With
cargo-fuzz: `cargo fuzz run agent_args -- -dict=fuzz/agent_args.dict`. On stable Rust without cargo-fuzz:

    RUSTFLAGS="--cfg fuzzing -Cpasses=sancov-module -Cllvm-args=-sanitizer-coverage-level=4 \
      -Cllvm-args=-sanitizer-coverage-inline-8bit-counters -Cllvm-args=-sanitizer-coverage-pc-table \
      -Cllvm-args=-sanitizer-coverage-trace-compares" \
      cargo build --release --manifest-path fuzz/Cargo.toml --target x86_64-unknown-linux-gnu --target-dir target/fuzz
    mkdir -p target/fuzz/corpus
    target/fuzz/x86_64-unknown-linux-gnu/release/agent_args target/fuzz/corpus -dict=fuzz/agent_args.dict -max_len=512 -max_total_time=600

2026-11-11: 106 million command lines in ten minutes (coverage 1156, 1685 inputs kept), none outside the allow-list. With the old
`-n` handling put back, it found `logs --lines --l...` (journalctl would read the second word
as an option) within seconds.

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

They stay in `profile/airootfs/usr/local/bin` (labyrinth-agent in `profile/airootfs/usr/local/sbin`; after the switch, in
`packages/tools/python/` with their tests) for a rollback,
until these have run a while. The first seven were switched on 2026-11-11 (the package `labyrinth-tools` installs them in place of
the scripts); the last four with pkgrel 2 (branch `tools-rust-switch-2`): the image's autostart runs
/usr/bin/labyrinth-first-components and `labyrinth-gaming setup` runs /usr/bin/steam_playall.

Formatting: `cargo fmt` with rustfmt.toml (Heimdall's settings: 140 columns); `cargo clippy --all-targets -- -D warnings` is clean.
