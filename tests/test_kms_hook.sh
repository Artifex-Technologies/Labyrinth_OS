#!/usr/bin/env bash
# The start-up image hook labyrinth-kms (profile/airootfs/usr/lib/initcpio/install) and its drop-in: kms's modules without
# the ones blacklisted in modprobe.d, in kms's place in HOOKS.
# shellcheck disable=SC2034,SC2329  # the stand-ins and variables are used by the sourced hook and inside eval
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
pass=0; fail=0
check() { if eval "$2"; then echo "PASS $1"; pass=$((pass + 1)); else echo "FAIL $1"; fail=$((fail + 1)); fi; }

MODS=(
  /m/kernel/drivers/char/agp/intel-agp.ko.zst
  /m/kernel/drivers/gpu/drm/i915/i915.ko.zst
  /m/kernel/drivers/gpu/drm/amd/amdgpu/amdgpu.ko.zst
  /m/kernel/drivers/gpu/drm/nouveau/nouveau.ko.zst
  /m/kernel/drivers/gpu/drm/nova/nova-drm.ko.zst
)

# stand-ins for mkinitcpio's functions: add_checked_modules prints what it would add, filtering as all_modules does (an ERE
# per -f, matched against the module's path)
run() {  # $1 = what modprobe -c prints; $2 = the kernel version
  (
    add_checked_modules() {
      local OPTIND=1 flag f mod
      local -a filters=()
      while getopts ':f:' flag; do [[ $flag == f ]] && filters+=("$OPTARG"); done
      shift $((OPTIND - 1))
      for mod in "${MODS[@]}"; do
        [[ $mod == *"$1"* ]] || continue
        for f in "${filters[@]}"; do [[ $mod =~ $f ]] && continue 2; done
        mod=${mod##*/}; echo "${mod%.ko*}"
      done
    }
    add_checked_modules_from_symbol() { echo "symbol:$1"; }
    modprobe() { [[ $* == -c ]] && printf '%s\n' "$CONF"; }
    CONF=$1 KERNELVERSION=${2:-7.3.5-2-cachyos}
    # shellcheck source=/dev/null
    . "$here/profile/airootfs/usr/lib/initcpio/install/labyrinth-kms"; build
  ) | tr '\n' ' '
}

nvidia='# Blacklist nouveau and nova
blacklist nouveau
blacklist nova_core
blacklist nova_drm
softdep nvidia post: nvidia-uvm nvidia-drm
alias pci:v000010DEd* nouveau'
all='intel-agp i915 amdgpu nouveau nova-drm symbol:drm_privacy_screen_register '

check "NVIDIA's driver installed: nouveau and nova left out, Intel and AMD kept" \
  "[[ \"\$(run \"\$nvidia\")\" == 'intel-agp i915 amdgpu symbol:drm_privacy_screen_register ' ]]"
check "nothing blacklisted: everything kms adds" "[[ \"\$(run '')\" == '$all' ]]"
check "another module's blacklist leaves the graphics drivers alone" "[[ \"\$(run 'blacklist pcspkr')\" == '$all' ]]"
check "a blacklisted name with a dash matches the module either way" \
  "[[ \"\$(run 'blacklist nova-drm')\" != *nova-drm* && \"\$(run 'blacklist i915')\" == *nova-drm* ]]"
check "a kernel before 5.17: no privacy screen modules, as kms" "[[ \"\$(run '' 5.15.0)\" != *symbol* ]]"

conf=$here/profile/airootfs/etc/mkinitcpio.conf.d/zz-labyrinth-kms.conf
check "the drop-in puts labyrinth-kms in kms's place, the boot screen after it" \
  "(HOOKS=(base udev autodetect microcode kms plymouth modconf block); . '$conf'; [[ \"\${HOOKS[*]}\" == 'base udev autodetect microcode labyrinth-kms plymouth modconf block' ]])"
check "the drop-in read twice changes nothing more" \
  "(HOOKS=(base kms filesystems); . '$conf'; . '$conf'; [[ \"\${HOOKS[*]}\" == 'base labyrinth-kms filesystems' ]])"
check "the drop-in leaves the live medium's image alone" \
  "(HOOKS=(base udev archiso kms); . '$conf'; [[ \"\${HOOKS[*]}\" == 'base udev archiso kms' ]])"
check "the drop-in leaves a list without kms alone" "(HOOKS=(base systemd block); . '$conf'; [[ \"\${HOOKS[*]}\" == 'base systemd block' ]])"
check "the drop-in is read after the boot screen's and before the GPU unlock's" \
  "[[ \$(printf '%s\n' yggdrasil-plymouth.conf zz-labyrinth-kms.conf zz-turing-unlock.conf labyrinth-resume.conf | LC_ALL=C sort | tr '\n' ' ') == 'labyrinth-resume.conf yggdrasil-plymouth.conf zz-labyrinth-kms.conf zz-turing-unlock.conf ' ]]"

# labyrinth-kms repeats kms's calls: flag a mkinitcpio whose kms changed (only where mkinitcpio is installed)
kms=/usr/lib/initcpio/install/kms
if [[ -r $kms ]]; then
  check "mkinitcpio's kms still adds what labyrinth-kms repeats" \
    "grep -qxF \"    map add_checked_modules '/drivers/char/agp/' '/drivers/gpu/drm/'\" $kms && grep -qF \"add_checked_modules_from_symbol 'drm_privacy_screen_register' '=drivers/platform'\" $kms && [[ \$(grep -c 'add_' $kms) == 2 ]]"
else
  echo "SKIP mkinitcpio's kms is not installed here"
fi
echo; echo "$pass passed, $fail failed"
((fail == 0))
