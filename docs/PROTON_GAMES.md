# Proton (Valve's game compatibility layer)

Not related to Proton AG (Proton Drive, Proton Pass): see [PROTON_AG.md](PROTON_AG.md).

## What it is

- Valve's Proton ([ValveSoftware/proton](https://github.com/valvesoftware/proton),
  BSD-3-Clause) is delivered **by the Steam client**; its own README says most
  users should use that. Building it yourself needs Docker/Podman, so Labyrinth
  does not build it.
- Labyrinth ships `steam` plus: `proton-cachyos` (CachyOS's tuned build),
  `protonup-qt` (install/switch Proton-GE and others), `protontricks`,
  `umu-launcher` (run Proton outside Steam), `gamemode`, `mangohud`.
  Names are unverified against the repos.
- 32-bit NVIDIA/Vulkan libs are in `packages.x86_64` (multilib), which Proton needs.
- Anti-cheat: see ARCHITECTURE.md section 6; support is per game.
- Not yet tested on the target machine: Vulkan on the 2080 Ti, Proton runs.


## Setup and verification (built)

`labyrinth-gaming` (`/usr/local/bin/labyrinth-gaming`):

- `labyrinth-gaming check` is read-only. It reports PASS/WARN/FAIL for: Steam and
  32-bit NVIDIA/Vulkan packages, the multilib repo, whether the NVIDIA driver is
  loaded and Vulkan sees the 2080 Ti, `nvidia_drm.modeset`, the kernel limits
  some games need (`vm.max_map_count`, open-file limit), Valve's Proton, extra
  tools, and whether "Steam Play for all titles" is on.
- `labyrinth-gaming setup` installs what is missing, then enables Steam Play for
  all titles by adding a default tool to Steam's `config.vdf` (Proton-CachyOS if
  present, else Proton Experimental). It never overrides a tool you already
  chose, backs the file up first, and refuses if Steam is running or the file is
  not shaped as expected. Steam must have been started once so that file exists.

Tested: the `config.vdf` editor (7 unit tests, Windows). **Not tested:** the shell
script itself beyond syntax. It has never run on Linux; run `check` first on the
real machine.
