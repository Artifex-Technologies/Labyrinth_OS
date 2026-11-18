# Location sounds on Labyrinth OS

Two sounds in almost every recording can say where and when it was made: the tone of an aircraft overhead (its Doppler
curve, matched against public flight tracks, places a recording to within a few hundred metres, even when nobody can hear
the plane) and mains hum (its 50 or 60 Hz wanders with the power grid, which dates a recording and names the grid). On
Labyrinth both are removed from everything the microphone records, by Heimdall (2026-11-15). The full description, the
measurements and the limits are in Heimdall's
[docs/LOCATION_SOUNDS.md](https://github.com/Artifex-Technologies/Heimdall/blob/main/docs/LOCATION_SOUNDS.md).

## What I get

- **The private microphone, on by default.** "Microphone (private)" is the real microphone with those sounds removed as it is
  recorded (21 ms added, about 1.4 % of one desktop core). Every app records it: calls in Vivaldi (Meet), Teams, Zoom, Webex,
  Discord, OBS, the voice recorder, and Sarina's voice input (no exception for her on Labyrinth OS: speech is kept, and an
  exception would be something to bypass). Device lists show only it; an app that asks for the real
  microphone by name gets it anyway. Speech and everything above 2 kHz are kept.
- **The switch:** Settings > Security > Privacy > Private microphone (also under Peripherals > Microphone), or
  `heimdall-private-mic on|off`. Off is for recording music with full bass; it takes effect at once and is remembered.
- **Files:** right-click a recording or a video in Minos > Scrub location sounds writes "name (scrubbed).ext" beside it (the
  original is never changed; the video picture is copied untouched; GPS, dates and device tags are removed). In a terminal:
  `heimdall-scrub IN OUT`, or `heimdall-scrub --check IN` to see whether a file has gliding tones or mains hum.

## Where it comes from

- The files are in their own package, `heimdall-private-mic-git` (`packages/pkgbuilds/heimdall-private-mic-git`, built from
  Heimdall's Rust crates `heimdall-scrub` and `heimdall-scrub-ladspa`; its file list is the private-mic part of Heimdall's
  `packaging/install.sh`). It is in the base system (`profile/packages.base`), so every install has it, with or without the
  optional Heimdall, and Settings will not uninstall it (`/etc/yggdrasil/protected-packages`). heimdall-git ships none of
  its files. It is published to [labyrinth] with the rest (`scripts/build-local-packages.sh`).
- ffmpeg, which the file tool uses, is on the ISO already (ffmpegthumbnailer and others need it). PipeWire and WirePlumber are
  the ISO's sound system.
- Nothing phones out: the scrub needs no network, and no flight data is fetched anywhere.

## Limits (short)

It protects what this computer records, not a phone, a camera or someone else's recording of me, and not a file I receive
unless I scrub it. A program that opens the sound card directly (bypassing PipeWire) is not covered. An aircraft tone under
loud continuous speech is the most likely thing to leak. It is evidence, not a guarantee; the measurements are on synthetic
audio.
