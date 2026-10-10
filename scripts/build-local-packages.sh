#!/usr/bin/env bash
# Builds every PKGBUILD under packages/pkgbuilds/ and adds the results to the local
# [labyrinth] pacman repo at repo/x86_64/. Run as a regular user (makepkg refuses root),
# before scripts/build.sh, and again whenever a package's source changes.
#
# Requires: Arch Linux (or a container), base-devel, git, rust (finder, weather).
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

only=("$@")   # optional: build just these package directory names
for dir in "$pkgbuilds_dir"/*/; do
  name="$(basename "$dir")"
  if (( ${#only[@]} )) && [[ " ${only[*]} " != *" $name "* ]]; then continue; fi
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
