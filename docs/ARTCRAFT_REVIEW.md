# ArtCraft apps: pre-install review (2026-10-16)

Apps asked about: PdfCraft, DesignCraft, VectorCraft, FilmCraft, PhotoCraft, FigCraft, WordCraft, SoundCraft, DeckCraft, CADCraft, GridCraft.

## What they are
Rust desktop apps from the GitHub organisation `storytold` ("ArtCraft", getartcraft.com): clean-room reimplementations of Acrobat,
InDesign, Illustrator, Premiere, Photoshop, Figma, Word, Pro Tools, PowerPoint, AutoCAD and Excel. The AUR descriptions say
"vibe coded" (largely AI-written code). Versions are 0.6 to 0.10: young software, expect bugs. The pdfcraft repo was created 2026-09-30
(14.7k stars, 5.7k forks, 1.3k open issues); releases are built by GitHub Actions and ship a SHA256SUMS file; a Flatpak is also published.

## In the AUR
All except FigCraft have a `-bin` package (prebuilt release tarball, no compiling): pdfcraft-bin, designcraft-bin, vectorcraft-bin,
filmcraft-bin, photocraft-bin, wordcraft-bin, soundcraft-bin, deckcraft-bin, cadcraft-bin, gridcraft-bin. FigCraft only has source
packages (`figcraft`, `figcraft-git`, builds with cargo). `cadcraft-studio` is a bundle of every CadCraft app (not reviewed).
The packagers differ (elonux, modinfo, QiE2035, one AUR account per app group); some have 0 votes: new packages, little community review.

## What was checked (nothing was run)
- **PKGBUILDs (all 10 -bin):** sources only from `github.com/storytold/*` releases (design/vector/photo also fetch a NOTICE file from
  raw.githubusercontent.com/storytold), every source has a sha256 (no SKIP), no `.install` script, no curl/wget/sudo/systemctl/eval in
  package(). Packages built locally with makepkg: checksums verified by makepkg.
- **Package contents:** only files under /usr (binaries, icons, desktop files, docs, licenses, models); no setuid files, no systemd
  units, no hooks, nothing outside /usr.
- **The pdfcraft tarball** matched the checksum in the PKGBUILD, and the GitHub release lists the same asset.
- **Static analysis of all 20 binaries inside a Limbo guest with no network (profile `decompile`):**
  - Every GUI app imports `getaddrinfo/socket/connect` (they can use the network), plus `dlopen`, `fork`, `setuid`. These are normal for a
    Rust GUI with a TLS library (rustls/ureq are present); they do not by themselves mean anything bad.
  - The only web addresses in the binaries: GitHub (project links; PdfCraft also `api.github.com/repos/storytold/pdfcraft/releases/latest`,
    an update check), getartcraft.com (help pages), discord.gg/artcraft (help link), and FilmCraft's huggingface.co links (optional downloads of
    the Whisper speech and Kokoro voice models). No other hosts, no hard-coded IP addresses, no analytics service names (posthog, mixpanel,
    sentry.io, google-analytics ...). The word "amplitude" appears because of audio, not usage tracking.
- Not done: reading the source, dynamic analysis (running them in a guest with a network capture), checking what each writes to disk.

## Verdict
Nothing alarming from the outside. The remaining risk is the ordinary one for very new, quickly written software: bugs in file
parsers (PDF, Office files and images are hostile input), and an update check that talks to GitHub. Sensible handling:
1. Install the `-bin` packages, not the source ones.
2. Give each app its network switch in Settings > Apps: off for the ones that do not need it (all but FilmCraft's model downloads, and the
   update checks). Heimdall's connection log then shows any attempt, and flags an app that connects while switched off.
3. Open untrusted documents only after that, ideally with the sandbox switch on.
4. Heimdall can watch the binaries (file-integrity list) so a changed binary raises an alert.

## Status (2026-11-07)
The ten `-bin` packages are installed on my PC (since 2026-11-03; `pacman -Qs craft`), at newer versions than the ones reviewed
(0.15 to 0.21). FigCraft is not installed. The binaries are on Heimdall's file-integrity list. Agora lists the apps under
ArtCraft, with this review as its note. The newer versions were not reviewed again.
