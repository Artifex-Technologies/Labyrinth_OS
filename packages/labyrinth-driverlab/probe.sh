#!/usr/bin/env bash
# Collect hardware/driver facts as JSON on stdout. Read-only; changes nothing.
# Needs python3 only for JSON string escaping.
set -u

esc() { python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'; }
sec() { printf '"%s": ' "$1"; shift; "$@" 2>&1 | esc; }

iommu_groups() {
  for g in /sys/kernel/iommu_groups/*; do
    [ -d "$g" ] || continue
    echo "group ${g##*/}:"
    for d in "$g"/devices/*; do echo "  $(lspci -nns "${d##*/}")"; done
  done
}
dmesg_issues() { dmesg --level=err,warn 2>/dev/null | tail -200; }
nvidia_info() { nvidia-smi -q 2>/dev/null | head -60; }

echo '{'
sec kernel uname -r; echo ','
sec cmdline cat /proc/cmdline; echo ','
sec pci lspci -nnk; echo ','
sec usb lsusb; echo ','
sec modules lsmod; echo ','
sec dmesg_issues dmesg_issues; echo ','
sec iommu_groups iommu_groups; echo ','
sec nvidia nvidia_info
echo '}'
