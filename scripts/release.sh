#!/usr/bin/env bash
# Versions and releases (docs/VERSIONING.md). The repositories sit side by side: this one, Yggdrasil, Heimdall, ...
#
#   scripts/release.sh                       every project's version, and the version each package would be built as now,
#                                            next to the one installed on this computer (read only: builds and changes nothing)
#   scripts/release.sh PROJECT NEW           a release: NEW is X.Y.Z, or patch / minor / major (raises that part of the
#                                            current version). Writes the VERSION file (for the system also Yggdrasil's and
#                                            the medium's /etc/os-release; for Sarina also its Go copy and its tarball
#                                            recipe), commits only those, tags the commit vX.Y.Z and says how to push.
#                                            Pushes nothing; every build number starts again at 1.
#
# PROJECT: system (Labyrinth OS: this repository and Yggdrasil), heimdall, limbo, sarina, kairos, argus (the app; the firmware
# keeps its own ARG-... names), daedalus, huginn, icarus.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
parent="$(dirname "$root")"
ygg="${YGGDRASIL_DIR:-$parent/Yggdrasil}"
declare -A dirs=([system]="$root" [heimdall]="$parent/Heimdall" [limbo]="$parent/Limbo" [sarina]="$parent/Sarina"
                 [kairos]="$parent/Kairos" [argus]="$parent/Argus_App" [daedalus]="$parent/Daedalus_CAD"
                 [huginn]="$parent/Huginn" [icarus]="$parent/Icarus")
order=(system heimdall limbo sarina kairos argus daedalus huginn icarus)
die() { echo "error: $*" >&2; exit 1; }

# --- the list ------------------------------------------------------------------------------------------------------------

# recipe_version DIR: the version the recipe in DIR would get if it were built now, from the checkouts next to this one (a
# -git recipe's clone is stood in for by the checkout of the same repository)
# shellcheck disable=SC2034,SC2154  # makepkg's variables, set here for the recipe and set by it
recipe_version() {
  local d=$1 fake
  fake=$(mktemp -d)
  (
    set +eu
    cd "$d" || exit 1
    startdir=$d BUILDDIR=$d srcdir=$fake
    # shellcheck disable=SC1091
    source ./PKGBUILD >/dev/null 2>&1
    for s in "${source[@]}"; do
      [[ $s == *::git+* && $s != *'#'* ]] || continue
      local name=${s%%::*} url=${s#*::git+} repo
      if [[ $url == file://* ]]; then repo=${url#file://}; else repo=$parent/$(basename "$url" .git); fi
      [[ -d $repo/.git ]] && ln -s "$repo" "$fake/$name"
    done
    source ./PKGBUILD >/dev/null 2>&1
    echo "$pkgname ${epoch:+$epoch:}$pkgver-$pkgrel"
  )
  rm -rf "$fake"
}

list() {
  printf '%-10s %-8s %-9s %s\n' PROJECT VERSION RELEASED FOLDER
  for p in "${order[@]}"; do
    local d=${dirs[$p]} v base
    [[ -d $d ]] || continue
    v=$(git -C "$d" show HEAD:VERSION 2>/dev/null || echo -)
    base=$(git -C "$d" log -1 --format=%h -- VERSION 2>/dev/null)
    printf '%-10s %-8s %-9s %s\n' "$p" "$v" "${base:--}" "$d"
  done
  echo
  printf '%-30s %-28s %-16s %s\n' PACKAGE INSTALLED "WOULD BE BUILT" ""
  local d line name new old c verdict
  for d in "$root"/packages/pkgbuilds/*/ "$root"/profile/airootfs/usr/share/labyrinth/pkgbuilds/*/ "$ygg"/packages/pkgbuilds/*/; do
    [[ -f $d/PKGBUILD ]] || continue
    line=$(recipe_version "${d%/}")
    name=${line%% *} new=${line#* }
    old=$(pacman -Q "$name" 2>/dev/null | cut -d' ' -f2 || true)
    if [[ -z $old ]]; then verdict="(not installed)"
    else
      c=$(vercmp "$new" "$old")
      if ((c > 0)); then verdict="newer"; elif ((c == 0)); then verdict="the same"; else verdict="OLDER: would not be installed as an update"; fi
    fi
    printf '%-30s %-28s %-16s %s\n' "$name" "${old:--}" "$new" "$verdict"
  done
}

# --- a release -------------------------------------------------------------------------------------------------------------

release() {
  local project=$1 want=$2 d=${dirs[$1]:-} cur new tag
  [[ -n $d ]] || die "unknown project '$project' (one of: ${order[*]})"
  [[ -d $d/.git ]] || die "$d is not a git checkout"
  cur=$(git -C "$d" show HEAD:VERSION 2>/dev/null) || die "$d has no committed VERSION file"
  [[ $cur =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]] || die "$d/VERSION is '$cur', not MAJOR.MINOR.PATCH"
  case $want in
    patch) new="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.$((BASH_REMATCH[3] + 1))" ;;
    minor) new="${BASH_REMATCH[1]}.$((BASH_REMATCH[2] + 1)).0" ;;
    major) new="$((BASH_REMATCH[1] + 1)).0.0" ;;
    *) new=$want ;;
  esac
  [[ $new =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "'$new' is not MAJOR.MINOR.PATCH"
  (($(vercmp "$new" "$cur") > 0)) || die "$new is not newer than $cur"
  tag=v$new

  # every repository and file first, then the changes: nothing is half done
  local repos=("$d") files
  [[ $project == system ]] && repos+=("$ygg")
  for r in "${repos[@]}"; do
    [[ -d $r/.git ]] || die "$r is not a git checkout"
    git -C "$r" symbolic-ref -q HEAD >/dev/null || die "$r is not on a branch"
    git -C "$r" rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "$r already has the tag $tag"
    [[ -z $(git -C "$r" status --porcelain -- VERSION) ]] || die "$r/VERSION has changes that are not committed"
    [[ $(git -C "$r" show HEAD:VERSION 2>/dev/null) == "$cur" ]] || die "$r/VERSION is not $cur (the system's two must agree)"
  done
  # Sarina's Go module (go/) cannot embed the VERSION file above it, so it keeps the number as a constant here (its test holds
  # the two equal); the programs of every other project read VERSION when they are built
  local gv=go/internal/version/version.go
  if [[ -f $d/$gv ]]; then
    grep -q '^const Version = "' "$d/$gv" || die "$d/$gv has no 'const Version = \"...\"' line"
    [[ -z $(git -C "$d" status --porcelain -- "$gv") ]] || die "$d/$gv has changes that are not committed"
  fi
  local osr=profile/airootfs/etc/os-release
  if [[ $project == system ]]; then
    [[ -z $(git -C "$root" status --porcelain -- "$osr") ]] || die "$osr has changes that are not committed"
  fi

  local show=${new%.0}   # 1.1.0 is shown as 1.1, 1.1.1 as it is
  for r in "${repos[@]}"; do
    files=(VERSION)
    echo "$new" > "$r/VERSION"
    if [[ $r == "$root" && $project == system ]]; then
      sed -i -e "s/^VERSION=.*/VERSION=\"$show\"/" -e "s/^VERSION_ID=.*/VERSION_ID=$new/" "$root/$osr"
      files+=("$osr")
    fi
    # a recipe for a released tarball (Sarina's packaging/arch, for the AUR) names the version itself
    local pb=packaging/arch/PKGBUILD
    if [[ -f $r/$pb ]] && ! grep -q '_version' "$r/$pb" && grep -q '^pkgver=' "$r/$pb"; then
      sed -i -e "s/^pkgver=.*/pkgver=$new/" -e "s/^pkgrel=.*/pkgrel=1/" "$r/$pb"
      files+=("$pb")
    fi
    if [[ -f $r/$gv ]]; then
      sed -i "s/^const Version = \".*/const Version = \"$new\"/" "$r/$gv"
      files+=("$gv")
    fi
    # Sarina's Python helpers name the version in their package metadata too
    local hp=python/sarina_helpers/pyproject.toml hi=python/sarina_helpers/sarina_helpers/__init__.py
    if [[ -f $r/$hp && -f $r/$hi ]]; then
      sed -i "s/^version = \".*\"/version = \"$new\"/" "$r/$hp"
      sed -i "s/^__version__ = '.*'/__version__ = '$new'/" "$r/$hi"
      files+=("$hp" "$hi")
    fi
    git -C "$r" commit -q -m "Release $new" -m "$(basename "$r") $cur -> $new (docs/VERSIONING.md in Labyrinth_OS)." -- "${files[@]}"
    git -C "$r" tag -a "$tag" -m "$(basename "$r") $new"
    echo "==> $(basename "$r"): $cur -> $new, committed and tagged $tag"
    echo "    push: git -C $r push origin HEAD $tag"
  done
}

if (($# == 0)); then list
elif (($# == 2)); then release "$1" "$2"
else sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 2
fi
