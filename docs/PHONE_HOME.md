# What Labyrinth OS talks to (phone-home audit, first pass)

GO_LIVE.md §3 asks for a list of every host an installed system contacts, each one justified. This first pass, written
2026-11-07, is not yet the idle-VM capture that section describes: it reads a week of Heimdall's connection record on my own PC
(in daily use: games, coding, browsing), grouped by program and by the organisation behind the address. No address, network
name or account is written here.

**How:** Heimdall's `connection-watch` notes each program's first connection to each destination, with the destination's
organisation from its offline databases (`docs/` of Heimdall, ipintel). Read with
`sudo journalctl -u heimdall --since -7days -o cat | grep connection-watch`, then counted by program and organisation.

**Limits:** Heimdall records TCP only. DNS lookups, the clock (NTP) and QUIC (HTTP/3 over UDP, which browsers use a lot) do not
appear, so Vivaldi's and Google's share below is low. A program's later connections to a place it already used are not
counted again.

## What the system itself starts

| Program | Talks to | Why, and when |
| --- | --- | --- |
| `yggdrasil-weather` | Open-Meteo (its API runs on Microsoft's cloud), NOAA's aviationweather.gov (measured temperatures from nearby airports) | The weather in the bar and Fylgja, and every 10 minutes the rain warning, for the place chosen in Settings. BeaconDB only when the place is found automatically (every few hours). |
| Fylgja's speed test | Cloudflare (`speed.cloudflare.com`) | Only when "Run speed test" is pressed. |
| Agora | Flathub (`flathub.org`), GitHub | App pages' descriptions and pictures, only after the person says yes to loading them (asked once). Installs: Arch's mirrors and Labyrinth's package repository on GitHub. |
| pacman, Agora's updates, the daily publish on the build PC | Arch and CachyOS mirrors, GitHub (Labyrinth's package repository) | Updates, when they are checked or installed. |
| Heimdall's `heimdall-intel-update` | the threat-list and IP-database sources (Heimdall's README) | Once a day; heimdalld itself never connects off the computer. |
| ClamAV's `freshclam` | ClamAV's signature mirrors | Signature updates, several times a day. |
| Tailscale (`tailscaled`) | Tailscale's coordination and relay servers | Only on a computer where the owner set it up (mine, for the iPad). Not on the image. |
| Sunshine | the owner's own devices | Only while streaming to them; not on the image. |

Not seen in the week: NetworkManager's connectivity check, any telemetry from the desktop's own programs, Sarina (her rule is
loopback only; Heimdall makes any connection of hers off the computer a critical alert).

## What programs the person starts talk to

| Program | Organisations seen | Note |
| --- | --- | --- |
| Vivaldi | Google, Cloudflare, Amazon, others | Websites, plus Google's Safe Browsing and component updates. The image now sets Vivaldi's privacy policies (below). |
| Steam, its web helper, games | Valve, Akamai (Valve's content network), Google, Amazon, EA | Store, downloads, game servers. |
| VS Code, GitHub Copilot, `gh`, `cargo`, `git` | Microsoft, GitHub, Fastly | Extensions, Copilot, code hosting. |
| Claude Code | Anthropic (on Amazon and Cloudflare) | The assistant I work with. |

## Vivaldi's privacy defaults (on the image since 2026-11-07)

`/etc/vivaldi/policies/managed/labyrinth-privacy.json`, locked, each one only sends less: no usage metrics
(`MetricsReportingEnabled`), no URL-keyed data collection, no Google spelling service (Labyrinth's own spell check is local), no
Google error-page suggestions, none of the Privacy Sandbox ad features. `/etc/vivaldi/policies/recommended/labyrinth-privacy.json`,
defaults the person can change in Vivaldi's settings: no prefetching of pages before they are opened, and WebRTC never shows
websites the computer's local addresses. Vivaldi says "managed by your organisation" because of the locked file.

Not set by policy: Vivaldi's own anonymous usage ping (a Vivaldi setting, not a Chromium policy; Settings > Privacy in Vivaldi),
and Safe Browsing (a security feature; left on).

## Still to do

1. The capture GO_LIVE.md asks for: a fresh install in a VM, idle for ten minutes, every connection (UDP too) listed.
2. Encrypted DNS (DNS over TLS through systemd-resolved, with a choice of provider in Settings) and tracker blocking: to be
   tested in a VM first, because the resolver also serves Tailscale's names and the VPN.
