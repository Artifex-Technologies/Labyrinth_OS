#!/usr/bin/env bash
# Run Labyrinth's checks on real Linux (an Arch WSL distro or any Arch box).
#   scripts/wsl-test.sh deps      install what the checks need (needs root)
#   scripts/wsl-test.sh tests     run every test suite
#   scripts/wsl-test.sh packages  check each package name against the repos pacman knows
#   scripts/wsl-test.sh all       deps, tests, packages
# Run from a copy of the repo on the Linux filesystem (not /mnt/c: permissions and
# symlinks break there; see Chymaera_OS/CLAUDE.md).
set -u
cd "$(dirname "$0")/.."

fails=0
step() { printf '\n== %s\n' "$*"; }
run()  { if "$@"; then echo "   ok: $*"; else echo "   FAILED: $*"; fails=$((fails + 1)); fi; }

deps() {
  step "installing test dependencies"
  pacman -S --needed --noconfirm base-devel git python rust shellcheck pciutils usbutils \
    wayland libxkbcommon libglvnd vulkan-icd-loader fontconfig xdg-utils libx11 libxcursor libxi libxrandr
}

tests() {
  step "installer cleanup script (mock root)"
  run bash tests/test_install_cleanup.sh
  step "desktop scripts"
  run bash tests/test_desktop_scripts.sh
  step "components, app manager, package list"
  run python3 tests/test_labyrinth_apps.py
  step "bar tools"
  run python3 tests/test_bar_tools.py
  step "display layout"
  run python3 tests/test_displays.py
  step "steam config editor"
  run python3 tests/test_steam_playall.py
  step "shell syntax and shellcheck"
  for f in profile/airootfs/usr/local/bin/labyrinth-install-* profile/airootfs/usr/local/bin/labyrinth-shell \
           profile/airootfs/usr/local/bin/labyrinth-quick-settings profile/airootfs/usr/local/bin/labyrinth-gaming \
           packages/labyrinth-driverlab/probe.sh profile/profiledef.sh; do
    run bash -n "$f"
  done
  step "Rust: file explorer"
  (cd packages/minos && run cargo test --lib)
  step "Rust: weather"
  (cd packages/yggdrasil-weather && run cargo test --lib)
}

packages() {
  step "package names (needs a synced pacman database: pacman -Sy)"
  python3 scripts/gen-packages.py --check || { echo "package list stale"; fails=$((fails + 1)); }
  missing=0
  while read -r p; do
    if ! pacman -Si "$p" >/dev/null 2>&1; then
      echo "   not found by pacman: $p"; missing=$((missing + 1))
    fi
  done < <(sed '/^[[:blank:]]*#.*/d;s/#.*//;/^[[:blank:]]*$/d' profile/packages.x86_64)
  echo "   $missing name(s) not found (expected on plain Arch: linux-cachyos*, proton-cachyos, protonup-qt, and our own: calamares, labyrinth-*, proton-pass, visual-studio-code-bin)"
}

case "${1:-all}" in
  deps) deps ;;
  tests) tests ;;
  packages) packages ;;
  all) deps; tests; packages ;;
  *) echo "usage: $0 deps|tests|packages|all"; exit 2 ;;
esac
echo; [ "$fails" -eq 0 ] && echo "RESULT: all steps passed" || echo "RESULT: $fails step(s) failed"
[ "$fails" -eq 0 ]
