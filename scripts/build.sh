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
  # The key id is CachyOS's published signing key. Verify the fingerprint against
  # https://cachyos.org before trusting a build made on a new machine.
  pacman-key --recv-keys F3B607488DB35A47 --keyserver keyserver.ubuntu.com
  pacman-key --lsign-key F3B607488DB35A47
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

# Service wiring as symlinks (mkarchiso copies airootfs over the pacstrapped root as-is).
sysd="$profile_build_dir/airootfs/etc/systemd/system"
mkdir -p "$sysd/multi-user.target.wants" "$sysd/bluetooth.target.wants"
ln -sf /usr/lib/systemd/system/graphical.target "$sysd/default.target"
ln -sf /usr/lib/systemd/system/sddm.service "$sysd/display-manager.service"
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
# User services: drop KWin's stale compiled-script cache before KWin starts (labyrinth-kwin-cache).
usd="$profile_build_dir/airootfs/etc/systemd/user/plasma-workspace.target.wants"
mkdir -p "$usd"
ln -sf /usr/lib/systemd/user/labyrinth-kwin-cache.service "$usd/labyrinth-kwin-cache.service"
# KDE's password agent stays off: Labyrinth's own (labyrinth-polkit, autostarted) answers password requests
mkdir -p "$profile_build_dir/airootfs/etc/systemd/user"
ln -sf /dev/null "$profile_build_dir/airootfs/etc/systemd/user/plasma-polkit-agent.service"

# mkarchiso remembers the steps it finished in the work directory and skips them next time, so a second build in the same folder
# would reuse the first build's packages and ignore everything changed since. Every build starts from nothing.
echo "==> Clearing the previous mkarchiso work directory"
rm -rf --one-file-system "${work_dir:?}/iso"

echo "==> Running mkarchiso"
mkdir -p "$out_dir"
mkarchiso -v -w "$work_dir/iso" -o "$out_dir" "$profile_build_dir"
echo "==> Done: $out_dir"
