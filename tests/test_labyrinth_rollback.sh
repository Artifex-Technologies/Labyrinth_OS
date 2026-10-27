#!/usr/bin/env bash
# Tests labyrinth-rollback on a throwaway btrfs image with Labyrinth's layout (@, @home, @/.snapshots/N/snapshot), never on a
# real disk. The image is built without root (mkfs.btrfs --rootdir) and attached with udisksctl; only the tool itself runs as
# root, through $SUDO (default sudo).
#
#   SUDO=sudo bash tests/test_labyrinth_rollback.sh [path/to/labyrinth-rollback]
set -euo pipefail
TOOL=${1:-$(dirname "$0")/../profile/airootfs/usr/local/bin/labyrinth-rollback}
SUDO=${SUDO:-sudo}
W=$(mktemp -d)
LOOP=""
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok - $*"; }
done_() {
  if [[ -n $LOOP ]]; then
    udisksctl unmount --no-user-interaction -b "$LOOP" >/dev/null 2>&1 || true
    udisksctl loop-delete --no-user-interaction -b "$LOOP" >/dev/null 2>&1 || true
  fi
  rm -rf "$W"
}
trap done_ EXIT

info() { printf '<?xml version="1.0"?>\n<snapshot>\n  <type>single</type>\n  <num>%s</num>\n  <date>%s</date>\n  <description>%s</description>\n</snapshot>\n' "$1" "$2" "$3"; }

# the layout: the system now, two snapshots of it, a home
R=$W/root
mkdir -p "$R/@/etc" "$R/@/boot/grub/x86_64-efi" "$R/@home/matt" "$R/@/.snapshots"
echo current >"$R/@/etc/marker"
echo grub-now >"$R/@/boot/grub/x86_64-efi/normal.mod"
echo "Sarina lives here" >"$R/@home/matt/vault.md"
for n in 1 2; do
  mkdir -p "$R/@/.snapshots/$n/snapshot/etc" "$R/@/.snapshots/$n/snapshot/boot/grub/x86_64-efi" "$R/@/.snapshots/$n/snapshot/.snapshots"
  echo "snap$n" >"$R/@/.snapshots/$n/snapshot/etc/marker"
done
echo grub-now >"$R/@/.snapshots/1/snapshot/boot/grub/x86_64-efi/normal.mod"
echo grub-old >"$R/@/.snapshots/2/snapshot/boot/grub/x86_64-efi/normal.mod"   # grub-install ran after snapshot 2
info 1 "2026-10-26 22:00:01" "timeline" >"$R/@/.snapshots/1/info.xml"
info 2 "2026-10-26 23:30:00" "before NVIDIA drivers" >"$R/@/.snapshots/2/info.xml"
truncate -s 400M "$W/disk.img"
mkfs.btrfs -q -L RBTEST --rootdir "$R" -u rw:@ -u rw:@home -u rw:@/.snapshots -u ro:@/.snapshots/1/snapshot -u ro:@/.snapshots/2/snapshot "$W/disk.img" >/dev/null
LOOP=$(udisksctl loop-setup --no-user-interaction -f "$W/disk.img" | grep -o '/dev/loop[0-9]*')
[[ -b $LOOP ]] || fail "no loop device"
udisksctl unmount --no-user-interaction -b "$LOOP" >/dev/null 2>&1 || true   # in case something mounted it

T() { $SUDO "$TOOL" --device "$LOOP" "$@"; }
look() { # mount the top level as the user (udisks) to check the result, then unmount
  local m; m=$(udisksctl mount --no-user-interaction -b "$LOOP" | sed -n 's/.* at \(.*\)$/\1/p'); m=${m%.}
  "$@" "$m"; local rc=$?
  udisksctl unmount --no-user-interaction -b "$LOOP" >/dev/null
  return $rc
}

out=$(T list)
grep -q "before NVIDIA drivers" <<<"$out" && grep -qE '^1 .*timeline' <<<"$out" || fail "list: $out"
pass "list shows both snapshots with their descriptions"

out=$(T 2 --dry-run)
grep -q "Dry run: nothing was changed" <<<"$out" && grep -q "GRUB was reinstalled" <<<"$out" || fail "dry run: $out"
look bash -c '[[ $(cat "$0/@/etc/marker") == current ]] && ! compgen -G "$0/@broken-*" >/dev/null' || fail "dry run changed something"
pass "dry run shows the plan (including the GRUB carry-over) and changes nothing"

if T 2 </dev/null >/dev/null 2>&1; then fail "ran without --yes and without a terminal"; fi
pass "refuses without --yes when there is no terminal"
if T 9 --yes >/dev/null 2>&1; then fail "accepted a snapshot that does not exist"; fi
pass "refuses a snapshot that does not exist"

T 2 --yes >/dev/null || fail "rollback to 2"
check_rolled() {
  local m=$1
  [[ $(cat "$m/@/etc/marker") == snap2 ]] || { echo "marker: $(cat "$m/@/etc/marker")"; return 1; }
  local b; b=$(compgen -G "$m/@broken-*" | head -1); [[ -n $b && $(cat "$b/etc/marker") == current ]] || { echo "broken copy missing"; return 1; }
  [[ -f $m/@/.snapshots/2/info.xml && -d $m/@/.snapshots/1/snapshot ]] || { echo "snapshots not moved"; return 1; }
  [[ ! -e $b/.snapshots ]] || { echo "snapshots left behind"; return 1; }
  [[ $(cat "$m/@/boot/grub/x86_64-efi/normal.mod") == grub-now ]] || { echo "grub not carried"; return 1; }
  [[ $(cat "$m/@home/matt/vault.md") == "Sarina lives here" ]] || { echo "home touched"; return 1; }
  grep -q "from snapshot 2" "$m/@/.labyrinth-rollback" || { echo "no note"; return 1; }
}
look check_rolled || fail "after rollback"
pass "rollback: snapshot 2 is the system, the old one kept, snapshots moved, GRUB carried over, home untouched"

out=$(T status)
grep -q "from snapshot 2" <<<"$out" && grep -q "@broken-" <<<"$out" || fail "status: $out"
pass "status says where the system came from and what is kept"

T undo --yes >/dev/null || fail "undo"
check_undone() {
  local m=$1
  [[ $(cat "$m/@/etc/marker") == current ]] || return 1
  [[ -d $m/@/.snapshots/2/snapshot && ! -e $m/@/.labyrinth-rollback ]] || return 1
  compgen -G "$m/@rolled-back-*" >/dev/null || return 1
  ! compgen -G "$m/@broken-*" >/dev/null
}
look check_undone || fail "after undo"
pass "undo puts the previous system back, keeps the rolled-back one aside"

# a rollback while @ is in use (mounted), as when it is run from the running system
m2=$(udisksctl mount --no-user-interaction -b "$LOOP" -o subvol=@ 2>/dev/null | sed -n 's/.* at \(.*\)$/\1/p') || true
m2=${m2%.}
if [[ -n $m2 ]]; then
  T 1 --yes >/dev/null || fail "rollback while @ is mounted"
  [[ $(cat "$m2/etc/marker") == current ]] || fail "the running system changed under it"
  udisksctl unmount --no-user-interaction -b "$LOOP" >/dev/null
  look bash -c '[[ $(cat "$0/@/etc/marker") == snap1 ]]' || fail "after rollback while mounted"
  pass "rollback works while the system is running from @ (it keeps running; the change applies at the next start)"
else
  echo "skip - could not mount @ through udisks to test a rollback while it is in use"
fi
echo "all passed"
