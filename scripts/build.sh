#!/usr/bin/env bash
# Builds the Labyrinth OS ISO with mkarchiso. Run as root on Arch Linux (VM, bare metal or
# WSL), after scripts/build-local-packages.sh has filled repo/x86_64/.
#
#   1. Makes the CachyOS keyring and mirrorlists available on this build host.
#   2. Copies profile/ to a work directory, points [labyrinth] at the absolute repo path,
#      and creates the service symlinks (they are not kept in git: Windows checkouts turn
#      symlinks into text files).
#   3. Runs mkarchiso.
#
# Output: out/*.iso. Env: LABYRINTH_WORK_DIR, LABYRINTH_OUT_DIR override work/ and out/.
# LABYRINTH_PRESET=minimal (or gaming, creative; see components.json) builds a lighter medium from the same profile: only that
# preset's components are on it, and the ISO is named labyrinth-PRESET-DATE.iso. The installer still offers the browsers and the AI
# (Sarina, Heimdall); what is not on the medium is fetched during the install or at the first login.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work_dir="${LABYRINTH_WORK_DIR:-$repo_root/work}"
out_dir="${LABYRINTH_OUT_DIR:-$repo_root/out}"
profile_build_dir="$work_dir/profile"

[[ $EUID -eq 0 ]] || { echo "error: run as root (mkarchiso requires it)" >&2; exit 1; }
command -v mkarchiso >/dev/null || { echo "error: install archiso: pacman -S --needed archiso mtools libisoburn squashfs-tools dosfstools" >&2; exit 1; }

echo "==> Checking the generated package list is current"
python3 "$repo_root/scripts/gen-packages.py" --check

echo "==> CachyOS keyring and mirrorlists on the build host"
if [[ ! -f /etc/pacman.d/cachyos-mirrorlist ]]; then
  # CachyOS's signing key, by its full fingerprint (a short id can be forged): the one trusted on the owner's PC, as published at
  # https://cachyos.org. The key is fetched, its fingerprint compared, and only then signed locally; pacman then checks the
  # keyring and mirrorlist packages below against it.
  CACHYOS_FPR=882DCFE48E2051D48E2562ABF3B607488DB35A47
  pacman-key --recv-keys "$CACHYOS_FPR" --keyserver keyserver.ubuntu.com
  pacman-key --finger "$CACHYOS_FPR" 2>/dev/null | tr -d ' ' | grep -q "$CACHYOS_FPR" \
    || { echo "error: the CachyOS key fetched does not have fingerprint $CACHYOS_FPR" >&2; exit 1; }
  pacman-key --lsign-key "$CACHYOS_FPR"
  base=https://mirror.cachyos.org/repo/x86_64/cachyos
  listing=$(curl -fsSL "$base/")
  urls=()
  for pkg in cachyos-keyring cachyos-mirrorlist cachyos-v3-mirrorlist; do
    f=$(grep -oE "${pkg}-[0-9][^\"']*\.pkg\.tar\.zst" <<<"$listing" | grep -v '\.sig$' | sort -V | tail -n1)
    [[ -n $f ]] || { echo "error: could not find $pkg at $base" >&2; exit 1; }
    urls+=("$base/$f")
  done
  pacman -U --noconfirm "${urls[@]}"
else
  echo "    already present"
fi

compgen -G "$repo_root/repo/x86_64/labyrinth.db*" >/dev/null \
  || { echo "error: no local repo at repo/x86_64; run scripts/build-local-packages.sh as a regular user first" >&2; exit 1; }

echo "==> Preparing the work copy of profile/"
rm -rf --one-file-system "${profile_build_dir:?}"
mkdir -p "$work_dir"
cp -r "$repo_root/profile" "$profile_build_dir"
sed -i "s|@@LABYRINTH_REPO_DIR@@|$repo_root/repo/x86_64|g" "$profile_build_dir/pacman.conf"
if [[ -n ${LABYRINTH_PRESET:-} ]]; then
  echo "==> A lighter medium: the $LABYRINTH_PRESET preset"
  python3 "$repo_root/scripts/gen-packages.py" --preset "$LABYRINTH_PRESET" --out "$profile_build_dir/packages.x86_64"
  sed -i "s|^iso_name=\"labyrinth\"|iso_name=\"labyrinth-$LABYRINTH_PRESET\"|" "$profile_build_dir/profiledef.sh"
fi

# Service wiring as symlinks (mkarchiso copies airootfs over the pacstrapped root as-is).
sysd="$profile_build_dir/airootfs/etc/systemd/system"
mkdir -p "$sysd/multi-user.target.wants" "$sysd/bluetooth.target.wants"
ln -sf /usr/lib/systemd/system/graphical.target "$sysd/default.target"
# the login screen: greetd with Yggdrasil's greeter (on the live medium it signs the live user in, see greetd.service.d/live.conf)
ln -sf /usr/lib/systemd/system/greetd.service "$sysd/display-manager.service"
ln -sf /usr/lib/systemd/system/NetworkManager.service "$sysd/multi-user.target.wants/NetworkManager.service"
ln -sf /usr/lib/systemd/system/bluetooth.service "$sysd/bluetooth.target.wants/bluetooth.service"
ln -sf /dev/null "$sysd/systemd-firstboot.service"
ln -sf /etc/systemd/system/pacman-init.service "$sysd/multi-user.target.wants/pacman-init.service"
mkdir -p "$sysd/getty.target.wants"
ln -sf /usr/lib/systemd/system/serial-getty@.service "$sysd/getty.target.wants/serial-getty@ttyS0.service"
# Printing is built in (Settings > Printers): CUPS starts when something prints, Avahi finds the printers on the network
mkdir -p "$sysd/sockets.target.wants"
ln -sf /usr/lib/systemd/system/cups.socket "$sysd/sockets.target.wants/cups.socket"
ln -sf /usr/lib/systemd/system/avahi-daemon.service "$sysd/multi-user.target.wants/avahi-daemon.service"
# Hybrid graphics: lets the desktop start an app on the dedicated GPU (harmless on a computer with one)
ln -sf /usr/lib/systemd/system/switcheroo-control.service "$sysd/multi-user.target.wants/switcheroo-control.service"
# Services the components in the image ask for (components.json "enable_global"), enabled as labyrinth-apps would on an installed system
python3 - "$repo_root/profile/airootfs/usr/share/labyrinth/components.json" "$profile_build_dir/packages.x86_64" <<'PY' | while read -r unit; do
import json, re, sys
comps = json.load(open(sys.argv[1]))["components"]
present = set(re.findall(r"^# component: (\S+)", open(sys.argv[2]).read(), re.M))
for c in comps:
    if c["id"] in present:
        for u in c.get("enable_system", []):
            print("system", u)
        for u in c.get("enable_global", []):
            print("user", u)
PY
  read -r kind name <<<"$unit"
  case "$name" in
    *.socket) want=sockets.target.wants ;;
    *.timer) want=timers.target.wants ;;
    *) want=$([[ $kind == user ]] && echo default.target.wants || echo multi-user.target.wants) ;;
  esac
  if [[ $kind == user ]]; then dir="$profile_build_dir/airootfs/etc/systemd/user/$want"; src="/usr/lib/systemd/user/$name"
  else dir="$sysd/$want"; src="/usr/lib/systemd/system/$name"; fi
  mkdir -p "$dir"
  ln -sf "$src" "$dir/$name"
  echo "    enabled $kind unit $name"
done

# mkarchiso remembers the steps it finished in the work directory and skips them next time, so a second build in the same folder
# would reuse the first build's packages and ignore everything changed since. Every build starts from nothing.
echo "==> Clearing the previous mkarchiso work directory"
rm -rf --one-file-system "${work_dir:?}/iso"

echo "==> Running mkarchiso"
mkdir -p "$out_dir"
mkarchiso -v -w "$work_dir/iso" -o "$out_dir" "$profile_build_dir"
echo "==> Done: $out_dir"
