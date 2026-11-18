# Third-party notices

Labyrinth OS's own files (build scripts, settings, helpers, artwork made for it) are under the Mozilla Public License 2.0
([LICENSE](LICENSE)) unless a file or folder says otherwise. The packages it installs keep their own licences. What it includes or
builds on from others is listed here.


- **EdenExplorer** (MIT, mtucciarone): feature set and UX are the specification for
  `packages/minos`. No code is copied yet; if any is, retain its MIT
  copyright notice here.
- **Omarchy** (MIT, David Heinemeier Hansson): desktop model; consumed as upstream
  where possible. Add attribution for any copied configuration.
- **YARA-X** (BSD-3-Clause, VirusTotal): built unmodified from its release source into the `yara-x` package
  (`packages/pkgbuilds/yara-x`); the package installs its licence at `/usr/share/licenses/yara-x/LICENSE`.
- **Atkinson Hyperlegible Next and Atkinson Hyperlegible Mono** (SIL OFL 1.1, Braille Institute of America), **Lexend** (SIL OFL
  1.1, The Lexend Project Authors) and **OpenDyslexic** (SIL OFL 1.1, Abbie Gonzalez): packaged unmodified from Google Fonts and
  the OpenDyslexic release (`packages/pkgbuilds/ttf-atkinson-hyperlegible-next`, `ttf-atkinson-hyperlegible-mono`, `ttf-lexend`,
  `otf-opendyslexic`); each package installs its OFL.txt under `/usr/share/licenses/<package>/`.
- **OpenDyslexic Mono** (Bitstream Vera licence; OpenDyslexic is derived from Bitstream Vera Sans), the 2016 release: packaged
  unmodified (`packages/pkgbuilds/otf-opendyslexic-mono`); the licence text, from that release's README, is installed at
  `/usr/share/licenses/otf-opendyslexic-mono/LICENSE`.
- **CachyOS** (GPL-2.0 for linux-cachyos packaging): repos and kernels consumed as
  packages, not forked.
