#!/usr/bin/env bash
# The ISO's own files (profile/airootfs) that the labyrinth-system-files package owns, so that an update refreshes them on an
# installed system: every file and link there except those profile/not-packaged.txt names. One path per line, from the
# airootfs root (/usr/share/labyrinth/agora/catalog.json). The package's recipe installs them; scripts/build.sh takes them,
# and the files another recipe ships, out of the ISO's copy of airootfs, since a file both there and in a package stops
# mkarchiso's pacstrap ("exists in filesystem").
#   scripts/system-files.sh            the package's files
#   scripts/system-files.sh recipes    the files another of our recipes ships
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
want=${1:-packaged}
[[ $want == packaged || $want == recipes ]] || { echo "usage: $0 [recipes]" >&2; exit 2; }
mapfile -t rules < <(grep -vE '^[[:space:]]*(#|$)' "$here/profile/not-packaged.txt")
cd "$here/profile/airootfs"
find . \( -type f -o -type l \) -printf '/%P\n' | LC_ALL=C sort | while IFS= read -r f; do
  kind=packaged
  for r in "${rules[@]}"; do
    # shellcheck disable=SC2053  # the rule is a pattern
    if [[ $r == recipe:* ]]; then [[ $f == ${r#recipe:} ]] && { kind=recipes; break; }
    elif [[ $f == $r ]]; then kind=none; break; fi
  done
  if [[ $kind == "$want" ]]; then printf '%s\n' "$f"; fi
done
