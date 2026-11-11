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
| `labyrinth-agent` (was in /usr/local/sbin) | the gate: the root actions the engineering agent may take alone, each checked (sudo lets its account run this and nothing else as root) | on demand, through `claude-sudo` | done |

Each module (`src/*.rs`) says what it is and where it is not 1:1. Where a script stopped with a traceback (exit code 1), its port
stops too, with exit code 1 and one line on standard error. `src/pycompat` is a copy of Yggdrasil's `packages/pycompat` (the few
Python behaviours the ports need: json.dumps' bytes, str.strip(), splitlines() ...); `src/py.rs` is how the scripts met the outside
(os.path, shutil.move and rmtree as Python 3.14 does them, subprocess, time.strftime, shlex); `src/pyre.rs` gives the regex crate
Python's own \w, \d and \s.

## The shell scripts (started 2026-11-11)

The bash and sh scripts of `profile/airootfs/usr/local/{bin,sbin}` get Rust ports too, for security and stability more than
speed: no quoting or word-splitting bugs, programs started by argv rather than command strings, strict checks of what comes in,
clear errors. Each is a bin with the script's name, arguments, output, files, calls and exit codes (`src/sh.rs` holds what they
share); `parity/cases_root.py`, `cases_root2.py` and `cases_user.py` run each against its script (below).

The switch (branch `scripts-rust-switch`, merged with `agent-gate-rust` into one switch): labyrinth-tools 0.1.0-4 installs the
20 programs in /usr/bin, and the agent's gate with them; the scripts leave
the image and are kept in `packages/tools/shell/` for a rollback (with the switched paths, so they stay the ports' specification,
and the parity runs against them); `cls` is a link to /usr/bin/clear. The paths to them are /usr/bin now: the installer's
integration file (clean-up, finish, boot menu theme), the security and Sarina integration files (sarina-git 0.1.0-2 / -3),
the notifications' D-Bus activation, labyrinth-sarina-reflect.service, the gate's rollback (the Python gate and the Rust one),
labyrinth-agent-setup's checks of the gate and the window (both /usr/bin; it removes the Python gate from /usr/local/sbin, script
and port alike). The window's rules name /usr/bin/labyrinth-* beside /usr/local/bin/labyrinth-* (the tools moved; the window
already allows any pacman and systemd-run, so it gives nothing more).

| Script | Port | Parity |
| --- | --- | --- |
| labyrinth-rollback | src/rollback.rs | 30/30 |
| labyrinth-install-bootfinish | src/bootfinish.rs | 32/32 |
| labyrinth-install-cleanup | src/install_cleanup.rs | 18/18 (and tests/test_install_cleanup.sh's 21 with LABYRINTH_CLEANUP) |
| labyrinth-agent-setup | src/agent_setup.rs | 20/20 |
| labyrinth-agent-window (sbin) | src/agent_window.rs | 11/11 |
| labyrinth-ecosystem-setup | src/ecosystem.rs | 14/14 |
| labyrinth-hardware-setup | src/hardware.rs | 9/9 |
| labyrinth-grub-theme | src/grub_theme.rs | 15/15 |
| labyrinth-security-setting | src/security_setting.rs | 10/10 |
| labyrinth-gaming | src/gaming.rs | 14/14 |
| labyrinth-sarina | src/sarina.rs | 12/12 |
| labyrinth-sarina-setting | src/sarina_setting.rs | 12/12 |
| labyrinth-rea, labyrinth-rea-setup | src/rea.rs | 10/10, 8/8 |
| labyrinth-claude-code, labyrinth-install, labyrinth-notify-activate | src/small.rs | 5/5, 4/4, 4/4 |
| labyrinth-sarina-reflect, -warm, -voice-setup | src/sarina_small.rs | 14/14, 7/7, 7/7 |
| cls | none: on the switch it becomes a link to /usr/bin/clear (it was `exec clear "$@"`; clear does the same under any name, and no shell runs) | |
| labyrinth-sarina-ask, labyrinth-sarina-speak | not ported: they wait for my voice test | |

All 20 ports are done; 256 cases, all the same. Run against the scripts as they were before the fixes below (`--shell` with
the scripts of 921bf80), the cases that show a fix differ, and only those and the ones built on them (36 of 256: in
ecosystem-setup and claude-code nearly every case, as `ollama list` is read once and the installer is downloaded first; in
grub-theme every install, as the made-up grub.cfg has a class with `../` in it, which the old script copied an icon to,
outside the icons' folder).

How the cases run (parity/cases_root.py): a user and mount namespace (and a network namespace: nothing reaches the network)
whose root folder is the case's own (chroot): its /etc, /var, /run, /boot, /home, /tmp and /dev (null, zero, urandom and two
"partitions", which are this computer's first block device bound in so that `[[ -b ]]` holds; nothing opens them), this
computer's /usr with overlays (nothing written reaches it) and the folders a case owns bound over it. PATH is the case's
stand-ins, then a folder of links to the few real tools the scripts use (bash, sed, grep, cp ...), so a program a case does not
give is not there. Each stand-in writes down its arguments and answers as the case says (a mount binds a folder of the case's
as the partition's top level, btrfs's snapshot copies one). Programs that look at running processes (gaming's Steam,
notify-activate's panel) run in a process namespace of their own with lingering stand-ins; the waits of labyrinth-sarina and
notify-activate pass at once (parity/fastclock.c). Uids 1000-2999 are this account's subordinate ids, so the files a case gives
another owner (sarina's, the agent's) are owned so.

Found while porting, fixed in the scripts and the ports alike:

- `labyrinth-rollback N` and `N --dry-run` stopped silently with exit code 2 when the snapshot had no info.xml (an interrupted
  snapper run leaves such a snapshot): sed's failure reached `set -e` through `pipefail`.
- `labyrinth-rollback undo` did not check that its `@rolled-back-DATE` name was free: `mv @ @rolled-back-DATE` would have moved
  the system *into* a folder of that name, snapshots and all. The port renames with RENAME_NOREPLACE everywhere (nothing is ever
  replaced or entered), and the script checks the name now.
- `labyrinth-rollback list --device` (no value) stopped silently with exit code 1; it is a usage error now.
- `labyrinth-install-bootfinish`: a setting added to the end of /etc/default/grub or /etc/fstab (`echo X >> file`) was glued
  to the last line when the file did not end with a newline (in /etc/fstab, that breaks the last mount); the file system's
  UUID went unchecked into the sed expression that writes the kernel command line; "configured zram swap" was said even when
  its file could not be written.
- `labyrinth-agent-window`: when systemd-run could not set the timer that closes the window (a leftover unit of the same name
  does that: open, close, open again quickly), the rules stayed with nothing to remove them, for good: a sudoers file outlives
  a restart. They are removed again now and it says so. SUDO_USER went into the rules unchecked; it must be an account's name.
- `labyrinth-agent-setup`: the owner's name went into sudoers checked only by `id` (which also takes a uid); `--agent root`
  (or any account below uid 1000) locked that account's password and gave the owner root without one; the owner as the agent
  was taken. All refused now.
- `labyrinth-ecosystem-setup`: a failed `openssl rand` inside the here-document wrote an empty SARINA_SERVICE_TOKEN (the port
  takes the token from getrandom); the image's hash and the key's public half could be left empty likewise; the model's name went
  into service.env unchecked (a newline in it adds a setting); `ollama list | grep -q` under pipefail said a model was missing
  when grep stopped reading early; `--help` printed four lines of code after the text.
- `labyrinth-grub-theme --preview FILE` with a relative FILE wrote the picture into its work folder, which it then removed; a
  GRUB_GFXMODE line was glued to a last line without a newline; an icon was copied under any class name grub.cfg gave (a `/` in
  one would have written outside the icons' folder).
- `labyrinth-security-setting scan -x` handed `-x` to heimdalld as an option; it is `./-x` now.
- `labyrinth-gaming setup` never chose the system-wide Proton-CachyOS (one `ls` over two folders fails when either has
  none; `check` had been fixed so, `setup` not).
- `labyrinth-claude-code` ran `curl ... | bash`: a download cut short ran as far as it came, and a failed download (bash read
  nothing and succeeded) was never reported. The installer is downloaded whole first now; `update` no longer says "not
  installed yet" when the update itself failed.
- `labyrinth-rea-setup` took REA_VERSION (any npm spec: a git URL would have been installed), GHIDRA_TAG and GHIDRA_ZIP (the
  download's address) and GHIDRA_SHA256 from the environment unchecked.
- `labyrinth-sarina-reflect` exported every KEY=VALUE of sarina.env that was not set, LD_PRELOAD and the interpreters'
  variables too, into nvidia-smi, curl and sarina; those are skipped now.
- `labyrinth-sarina-warm` put the model's name from sarina.env unchecked into the JSON it sends; `labyrinth-sarina-voice-setup`
  passed a voice name starting with `-` to the downloader as an option.

### Why Rust for the shell scripts

Not speed first: my order is security, then stability, then speed. What the ports bring, script by script:

| Script (runs as) | What is safer or steadier now |
| --- | --- |
| labyrinth-rollback (root) | Renames never replace or enter anything (RENAME_NOREPLACE; `mv` moved a subvolume *into* a folder of the same name); a snapshot without info.xml no longer stops it silently through `pipefail` + `set -e`; the snapshot list, GRUB's comparison and the log need no `ls`, `sort`, `diff`, `sed`, `date`; the partition's top level is unmounted and its folder removed on every way out |
| labyrinth-install-bootfinish (root, installer) | Lines are added on a line of their own (no setting glued to the last one of /etc/fstab or /etc/default/grub); the UUID that goes into the kernel command line is checked; every edit is written whole and renamed into place with the old file's mode and owner (no half-written /etc/default/grub when the disk fills) |
| labyrinth-install-cleanup (root, installer) | No word splitting over kernel and preset names; the kernels are copied whole and renamed into place; /etc/skel's owner and modes set without following links |
| labyrinth-agent-setup (root) | Account names checked before they go into sudoers (no uid as a name, no root or system account as the agent); the sudoers file is written whole, checked and renamed into place (sudo never sees half a file or none); Heimdall's config is edited without starting Python and written whole |
| labyrinth-agent-window (root) | The window's rules are removed again when the timer that closes them cannot be set (they stayed for good); SUDO_USER checked before it goes into the rules; the rules written whole and renamed into place |
| labyrinth-ecosystem-setup (root) | Sarina's token comes from the kernel's random numbers and is never empty (a failed openssl wrote an empty one); its file is 0640 and Sarina's from the start; the model's name is checked before it goes into service.env; no `ollama list \| grep -q` race under `pipefail`; JSON written as JSON |
| labyrinth-hardware-setup (root, installer) | Package lists passed as lists (the script relied on word splitting of `$pk`) |
| labyrinth-grub-theme (root, installer) | `--preview` with a relative file no longer loses the picture; icon names from grub.cfg checked (a `/` wrote outside the icons' folder); /etc/default/grub's lines added on their own line |
| labyrinth-security-setting | A folder named like an option is not passed to heimdalld as one |
| labyrinth-gaming | The system-wide Proton-CachyOS is found; config.vdf is read without starting Python; process and limit checks without `pgrep` and `ulimit`; Proton names with spaces are not split by `xargs` |
| labyrinth-sarina, -setting, -reflect, -warm, -voice-setup | sarina.env no longer puts LD_PRELOAD or interpreter variables into what reflect starts; the model's name is put into JSON as JSON; voice names cannot be options; sarina.env written whole under a name only the program picks (the script used a fixed .tmp name) |
| labyrinth-rea, labyrinth-rea-setup | The sandbox's arguments are a list built in one place; the version, the download's address and the hash taken from the environment are checked; Ghidra's hash is computed in-process |
| labyrinth-claude-code | The installer is downloaded whole before it runs (no `curl \| bash`), and a failed download is reported |
| labyrinth-install, labyrinth-notify-activate | Nothing to fix; no shell, `pgrep` or `seq` started for every notification activation |

Measured on 2026-11-11 on my desktop (16 threads, 62 GB), bash 5.3.20, Python 3.14.9 (for the two scripts that started it),
Rust 1.99.1. Old: each script as git has it at 3d27d69 (the image's, with the fixes above). New: the Rust release build of
3d27d69. Times are the median of 30 runs each, p10-p90 in brackets; "less" is old minus new.

| Program and case | Script | Rust | Less | CPU, script → Rust | Peak RSS, script → Rust |
| --- | --- | --- | --- | --- | --- |
| security-setting: auto-quarantine status | 1.71 ms (1.67-1.75) | 0.45 ms (0.44-0.47) | −74 % | 1.63 → 0.39 ms | 3.8 → 2.0 MB |
| sarina-setting: forge status | 2.56 ms (2.52-2.64) | 0.46 ms (0.44-0.47) | −82 % | 2.99 → 0.39 ms | 3.8 → 2.1 MB |
| sarina-setting: voice list | 3.50 ms (3.43-3.57) | 0.51 ms (0.49-0.54) | −85 % | 3.95 → 0.44 ms | 3.8 → 2.4 MB |
| agent-window: --status | 2.83 ms (2.76-2.86) | 0.80 ms (0.76-0.84) | −72 % | 3.12 → 0.74 ms | 4.3 → 2.1 MB |
| hardware-setup: --dry-run, an ASUS laptop | 5.34 ms (5.26-5.46) | 0.51 ms (0.49-0.53) | −90 % | 5.19 → 0.44 ms | 3.9 → 2.4 MB |
| gaming: check, everything there | 34.4 ms (34.0-34.9) | 4.29 ms (4.04-4.50) | −88 % | 37.5 → 4.08 ms | 10.9 → 2.3 MB |
| rollback: list | 47.5 ms (47.0-48.1) | 9.23 ms (9.11-9.41) | −81 % | 63.4 → 9.80 ms | 4.5 → 3.9 MB |
| rollback: N --dry-run | 20.2 ms (20.0-20.6) | 13.1 ms (12.9-13.3) | −35 % | 22.3 → 14.1 ms | 4.1 → 4.0 MB |
| sarina-reflect: a run with the Inbox | 13.3 ms (13.1-13.4) | 10.2 ms (10.0-10.5) | −23 % | 14.5 → 11.0 ms | 4.5 → 3.9 MB |
| sarina-warm | 2.41 ms (2.36-2.50) | 0.81 ms (0.78-0.88) | −66 % | 2.86 → 0.73 ms | 3.8 → 2.1 MB |
| rea: the sandbox's arguments, then bwrap | 4.66 ms (4.60-4.82) | 1.29 ms (1.25-1.32) | −72 % | 5.64 → 1.21 ms | 3.8 → 2.3 MB |
| claude-code: status | 2.24 ms (2.16-2.32) | 0.48 ms (0.45-0.50) | −79 % | 2.18 → 0.42 ms | 3.9 → 2.1 MB |
| install-cleanup: a mock root | 17.0 ms (16.6-17.6) | 1.28 ms (1.25-1.32) | −92 % | 16.8 → 1.19 ms | 4.2 → 2.6 MB |
| install-bootfinish: everything (btrfs, ufw, theme) | 40.0 ms (39.3-41.1) | 18.6 ms (18.2-18.9) | −54 % | 41.9 → 19.2 ms | 4.0 → 4.0 MB |
| agent-setup: first set-up, Heimdall told | 36.3 ms (35.8-37.3) | 11.3 ms (11.1-11.5) | −69 % | 37.2 → 12.3 ms | 11.1 → 4.0 MB |
| ecosystem-setup: everything, a base image | 35.1 ms (34.6-36.1) | 14.4 ms (14.1-14.8) | −59 % | 36.8 → 15.6 ms | 7.5 → 3.9 MB |
| grub-theme: install, a 3440x1440 monitor | 37.5 ms (36.9-38.1) | 5.38 ms (5.30-5.47) | −86 % | 50.6 → 5.73 ms | 4.0 → 3.8 MB |
| **on disk, the 20 programs** | 86 KB of scripts (+ bash, coreutils, sed, grep, gawk already there) | 7.4 MB (323-423 KB each; 0.8 MB with zstd -19) | **+7.3 MB** | | |

- Every case printed the same output and exit code on both sides (the bench compares the first run of each).
- Where the Rust times stay high, the stand-ins are most of them: bootfinish starts about 30 programs (systemctl, snapper,
  grub-mkconfig ...), reflect and the dry run start a shell stand-in that acts (a mount, a snapshot), and those cost the same on
  both sides. agent-setup's and gaming's peak memory was Python's (the script started it for Heimdall's config and Steam's).
- Worse: the disk. Twenty small programs carry Rust's standard library twenty times. One program with twenty names (as Heimdall's
  Wi-Fi helpers are links to heimdalld) would be under 1 MB, but then which program runs depends on argv[0], and some of these
  run as root through sudo: I kept one program per script. The ISO compresses them to well under 1 MB.
- Not measured: the scripts' waits (sarina's 30 s for the server, notify-activate's 20 s for the panel), the same on both sides.

How these numbers were measured: `cargo build --release && python3 -I parity/bench-scripts.py --runs 30` (a few minutes;
`--commit` picks the scripts' revision, `--only` programs, `--json` a file). It builds each case's parity world (the cases named
in the script), runs one unmeasured run of each side and compares their output, then 30 of each, alternating which goes first,
one process at a time, at nice 19. A small C launcher (built with cc), started inside the world in place of the program, runs it
and reports wall time, user + system CPU and peak RSS from wait4, so the namespace's own set-up is not counted; CPU and peak RSS
include the stand-ins a program starts (a small C program answering from files; the parity's shell stand-in where a case's
answer acts). A case that changes its world gets a fresh one before every run. Nothing on this computer is changed, nothing
reaches the network.

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
is closed in the script and the port alike, with a test in each (python/tests/test_labyrinth_agent.py, src/agent/tests.rs).

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

Three more, closed the same day (my rule: security over speed):

8. **pkg-install installed the file the caller named, after checking it.** The package's name was read from the file, then
   pacman installed the same path, so a file swapped in between was what got installed (a package named sudo, say). Now each file
   is copied first, read as the account sudo ran the gate for and never through a link at its end, into a fresh folder only root
   may enter (/var/cache/labyrinth-agent/install-XXXXXXXX, 0700; /var/cache/labyrinth-agent itself must be root's, 0700, or
   the install is refused); the name is read from the copy, only the copies are installed, and the folder is removed afterwards.
   A package file the caller cannot read is refused. A package of ours can still hold anything: pkg-install stays a root path
   by design, made deliberate and visible.
9. **deploy wrote the settings Settings asks my password for.** The fan and pump settings (/etc/yggdrasil/case-fans.json,
   gpu-fan.json), the power limits (power-limits.json) and the tuning (tuning.json) are written by casefan, gpufan, powerlimit and
   tune through polkit with auth_admin_keep; deploy put them in place without a password, and root's helpers applied them. They
   are refused now, and so are protected-accounts and protected-packages (what the account helper and Agora's pkg-root keep).
   Found in Yggdrasil's power-helpers and settings-helpers and their .policy files; nothing protected by polkit writes another
   /etc/yggdrasil file (greeter.conf, gametime.conf and dock are written by the installer or the package).
10. **Whoever could write a manifest's folder decided what limbo starts as root.** A manifest for `limbo up|down|doctor|status`
    is now refused unless it and every folder above it, to /, are owned by root, writable by root only, and no link.
    labyrinth-agent-setup makes /var/lib/limbo/build-agent root's own (0755): only root puts manifests there.

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
| python/tests/test_labyrinth_agent.py (was tests/; 6, and 16 for the holes) | src/agent/tests.rs (with 60,000 random command lines against src/agent/audit.rs), tests/agent.rs |

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
| labyrinth-agent | 77/77 (432 command lines) and 400/400 random command lines (2026-11-11), every call within the cases' allow-list |

All 484 cases and the 400 random ones are the same. Run against the scripts as they were before the fixes, the cases that show the
fixes differ (and only those, with the cases built on them); for labyrinth-agent nearly every case differs, because every program
it starts now gets a clean environment, and the allow-list check finds each hole above in the old script.

labyrinth-agent's cases (parity/cases_agent.py) run as root of a user and mount namespace whose uids 1000-2999 are this account's
subordinate ids (/etc/subuid), so the caller, uid 2001, is another account, in a root folder of the case's own (chroot): / and
/var are the case's, owned by root there (limbo's manifest check looks at every folder up to /), /usr is this computer's with
/usr/bin, /usr/share, /usr/lib and /usr/local laid over (overlays), /etc is an overlay, /proc, /dev, /tmp and /home are bound in.
The stand-ins for pacman, systemctl, journalctl, logger, heimdalld, limbo, ufw and labyrinth-rollback are where the gate looks for
the real ones, and every place deploy writes to or the gate searches is a folder of the case's own (no file of this computer
shows through, none is changed). The stand-in pacman can change the file it was asked about (the swap of item 8) and writes down
the name of every package it is given to install. Each stand-in writes down its arguments and its whole environment. Beside
the comparison, each run is checked against an allow-list written a third time in the cases (it models journalctl's option
parsing, optional values included, and Go's flags for heimdalld): no stand-in called outside it, no environment but the clean one,
no file written outside deploy's places, no package installed that is not ours, no copy left behind. LABYRINTH_AGENT_RANDOM sets the number of random command lines (default 400).

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
as an option) within seconds. After items 8-10 (and the audit's copies and root's-alone manifests): 50 million more in five
minutes (coverage 1233), none outside.

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
until these have run a while. The first seven were switched on 2026-11-11 (the package `labyrinth-tools` installs them in place of
the scripts); the last four with pkgrel 2 (branch `tools-rust-switch-2`): the image's autostart runs
/usr/bin/labyrinth-first-components and `labyrinth-gaming setup` runs /usr/bin/steam_playall. labyrinth-agent with pkgrel 4
(branch `agent-gate-rust`, switched together with the shell scripts): sudo's rule written by labyrinth-agent-setup names /usr/bin/labyrinth-agent, and the setup removes
the Python gate from /usr/local/sbin (sudo would find it first).

Formatting: `cargo fmt` with rustfmt.toml (Heimdall's settings: 140 columns); `cargo clippy --all-targets -- -D warnings` is clean.
