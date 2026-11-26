#!/usr/bin/env bash
# The install medium's boot menu (profile/efiboot/loader): the desktop starts at once from the stick (copytoram=n, so the
# boot does not wait for the image to be read into memory), and one entry runs it from memory for anyone who wants the
# stick out.
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
e=$here/profile/efiboot/loader/entries
pass=0; fail=0
check() { if eval "$2"; then echo "PASS $1"; pass=$((pass + 1)); else echo "FAIL $1"; fail=$((fail + 1)); fi; }

linux=$(grep -l 'archisobasedir=' "$e"/*.conf)
bad=; for f in $linux; do [[ $(grep -o 'copytoram=[a-z]*' "$f" | wc -l) == 1 ]] || bad+=" ${f##*/}"; done
check "every system entry says copytoram once" "[[ -z '$bad' ]]"
ram=$(grep -l 'copytoram=y' "$e"/*.conf)
check "exactly one entry runs from memory" "[[ \$(wc -w <<<'$ram') == 1 ]]"
check "its title says so" "grep -q '^title .*run from memory' '$ram'"
default=$(awk '$1 == "default" {print $2}' "$here/profile/efiboot/loader/loader.conf")
check "the default entry exists and starts from the stick" "grep -q 'copytoram=n' '$e/$default'"
check "no two entries share a sort key" "[[ -z \$(grep -h '^sort-key' '$e'/*.conf | sort | uniq -d) ]]"
echo; echo "$pass passed, $fail failed"
((fail == 0))
