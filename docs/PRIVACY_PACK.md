# The privacy pack on Labyrinth OS

Four protections, on by default for everyone who installs Labyrinth OS (2026-11-15). They are about what files and apps give
away without being asked: a document that phones home when it is opened, the hidden information in a photo, a viewer app
that looks things up online, and a name that pretends to be another. The private microphone
([LOCATION_SOUNDS.md](LOCATION_SOUNDS.md)) is the fifth switch on the same page.

The switches: **Settings > Security > Privacy**, one per part, all on by default. Turning one off is remembered per person.
Heimdall's side (how each check works, the formats, the tests) is in Heimdall's
[docs/PRIVACY_PACK.md](https://github.com/Artifex-Technologies/Heimdall/blob/main/docs/PRIVACY_PACK.md).

## 1. The downloads guard

Some files contact a server the moment they are opened, and that tells the sender that, when and from where (the address)
the file was opened: a Word document with a remote template, a spreadsheet that refreshes a web query, a PDF with an
automatic action or JavaScript, an SVG or HTML file that loads a picture, font or script from the web, a playlist whose entries
are web addresses, a `.desktop`, `.url` or `.webloc` shortcut.

Heimdall's `heimdall-fileguard` (a user service, started at login) watches `~/Downloads`, the download folders of the
torrent clients that have one set (Deluge, qBittorrent, Transmission) and any folder I add to its settings. When a file is
finished (partial names such as `.part`, `.crdownload` and `.!qB` are skipped; it waits until the file is closed and has
been quiet for a second) it reads it and looks for such content. Archives (zip, 7z, tar) are listed, never unpacked, and a
file of that kind inside is named.

It **never changes the file**: a torrent keeps seeding and a signature stays valid. When it finds something, a notification
says so in plain words ("This file tries to contact a server when opened.") with:

- **Show details**: what it found and where in the file;
- **Show in Minos**: the folder the file is in;
- **Dismiss**.

There is no "open offline" action: Limbo has no way yet to open a document without network (its Wine sandbox is for Windows
programs and its sample runs are root-only analysis), so the safe way is to open such a file with the app's network on
"ask" or "block" (part 3), or not at all. The same check by hand: `heimdall-fileguard check FILE`.

Switch: Settings > Security > Privacy, the downloads guard (`downloads_guard` in `~/.config/heimdall/privacy.json`).

## 2. Remove hidden info

Photos, videos, music and documents carry information that is not seen: where a photo was taken (GPS), the camera and its
serial number, the author, the company, who last changed a document and the folder its template came from. Right-click a
file in Minos > **Remove hidden info** (beside Scrub location sounds) writes a copy, "name (clean).ext", beside it and
lists what it removed. The original is never changed and an existing file is never overwritten. It covers JPEG, PNG, WebP,
HEIC and TIFF pictures, MP4, MOV and MKV videos (the streams are copied as they are), audio tags, PDF, Office and
OpenDocument files, and ZIP archives. In a terminal: `heimdall-strip IN [OUT]`.

Switch: Settings > Security > Privacy, the Minos entry (`remove_hidden_info` in `~/.config/heimdall/privacy.json`).

## 3. Viewer apps ask before going online

PDF viewers, office suites, image viewers and media players open files that came from anywhere, and part 1's tricks work
through them; music and video players also look up album art and track names online. So these apps start on **Ask** in
Settings > Apps > Network: the first time one tries to go online I get the usual question (`labyrinth-appnet-ask`), and
"Allow all" puts the app back on any connection for good.

How it works: `labyrinth-appnet apply` (at boot and at every login) gives each app in its built-in list the rule "ask" for
every person logged in who has no rule for it yet, once per app. What it did is kept in
`/var/lib/labyrinth/app-network-viewers.json`, so my own choice later (Allow, Block, Wi-Fi only) is never overridden. Apps
that are not installed get the rule too; it does nothing until the app is installed and started.

The list (`packages/tools/src/appnet/viewers.rs`), by desktop id:

- **on the ISO:** Yggdrasil's media player (`yggdrasil-player`); darktable (its map view downloads map tiles around the
  places the photos were taken, which says where they were taken). Saga MD is not on the list: it never loads pictures from
  the web.
- **PDF:** Evince, Papers, Okular, zathura, MuPDF, qpdfview, Atril, Xreader, Sioyek, PdfCraft.
- **office:** LibreOffice (each app, and the Flathub build), ONLYOFFICE, WPS Office, Calligra, AbiWord, Gnumeric, WordCraft,
  GridCraft, DeckCraft.
- **pictures:** Loupe, Eye of GNOME, Gwenview, nomacs, feh, imv, Ristretto, gThumb, qimgv, swayimg, nsxiv, sxiv, Eye of
  MATE, LXImage-Qt.
- **media:** VLC, mpv (both also from Flathub), Celluloid, Clapper, Totem, Showtime, Haruna, Dragon Player, SMPlayer, Parole,
  Elisa, Rhythmbox, Lollypop, Audacious, Strawberry, Clementine, Quod Libet, DeaDBeeF.

Not on it: browsers, mail, chat and streaming apps (going online is what they are for), and editors such as GIMP or Krita.

Yggdrasil's media player also starts libVLC with its online lookups off (no album art or metadata from the network).

Switch: Settings > Security > Privacy, viewer apps (`labyrinth-appnet viewers on|off`, asks for the password like any network
rule; `labyrinth-appnet viewers status --json` says how it is). Off takes back the rules it added that are still "ask" and
adds no more; on adds them again, also for apps I had set to Allow.

## 4. Look-alike warnings

A name can imitate another: "Vivaldi" written with a Cyrillic "а", "Steam" in Cherokee letters, fullwidth or mathematical
letters, or a hidden character that turns the text around so "invoice<U+202E>fdp.exe" reads as "invoiceexe.pdf". Wherever
Labyrinth shows a name I may trust, such a name gets a warning and the real thing beside it:

- **the network question** (part 3 and any app on Ask): the app is named as its launcher names it and the notification
  always says which program really runs; a look-alike name, program or host name gets "Look-alike name: ..." (or "Look-alike
  address: ...") in the title, the reasons in plain words, and the characters that hide or turn text written out;
- **Agora's listings** and **Settings > Apps**: a warning badge and the real package or program;
- **the downloads guard's notice**: the file's name with hidden characters written out.

The check follows Unicode's security rules (TR39: one alphabet per name, or the usual Chinese, Japanese and Korean mixes;
letters that imitate ASCII ones; invisible and direction characters). It is the same code in Heimdall, Labyrinth's tools and
Yggdrasil (`lookalike.rs`), so every place decides the same way. Names in one alphabet (Russian, Greek, Japanese...) are fine.

Switch: Settings > Security > Privacy, look-alike warnings (`lookalike_warnings` in `~/.config/heimdall/privacy.json`).

## Honest limits

- The per-app rules cover apps started by Labyrinth's launcher: the dock, the start menu and Settings. An app started another
  way (a terminal, another program, a file opened with xdg-open, which is how Minos opens files today) is not in its slice,
  so its rule does not apply there.
- The downloads guard reads the file's own content; it does not follow what a file loads after it is opened, and it cannot see
  inside an encrypted archive or document. A file that is not flagged can still be harmful in other ways: Heimdall's malware
  scan and quarantine are a separate protection.
- Ask mode asks about addresses, not names, and the person's own programs can answer its questions too.
- Look-alike checks catch letters that imitate ASCII and mixed alphabets; a name that is simply misspelt ("Vivaldl") is a
  different name to them.
- Remove hidden info leaves what is in the picture, the sound or the text itself (a street sign, a voice, a name in a
  document).
