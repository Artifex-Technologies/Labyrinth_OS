# Proton AG services: Proton Drive and Proton Pass

Not related to Valve's Proton game layer: see [PROTON_GAMES.md](PROTON_GAMES.md).

## Proton Drive

There is no official Proton Drive client for Linux. What exists:

| Option | How it works | Verdict |
| --- | --- | --- |
| `donniedice/protondrive-linux` | Tauri window around Proton's web app; AGPL-3.0; experimental folder sync, no mount | Not a file-manager integration, and AGPL; rejected |
| rclone `protondrive` backend | rclone speaks to Proton Drive and can FUSE-mount it | **Chosen for now**: packaged, mounts at `~/ProtonDrive` |
| The `proton-drive` CLI in your `Documents\Github` folder | A single Bun-compiled executable (Linux ELF + Windows exe). Embeds `@protontech/drive-sdk`, Proton's own SDK. Not signed | Promising; blocked on provenance and its command list (see below) |

### How it appears in minos

The explorer does not speak any cloud protocol. A provider is a **folder kept
populated by another process**, exactly how OneDrive looks in Windows Explorer.
`places::proton_drive()` looks for `~/ProtonDrive` (our mount) then
`~/Proton Drive` (the official Windows client's name), or whatever
`LABYRINTH_PROTON_DRIVE` points to. It shows under **Cloud** in the sidebar, or
greyed out as "not mounted" with the expected path. The mount unit is
`/usr/share/labyrinth/proton-drive.service` (untested).

### The official `proton-drive` CLI

Downloaded by the owner from Proton's own website. It is not needed: the plan
is rclone plus the folder links below. It is unsigned and was not run here. It
stays an option if it turns out to offer hashes or a mount that rclone lacks.

The `Documents/Github` folder also holds a recovery-codes text file; it was not
opened. Those codes belong in a password manager, not a synced file.

## Folder links and verification (built)

`labyrinth-cloud` (and the sidebar's **Cloud** section in the explorer):

    labyrinth-cloud link docs ~/Documents/Github "proton:My files/Documents/Github"
    labyrinth-cloud verify docs

Verification is read-only and compares `rclone lsjson -R --hash` of the local
folder against the remote one: missing on either side, size differences, and
content (hash) differences. It reports "Verified" only if **every** file was
matched by a hash both sides provide; if Proton Drive offers no hash rclone can
use, files are matched by size only and the result says so (amber), never green.
Exit code 0 means fully verified. The explorer shows the last result as a badge
next to each link (check = verified, warning = size-only, cross = problems,
`?` = not run) and "Verify now" in its context menu.

Tested: the comparison logic (5 unit tests) and, with rclone 1.75.1 on Windows,
the whole CLI flow against two local folders: it flagged a file missing on one
side and a same-size file with different content (exit 1), then reported
"Verified: 3 files match by hash" once fixed (exit 0). **Not tested:** against a
real Proton Drive remote. Whether Proton Drive exposes hashes through rclone is
still unknown; if it doesn't, results will show amber "size only". Note this
checks that files match; it is not a sync tool and does not copy anything.

## Proton Pass

Proton publishes the desktop app only as `.deb` and `.rpm`
([setup page](https://proton.me/support/set-up-proton-pass-linux)); there is no
official Arch package, and the community AUR package (`proton-pass-bin`) has had
reported launch problems on CachyOS. Labyrinth builds its own package instead:
`packages/pkgbuilds/proton-pass` repackages Proton's **official RPM**, pinned to
one version with its SHA-512 (taken from Proton's `version.json`, and re-hashed
independently here: it matched). It is listed in `packages.x86_64`.

- The RPM is an Electron app (`/usr/lib/proton-pass`), so the PKGBUILD lists the
  usual Chromium runtime libraries and sets `chrome-sandbox` setuid, as Arch's
  Electron packages do.
- Updating is manual: bump `pkgver` and the hash from `version.json`. Pinned on
  purpose, so nothing changes under you without a visible commit.
- **Not built or run on Linux.** Unknowns: whether the dependency list is
  complete, and whether the desktop app's browser-extension unlock works.
- The browser extension (Firefox/Chrome/Brave) installs from the browser's store,
  not from us. Proton also has a separate `proton-pass-cli` (AUR); not packaged.

## Sync badges in the explorer

Files and folders inside a linked folder get an icon from the link's **last
verification** (not live transfer state; Proton's client exposes none):
green check = matched by hash, amber warning = matched by size only, red cross =
differs (a folder shows red if anything inside differs), grey `?` = never
verified, blue arrows = a folder that contains a linked folder. Hover for the
explanation. Re-run **Verify now** (sidebar, Cloud) to refresh them. Seen working
on the real `.obsidian` folder of the book vault (Windows). Note that folder holds
three `workspace (# Edit conflict …).json` files: Proton Drive's conflict copies
from Obsidian's constantly-rewritten workspace file.

## Connecting from Minos (built)

In Minos, **Cloud → Proton Drive (click to connect)** opens a sign-in form (email, password,
2FA code if you use one, mailbox password only if you set a separate one). It creates the rclone
remote `proton` (type `protondrive`; passwords are scrambled with `rclone obscure`, read from
stdin), copies `/usr/share/labyrinth/proton-drive.service` to `~/.config/systemd/user/`, and runs
`systemctl --user enable --now proton-drive`. Once signed in, a click on a not-mounted entry just
starts the mount. A wrong sign-in deletes the half-made remote. The 2FA code only has to be fresh
for this one sign-in. **Not yet tested against a real Proton account.**
