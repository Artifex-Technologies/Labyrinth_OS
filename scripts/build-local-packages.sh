#!/usr/bin/env bash
# Builds every PKGBUILD under packages/pkgbuilds/ (and the desktop's, from the Yggdrasil checkout next to this repository) and adds the results to the local
# [labyrinth] pacman repo at repo/x86_64/. Run as a regular user (makepkg refuses root),
# before scripts/build.sh, and again whenever a package's source changes.
#
# Requires: Arch Linux (or a container), base-devel, git, rust (minos, weather).
set -euo pipefail

if [[ $EUID -eq 0 ]]; then
  echo "error: makepkg refuses to run as root; run this as a regular user" >&2
  echo "  (it uses sudo for dependency installs)." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pkgbuilds_dir="$repo_root/packages/pkgbuilds"
# The desktop environment (panel, Settings, Minos, Marketplace, lock screen, ...) lives in its own repository: https://github.com/Artifex-Technologies/Yggdrasil
# Clone it next to this one, or point YGGDRASIL_DIR at it.
ygg_dir="${YGGDRASIL_DIR:-$repo_root/../Yggdrasil}"
if [[ ! -d "$ygg_dir/packages/pkgbuilds" ]]; then
  echo "error: the Yggdrasil repository is not at $ygg_dir (git clone https://github.com/Artifex-Technologies/Yggdrasil, or set YGGDRASIL_DIR)" >&2
  exit 1
fi
repo_dir="$repo_root/repo/x86_64"
mkdir -p "$repo_dir"

# Packages built from source in this repo (or the Yggdrasil or Sarina checkout next to it): their version does
# not change when the code does, so they are always rebuilt. Every other package is pinned to a
# released version and checksum, and is skipped when that exact version is already in the repo
# (Calamares alone is a ~10-minute C++ build). Naming packages rebuilds them regardless.
local_source=(liki-icon-theme yggdrasil-theme yggdrasil-minos yggdrasil-settings yggdrasil-spell yggdrasil-calculator yggdrasil-panel yggdrasil-weather yggdrasil-idle yggdrasil-keyring-sync yggdrasil-lock yggdrasil-polkit yggdrasil-portal yggdrasil-agora yggdrasil-session yggdrasil-xmove yggdrasil-installer yggdrasil-player yggdrasil-spell yggdrasil-clipboard yggdrasil-colorpicker labyrinth-distress labyrinth-ollama-tuning labyrinth-keyring sarina-git heimdall-git limbo-git)

# A package built from source here keeps the version its PKGBUILD names (0.1.0-27) until someone raises it by hand, so an
# installed system would never see a new build as an update. Its version is stamped with the number of commits in the
# repository it is built from: 0.1.0-27 becomes 0.1.0.r405-27, which pacman orders after 0.1.0-27 and after every earlier
# stamp. Not for -git recipes (their pkgver() already does this), recipes pinned to a download, or the keyring (its date is
# its version).
stamped_pkgbuild() {   # stamped_pkgbuild DIR NAME: prints the build script to use (PKGBUILD, or a stamped copy next to it)
  local dir=$1 name=$2 n
  if [[ " ${local_source[*]} " != *" $name "* || $name == *-git || $name == labyrinth-keyring ]] || grep -q '^pkgver()' "$dir/PKGBUILD" \
     || grep -A6 '^source=' "$dir/PKGBUILD" | grep -q 'https\?://' \
     || ! n=$(git -C "$dir" rev-list --count HEAD 2>/dev/null); then
    echo PKGBUILD; return
  fi
  sed -E "s/^pkgver=([0-9A-Za-z._]+)\$/pkgver=\1.r$n/" "$dir/PKGBUILD" > "$dir/PKGBUILD.stamped"
  echo PKGBUILD.stamped
}

only=("$@")   # optional: build just these package directory names (always rebuilt)
for dir in "$pkgbuilds_dir"/*/ "$ygg_dir/packages/pkgbuilds"/*/; do
  name="$(basename "$dir")"
  # Packages that are not on the ISO are left out unless named: Daedalus CAD (only in the Marketplace, its repository is private),
  # Proton Pass and VS Code (Agora installs them from the AUR: their licences do not let Labyrinth pass them on).
  if (( ! ${#only[@]} )) && [[ " ${LABYRINTH_SKIP-daedalus-cad-git proton-pass visual-studio-code-bin} " == *" $name "* ]]; then echo "==> $name: not on the ISO, skipping"; continue; fi
  if (( ${#only[@]} )) && [[ " ${only[*]} " != *" $name "* ]]; then continue; fi
  if (( ! ${#only[@]} )) && [[ " ${local_source[*]} " != *" $name "* ]]; then
    built=1
    while read -r f; do
      [[ -e "$repo_dir/$(basename "$f")" ]] || built=0
    done < <(cd "$dir" && makepkg --packagelist 2>/dev/null)
    if (( built )); then echo "==> $name: this version is already built, skipping"; continue; fi
  fi
  echo "==> Building $name"
  (
    cd "$dir"
    rm -rf pkg src ./*.pkg.tar.zst
    script=$(stamped_pkgbuild "$dir" "$name")
    makepkg -p "$script" -f -s --noconfirm
    rm -f PKGBUILD.stamped
    cp -f ./*.pkg.tar.zst "$repo_dir/"
  )
done

rm -f "$repo_dir"/labyrinth.db* "$repo_dir"/labyrinth.files*
repo-add "$repo_dir/labyrinth.db.tar.gz" "$repo_dir"/*.pkg.tar.zst
echo "Built and added to local repo: $repo_dir"
