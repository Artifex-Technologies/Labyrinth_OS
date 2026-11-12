#!/usr/bin/env bash
# shellcheck disable=SC2034
# Modelled on Chymaera_OS/profile/profiledef.sh.
iso_name="labyrinth"
iso_label="LABYRINTH_$(date --date="@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y%m)"
iso_publisher="Labyrinth OS <https://github.com/Artifex-Technologies/Labyrinth_OS>"
iso_application="Labyrinth OS Live/Install Medium"
iso_version="$(date --date="@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y.%m.%d)"
install_dir="labyrinth"
buildmodes=('iso')
# UEFI only: the target machine boots UEFI.
bootmodes=('uefi.systemd-boot')
pacman_conf="pacman.conf"
airootfs_image_type="squashfs"
airootfs_image_tool_options=('-comp' 'zstd' '-Xcompression-level' '19' '-b' '1M')
file_permissions=(
  ["/etc/shadow"]="0:0:400"
  ["/root"]="0:0:750"
  ["/etc/sudoers.d/00-live-nopasswd"]="0:0:440"
  ["/home/live/"]="1000:1000:700"
  ["/usr/local/bin/labyrinth-sarina-ask"]="0:0:755"
  ["/usr/local/bin/labyrinth-sarina-speak"]="0:0:755"
  ["/usr/share/labyrinth/grub-evodevo/install.sh"]="0:0:755"
  ["/home/live/Desktop/install-labyrinth.desktop"]="1000:1000:755"
)
