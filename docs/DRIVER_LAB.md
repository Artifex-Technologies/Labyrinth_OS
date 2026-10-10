# Driver Lab (Sarina + Limbo)

Goal: when hardware does not work well, Sarina (or one of her agents) finds
candidate drivers, Limbo tests them, and a validated one is offered to the user.

## Honest constraint

A VM only exercises a driver if the VM can reach the real device. Three test
tiers, safest first:

| Tier | What it tests | How |
| --- | --- | --- |
| 0 Static | Builds against this kernel; `modinfo` matches the PCI/USB IDs | Build in a disposable Limbo guest with the same kernel and headers |
| 1 Passthrough | Real device behavior, isolated from the host | VFIO passthrough of the device into a Limbo guest. Works for the 2080 Ti because the 9900K's iGPU drives the host display. Also fits USB controllers, NICs, Wi-Fi, audio |
| 2 On-host trial | What cannot be passed through (the display GPU in use, chipset) | Btrfs snapshot + one-shot boot entry (`bootctl set-oneshot`) + a watchdog that reverts to the last good entry if the session is not healthy within N seconds |

## Pipeline

1. **Detect**: `packages/labyrinth-driverlab/probe.sh` collects `lspci -nnk`, `lsusb`,
   dmesg errors, loaded modules, IOMMU groups, kernel version.
2. **Candidates** (Sarina agent): repo packages, DKMS variants, out-of-tree
   sources. Output: a ranked list with versions.
3. **Validate** (Limbo, via the existing agent socket `/run/limbo/agent.sock`,
   `sarina limbo-validate PROFILE COMMAND...`): tier 0 then 1. Structured result
   per candidate: loads / binds / stable under a workload (vkcube or glmark2,
   iperf3, audio loopback) / dmesg clean.
4. **Debug loop**: on failure Sarina reads logs and proposes a change (module
   parameter, blacklist, firmware, patch), then re-runs. Bounded to N iterations
   per candidate; every attempt is logged.
5. **Offer**: the best passing candidate is presented with its evidence. The
   **user approves**; only then is it installed on the host (tier 2 for the
   display GPU). Nothing is auto-installed or auto-loaded on the host.
6. **Heimdall** watches Sarina throughout, as elsewhere in the ecosystem.

## "Adapt her own driver"

Realistic scope: patching an existing open driver (DKMS patch, quirk table,
module parameter) for a device that is almost supported. Generated code is
built and loaded only in a Limbo guest (tier 0/1) until a human verdict, the same
quarantine rule Limbo already applies to untrusted artifacts. Writing a driver
from scratch for undocumented hardware is not a realistic target.

## Open questions

- Limbo's VFIO passthrough support is not confirmed in its docs; it may need new
  work in Limbo.
- IOMMU groups on this motherboard are unknown; run `probe.sh` first.
