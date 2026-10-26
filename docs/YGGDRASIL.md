# Yggdrasil

**Yggdrasil** is the desktop environment of Labyrinth OS (the OS keeps the name Labyrinth). It lives in its own repository:
<https://github.com/Artifex-Technologies/Yggdrasil>. Its README, the plan to replace KDE, the compositor, the lock and login screens, Settings, Minos
and the Marketplace are documented there: `docs/YGGDRASIL.md`, `docs/DESKTOP_ROADMAP.md`, `docs/KDE_INVENTORY.md`, `docs/COMPOSITOR.md` and the rest.

## What stays in this repository
The operating system: the ISO profile, the installer and first boot, the boot theme, the games stack, the Sarina, Heimdall and Limbo setup, the system helper scripts, the
Labyrinth package recipes, the brand icon pack, and the security parts that belong to the OS rather than the desktop: the distress password helper and its PAM module
(`packages/distress`, `packages/distress-pam`; [DISTRESS_PASSWORD.md](DISTRESS_PASSWORD.md)). Settings shows the distress password page only when that helper is installed.

## How the two fit together
- `scripts/build-local-packages.sh` builds the desktop's packages from a Yggdrasil checkout next to this repository (`../Yggdrasil`, or `YGGDRASIL_DIR`).
- Chymaera OS will use Labyrinth as a template (installer, boot theme, its own branding) and Yggdrasil as its desktop.
- **Icons:** the system's icon pack is **Liki** (`liki-icon-theme`, in the Yggdrasil repository: [LIKI.md](https://github.com/Artifex-Technologies/Yggdrasil/blob/main/docs/LIKI.md)), the same for Labyrinth and Chymaera. No KDE (Breeze) or macOS (MacTahoe) icons are used.
