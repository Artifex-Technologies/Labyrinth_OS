# GPU unlock: plan for a Settings option that raises a GeForce card's tensor rate safely

Written 2026-10-26 for whoever implements it (Claude Opus in a later session) and for Matt, who decides at each gate.
Everything below was checked against Matt's machine on that date; the "Facts" section says how.

## 1. The decision in one page

**What was asked.** An option in Settings > Power, under the graphics-card controls, that "unlocks a GPU's performance" the way
these three projects do, for gaming and to give Sarina and Heimdall more to work with. Only with Developer options on, only
for NVIDIA cards, and only when the card in the machine is one the method supports. It must not be able to crash the system
or lose anything.

- https://github.com/haohervchb/Turing-unlock
- https://github.com/Zagitta/gpu-unlock
- https://github.com/JustAResearcher/gpu-unlock-ada

**What these projects actually do.** None of them overclocks. Each one flips two "speed selector" settings inside the GPU
(SS0 and SS1) that NVIDIA uses to run some tensor-core instructions at half rate on GeForce cards. The unlock sets them to the
rate of the professional cards. The measured gain is in one narrow kind of work: FP16 or BF16 matrix multiply with FP32
accumulation (about 2x in a microbenchmark, 1.77x in cuBLAS on the 2080 Ti). The Turing author is explicit: "Do not interpret
these results as a 2x improvement in all compute, games, AI workloads or mining." Mining gained nothing. Games are not
measured in any of the three, and games barely use this instruction, so expect nothing there.

**What applies to Matt's card.** Matt's card is a Turing chip (RTX 2080 Ti, TU102). Two of the three projects do not cover
Turing at all: gpu-unlock is Ampere (RTX 30), gpu-unlock-ada is Ada (RTX 40) plus the Ampere list; gpu-unlock has open issues
asking for Turing (#30, #97) with no implementation. Only Turing-unlock targets TU102, and it is the least mature of the
three: a research archive of **one successful run on 2026-10-22**, on an ASUS 2080 Ti with a different VBIOS, in a Dell
Alienware with a different motherboard, started from a UEFI program that runs before the operating system. Its README says it
"is not a general-purpose installer or VBIOS mod", publishes no installer or boot image, does not redistribute the firmware
inputs it needs, and its identity guards refuse any card that is not the exact tested one. Matt's card fails those guards four
ways (section 3).

**Therefore, the plan has gates.** Phase 0 builds the Settings option itself, with the support check and all the safety
plumbing. On a card that is not supported, which includes Matt's today, **the option does not appear at all** (Matt's
decision, 2026-10-26); the helper still records why, for Opus and Heimdall. Nothing touches the GPU. Phases 1 and 2 research
and dry-run without writing to the GPU. A live attempt (Phase 3) happens only if Phase 1 produces evidence that the method
applies to an MSI 2080 Ti with VBIOS 90.02.17.00.3E, only with Matt at the keyboard, and only after the measurements in Phase
4 are planned so we know whether it was worth it. If it works, Phase 5 reports the findings to Turing-unlock. It is entirely
possible, and acceptable, that the plan stops after Phase 1 with the option never appearing on this computer. That outcome
still delivers what was asked: a correct, gated feature that appears for a card the method is proven on.

**Expected benefit, honestly.**

| Who | Effect of the unlock | Why |
| --- | --- | --- |
| Games | None measurable | They use shaders and the FP32 path; the unlocked instruction is tensor FP16->FP32 accumulate |
| Sarina speaking (token generation) | None | qwen3:8b Q4_K_M generation is limited by memory bandwidth, not by tensor rate |
| Sarina reading a long prompt (prefill), embeddings, Whisper | Possibly faster; unmeasured | Prefill is compute-bound. llama.cpp on Turing uses int8 tensor MMA for quantised weights; the Turing author says the integer path (IMLA4) "has not received a dedicated integer-throughput A/B test" |
| Heimdall | None | It is a CPU process at 0.3% CPU; it does not use the GPU |
| Voice (chatterbox ONNX, fp16) | Possibly faster; unmeasured | Transformer inference on tensor cores |

For gaming, the lever that exists today is the per-mode GPU power limit already in Settings > Power (Developer options).
Performance mode may use the card's full 290 W (Matt's decision, 2026-10-26); section 6 checks that against the 750 W supply.

## 2. Facts about this machine (checked 2026-10-26)

| Item | Value | How it was read |
| --- | --- | --- |
| GPU | NVIDIA GeForce RTX 2080 Ti Rev. A, TU102, 68 SMs, 11 GiB | `lspci -nn`, `nvidia-smi` |
| PCI identity | vendor:device `10de:1e07`, subsystem `1462:3711` (MSI) | `lspci -nn -vv -s 01:00.0` |
| Bus address (BDF) | `0000:01:00.0` | same |
| VBIOS | `90.02.17.00.3E` | `nvidia-smi --query-gpu=vbios_version` |
| BAR0 / BAR1 | `0xde000000` (16 MiB) / `0xc0000000` (256 MiB; a 256 MiB BAR1 means Resizable BAR is off) | `lspci -vv` |
| BIOS settings | Above 4G Decoding and CSM: **not known yet**. The only Turing run had Above 4G off and CSM on; Phase 1 reads them | Matt, in the BIOS setup |
| Driver | NVIDIA **open** kernel modules 615.78.08 (`nvidia-utils 615.78.08-2`, `linux-cachyos-nvidia-open 7.3.0-4`) | `/proc/driver/nvidia/version`, `pacman -Q` |
| Module options | `NVreg_PreserveVideoMemoryAllocations=1`; `NVreg_OpenRmEnableUnsupportedGpus=1` is in effect | `/etc/modprobe.d`, `/sys/module/nvidia/parameters` |
| Kernels | `linux-cachyos 7.3.0-4` (default), `linux-cachyos-lts 6.18.55-2` (spare) | `pacman -Q` |
| Power limits | default 260 W, min 130 W, max 290 W; idle draw about 40 W | `nvidia-smi -q -d POWER` |
| Boot | UEFI, GRUB 2.16 at `\EFI\LABYRINTH\GRUBX64.EFI`, `GRUB_DEFAULT=0`, timeout 2 s, `grub-reboot` and `grub-set-default` installed | `efibootmgr`, `/etc/default/grub` |
| ESP | partition GUID `6bb1790a-df16-434b-8e1e-486fd65418c2`, filesystem UUID `BD65-5893`, mounted at `/boot/efi` | `efibootmgr`, `/etc/fstab` |
| Secure Boot | **disabled** | `bootctl status` |
| Display | The 2080 Ti drives the display. The i9-9900K's integrated GPU is **off** in the BIOS (`lspci` lists no Intel VGA) | `lspci` |
| CPU / board | Intel i9-9900K, MSI Z370I (MS-7B43), BIOS 2.QJ | `/sys/class/dmi` |
| PSU | 750 W (Matt) | |
| Snapshots | snapper config `root` (subvolume `@` only), hourly timeline; grub-btrfsd on; boot menu has "Labyrinth OS snapshots" | `snapper`, `systemctl` |
| Sarina's model | Ollama `qwen3:8b` Q4_K_M (199 q4_K, 19 q6_K, 36 f16, 145 f32 tensors), `OLLAMA_FLASH_ATTENTION=1`, `OLLAMA_KV_CACHE_TYPE=q8_0`, context 16384 | `ollama show`, `systemctl cat ollama` |
| Heimdall | `heimdalld` at 0.3% CPU; no GPU use | `ps` |

Two of these matter more than the rest. **The 2080 Ti is the display card**, and both kernel-module projects say the card goes
blank from its reset until `nvidia-drm` takes over and that one must not "run a GPU reset/unbind against the active Linux
display card" from inside Linux; that is why every working method runs *before* the operating system. **Secure Boot is off**,
which is what a pre-OS unsigned EFI program or an unsigned kernel module needs; if Matt ever turns Secure Boot on, the feature
must refuse to arm (section 6).

## 3. The three projects, side by side

| | Turing-unlock (haohervchb) | gpu-unlock (Zagitta) | gpu-unlock-ada (JustAResearcher) |
| --- | --- | --- | --- |
| Chips | TU102 only: one ASUS RTX 2080 Ti | GA102/GA103/GA104/GA106 (RTX 30) | AD102 (RTX 4090) first; 16 other Ada IDs "experimental"; the Ampere list kept |
| Applies to Matt's TU102 | Only this one | No (issues #30, #97 open) | No |
| Form | UEFI program run once before the OS from a one-shot GRUB entry; no installer, no prebuilt image | Linux kernel module (DKMS) that runs before the NVIDIA driver binds, `gpu_unlock.targets=` on the kernel command line, re-applies after suspend | Same design as gpu-unlock, for Ada |
| What it writes | GPU runtime state through signed NVIDIA firmware ("RM GET controls": FMLA32 and IMLA4 to full rate). "No GPU VBIOS or motherboard BIOS was flashed" | SS0 `0x88888888`, SS1 `0x8`, then a PCIe function-level reset, through a locally generated firmware buffer | Same, with a payload from TOTORONG's AD103 unlocker |
| Persists? | No: "A subsequent normal restart restored half-rate". Automatic re-application "not implemented or enabled" | Yes while the module is on the command line; a cold boot is needed the first time | Same |
| Driver tested | open 580.178.04 only | pins 610.43.03 to extract firmware; driver version otherwise free | 580 to 615, open modules |
| Inputs | FWSEC "extracted from the test card's own ROM", four blobs from `xrip/cmp50hx-unlock` at a pinned commit, an "audited Dell A14 root-bridge PE image". `inputs_redistributed: false` | NVIDIA driver package (fetched) | NVIDIA driver package (fetched) |
| Identity guards | BDF `02:00.0`, `10de:1e07`, subsystem `1043:866a`, BAR0 `fa000000`, PCI command `0007`, parent bridge, firmware descriptors, "a finite set of observed selector tuples"; "Unknown pre-write states refuse the attempt" | PCI device ID list; refuses bound cards | Same, with an opt-in flag per experimental ID |
| Failure handling | "A failed, possibly partial GPU mutation holds the machine in UEFI with DMA backing retained: a full power cycle is required" | "If card firmware hangs, the module logs it and keeps the DMA buffers ... Power off before trying again" | Same; "Do not force-unload it in this state" |
| Measured gain | FP16->FP32 MMA 46.9 -> 93.7 TFLOPS; cuBLAS 8192 GEMM 48.2 -> 85.2; FP16->FP16 unchanged; mining unchanged | 3090 Ti: FP16 1.35x, BF16 1.80x, TF32 1.94x | 4090: FP16->FP32 2x in registers; real cuBLAS +12-34%; a quantised MoE serving workload 0.98x |
| Licence | copyleft-next 0.3.1 with MIT notices | GPL-2.0-only; firmware under NVIDIA terms | GPL-2.0 |
| Maturity (2026-10-26) | 94 stars, created and last pushed 2026-10-22, one run | 10,056 stars, active since 2026-10-18 | active |

**Where Matt's card differs from the only tested Turing card** (each one alone makes Turing-unlock's guards refuse):

| Guard | Tested card | Matt's card |
| --- | --- | --- |
| Subsystem | `1043:866a` (ASUS) | `1462:3711` (MSI) |
| VBIOS | `90.02.17.00.47` | `90.02.17.00.3E` |
| BDF | `0000:02:00.0` | `0000:01:00.0` |
| BAR0 | `0xfa000000` | `0xde000000` |

The guards are not red tape: the payload addresses come from the firmware (FWSEC) of that specific ROM, and the loader reads
"a finite set of observed selector tuples" from that card before writing. Matching Matt's card means obtaining the FWSEC from
*his* ROM, reproducing the author's analysis for it, and proving the pre-write state is one the loader knows. That is Phase
1, and it may conclude "not feasible without the author".

The common ancestor is `xrip/cmp50hx-unlock`: a UEFI program (plus patched kernel modules for other features) for the CMP
50HX mining card, which is also TU102. Its compute unlock "needs no patched kernel module at all" and "chainloads the Linux
boot manager" afterwards. It is the closest thing to a reusable TU102 unlock, but it targets `10de:1e09` (the CMP), not
`10de:1e07`, and it notes that a pre-OS PCIe retrain "hung some Intel X99/X299 boards": pre-OS GPU work does hang some
machines, which is why section 6 treats "hangs before the OS" as the expected failure and designs for it.

## 4. What can go wrong, and what cannot

**Cannot happen, by the nature of the method**

- **Data loss.** Nothing writes to the disk, the VBIOS or the motherboard firmware. Matt's files, Sarina (home folder), the
  repos and the system are untouched by the unlock itself. The only files the *feature* writes are its own: a GRUB entry, a
  state file, logs.
- **A persistent bad state in the GPU.** The selectors return to half rate on every normal restart (volatile); a power cycle
  clears everything, including a hung firmware transaction.

**Can happen, and must be designed for**

| Failure | Likelihood | Consequence | Defence (section 6) |
| --- | --- | --- | --- |
| The pre-OS loader hangs or refuses, and the machine sits in UEFI | Real: the author requires a power cycle for exactly this | Black screen until Matt holds the power button. No damage. The one-shot entry is consumed, so the next boot is normal | One-shot boot only; "armed" marker that disarms on the next boot; Matt present; USB stick ready |
| The unlock "succeeds" but the GPU computes wrong results | Possible on an untested board | Garbage output from Sarina or the voice, artefacts in games | Verifier compares a tensor benchmark's outputs to a CPU reference (the author's nonzero validation did 295,035 outputs), and disarms on any mismatch |
| Higher power or heat at the same clocks | Expected, small: "It may heat up your GPU slightly because it now does more compute" but "throttling and power management remains completely intact" | The card's 260 W cap and thermal limits still apply | Power limit forced to the default or lower while unlocked; temperature watch for the first run |
| The display never comes back after the loader | Possible (blank until `nvidia-drm` binds) | Looks like a hang | Treat as a hang: power cycle. Also a text login (Ctrl+Alt+F3) and the LTS kernel entry as fallbacks |
| Suspend/resume loses the unlock | Certain (suspend powers the card off) | Half rate until the next armed boot | The feature never re-applies by itself; the status tile simply shows "Locked" |
| The firmware inputs or the driver change (an `nvidia-utils` upgrade) | Certain over time | A payload built for one firmware set may refuse or misbehave | The support table pins driver and VBIOS; a mismatch greys the tile out until re-validated |
| Heimdall sees changes to `/boot/efi` and GRUB | Certain | A critical alert, or (in enforce mode) a refusal | The helper's files are added to Heimdall's trust list and signed by the owner's step, as for other owner-approved changes |
| Sarina or an agent arms the unlock | Must be impossible | A reboot into a risky path without Matt | The helper is root-only through polkit `auth_admin` (a password every time); it is not a Sarina tool; Sarina's verify hook and exfil guard do not grant it |

## 5. The feature: what Matt sees

Location: **Settings > Power**, in a new group **"Graphics card"** that appears only while Developer options are on, only
when an NVIDIA card is present, and only when the `powerlimit` tool already reports it (so the existing per-mode watt slider
and this new group are the one place for GPU adjustments).

**On a card that is not supported, there is no unlock row at all** (Matt, 2026-10-26: "If the card is not supported, then the
option should not be available"). The group then holds only the power slider, exactly as today. No greyed row, no "not
supported" text; the reason is kept in `gpuunlock status --json` and the log, for Opus and Heimdall.

**Row 1: "Tensor-rate unlock"**, shown only on a supported card, with one of these states, decided by the helper (section 7)
from a support table:

| State | Text under the row | Control |
| --- | --- | --- |
| Supported, not validated here | "This card matches a tested one, but the unlock has not been run on this computer. Experimental." | "Arm for next restart..." (asks for the password, then confirms in a dialog that names every risk in one screen and shows the date of the last Proton Drive backup of Sarina) |
| Secure Boot on | "Needs Secure Boot off: the unlock runs a boot program that is not signed." | None |
| Armed | "The next restart will try the unlock once, then boot normally. If the screen stays black for more than a minute, hold the power button, start again; the computer boots normally and this switch turns itself off." | "Cancel" |
| Unlocked (this session) | "Unlocked since 09:12. FP16->FP32 tensor rate 2.0x of locked (checked). A restart or sleep locks it again." | "Benchmark", and "Do not try again" |

A fifth line appears after a failed attempt: "The last attempt (26 Oct 09:10) did not reach the desktop. The unlock is off.
Open the log." The tile never offers "always on". Persistence is deliberately out of scope until Phase 4 has evidence, for the
reason the Turing author gives: a permanent GRUB default "can loop".

**Row 2: the existing power-limit slider** moves into this group, unchanged: Performance mode may go to the card's maximum,
290 W. One rule is new: while the unlock is armed, and during the first unlocked session and the Phase 4 measurements, the
slider's maximum is the card's default (260 W) and the text says why. After Phase 4 is recorded, the full range returns.

Not shown anywhere: driver versions, register names, BAR addresses. Those go in the log and the `status --json` output for
Opus and Heimdall.

## 6. Safety rules (each one is a test in Phase 0)

1. **Identity match or nothing.** The helper reads `lspci -nn -D`, the VBIOS from `nvidia-smi`, BAR0 from `lspci -vv`, and
   `/proc/driver/nvidia/version`. The support table entry must match vendor:device, subsystem, VBIOS and driver series
   exactly. BDF and BAR0 are recorded into the generated boot entry at arm time and re-checked by the loader. Any
   difference: state "Not supported", no arm button.
2. **One shot, never default.** Arming runs `grub-reboot "<entry id>"` (which sets GRUB's `next_entry` once) for an entry
   generated by `/etc/grub.d/41_gpu_unlock`. `GRUB_DEFAULT` is never changed, `/etc/default/grub` is never edited. The entry
   chainloads the loader, and the loader's success path chainloads the normal GRUB path; it must not re-enter the menu with
   `next_entry` still set (the author's loop warning). The helper verifies after `grub-mkconfig` that the normal entry is still
   entry 0.
3. **Armed marker before, cleared after.** Before `grub-reboot`, the helper writes `/var/lib/yggdrasil/gpu-unlock/armed`
   with the time and the entry id. A systemd unit `yggdrasil-gpu-unlock-verify.service` (after `nvidia-persistenced` or on
   the first `nvidia-smi` success) removes it when the desktop session comes up. At boot, if the marker is **already there**
   and its boot id is not this boot, the previous attempt did not reach the desktop: the helper sets state `failed`, disables
   the arm button until Matt re-enables it, and logs. This is the "failure marker written before any GPU change, so a hang or
   power loss causes the next boot to skip the experiment" that the Turing author lists as a requirement for any automatic
   use.
4. **Snapshot first.** Arming takes a snapper snapshot "before GPU unlock" (same as `before-nvidia.sh` in Proton Drive >
   Labyrinth OS). The unlock does not touch the system, but the surrounding package work (a DKMS module, an EFI loader on the
   ESP) might, and the snapshot makes that reversible by the guide already written for Matt (`GPU driver recovery guide.md`).
   Snapshots start as a normal desktop when chosen in the boot menu, because grub-btrfs' overlay hook is in the start-up
   image (drop-in `/etc/mkinitcpio.conf.d/labyrinth-snapshot-overlay.conf`, added to the installer and to `before-nvidia.sh`
   on 2026-10-26). Only snapshots taken after the drop-in is in place have it, since each snapshot carries its own `/boot`.
5. **Never from inside Linux.** The helper never unbinds, resets or rebinds the NVIDIA driver on the display card. All GPU
   writes happen before the kernel (EFI loader) or before the driver binds (kernel module on the command line, if a Turing
   port ever exists). Loading such a module after boot "has no effect" (gpu-unlock) and is not attempted.
6. **Power budget.** The unlock does not raise the card's power cap. The 750 W budget: i9-9900K up to about 200 W under
   all-core AVX (this board does not limit PL2), the 2080 Ti at its limit with transient spikes to roughly 1.4x for
   milliseconds, drives, fans, the Corsair controller, USB and RGB about 60-80 W. At 260 W: about 540 W sustained, transients
   near 650 W. At 290 W: about 570 W sustained, transients near 700 W. Both are inside 750 W, so **Performance mode may use
   290 W** (Matt, 2026-10-26). The limit caps the whole card, whatever runs on it, so a game and Sarina's model together still
   draw no more than the limit; an earlier draft's "not while a model is loaded" rule was dropped for that reason. The only
   extra rule: while the unlock is armed, and during the first unlocked session and the Phase 4 measurements, every mode is
   held at the default 260 W, so the first run has margin and the locked and unlocked numbers are taken at the same limit.
7. **Thermal watch.** For 15 minutes after a successful unlock the verifier samples `nvidia-smi` every 10 s; if the GPU
   passes 85 °C or reports `HW Slowdown: Active`, it logs, sets the power limit to 200 W for the session, and marks the state
   "throttled" so the tile says so.
8. **Correctness check before "Unlocked".** The verifier runs a small CUDA (or cuBLAS) FP16->FP32 GEMM against a CPU
   reference on a few thousand outputs and the tensor microbenchmark (ported from Turing-unlock's `tools/tensor_baseline.cu`,
   guarded to this card). "Unlocked" is shown only if the outputs match and the rate is at least 1.5x the locked baseline
   recorded in Phase 2. Otherwise the state is "failed, wrong results" and the arm button is disabled.
9. **Secure Boot.** If `bootctl status` reports Secure Boot enabled, the helper refuses to arm ("needs an unsigned boot
   program; Secure Boot is on") rather than installing a signing hook.
10. **Heimdall.** The ESP loader file, `/etc/grub.d/41_gpu_unlock` and the support table are on Heimdall's watch list and
    signed when installed; the verifier writes an `info` line to Heimdall's alert log on arm, on success and on failure, so
    the Monitor page shows what happened. Heimdall's `enforce` mode is respected: if it refuses the loader, the tile says so.
11. **Nobody but Matt.** The helper is run through polkit with `auth_admin` (not `auth_admin_keep`): a password per action.
    It is not listed in any Sarina tool, skill or `LABYRINTH_VOICE_EXTRA_TOOLS`. The `claude-agent` account's sudo rules do
    not include it. Opus must not add it to `claude-sudo`.
12. **A spare way in.** The plan assumes the LTS kernel entry and a text login remain available, and that the Labyrinth USB
    stick is at hand for the first live attempt. The verifier's failure path never removes the normal entry.

## 7. Implementation (for Opus)

All code lives in the Yggdrasil repo (desktop), with the plan here in Labyrinth. Follow the existing `powerlimit` helper as
the pattern: a Python helper in `packages/settings/helpers/`, a polkit policy in `packages/settings/data/`, parsing in
`packages/settings/src/sys.rs`, the rows in `packages/settings/src/main.rs` next to the per-mode limits (search for
`"mode limits"`), and `packages/pkgbuilds/yggdrasil-settings/PKGBUILD` installs it.

### 7.1 Files

| File | Purpose |
| --- | --- |
| `packages/settings/helpers/gpuunlock` | The helper. Commands: `status [--json]`, `arm`, `disarm`, `verify` (run by the service), `bench`, `reenable` (after a failure). Python, no third-party modules, same style as `powerlimit` |
| `packages/settings/data/gpu-unlock-supported.json` | The support table (7.2). Installed to `/usr/share/yggdrasil/gpu-unlock/supported.json` |
| `packages/settings/data/org.yggdrasil.gpuunlock.policy` | polkit: action `org.yggdrasil.gpuunlock.arm` with `auth_admin` for `arm`, `disarm`, `reenable`; `status` and `bench` need no root |
| `packages/settings/data/yggdrasil-gpu-unlock-verify.service` | `Type=oneshot`, `After=graphical.target nvidia-persistenced.service`, runs `gpuunlock verify` |
| `packages/settings/data/41_gpu_unlock` | `/etc/grub.d/41_gpu_unlock`: emits the one-shot menu entry only if `/var/lib/yggdrasil/gpu-unlock/loader.conf` exists (written by `arm`); otherwise nothing, so `grub-mkconfig` on a normal machine is unchanged |
| `packages/settings/src/sys.rs` | `parse_gpu_unlock(json) -> Option<GpuUnlock>` with state, reason, since, ratio, log path |
| `packages/settings/src/main.rs` | The "Graphics card" group: state text, buttons, the confirmation dialog, the slider cap |
| `packages/settings/helpers/tests/test_gpuunlock.py` | Tests for every rule in section 6 that can be tested without a GPU (identity matching, marker logic, power cap, Secure Boot refusal, grub entry generation) |

State directory: `/var/lib/yggdrasil/gpu-unlock/` (root-owned, 0755, files 0644 except `armed`): `state.json`, `armed`,
`baseline.json` (locked benchmark numbers), `log` (append-only, one line per event with the boot id).

### 7.2 The support table

```json
{
  "entries": [
    {
      "method": "turing-unlock-efi",
      "device": "10de:1e07",
      "subsystem": "1043:866a",
      "vbios": "90.02.17.00.47",
      "driver_series": "open",
      "validated_on": "2026-10-22",
      "evidence": "https://github.com/haohervchb/Turing-unlock (one run; README results/2080ti-v4-comparison.json)",
      "status": "experimental",
      "loader": "tu102_experiment_v4.efi",
      "notes": "Author's ASUS card. Firmware inputs are not redistributed; the loader must be built locally from the owner's own ROM."
    }
  ]
}
```

Matching is exact on `device`, `subsystem`, `vbios` and `driver_series`. There is no wildcard and Opus must not add one. A
new entry is added only with an `evidence` link to a run on that exact board, and `status` starts at `experimental`. Matt's
MSI card (`1462:3711`, `...3E`) gets an entry only when Phase 1 produces the evidence (section 8).

### 7.3 The helper's `status` output

```json
{"gpu": {"name": "NVIDIA GeForce RTX 2080 Ti", "device": "10de:1e07", "subsystem": "1462:3711", "vbios": "90.02.17.00.3E",
         "bdf": "0000:01:00.0", "bar0": "0xde000000", "driver": "615.78.08", "driver_series": "open"},
 "secure_boot": false,
 "state": "unsupported",
 "reason": "No tested entry for MSI 1462:3711 with VBIOS 90.02.17.00.3E (tested: ASUS 1043:866a, VBIOS 90.02.17.00.47)",
 "last_attempt": null, "since": null, "ratio": null, "log": "/var/lib/yggdrasil/gpu-unlock/log"}
```

`state` is one of `unsupported`, `supported`, `armed`, `unlocked`, `failed`, `throttled`, `secure-boot`, `no-nvidia`.
The Settings tile is a pure function of this JSON.

### 7.4 Arm sequence (`gpuunlock arm`, root)

1. Refuse unless `state == supported` (re-evaluated now, not cached) and Secure Boot is off.
2. Refuse if `ollama ps` shows a resident model or `nvidia-smi` shows any compute process: the first run is done on an idle
   card, and Sarina is stopped by Matt beforehand (`systemctl --user stop labyrinth-voiced; systemctl stop ollama` are
   *suggested in the dialog*, not run by the helper).
3. `snapper -c root create --type single --cleanup-algorithm "" --description "before GPU unlock"`.
4. Write `loader.conf` (BDF, BAR0, ESP GUID, loader file name) and run `grub-mkconfig -o /boot/grub/grub.cfg`; assert entry
   0 is unchanged and the new entry id exists; otherwise roll `loader.conf` back and fail.
5. Hold the GPU power limit at the card's default for every mode (`powerlimit set MODE gpu=260` where higher). The previous
   values go into `state.json`; `disarm` restores them, and so does recording Phase 4 (Performance mode's 290 W returns).
6. Write `armed` (time, entry id, current boot id), then `grub-reboot <entry id>`.
7. Log, and return the text for the tile. The helper does **not** reboot; Matt restarts from the menu when ready.

`disarm` removes `armed`, runs `grub-reboot` with the normal entry (which clears `next_entry`), and restores the power caps.

### 7.5 Verify sequence (`gpuunlock verify`, the service, at every boot)

1. If `armed` exists and its boot id is not this boot's: previous attempt failed to reach the desktop. State `failed`,
   remove `armed`, Heimdall `critical` info line, done.
2. If `armed` exists and is this boot's: remove it, then test. Read the selector state the way the Turing author does (the
   microbenchmark ratio against `baseline.json`; there is no NVML field for it). Run the correctness check. Set `unlocked`
   (with ratio and time) or `failed` (wrong results / no speed-up). Start the 15-minute thermal watch in the background.
3. If nothing is armed: if `state` was `unlocked` from a previous boot, set it back to `supported` (volatile), done.

### 7.6 Settings UI

In `page_power`, after the idle rows and before the per-mode limits, when `p.limits.is_some()`: a
`ui::group(ui, pal, Some("Graphics card"), ...)` holding the existing GPU slider and, only when the helper's state is
anything but `unsupported` or `no-nvidia`, a `ui::row` for the unlock (the state sentence in section 5) with a `ui::quiet`
button whose label depends on the state. An unsupported card draws no unlock row. For tests and screenshots, a debug build
(`cfg!(debug_assertions)`) reads the status JSON from `$YGGDRASIL_GPUUNLOCK_FIXTURE` instead of running the helper; release
builds ignore it, and `arm` always re-checks everything as root. The confirmation
dialog is an `egui::Window` like "Turn on Developer options?" with a checkbox "I have read this and I am at the computer" that
must be ticked before "Arm" enables. Actions go through `sys::act("pkexec", &["/usr/lib/yggdrasil/gpuunlock", "arm"])`.

### 7.7 Tests Opus must write before any GPU work

- Identity matching: Matt's card against the table -> `unsupported` with the exact reason text; the ASUS tuple -> `supported`;
  a wildcard in the table -> the helper refuses to load the table.
- Marker logic: `armed` from another boot id -> `failed`; same boot id -> verify path; none -> idle path.
- Secure Boot true -> `secure-boot`, `arm` exits 2 without writing anything.
- Power cap: `arm` with performance at 290 W results in 260 W for every mode; `disarm` restores the saved values.
- GRUB: `41_gpu_unlock` prints nothing without `loader.conf`, and exactly one `menuentry` with it; `arm` aborts if entry 0 changes.
- `status --json` validates against the schema in 7.3 for every state.
- Settings: `parse_gpu_unlock` for each state; no unlock row is drawn for `unsupported` or `no-nvidia` (the group shows only
  the slider); the `status` reason for `unsupported` names the card's own subsystem and VBIOS (for the log).
- Power: Performance mode accepts 290 W when nothing is armed; `arm` holds it at 260 W; recording Phase 4 restores 290 W.

## 8. Phases and gates

Each phase ends with a written result in this file (append a dated "Result" subsection) and Matt's go/no-go. Opus does not
start a phase whose predecessor has no result.

### Phase 0: the gated feature, with no GPU change

Everything in section 7 except a loader. On this machine no unlock row appears; `gpuunlock status --json` records why. Deliver:
the helper, table, policy, service, GRUB fragment, Settings rows, tests, PKGBUILD `pkgrel` bump, `docs/YGGDRASIL.md` line. The
Heimdall watch-list addition is a line in Heimdall's `install.sh` (Heimdall owns security; see its repo).

Gate: tests pass; `gpuunlock status --json` on Matt's PC says `unsupported` with the MSI/VBIOS reason; Settings shows the
"Graphics card" group with only the power slider (no unlock row) while Developer options are on, and nothing new while they
are off; a debug build with a `supported` fixture shows the row, for a screenshot; Heimdall shows no new alerts after a
reboot. Nothing was written to the GPU.

### Phase 1: research, no GPU writes (read-only on the card)

1. Dump Matt's own VBIOS, read-only (the kernel's PCI ROM interface; nothing is written to the card):
   `echo 1 | sudo tee /sys/bus/pci/devices/0000:01:00.0/rom`, then
   `sudo cat /sys/bus/pci/devices/0000:01:00.0/rom > rom.bin`, then `echo 0 | sudo tee /sys/bus/pci/devices/0000:01:00.0/rom`.
   Keep the dump on Proton Drive (Labyrinth OS/gpu-unlock/), not in a public repo.
2. Run Turing-unlock's host tests (`bash tools/test_preflight.sh`, no GPU) and `tools/analyze_prom_capture.py` and
   `tools/tu102_port_profile.py` against the dump, to learn whether the author's port profile recognises this ROM's layout
   and FWSEC.
3. Read `xrip/cmp50hx-unlock/efi-unlock` to see how the identity table is structured there and whether a GeForce `1e07` with
   an MSI subsystem has ever been listed by anyone.
4. Watch gpu-unlock issues #30 and #97 (Turing). A kernel-module Turing port from that project, if it appears, is the
   preferred method over the EFI loader, because it has a tested failure path, suspend handling and a Debian/Arch installer.
5. Ask Matt to read two BIOS settings and note them here: Above 4G Decoding and CSM (the only Turing run had Above 4G off and
   CSM on). Do not change them for this.
6. Write the result: feasible with the current public material, feasible only with the author's help, or not feasible.
7. If "only with the author's help": open an issue on Turing-unlock (Matt agreed, 2026-10-26) with the card's identity, the
   board and BIOS settings, the Phase 1 analysis output and, if Phase 2's probe could be built, the probe log. Draft it in
   `docs/gpu-unlock-upstream.md` following the rules in Phase 5, show Matt the exact text, and post it only after he says yes,
   from his account: `gh issue create --repo haohervchb/Turing-unlock --title "..." --body-file docs/gpu-unlock-upstream.md`.
   (On 2026-10-26 the repo had issues enabled and none opened yet.)

Gate: a result section here. If "not feasible", the plan stops; the feature stays at Phase 0, which is a correct, shippable
state. No GPU register was written in this phase.

### Phase 2: dry run (probe only)

Only if Phase 1 says feasible, or says the author's help is needed and the probe can still be built (it writes nothing to
the GPU; its log is what the author would need). Build the EFI preflight in **probe mode** (Turing-unlock's `tu102_preflight.c` and
`tu102_prom_capture.h` read descriptors and capture state; the payload write is a separate step and must be compiled out).
Boot it once through the one-shot entry with Matt present. It must log the identity, BAR0, PCI command, firmware descriptors
and the selector tuple, write nothing, and chainload the normal boot.

Record the locked baseline: `gpuunlock bench` writes `baseline.json` (tensor microbenchmark, cuBLAS GEMM, and the
application numbers in section 9).

Gate: the probe log shows a selector tuple that Turing-unlock's loader lists as a known pre-write state, and the machine
chainloaded normally. If the tuple is unknown, stop: "Unknown pre-write states refuse the attempt" is the author's rule and ours.

### Phase 3: first live attempt

Conditions, all required: Matt at the keyboard; Sarina stopped (`labyrinth-voiced`, `ollama`); no game or compute running;
the snapshot from `arm`; the Labyrinth USB stick plugged in but not selected; the card at the default power limit; a **cold
boot** (power off, wait ten seconds, power on), not a warm restart, as both kernel-module projects require for a first run.
Matt arms it in Settings, restarts, and waits up to two minutes at a black screen before holding the power button.

Gate: the verifier reports `unlocked` with the correctness check passed and ratio >= 1.5. Anything else sets `failed` and the
plan returns to Phase 1 with the log.

### Phase 4: is it worth keeping?

Run the section 9 measurements unlocked and compare with `baseline.json`. Decide per workload. Only if Sarina's prefill or
the voice gain at least 20% does the "Experimental" state stay enabled for Matt's entry; otherwise the entry's `status`
becomes `"measured-no-benefit"` and the tile says "Tested on 2026-..: no useful gain on this computer", with the arm button
hidden. Persistence (re-arming automatically at every boot) is a separate decision after at least ten clean armed boots, and
needs the author's five requirements (BOOT-STATUS.md) met, including the same-boot guard. Recording this phase lifts the
260 W hold: Performance mode's 290 W applies with the unlock too.

### Phase 5: report the findings to Turing-unlock (Matt agreed, 2026-10-26)

Only after Phase 3 succeeded and Phase 4 is recorded. The author published one run on one card, so a second board with a
different VBIOS is exactly the evidence the project lacks, and a negative application result (no gain in games) is as useful
as a positive one.

**What goes in:**
- Hardware: RTX 2080 Ti `10de:1e07`, subsystem `1462:3711` (MSI), VBIOS `90.02.17.00.3E`, 68 SMs, 11 GiB; BDF `0000:01:00.0`;
  MSI Z370I (MS-7B43) BIOS 2.QJ, i9-9900K; Above 4G and CSM as read in Phase 1; Resizable BAR off; Secure Boot off.
- Software: CachyOS-based Labyrinth OS, kernel `linux-cachyos` version at the time, NVIDIA open modules version at the time.
- What had to differ from the author's build (FWSEC from this card's own ROM, any port-profile change), described in words.
- The tensor results in the repo's own format, `results/2080ti-v4-comparison.json` (`baseline` and `unlocked`, each with
  `mma_f16_acc_fp32`, `mma_f16_acc_fp16`, `cublas_f16_acc_fp32`, `cublas_fp32_acc_cpu_reference`, then `ratios`), produced
  by the same benchmark sources (`tools/tensor_baseline.cu`, `tools/tensor_validate.cu`) with 7 samples each.
- The application results from section 9: Sarina prefill and generation, voice, Whisper, the game, power and temperature.
- The probe log and the verifier log for the successful boot.

**What stays out:** the ROM dump, the FWSEC, the built loader, and any NVIDIA firmware (the repo's own manifest says
`inputs_redistributed: false`); the GPU UUID (`GPU-924c...`), serial numbers, MAC addresses, the hostname, and anything from
Matt's home folder. Opus checks the draft for these with `grep` before showing it.

**How:** an issue first (the repo has no CONTRIBUTING file; a pull request adding a results file only if the author asks for
one). Draft in `docs/gpu-unlock-upstream.md`, show Matt the exact text, post only after his yes, from his account:
`gh issue create --repo haohervchb/Turing-unlock --title "Second TU102 board: MSI RTX 2080 Ti, VBIOS 90.02.17.00.3E" --body-file docs/gpu-unlock-upstream.md`.
Record the issue link here, and answer follow-up questions from the author only with Matt's yes for each reply.

## 9. Measurements (before, in Phase 2; after, in Phase 4)

| What | Command | Expect |
| --- | --- | --- |
| Tensor microbenchmark | `gpuunlock bench` (port of `tools/tensor_baseline.cu`, locked to `10de:1e07`, 68 SMs, BDF `01:00.0`) | the ratio the tile shows |
| Sarina prefill | `curl localhost:11434/api/generate` with a 6,000-token prompt from the vault, `"options":{"num_predict":1}`; read `prompt_eval_duration` | the number that matters for long contexts |
| Sarina generation | same with `num_predict: 256`; read `eval_duration` | unchanged (bandwidth-bound) |
| Voice | `labyrinth-voiced` timing line for a fixed 40-word sentence (it logs synthesis time) | maybe faster |
| Whisper | `labyrinth-ask-sarina --voice` transcription time on a fixed 10 s clip | maybe faster |
| A game | `mangohud` average and 1% low over a fixed 60 s scene in one Proton game Matt plays | unchanged |
| Power and heat | `nvidia-smi --query-gpu=power.draw,temperature.gpu,clocks.gr --format=csv -l 1` during the above | within 260 W, under 85 °C |

Each run three times; keep the median. The results go into this file and into `baseline.json` / `unlocked.json`.

## 10. Hard rules for Opus

- Never write to the GPU (registers, firmware, VBIOS) from a Linux session on the display card. Never run `nvflash`.
- Never set `GRUB_DEFAULT` to the unlock entry, never edit `/etc/default/grub` for it, never add `gpu_unlock.targets=` or any
  loader to the normal kernel command line. One shot only, through `grub-reboot`.
- Never weaken an identity guard, never add a wildcard to the support table, never copy another board's FWSEC or payload to
  make a build run. The Turing author: "Do not remove identity or DMA guards just to make a build run."
- Never raise the power limit above the card's default (260 W) while the unlock is armed, or during the first unlocked
  session and the Phase 4 measurements. Outside those, Performance mode may use 290 W (Matt, 2026-10-26).
- Never show the unlock row on a card that is not in the support table.
- Never post anything to GitHub (issue, pull request, comment) before Matt has read the exact text and said yes. Never
  attach the ROM dump, FWSEC, a built loader or any NVIDIA firmware; remove the GPU UUID and serial numbers.
- Never let Sarina, a skill, `claude-sudo` or any service arm the unlock. Only Matt, with his password, in Settings or at a
  root shell.
- Never ship a phase without its tests and its "Result" subsection here. Never run Phase 3 without Matt present and a cold boot.
- Keep the ROM dump and any built loader off public repos (Proton Drive only). The repos involved are public.
- If anything in the GPU's identity, driver series or VBIOS changes, the state goes back to `unsupported` until re-validated.

## 11. Matt's decisions (2026-10-26)

1. **An unsupported card shows no option at all** ("If the card is not supported, then the option should not be
   available"). Sections 5, 7.6, 7.7 and the Phase 0 gate follow this.
2. **Performance mode may use 290 W.** Section 6 rule 6; the 260 W hold applies only around the experiment.
3. **Backups:** the plan as written is fine. The arm dialog shows the date of the last Proton Drive backup of Sarina as a
   reminder; it does not upload anything by itself.
4. **Yes to an issue on Turing-unlock** with the card's identity and the probe log if Phase 1 needs the author (Phase 1,
   step 7), and **yes to reporting the findings** if it works (Phase 5). Matt reads every text before it is posted.
5. Matt wants to see the real-life numbers (section 9), which Phase 4 provides either way.

Still open, to ask at Phase 2: which game to use for the before/after measurement.

## Results

(Opus appends a dated subsection per phase: what was run, the numbers, the decision.)
