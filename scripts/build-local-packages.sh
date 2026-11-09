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
# compiler settings for any 64-bit PC, not this machine's CPU (scripts/makepkg.conf)
makepkg_conf="$repo_root/scripts/makepkg.conf"

# Packages built from source in this repo (or the Yggdrasil or Sarina checkout next to it): their version does
# not change when the code does, so they are always rebuilt. Every other package is pinned to a
# released version and checksum, and is skipped when that exact version is already in the repo
# Naming packages rebuilds them regardless.
local_source=(liki-icon-theme yggdrasil-theme yggdrasil-minos yggdrasil-settings yggdrasil-spell yggdrasil-calculator yggdrasil-taskmanager yggdrasil-leakwatch saga-md yggdrasil-panel yggdrasil-weather yggdrasil-idle yggdrasil-keyring-sync yggdrasil-lock yggdrasil-polkit yggdrasil-portal yggdrasil-agora yggdrasil-session yggdrasil-xmove yggdrasil-installer yggdrasil-player yggdrasil-spell yggdrasil-clipboard yggdrasil-colorpicker labyrinth-distress labyrinth-ollama-tuning labyrinth-keyring kairos-git daedalus-cad-git argus-git sarina-git heimdall-git limbo-git)

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
  # Packages that are not on the ISO are left out unless named: Daedalus CAD (only in the Marketplace) and Argus (LykanOS's;
  # its firmware build fetches Flipper's ARM toolchain). The publish passes an empty list and builds them all.
  if (( ! ${#only[@]} )) && [[ " ${LABYRINTH_SKIP-daedalus-cad-git argus-git} " == *" $name "* ]]; then echo "==> $name: not on the ISO, skipping"; continue; fi
  if (( ${#only[@]} )) && [[ " ${only[*]} " != *" $name "* ]]; then continue; fi
  if (( ! ${#only[@]} )) && [[ " ${local_source[*]} " != *" $name "* ]]; then
    built=1
    while read -r f; do
      [[ -e "$repo_dir/$(basename "$f")" ]] || built=0
    done < <(cd "$dir" && makepkg --config "$makepkg_conf" --packagelist 2>/dev/null)
    if (( built )); then echo "==> $name: this version is already built, skipping"; continue; fi
  fi
  echo "==> Building $name"
  (
    cd "$dir"
    rm -rf pkg src ./*.pkg.tar.zst
    script=$(stamped_pkgbuild "$dir" "$name")
    # -c: makepkg removes its src/ and pkg/ folders after a successful build (they held gigabytes of build files)
    makepkg --config "$makepkg_conf" -p "$script" -f -s -c --noconfirm
    rm -f PKGBUILD.stamped
    cp -f ./*.pkg.tar.zst "$repo_dir/"
  )
done

# Every package and the database are signed with Labyrinth's key, so the ISO build checks them like any other repository
# (profile/pacman.conf: SigLevel = Required). The key lives in its own keyring (docs/PACKAGE_REPO.md); a build host without it can
# set LABYRINTH_UNSIGNED=1, and scripts/build.sh then trusts the local packages unsigned, saying so.
KEY=C9CE784C8BD9B661042075972D3BD4B1E7CD07A5
export GNUPGHOME=${LABYRINTH_GNUPGHOME:-$HOME/.local/share/labyrinth-signing/gnupg}
sign=()
if [[ ${LABYRINTH_UNSIGNED:-} != 1 ]]; then
  gpg --batch --list-secret-keys "$KEY" >/dev/null 2>&1 \
    || { echo "error: Labyrinth's signing key is not in $GNUPGHOME (restore it, or set LABYRINTH_UNSIGNED=1)" >&2; exit 1; }
  for f in "$repo_dir"/*.pkg.tar.zst; do
    [[ -e $f.sig && $f.sig -nt $f ]] && continue
    gpg --batch --yes --local-user "$KEY" --detach-sign --no-armor --output "$f.sig" "$f"
  done
  sign=(--sign --key "$KEY")
fi
rm -f "$repo_dir"/labyrinth.db* "$repo_dir"/labyrinth.files*
repo-add --quiet "${sign[@]}" "$repo_dir/labyrinth.db.tar.gz" "$repo_dir"/*.pkg.tar.zst
echo "Built and added to local repo: $repo_dir${sign:+ (signed)}"
