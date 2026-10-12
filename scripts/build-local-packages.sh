#!/usr/bin/env bash
# Builds every PKGBUILD under packages/pkgbuilds/ and adds the results to the local
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
repo_dir="$repo_root/repo/x86_64"
mkdir -p "$repo_dir"

# Packages built from source in this repo (or the Sarina checkout next to it): their version does
# not change when the code does, so they are always rebuilt. Every other package is pinned to a
# released version and checksum, and is skipped when that exact version is already in the repo
# (Calamares alone is a ~10-minute C++ build). Naming packages rebuilds them regardless.
local_source=(labyrinth-minos labyrinth-settings labyrinth-calculator labyrinth-panel labyrinth-weather sarina-git)

only=("$@")   # optional: build just these package directory names (always rebuilt)
for dir in "$pkgbuilds_dir"/*/; do
  name="$(basename "$dir")"
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
    rm -rf pkg src
    makepkg -f -s --noconfirm
    cp -f ./*.pkg.tar.zst "$repo_dir/"
  )
done

rm -f "$repo_dir"/labyrinth.db* "$repo_dir"/labyrinth.files*
repo-add "$repo_dir/labyrinth.db.tar.gz" "$repo_dir"/*.pkg.tar.zst
echo "Built and added to local repo: $repo_dir"
