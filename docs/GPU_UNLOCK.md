# GPU unlock: plan for a Settings option that raises a GeForce card's tensor rate safely

Written 2026-10-26 for whoever implements it (Claude Opus in a later session) and for Matt, who decides at each gate.
Everything below was checked against Matt's machine on that date; the "Facts" section says how.

> **Method changed on 2026-10-27 (Matt's yes to the Phase 1 proposals).** The unlock now uses **nlzy/turing-unlock**, a
> kernel module loaded from the initramfs before `nvidia`, and not haohervchb/Turing-unlock's EFI loader. The first try is
> a boot-menu entry Matt picks by hand. Section 12 says what this replaces. Where sections 1-10 describe the EFI loader,
> the ESP, `grub-reboot` or chainloading, section 12 rules.

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
   **Found in Phase 0 (2026-10-27): on this PC `grub-reboot` is not one-shot.** GRUB clears `next_entry` by writing
   `/boot/grub/grubenv` as it starts, and GRUB cannot write to btrfs. `/boot` is on the btrfs root here and `grubenv` names no
   `env_block`, so the choice would stay set and the unlock entry would start at every restart: exactly the loop the author
   warns about. `gpuunlock` therefore refuses to arm on btrfs without an `env_block` (`arm_blocker` in its status; tested).
   Phase 2 must pick another one-shot and prove it with a harmless entry before anything else boots through it:
   (a) **EFI `BootNext`** (recommended): `arm` adds a firmware boot entry for the loader and sets `BootNext` to it with
   `efibootmgr`; the firmware clears `BootNext` itself before it starts the entry, so a hang cannot repeat; `verify` and
   `disarm` delete the entry. It writes two small EFI variables, which is ordinary for boot managers.
   (b) a GRUB `env_block` in btrfs's reserved area (this GRUB, 2.16, reads one: see `00_header`), set up once and proven.
   (c) GRUB's environment on the EFI partition (FAT). Matt chooses at Phase 2; until then nothing can be armed.
3. **Armed marker before, cleared after, in two steps.** Before `grub-reboot`, the helper writes
   `/var/lib/yggdrasil/gpu-unlock/armed` with the time, the entry id and the boot id of the start that armed it. Early in the
   next start (before `sysinit.target`), `yggdrasil-gpu-unlock-boot.service` turns it into `trying` with **that** start's boot
   id: this start is the attempt. Once the desktop is up (`yggdrasil-gpu-unlock-verify.service`, after `graphical.target` and
   `nvidia-persistenced`), `verify` checks the card and removes `trying`. If the attempt never gets that far (a hang, the
   power button), the start after it finds `trying` from an earlier start: the helper sets state `failed`, disables the arm
   button until Matt re-enables it, and logs. This is the "failure marker written before any GPU change, so a hang or power
   loss causes the next boot to skip the experiment" that the Turing author lists as a requirement for any automatic use.
   (The first draft compared `armed`'s boot id with the current one at the verify step only; the attempt itself is a new
   start, so that would have called every attempt a failure. Both services start only while a marker exists.)
4. **Snapshot first.** Arming takes a snapper snapshot "before GPU unlock" (same as `before-nvidia.sh` in Proton Drive >
   Labyrinth OS). The unlock does not touch the system, but the surrounding package work (a DKMS module, an EFI loader on the
   ESP) might, and the snapshot makes that reversible by the guide already written for Matt (`GPU driver recovery guide.md`).
   Snapshots start as a normal desktop when chosen in the boot menu, because grub-btrfs' overlay hook is in the start-up
   image (drop-in `/etc/mkinitcpio.conf.d/labyrinth-snapshot-overlay.conf`, added to the installer and to `before-nvidia.sh`
   on 2026-10-26). Only snapshots taken after the drop-in is in place have it, since each snapshot carries its own `/boot`.
   The rollback itself is `labyrinth-rollback N` (keeps the previous system as `@broken-DATE`; `undo` reverses it); Opus runs
   it only when Matt asks.
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
| `packages/settings/data/yggdrasil-gpu-unlock-boot.service` | Added in Phase 0: `Type=oneshot`, early (`Before=sysinit.target`), runs `gpuunlock boot` (rule 3). Both services start only while a marker exists; `arm` enables them |
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
5. Hold the GPU power limit at the card's default for every mode. As built (Phase 0): `arm` writes
   `/var/lib/yggdrasil/gpu-unlock/power-hold`, and `powerlimit` never sets more than the card's default while it exists
   (or less, if the heat watch wrote a lower cap). The modes' own limits are not rewritten, so Performance mode's 290 W
   comes back by itself when the hold ends: `disarm` before an attempt, or `gpuunlock release` after the measurements
   (Settings: "End the hold…"). An unreadable hold file still holds.
6. Write `armed` (time, entry id, current boot id), then `grub-reboot <entry id>`.
7. Log, and return the text for the tile. The helper does **not** reboot; Matt restarts from the menu when ready.

`disarm` removes `armed`, runs `grub-reboot` with the normal entry (which clears `next_entry`), and restores the power caps.

### 7.5 Verify sequence (`gpuunlock verify`, the service, at every boot)

As built in Phase 0 (two services, see rule 3):

1. `gpuunlock boot`, early: if `trying` exists from an earlier start, that attempt never reached the desktop: state
   `failed`, remove `trying`, log. Then, if `armed` exists from an earlier start, this start is the attempt: `armed`
   becomes `trying` with this start's boot id.
2. `gpuunlock verify`, once the desktop is up: if `armed` still exists, the attempt is the next start; do nothing. If
   `trying` is this start's: test. Read the selector state the way the Turing author does (the microbenchmark ratio against
   `baseline.json`; there is no NVML field for it). Run the correctness check. Set `unlocked` (with ratio and time) or
   `failed` (wrong results / no speed-up), remove `trying`, take the entry out of the menu, then run the 15-minute heat
   watch. Until Phase 2 builds the check, every attempt ends `failed` ("not built yet").
3. In any other start where the one-try entry is still in the menu (after a failure), `verify` takes it out.
4. `unlocked` is kept with its boot id, so `status` shows it only in that start: the unlock is volatile.

Heimdall: each event is a line in `/var/lib/yggdrasil/gpu-unlock/log` and in the journal (tag `yggdrasil-gpu-unlock`).
Heimdall has no way for another program to add an alert, so showing these on the Monitor page is a Heimdall change (its
repository), not done in Phase 0.

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

Before the probe: the one-shot chosen in rule 2 is proven with a harmless entry (one that only chainloads the normal boot):
the first start goes through it, the second start is a normal one by itself.

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

6. **2026-10-27: yes to all six Phase 1 proposals** (section 12), with one condition: **"wait for me"**. The plan and the
   code may change now, but nothing is installed, built on this PC or benchmarked until Matt is home. Phase 2 starts with
   him there.

Still open, to ask at Phase 2: which game to use for the before/after measurement.

## 12. The kernel-module method (approved 2026-10-27)

What replaces what:

| Before (EFI loader) | Now (nlzy/turing-unlock) |
| --- | --- |
| A boot program on the ESP, chainloaded by a one-time GRUB entry chosen with `grub-reboot` | The normal kernel and initramfs. `turing_unlock` loads before `nvidia` at every start and does nothing unless `turing_unlock.targets=` is on the command line |
| `grub-reboot` (not one-shot on this btrfs `/boot`) | A boot-menu entry, "Labyrinth Linux, graphics card unlock", added by `arm` and **picked by hand** at the next restart. The default entry and `/etc/default/grub` are never touched, so a hang is followed by a normal start |
| The loader's identity guards, FWSEC from a ROM dump | The module accepts TU102/TU104/TU106 by device id and reads FWSEC from the card's own VBIOS. Our own guard stays: the exact device, board, VBIOS and driver series must be in the support table |
| `loader.conf`, the loader on the ESP | `entry.cfg`: a copy of the current first menu entry with the parameter added, which `41_gpu_unlock` prints only while it exists |
| The attempt known by the boot that follows the arming one | The attempt known by `turing_unlock.targets=` in `/proc/cmdline`. A start without it, while armed, simply did not try, so the entry stays offered |
| Success known by the benchmark only | First the module's own kernel log line (`TU102 FEAT UNLOCKED` or `unlock failed`), then the correctness check and benchmark (rule 8) before "Unlocked" |
| An evidence link for a run on the exact board | Runs on the same chip and VBIOS family (the reports in the Phase 1 result), then Matt's own supervised Phase 3 run before anything is kept |

**Unchanged:**
- The snapshot first.
- The power hold.
- The two-step failure markers (rule 3).
- The heat watch.
- Developer options only.
- No row on a card outside the table.
- A password every time.
- Never Sarina or `claude-sudo`.
- Phase 3 only with Matt present and a cold boot.

**Packaging (Phase 2, with Matt there).** Build `turing-unlock-dkms` and `turing-unlock-firmware` on this PC from the
project's Arch PKGBUILD. It downloads NVIDIA's public 610.43.03 open-module source and the CachyOS `nvidia-utils` package,
checks their SHA-256 and generates the firmware. The generated firmware is NVIDIA-licensed: it stays on this PC and never
enters a repository. Install, rebuild the initramfs and restart normally. Without the parameter, `journalctl -k -b | grep
turing_unlock` should show the module loaded and doing nothing, and the desktop should be unchanged. That restart is Phase
2's first gate.

**Rule 10, first line, reads:** never write to the GPU from a running session, after the driver has bound the card. A module
in the initramfs, before `nvidia`, is the case rule 5 names. Never `insmod` it later, never unbind `nvidia` to give it a turn.

**Code (2026-10-27, Yggdrasil 2e0a72a; not installed, by Matt's wish).**
- `gpuunlock` follows this section.
- Arming refuses until the module, its firmware (`/usr/lib/firmware/turing-unlock/610.43.03`) and its mkinitcpio drop-in
  are installed, and while the boot menu is hidden. This PC has `GRUB_TIMEOUT=2` and style `menu`, so the menu shows.
- The table has one entry: this card, `experimental`.
- "Unlocked" still waits for the correctness check and benchmark, which Phase 2 builds. Until then a start through the
  entry ends "not confirmed" even when the module reports success.

**Phase 5's target** is nlzy/turing-unlock: a compatibility report for the first MSI board, in the format of its existing
reports (card, VBIOS, driver, kernel, locked and unlocked numbers). Nothing goes to haohervchb/Turing-unlock unless Matt
asks.

## Results

(Opus appends a dated subsection per phase: what was run, the numbers, the decision.)

### Phase 0, 2026-10-27: done; nothing was written to the GPU

**Built** (Yggdrasil, `packages/settings`; installed on the owner's PC as `yggdrasil-settings 0.1.0-8`):
- `helpers/gpuunlock`: `status [--json]`, `arm`, `disarm`, `reenable`, `release`, `boot`, `verify`, `bench` (the last says
  "Phase 2" and runs nothing). It reads the card from sysfs and the driver's `/proc` files, without running a program.
- `data/gpu-unlock-supported.json`: the ASUS entry only.
- The polkit rule (`auth_admin`, a password every time).
- `data/41_gpu_unlock` (prints nothing without `loader.conf`, never fails, checks every value's shape before it reaches
  `grub.cfg`).
- The two services.
- In `powerlimit`: the power hold (section 7.4 step 5).
- In Settings: the "Graphics card" group.
- 34 new helper tests, `helpers/tests/test_gpuunlock.py`. They cover identity, the table refusing wildcards and missing
  fields, Secure Boot, the one-shot check, arming with every program faked, the attempt markers, the heat watch, the power
  hold and the GRUB fragment.
- Rust tests for parsing the status and the hold.

**Settings layout (Matt, 2026-10-27; 0.1.0-9).** Everything is in the "Power mode" group, under the mode selector, for the
mode that is selected: "Processor" (clock), then "Graphics card" (watts, its name in the line under it), each with a reset icon left of its slider.
Choosing another mode shows and changes that mode's limits. This replaces the three per-mode groups lower on the page. On a
card in the table a "Tensor-rate unlock" row follows the Graphics card row, and a "Power hold" row while a hold is on.
Section 5's "Graphics card group" means these rows.

**Gate:**
- Tests: 45 Python (all helpers) and 56 Rust tests pass, also inside `makepkg`'s check.
- `gpuunlock status --json` on the owner's PC gives `"state": "unsupported"`, `"reason": "No tested entry for MSI 1462:3711 with
  VBIOS 90.02.17.00.3E (tested: ASUS 1043:866a, VBIOS 90.02.17.00.47)"`. It reads BAR0 `0xde000000`, the open driver
  615.78.08, and Secure Boot off.
- Settings with Developer options on shows Mode, Processor and Graphics card (Performance at 290 W), and no unlock row.
- A debug build with a `supported` fixture shows the row and "Arm for next restart…".
- The services are installed and disabled. The GRUB fragment prints nothing.
- No reboot was needed for Phase 0. Heimdall's watch list gets the three files through step 13 of Matt's `finish-todo.sh`,
  because editing Heimdall's configuration is Matt's step.

**Found:**
1. On this PC `grub-reboot` would not be one-shot (rule 2): the attempt would repeat at every start. Arming is refused
   until Phase 2 picks and proves another way; EFI `BootNext` is recommended.
2. The first draft's single-marker check (7.5) would have marked every attempt as failed. It is now two steps (rule 3),
   with tests.

**Next:** Phase 1 (read-only research on the card: the ROM dump through the kernel's PCI ROM file, Turing-unlock's
host-side analysis). It needs Matt's go, and Matt reads the two BIOS settings (Above 4G Decoding, CSM).

### Phase 1, 2026-10-27: feasible, by another method (the change needs Matt's go)

Everything below was read, not run on the card. No GPU register was written; no ROM was dumped (see "This PC").

**haohervchb/Turing-unlock** (2 commits, last on 2026-10-22, no issues opened). Its EFI loader is pinned to the author's
machine well beyond the card, and refuses this one several times over:
- 91 bytes of the Alienware A14 firmware's `PciRootBridge.Map` (`tu102_direct_dma.h`);
- the Xeon E5-1660 v3's CPU signature (`306f2`, 46 physical address bits; this i9-9900K is `906ed`, 39);
- the ESP, BDF `02:00.0`, BAR0 `fa000000`, the ASUS subsystem;
- the ASUS card's FWSEC by SHA-256.

Porting it would mean auditing the MSI Z370I firmware and reworking the DMA path, which needs the author at least.

The author's numbers explain the "double" headlines:

| Measurement | Locked | Unlocked | Change |
| --- | ---: | ---: | ---: |
| FP16-input / FP32-accumulate MMA | 46.9 TFLOPS | 93.7 TFLOPS | 2.00× per clock |
| cuBLAS GEMM, same maths | 48.1 TFLOPS | 85.2 TFLOPS | 1.77× |
| FP16-accumulate | | | unchanged |
| Mining | | | no gain |

**xrip/cmp50hx-unlock** (`efi-unlock`), the CMP 50HX original:
- Lists an MSI CMP 50HX (`1462:371f`), never `1e07` / `1462:3711`.
- Uses `BootNext` as its one-shot, and wants Above 4G on and CSM off.
- Its `extract_fwsec.py` walks any TU102 VBIOS.

**gpu-unlock issues #30 and #97 (Turing)** both point to **nlzy/turing-unlock** (created 2026-10-24, 276 stars). It is the
kernel-module Turing port that step 4 names as the preferred method:
- **When it runs:** `turing_unlock` loads from the initramfs before `nvidia`, and does nothing without
  `turing_unlock.targets=` on the kernel command line.
- **Which cards:** it accepts TU102/TU104/TU106 by device id (`0x1e07` is listed).
- **FWSEC:** it reads FWSEC from the card's own VBIOS with an independent byte parser (`vbios.c`).
- **Firmware:** generated at build time from NVIDIA's public 610.43.03 open-module source and `nvidia-utils` package,
  hash-pinned and not redistributed.
- **What it writes:** the payload writes SS0 `0x88888888` / SS1 `0x8` and reads them back.
- **Failure:** a failure ends in a PCIe FLR and a normal `nvidia` bind. A firmware that will not stop keeps its DMA
  buffers until power-off.
- **Packaging and licence:** GPL-2.0, with an Arch PKGBUILD and a mkinitcpio drop-in.

**nlzy/turing-unlock's issues** (14 by 2026-10-27) are compatibility reports, none of a failure:
- **#1:** 2080 Ti 22 GB on 615.71.09, 65.2→103.6 TFLOPS. The power stayed at the limit.
- **#2:** two 2080 Ti, with and without NVLink.
- **PR #8:** Gigabyte 2080 Ti (`1e04`) with VBIOS `90.02.17.00.7C` and `90.02.30.00.81` ("different ROM layouts, both unlock").
- **#5:** an RTX 2080 that is the boot VGA driving a Wayland desktop, on driver **615.78.08**, a CachyOS-based kernel 7.3
  and GRUB: 39.0→64.5 TFLOPS, on every boot since.
- **#3, #4, #6:** 2070 and 2070 Super.
- **#13:** Tesla T10, +0% (never locked).
- **#14:** eGPUs are not supported (they appear on the bus too late).

No MSI board yet. This card's VBIOS (`90.02.17.00.3E`) is of the same `90.02.17` family as the reports.

**This PC, read from Linux:**
- **Above 4G Decoding:** probably off (BAR1 at `c0000000`).
- **CSM:** unclear. A UEFI GOP framebuffer was used, but a `0xC0000` ROM shadow exists.
- **VT-d:** probably on in the firmware (a DMAR table), unused by Linux.
- **Memory:** 62 GiB.
- **No ROM dump:** this boot-VGA card's sysfs ROM file is the 128 KiB legacy shadow, without FWSEC, so step 1's dump was
  skipped. TechPowerUp has no copy of `90.02.17.00.3E` for `1462:3711`.
- **Not run:** the author's host tests (step 2). Installing `gnu-efi` to build them was refused by the session's safety
  check; they test the author's simulated paths, not this card.

**Result: feasible with current public material, through nlzy/turing-unlock rather than the EFI loader.** Step 7's issue
on haohervchb/Turing-unlock is not needed. Phase 5's report would go to nlzy/turing-unlock (this would be the first MSI
board), under the same rules: Matt reads the exact text first.

**Proposed changes to this plan (not in force until Matt says yes):**
1. **Method:** the kernel module. It has no motherboard-firmware dependency and no board pins that refuse this card. It
   also has no EFI one-shot problem, and a failure ends in a normal boot.
2. **One try:** `arm` adds a boot-menu entry, "Labyrinth Linux, graphics card unlock": the normal kernel with
   `turing_unlock.targets=0000:01:00.0` added. Matt picks it by hand at the next restart.
   - Picking by hand is one-time by nature, so rule 2's btrfs `grub-reboot` problem disappears: the start after it is the
     normal entry.
   - `gpuunlock boot` / `verify` recognise the attempt by the parameter in `/proc/cmdline`; failures are handled as in
     rule 3.
   - The entry goes away after the attempt.
3. **The support table** gets the method `turing-unlock-module` and an entry for `1462:3711` / `90.02.17.00.3E` / open,
   `experimental`, with the reports above as evidence. For this method, "a run on that exact board" becomes "runs on the
   same chip and VBIOS family". Matt's own supervised Phase 3 run comes before anything is kept.
4. **Rule 10's "never write to the GPU from a Linux session"** reads as rule 5 always meant: never from a running session
   after the driver has bound the card. A module in the initramfs, before `nvidia`, is the case rule 5 names.
5. **Packaging:** build turing-unlock on this PC from its Arch PKGBUILD. It downloads NVIDIA's public 610.43.03 files and
   checks their hashes. The generated firmware stays on this PC under NVIDIA's licence, never in our repositories. The
   module then loads at every start and does nothing without the parameter.
6. **Phase 2 becomes:**
   - Install the package; a normal restart without the parameter shows that nothing changes.
   - Record the locked baseline at 260 W: the project's own GEMM benchmark (PR #8) and section 9, including Sarina's
     prefill and generation.
   - Then Phase 3 as written, with the hand-picked entry.

**Still for Matt, in the BIOS (no change, only reading):** Above 4G Decoding and CSM, plus VT-d and Fast Boot if they are on
the same screens. For the module method these are records, not requirements.
