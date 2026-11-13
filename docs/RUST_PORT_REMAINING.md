# What is not Rust yet

Written 2026-11-12. My rule: everything of ours that can be Rust becomes Rust, in the order security, stability, speed.
Something stays in another language only where the platform needs it (PKGBUILDs and pacman's scriptlets, archiso's profile,
GLSL/CUDA, Hyprland's Lua config and its C++ plugin until our own compositor, the Flipper-side C in Argus, model training). The
Python and shell copies kept after a switch, for a rollback, stay one week after the switched package runs on my PC, then go.
Pacman, the AUR, the Arch and CachyOS repositories and their tooling (makepkg, mkarchiso, repo-add, pacman-key) are not ours
and stay as they are.

This page covers this repository and Yggdrasil. Heimdall, Limbo and Sarina keep their own porting plans in their repositories
(Heimdall and Limbo have their Rust ports, switching now; Sarina's Go and Python are being ported, see its `docs/RUST_PORT.md`).

How it was made: every file with a `#!` line or a `.sh`/`.py`/`.lua` name in both repositories; every `install` line of the
PKGBUILDs (what ships); everything under `profile/airootfs` (what the image carries); every `ExecStart`/`Exec=` of the units,
autostart and D-Bus files; the pacman hooks and `.install` scriptlets; Yggdrasil's idle hooks; and Rust code that starts a
shell (`sh -c`) or an interpreter.

## Ported on 2026-11-12

Each was run against the old one on the same made-up worlds (stand-ins on the PATH, namespaces for fixed paths and processes,
nothing reaching the desktop or the system) before the switch; the bugs found were fixed in the old one first, then the
package was switched and the old one deleted (git keeps it). Numbers: Yggdrasil's `docs/PORT_NUMBERS.md`.

| Program | Was | Now | Differential |
| --- | --- | --- | --- |
| `yggdrasil-zoom`, `yggdrasil-screen-reader`, `yggdrasil-hypr-plugin`, `yggdrasil-hypr-logkeep` (session) | bash; the plugin loader also started python3 to read one JSON field at every login | Yggdrasil `packages/session` (new crate); `yggdrasil-session` no longer depends on Python | 11, 6, 14, 7 cases the same (`tools/parity-shell-ports.py`) |
| `yggdrasil-panelctl`, `yggdrasil-mediakey`, `yggdrasil-mount-open` (panel) | sh | Yggdrasil `packages/panel/src/bin` | 19, 17, 10 the same |
| `liki-icon`, `liki-minos-icon` | bash | Yggdrasil `packages/liki` | 15, 18 the same |
| `/usr/lib/agora/sudo` (Agora's sudo stand-in, in front of pkexec) | sh | Yggdrasil `packages/agora-helpers` (`agora-sudo`) | 9 the same |
| `/usr/lib/yggdrasil/spell-global` (pkexec, root) | sh | Yggdrasil `packages/spell` | 5 the same |
| The panel's speed test | `sh -c 'head -c 5000000 /dev/zero \| curl ...'` from Rust | curl by argv, the zeros written from the panel | a test runs the old lines and the new code against one stand-in curl: same arguments, same bytes |
| The distress password's factory reset and system wipe | `sh -c "userdel -rf {name}; rm -rf {home}"` and `sh -c "wipefs -af {disk}; dd ...; blkdiscard ..."` built from /etc/passwd and lsblk, run as root | argv steps (`Step::Each`, `Quiet`, `Remove`); only the commands I write myself in Settings stay shell lines | a test compares the programs and arguments with the old lines' words; a home with a space removed two other folders (test failed on the old code first) |
| The os-release edit in `labyrinth-tools.install` (VERSION and VERSION_ID at every update, as root) | an awk program in the scriptlet | `labyrinth-os-release` (labyrinth-tools); the scriptlet is one line calling it | the old scriptlet function and the program on 9 made-up files (Labyrinth's, quoted ID, no newline at the end, two PRETTY_NAME lines, another system, empty, a link, none) times 3 versions: 27 of 27 the same (contents and modes) |
| Pacman hook `60-labyrinth-nosuid` | `Exec = /bin/sh -c 'for f in ...; do [ -e "$f" ] && chmod u-s,g-s "$f"; done; true'` | two hooks, one per file, `Exec = /usr/bin/chmod u-s,g-s FILE` (pacman runs it with no shell, only when that file was just installed or upgraded) | the old line and the two commands on setuid files in a scratch folder, all four cases (both, one, the other, none): same modes, same results |

## Ported on 2026-11-13

| Program | Was | Now | Differential |
| --- | --- | --- | --- |
| `scripts/publish-repo.py` (the daily package publish: fresh copies from GitHub, the builds, signing, repo-add, the upload to the release) | Python, 349 lines, started by my user timer at 05:30 | `labyrinth-publish-repo` (`packages/publish-repo`), installed to `~/.local/bin` on my PC (not a package); the Arch tools it calls stay | 22 cases, 33 publishes the same (`packages/publish-repo/parity/compare.py`: user, mount, PID and network namespaces, a throw-away key, my keyring covered; the real gpg, bsdtar, vercmp and repo-add; every call in order, the files, the database, exit codes and messages, the refusal paths: a changed build that is not newer, no key, a failed build, a partial upload). Nothing wrong found in the script. Measured: the daily nothing-changed run 67.3 → 22.2 ms, its own peak memory 20.9 → 2.8 MB (the package's README) |

Bugs found on the way (fixed in the scripts first, Yggdrasil f54dc78): `yggdrasil-mount-open` opened the wrong folder for a
drive whose name has " at " in it ("Photos at home" opened "home"); `liki-minos-icon - - plain` stopped on a picture called
`-.svg`, and an accent given went into bash's arithmetic, which runs `a[$(command)]` as code; `liki-icon --import` counted a
folder named like a picture as imported; three usage texts printed lines of the script's code. In Labyrinth: the distress
reset's shell line split a home with a space.

## Copies kept for a rollback

They ship in no package; they are the old programs, kept beside the ports with the same fixes, plus the Python tests and
parity scripts that need them. Each goes one week after its switch ran on my PC (the dates below assume I installed the
switched package the day of its switch); the tests and parity scripts that only exist to compare against it go with it.

| Copy | Replaced by | Switched | Remove after |
| --- | --- | --- | --- |
| Yggdrasil `packages/settings/helpers/python/` tune, casefan, gpufan, powerlimit (and their tests) | `packages/power-helpers` (yggdrasil-power) | 2026-11-09 | 2026-11-16 |
| Yggdrasil `packages/settings/helpers/python/` account, bootscreen, game-fix, gpu-run (and their tests) | `packages/settings-helpers` | 2026-11-10 | 2026-11-17 |
| Yggdrasil `packages/appusage/python/` (labyrinth-appusage and its test) | `packages/appusage` | 2026-11-10 | 2026-11-17 |
| Yggdrasil `packages/market/helpers/python/pkg-root` (`tests/test_agora_pkg_root.py`) | `packages/agora-helpers` | 2026-11-10 | 2026-11-17 |
| Yggdrasil `packages/theme/python/ygg-theme` (`tests/test_ygg_theme.py`) | `packages/theme` | 2026-11-10 | 2026-11-17 |
| Yggdrasil `packages/theme/python/ygg-cursors` | `packages/theme` | 2026-11-11 | 2026-11-18 |
| Yggdrasil `packages/liki/python/` liki-build, liki-icns, liki-icon-import (`tests/test_liki_icon_import.py`) | `packages/liki` | 2026-11-11 | 2026-11-18 |
| Yggdrasil `packages/installer/helpers/python/install-system` (and its tests) | `packages/install-system` | 2026-11-11 | 2026-11-18 |
| `packages/tools/python/` (the 12 Python programs and their tests) | `labyrinth-tools` | 2026-11-11 | 2026-11-18 |
| `packages/tools/shell/` (the 20 shell scripts, and labyrinth-sarina-ask and -speak) | `labyrinth-tools` | 2026-11-11 | 2026-11-18 |
| `scripts/publish-repo.py` and `tests/test_publish_repo.py` (the old side of `packages/publish-repo/parity/`) | `labyrinth-publish-repo` (`packages/publish-repo`) | in the repository 2026-11-13; my timer still starts the script until I install the program and change the unit's `ExecStart=` (on my to-do list) | one week after the timer first runs the program |

The parity scripts that compare against them (`packages/*/parity/`, `packages/tools/parity/`) take the old versions from git
where they can; those that read the `python/` or `shell/` folders are changed to git or go with them.

## Must stay (the platform needs it)

| What | Language | Why |
| --- | --- | --- |
| Every `PKGBUILD`, `scripts/makepkg.conf`, `scripts/package-version.sh` (both repositories; sourced by every PKGBUILD) | bash | makepkg's format |
| `.install` scriptlets: labyrinth-keyring, labyrinth-tools, limbo-git, yggdrasil-gametime, yggdrasil-spell | sh | pacman runs scriptlets with bash; they only call systemctl, pacman-key and, in labyrinth-tools, labyrinth-os-release |
| `profile/profiledef.sh` | bash | archiso's format |
| `profile/airootfs/etc/profile.d/labyrinth-path.sh` | sh | read by every login shell |
| Pacman hooks (`61-labyrinth-proton-perms.hook`, Yggdrasil's `90-yggdrasil-power.hook`, the two nosuid hooks) | pacman's format | they name one program by argv (chmod, `/usr/lib/yggdrasil/power adopt`); no shell |
| Yggdrasil `packages/session/hyprland/*.lua` and `packages/session/tests/*.lua` | Lua | Hyprland's config language, until our own compositor; its four `sh -c '... \| systemd-cat'` lines stay too, as Hyprland runs every `exec` through `sh -c` anyway |
| Yggdrasil `packages/session/hyprland/plugin/minimize.cpp` | C++ | Hyprland's plugin interface, until our own compositor |
| Command lines a person types or writes: Task Manager's "Run new task", Minos running a script you double-click, the distress password's "Run commands" | sh | the shell is the feature; everything the computer itself supplies goes by argv |
| The compositor's key bindings run through `/bin/sh` (Yggdrasil `packages/compositor`) | sh | binding lines are command lines; the compositor's own work, not this pass |
| Sarina's Python helpers (embeddings, the ONNX classifier) and model training | Python | Sarina's repository and plan; training stays Python |
| The measuring and differential harnesses: Yggdrasil `tools/bench-port.py`, `tools/parity-shell-ports.py`, `packages/*/parity/`; `packages/tools/parity/` and `packages/publish-repo/parity/` here | Python (standard library) | they run the old Python and shell programs side by side with the ports; not shipped. They go, or move to Rust, when nothing old is left to compare against |
| `scripts/windows/Reset-UsbDrive.ps1` and `.cmd` | PowerShell | runs on Windows, to reset a USB stick |

## To port

Smallest risk first. None of these runs as a service; the first two ship or run on my PC.

| What | Language, size | Runs | Plan |
| --- | --- | --- | --- |
| Yggdrasil `packages/session/bin/yggdrasil-session` (the session's launcher: environment, MangoHud's config, Hyprland's crash watchdog and safe mode, the monitor log, the restart or shut-down after it) | bash, 164 lines | every login | next: into `packages/session` with the four ported today. A mistake there means no desktop, so it needs a nested-session test first (Hyprland stand-in that crashes on cue) |
| `profile/airootfs/usr/local/bin/labyrinth-voiced` and `/usr/local/lib/labyrinth/chatterbox_engine.py` (Sarina's voice server, Chatterbox Turbo on PyTorch) | Python | Sarina's spoken answers (user service) | the planned next big port: Chatterbox through ONNX in Rust (`scripts/voice/chatterbox_onnx_test.py` is the probe) |
| Yggdrasil `packages/settings/helpers/gpuunlock` and `packages/settings/data/41_gpu_unlock` (grub.d fragment) | Python 841 lines (root), sh 22 lines | Settings > Power > Graphics card unlock; grub-mkconfig | after the GPU plan (`docs/GPU_UNLOCK.md`); safety-critical, its tests carried over |
| `profile/airootfs/usr/share/labyrinth/grub-evodevo/install.sh` (the boot menu theme's own installer, vendored, MIT) | sh, 884 lines | as root, by `labyrinth-grub-theme` at install | labyrinth-grub-theme does the few steps Labyrinth needs itself (copy the theme, set GRUB_THEME) and the vendored script goes |
| `packages/labyrinth-driverlab/probe.sh` (hardware facts as JSON for the Driver Lab) | bash + python3 (only to escape JSON) | nothing runs it yet | a labyrinth-tools program when the Driver Lab is built |
| Build and release tooling here: `scripts/build.sh` (wraps mkarchiso), `build-local-packages.sh` (wraps makepkg), `release.sh`, `release-check.sh`, `gen-packages.py`, `gen-wallpaper.py`, `gen-plymouth-rain.py`, `gen-logo.sh`, `logo-to-braille.py`, `icons/*.py`, `vm/*`, `xvfb/xv.sh`, `wsl-test.sh`, `voice/chatterbox_onnx_test.py` | bash, Python | by hand on my PC; not shipped | as each is touched; the Arch tools they call stay |
| Yggdrasil's dev tooling: `scripts/screenshot-desktop.sh`, `tools/bakeoff/*`, `packages/saga-md/icon/gen-icon.py`, `packages/pycompat/gen_unicode.py` (and its copy `packages/tools/src/pycompat/gen_unicode.py` here) | bash, Python | by hand; not shipped | as each is touched |
| Tests written in Python or shell that are not referees of a rollback copy: here `tests/*.py`, `tests/*.sh`, `tests/appnet_ask/*`; Yggdrasil `tests/a11y/*`, `tests/tools/fake_sni.py`, `tests/test_keyring_sync.sh`, `packages/*/tests/*.sh` | Python, bash | by hand or in CI | into each crate's `tests/` as the code they test is touched |

Being ported by others now, not in this pass: Yggdrasil's `packages/idle/idle.d/keyboard-lights` (moved into yggdrasil-idle
on 2026-11-12), the compositor and window manager crates, and `packages/settings-helpers/src/autozone.rs`.
