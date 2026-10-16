# Security roadmap: a Vipre-style guard in Heimdall, and a curated app store

Wanted (owner, 2026-10-16): Heimdall also scans for viruses and malware and warns about apps with network-security concerns,
in the manner of Vipre: it notices when something asks for the computer's information or tries to connect to it, asks
"allow or deny", is easy to silence for a file, folder or app, and does not nag more than it must. Plus a small store of trusted,
tested, open-source apps that do not sell user data.

Heimdall today: watches Sarina's sessions, key files (file integrity), listening ports, and (new) which program connects where; it
alerts but does not block or ask. Limbo today: disposable guests, used for static analysis (radare2/Ghidra) of samples.

## 1. Malware scanning (a new Heimdall source)
- **Engine:** ClamAV (`clamav`, signatures kept current by `freshclam`; local) plus our own YARA rules. Heimdall orchestrates; it does
  not write its own signature engine.
- **When:** (a) new files in Downloads and in watched folders (inotify, scan on close-write), (b) a scheduled full scan at night, (c) on demand
  from Settings, (d) every new package before/after install (pacman hook: scan what a package put in /usr).
- **Suspicious but unknown files:** can be sent to a Limbo guest (no network) for static triage and, later, a run with a network capture;
  the answer comes back as one more finding. Sarina can explain a report in words.
- **What happens:** the file is moved to a quarantine folder (not deleted), a notification says what and why, with Restore / Delete /
  Always allow. Exceptions are a simple list (path, folder, hash or app) in `/etc/heimdall` plus `~/.config/labyrinth`, edited in Settings.
- **Cost to watch:** scanning uses CPU and disk; scheduled runs at low priority, skipped on battery and while a game or CAD job runs.

## 2. Per-app network control with a prompt
- **Have:** a per-app switch in Settings > Apps (allow / block / wired only / Wi-Fi only) using nftables on the app's systemd slice, and the
  connection log (which program, which address; alerts for a blocked app that connects, for Sarina/Heimdall/Limbo leaving the machine).
- **Next, in order:**
  1. Action buttons on notifications (Allow / Deny / Allow once), the piece the Notification Center lacks (see NOTIFICATIONS.md "not built yet").
  2. Mode **ask**: a new app's first connection is held (nftables `queue` rule on the app's slice, answered by a Heimdall verdict loop), the
     notification asks, the answer is remembered per app and destination (domain, not just IP), and stays editable in Settings > Apps.
  3. Names, not only addresses: resolve with reverse DNS and the SNI of the first packet (no payload kept) so the prompt says "pdfcraft wants
     to reach api.github.com", not an IP.
  4. Incoming: UFW already drops inbound by default; Heimdall already alerts on a non-loopback listener; add an alert when any app starts
     listening on a new port, and the same ask/deny choice.
- **Alternative to compare before building:** OpenSnitch (packaged for Arch; an application firewall that prompts per connection). Worth a
  trial install on the test machine to learn what is good and what is too noisy, then decide whether to wrap it or keep our own.
- **Do not repeat Vipre's over-protectiveness:** prompts are grouped (one question per app and destination, not per connection), have a
  "do not ask again for this app" and a quiet mode, and system components (the package manager, time sync, Heimdall's own sources) are
  pre-approved.

## 3. A curated app store ("Labyrinth Apps")
Goal: end users install apps that were checked, are open source, and do not sell data, without hunting the AUR.
- **Shape:** a signed package repository (pacman) for what we have vetted, plus the existing Settings > Apps "Optional software" tab as
  the front end (it already lists optional apps). Flatpak entries allowed where the app publishes a good one (sandboxed by default).
  Updates come with the normal system update; nothing is updated by hand.
- **Inclusion checklist (one page per app, kept in `docs/vetting/`):** open-source licence; builds from or matches the upstream release;
  PKGBUILD and package contents reviewed (no setuid, services, install scripts without reason); static analysis in a Limbo guest (imports,
  strings, hosts); a network capture of a first run in a guest (what it contacts, when idle and when used); declared telemetry or "none";
  does it sell or share user data (privacy policy and code); default network setting chosen for it (blocked unless it needs the network).
- **First candidates:** the ArtCraft suite (review in ARTCRAFT_REVIEW.md; static checks done, network capture and sandbox run still to do).
  New and fast-moving, so they would be marked "new, expect bugs" until they have a record.
- **Who does the work:** Claude drafts and runs the checks, Sarina assists with reports (read-only for now), Heimdall/Limbo supply the
  tools; the owner decides what goes in.

## Order of work (suggested)
1. Notification action buttons. 2. Network capture of a first run in Limbo (internet-only guest, capture on the host bridge) for the
vetting template, using PdfCraft. 3. ClamAV + folder scanning + quarantine with the exceptions list. 4. The "ask" network mode.
5. The curated repository and the front end.

## Status 2026-10-16 (later)
- **Malware scan of Downloads: built** (`labyrinth-scan-downloads` + `labyrinth-scan.timer`, user units, enabled by the installer's first boot step): every 5 minutes it asks
  `heimdalld scan-files` to scan new and changed files in ~/Downloads (and the folders in `~/.config/labyrinth/scan-folders`), skipping what is in
  `~/.config/labyrinth/scan-exclude` (a folder, file or pattern per line, `~` allowed), and raises a critical notification once per infected file. It never moves or deletes.
  Tested with stand-ins (4 tests) and for real with the EICAR test file. ClamAV and `gnome-keyring` are now in the ISO package list; `clamav-freshclam` is enabled at first boot.
  Needs the NEW `heimdalld` (the one on this machine is older and has no `scan-files`; the script then logs "scan did not run" and stays quiet).
- **Settings > Security: built** (recent alerts with a level filter, who connected where, a "Scan now" button for Downloads). It also showed the baseline-drift noise
  (CPU load of 0.3 flagged as unusual); fixed in Heimdall (only upward outliers above load 0.75 per CPU or memory 70%).
- **Not built:** quarantine and "restore / delete / always allow" buttons on the malware notification; the "ask" network mode (see section 2); exclusions edited in Settings.
