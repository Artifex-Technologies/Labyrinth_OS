# Reference plan (2026-11): ideas from 26 starred projects

On 2026-11-17 I went through 26 projects I had starred. They are **references only**: nothing from them is installed,
vendored or copied. Each idea below is rewritten from scratch in Rust in our own repositories, under MPL-2.0, and has to fit
the house rules: light enough for a Raspberry Pi, security before stability before speed, private by default (nothing about
this computer leaves it unless I switch that on, per feature).

Items for the security edition are planned in that edition's own repository, not here.

**Priority:** P1 small and worth doing now · P2 next · P3 later · P4 only if wanted.
**Status words** follow Heimdall's [docs/REFERENCES.md](https://github.com/Artifex-Technologies/Heimdall/blob/main/docs/REFERENCES.md):
Adapted, Design reference, Candidate.

## 1. The references at a glance

### Already Heimdall design references: what each gave, what is still unused

| Reference | Gave (built) | Still unused, taken now | Item |
| --- | --- | --- | --- |
| CrowdSec | Source → Detector → Sink engine; leaky-bucket burst detector | scenarios as data; decisions that expire; allow-lists that run after a match; replaying logs to test a scenario | R10, R7 |
| Zeek | connection state per process (no packet capture) | names beside addresses (DNS and TLS server names); an intel framework matching what is seen against loaded lists; notices with suppression | R11 |
| Wazuh | FIM with an mtime+size check before hashing | configuration checks as data (SCA policies); cross-view checks for hidden processes and ports; rule IDs mapped to ATT&CK | R13, R1 |
| Netdata | `/proc`-only telemetry, no fork/exec | alarm hysteresis (raise at one level, clear at a lower one); one anomaly bit per metric | R14 |
| TALON | learned baselines; "known vs never seen" for Sarina's tool names | the middle class: a close *variant* of something known; sequences, not single names | R15 |
| Paperclip | the approval-gate framing (grant gate, review) | budgets per agent with a hard stop; every agent decision in one audit trail with its "why" | R16 |
| CodeStrike | the approval-gate framing | a local model reviewing a change before it lands (an update's build script) | R17 |
| n8n | (candidate only) | trigger → steps workflows, each step with only the access it needs, every run logged | R7 |

### New references

| Reference | Licence | What I take | Goes to | Item | Prio |
| --- | --- | --- | --- | --- | --- |
| mitre-attack/attack-stix-data | MITRE ATT&CK terms (notice required) | a trimmed technique table (ID, name, tactic) to tag detections | Heimdall, Limbo, Sarina | R1 | P1 |
| mitre-attack/mitreattack-python | Apache-2.0 | idea only: a coverage map of my detectors (Navigator layer) | Heimdall | R2 | P2 |
| Ringmast4r/OUI-Master-Database | none stated | ideas only: all IEEE registries (MA-L, MA-M, MA-S, IAB), monthly refresh, export formats. **Its merged data is not used** (it mixes other projects' lists under their own licences) | Heimdall crate, Yggdrasil Settings | R3 | P1 |
| bauerj/mac_vendor_lookup | Apache-2.0 | confirms the offline-file design `oui.rs` already has; adds a refresh with a kept last-good copy | Heimdall | R3 | P1 |
| feross/SpoofMAC | none detected | already have: a stable random address per untrusted Wi-Fi network; extends to wired, IPv6 and Bluetooth identifiers | Heimdall wifi-guard, audits | R19 | P1 |
| VirusTotal/yara | BSD-3 | already have: rule drafts loaded by ClamAV, YARA in Limbo's analysis image. **In maintenance mode**; YARA-X is its successor | Heimdall immunity, Limbo | R4 | P1 |
| VirusTotal/yara-x | BSD-3 (Rust) | rule checking and formatting ideas (slow-pattern warnings); possibly the crate itself (my decision) | Heimdall immunity, Limbo image | R4 | P1 |
| abusech/URLhaus | data CC0 | already have: the URLhaus hosts filter in `heimdall-dns-blocklist`. Adds: its ClamAV URL signatures; downloaded hash lists instead of per-hash questions | Heimdall intel | R5 | P2 |
| Suryesh/OTX_AlienVault_URL | MIT | idea only: AlienVault OTX as an optional feed, **pull-only**: download subscribed indicator lists, never ask about what this computer saw | Heimdall intel | R6 | P3 |
| CrowdStrike/fusion-skills | MIT | declarative trigger / condition / action workflows, action discovery, run monitoring | Heimdall response | R7 | P2 |
| CrowdStrike/foundry-skills | MIT | packaging a workflow with its own data and permissions; skills that guide the author | Heimdall response, Sarina | R7 | P2 |
| claffin/cloudproxy | MIT | throwaway egress that is created per job and destroyed after; never the home address | Limbo | R8 | P2 |
| NationalSecurityAgency/datawave | Apache-2.0 | per-record visibility labels checked at read time | Heimdall API, Sarina vault and council, Limbo | R9 | P2 |
| storytold/realtime-voice-conversion | none stated | idea only: mic → model → speaker in real time, used to *disguise* my voice, never to imitate someone | Heimdall private mic, Sarina | R18 | P3 |
| compiler-explorer | BSD-2 | idea only: "what does this compile to", local and offline | Sarina tool | R20 | P3 |
| yukurash/16minds-plugin | MIT | idea only: many perspectives debating one question | Sarina council | R21 | P3 |
| NationalSecurityAgency/skills-service | Apache-2.0 | idea only: a small skill tree that teaches the tool inside the tool | Yggdrasil | R22 | P4 |

## 2. Work items

Each item: where it goes, what exists, the phases (smallest first), what it needs, and notes.

### R1 · ATT&CK technique tags · P1 · Heimdall, Limbo, Sarina

- **Exists:** nothing ATT&CK-related in any repo. Every Heimdall alert has a detector ID; Limbo's triage gives imports and strings.
- **Phases:**
  1. A build-time tool (`rust/tools/attack-table` in Heimdall) reads one pinned release of attack-stix-data's Enterprise
     bundle and writes a small table: technique ID, name, tactic(s), platforms Linux/Windows only. Deprecated and revoked
     objects dropped. Committed as data with `NOTICE-ATTACK` (MITRE's copyright line and licence text, verbatim).
  2. A static map detector → technique IDs in `rust/heimdall/src/attack.rs`; alerts gain `attack: ["T1059.004", ...]`.
     Settings > Security shows the technique name; the API returns it.
  3. Sarina's explanation of an alert names the technique and links MITRE's page; Limbo's report tags triage hints
     (for example a sample importing `ptrace`) with candidate IDs, marked "suggested".
- **Needs:** nothing. **Notes:** the table is regenerated by hand when I choose to move to a new ATT&CK release
  (row in REFERENCES.md). MITRE's disclaimer applies: a tag says what a detector looks for, never that a technique is
  "covered".

### R2 · Coverage map · P2 · Heimdall

- From mitreattack-python's layer generation: `heimdalld attack-layer` writes an ATT&CK Navigator layer (JSON) of the
  techniques my detectors watch, coloured by severity, plus a `docs/COVERAGE.md` generated from the same map: what is
  watched, what is not, and why (a gap list is as useful as a coverage list).
- **Needs:** R1.

### R3 · One MAC-vendor crate, all IEEE registries · P1 · Heimdall (shared crate), Yggdrasil

- **Exists:** `rust/heimdall/src/cli/lan/oui.rs` reads `/usr/share/hwdata/oui.txt` (MA-L only, 24-bit prefixes) and
  knows the locally administered bit.
- **Phases:**
  1. Crate `heimdall-macvendor` (in Heimdall's workspace, no dependencies): parses IEEE's MA-L, MA-M, MA-S and IAB CSVs,
     longest-prefix match (36 → 28 → 24 bits), reports "private address" for the locally administered bit and
     "multicast" for the group bit. Falls back to hwdata when no IEEE file is present. Fuzzed like the other parsers.
  2. `heimdall-intel-update` fetches the four IEEE files monthly (public, nothing sent), checked before replacing the
     last good copy. `oui.rs` becomes a thin wrapper.
  3. Yggdrasil's Settings device page and the Bluetooth page use the same crate through the Heimdall API.
- **Notes:** IEEE data only. No other project's list and no merged list ships (their licences differ). Memory budget: the four
  files as a sorted array of `(prefix, len, offset)` with names interned, a few MB, mapped read-only.
- **Status (2026-11-17):** phase 1 built: Heimdall `rust/macvendor` (crate `heimdall-macvendor`, no dependencies) and its Go
  twin `internal/macvendor`, the same answers on a shared fixture; special addresses, multicast and private addresses before any
  registry; the device list uses it in Go and Rust, the old readers are gone (Heimdall `e92c458`). Memory: one sorted array per
  block size with names stored once, under 8 MB for the whole benchmark with all four IEEE files (59,062 blocks, ~150 ns a
  lookup). The IEEE folder is `/var/lib/heimdall-ieee` (readable by everyone; `/var/lib/heimdall` is closed). Phase 2 is
  designed in Heimdall's `docs/DEVICES.md` ("Vendor data") and waits on decision 3. Phase 3: Settings' Wi-Fi pages use the
  crate directly as a git dependency (Yggdrasil `45fdcd9`); the device page shows what Heimdall's list says; the Bluetooth page
  is still to do.

### R4 · Rule drafts: lint, then YARA-X · P1 · Heimdall immunity, Limbo image

- **Exists:** IMMUNITY.md "Rules": Sarina drafts, Heimdall keeps one-rule drafts (no `import`/`include`, ≤16 KiB), root
  tests with ClamAV against the sample and `/usr/bin`, I approve. Limbo's analysis image ships C YARA.
- **Phases:**
  1. A Rust lint in `rust/heimdall/src/rules.rs` before a draft is kept: only the subset ClamAV loads; warn on short or
     very common strings (the "atoms" YARA-X complains about), on `filesize`-free rules with many wildcards, on missing
     `meta` (`sample_sha256`, `author`, `date`, optional `attack`). Pure Rust, no dependency.
  2. **My decision:** use the `yara-x` crate (BSD-3, a library dependency, not copied code) for an exact compile check
     and its slow-rule warnings, or stay with the own lint and ClamAV.
  3. Limbo's analysis image moves from C YARA to YARA-X's `yr` (a package inside a throwaway guest, as today).
- **Notes:** C YARA gets fixes only; new syntax lands in YARA-X. ClamAV's YARA subset stays the install target until
  ClamAV says otherwise.

### R5 · URLhaus beyond DNS · P2 · Heimdall intel

- **Exists:** `packaging/net/heimdall-dns-blocklist` loads malware-filter's URLhaus hosts list (CC0).
- **Phases:** 1. `heimdall-intel-update` also fetches URLhaus's ClamAV URL signatures (download only) into ClamAV's database
  folder, so a downloaded document or script that carries a known malware address is flagged by the scans that already
  run. 2. Where abuse.ch publishes downloadable payload hash lists, load them as an `intel-hashes` set that
  `heimdalld immunity check` and the downloads scan consult offline.
- **Never by default:** a per-hash or per-URL question to abuse.ch's API (it needs an account key and tells them what I
  have). If ever added: opt-in, per question, shown first, like Sarina's council.

### R6 · Optional OTX feed, pull-only · P3 · Heimdall intel

- **Exists:** `ipintel.rs` loads threat lists (`*.list`), the DNS block list, ClamAV.
- **Phases:** 1. With a key in `/etc/heimdall/otx-key` (absent = off), `heimdall-intel-update` downloads the indicators
  of the pulses I subscribe to, splits them into IP list, domain list (to the DNS block list's candidate file, reviewed),
  hash set (R5's). 2. Alerts name the pulse that matched.
- **Notes:** the account knows what I subscribe to, never what this computer saw. Check OTX's terms before shipping; the
  feature downloads for one machine and redistributes nothing.

### R7 · Response playbooks · P2 · Heimdall response, Sarina

- **Exists:** `response.rs` restricts Limbo guests (restrict-only); `requests` queue lets the daemon ask root for narrow
  jobs; the grant gate decides who may approve; Sarina advises on holds.
- **From:** Fusion skills (trigger / condition / action as YAML, actions discovered from a fixed catalogue, runs
  watched), Foundry skills (a workflow ships with its permissions), n8n (each step gets only its own credential, every run
  logged), CrowdSec (decisions with an expiry), Paperclip (approvals).
- **Phases:**
  1. Format: `/etc/heimdall/playbooks/*.toml`, root-owned. Trigger = detector + severity + field matches; conditions;
     actions from a fixed verb list only: `notify`, `limbo-restrict`, `immunity-hold`, `app-network-block`
     (with `for = "24h"`), `ask-sarina`. Every playbook is `propose` by default: the notification carries the buttons.
  2. Replay test: `heimdalld playbook test FILE` runs it over the last 30 days of alerts (`replay.rs`) and prints what it
     would have done. Same pattern as rule drafts: Sarina may draft, root tests, I approve; never approved from a command
     an AI agent started.
  3. `auto` only for restrict-only, reversible verbs I switch on per playbook.
- **Notes:** no free-form shell action, ever. Every run is an alert (`source: playbook`) with what it did and why.

### R8 · Limbo analysis network: sinkhole first, throwaway egress later · P2 · Limbo

- **Exists:** sample runs are sealed (network `none`); profiles also offer `internet-only`, `allowlist`, `proxy`;
  SECURITY_ROADMAP wants a first-run network capture for vetting apps.
- **From cloudproxy:** egress that is created for one job and destroyed after it, so the far side never sees the home
  address.
- **Phases:**
  1. Network mode `sinkhole`: the guest's DNS answers every name with a local responder that accepts connections and
     logs what was asked (names, ports, first bytes), all on the host's isolated bridge. A sample that calls home reaches
     a log on this computer, not the internet. Fits sample runs and app vetting.
  2. Network mode `egress` (opt-in per run, needs my approval): the guest's traffic leaves only through a WireGuard
     tunnel in its own network namespace (a VPN account or a throwaway server); nftables drops everything if the tunnel
     is down. Torn down with the guest.
- **Needs:** Limbo decision §9.1 (is running a sample in scope?). Phase 1 is useful without it (vetting).

### R9 · Visibility labels · P2 · Heimdall API, Sarina, Limbo

- **Exists:** API callers are known by account; council members get `allow_data: [no_secrets, no_captures]`;
  Sarina's vault has owner-only folders.
- **From DataWave:** every record carries a label expression; a reader's authorisations are checked per record, at
  read time, not per table.
- **Phases:** 1. Labels on alerts and evidence: `owner`, `sarina`, `local-model`, `online`, `export`. Evidence with
  personal paths or captures defaults to `owner`. 2. Sarina's vault notes get a `visibility:` front-matter field;
  council and compute tiers filter on it (an `online` member never gets a note without `online`). 3. Limbo's reports
  inherit the label of the sample's record.
- **Notes:** labels only ever narrow. A missing or damaged label means `owner` only.

### R10 · Detection as data · P3 · Heimdall

- Simple threshold and sequence detectors as files (CrowdSec scenarios, Wazuh rules): `/etc/heimdall/scenarios/*.toml`
  with ID, ATT&CK tag, leaky-bucket or sequence shape, an allow-list stage after the match, and a `replay` test.
  Compiled Rust detectors stay for anything complex. Files owner-signed (trust list), so a planted scenario never loads.

### R11 · Names beside addresses (Zeek-style) · P2 · Heimdall connection log

- **Exists:** connection log by process, TCP only, addresses only; dnsmasq runs locally with the block list.
- **Phases:** 1. dnsmasq writes its query log to a socket only Heimdall reads (no file on disk); Heimdall keeps a
  short-lived address → name map in memory and adds `name` to new-destination events. 2. Intel match (Zeek's intel
  framework): names and addresses checked against the loaded lists at the moment they are seen. 3. SECURITY_ROADMAP §2.3:
  the "ask" prompt says the name.
- **Notes:** names are kept no longer than the connection state; never written to a log file in clear by default.

### R12 · Shared signals between my own machines · P3 · Heimdall mesh

- CrowdSec's community list shares attackers' addresses with a central server. Mine: learned fingerprints (immunity),
  approved rules and blocked addresses are shared only between machines I enrolled in the mesh (MESH.md), signed by the
  sender, each one shown to me before it is used. Nothing goes to a central server.

### R13 · Configuration checks as data, and cross-view checks · P2 · Heimdall audits

- **Exists:** `hardening` checks in code (AUDITS.md).
- **Phases:** 1. Checks as data (Wazuh SCA): ID, rationale, the file or setting read, the expected value, the fix text.
  New checks from this review: Bluetooth address privacy in BlueZ, IPv6 privacy addresses, DHCP name sending on untrusted
  networks (R19). 2. Cross-view check (Wazuh rootcheck): processes in `/proc` vs. what `/proc/*/stat` walk and cgroups
  report; listening sockets in `/proc/net` vs. `ss`-equivalent netlink answers. A difference is critical.

### R14 · Alarm hysteresis · P3 · Heimdall baselines

- From Netdata: raise at one level, clear at a lower one, so a value near a threshold does not alert on and off; one
  "anomalous" bit per telemetry field kept for an hour, shown as a strip in Settings.

### R15 · Known, variant, novel · P3 · Heimdall agent baseline

- Extend TALON's idea from single tool names to short sequences (3-grams of Sarina's tool calls) and to each app's
  destinations: known / close variant (one step different, same domain family) / novel. Info for variants, warning for
  novel sequences that touch files or the network.

### R16 · Budgets for Sarina's agents · P2 · Sarina, Heimdall

- From Paperclip: each agent run (forge, council, Limbo runs) gets a budget in tokens, wall time and tool calls, set by
  me; at the limit it stops and reports. Heimdall reads the budget from the session and alerts on a run that goes past it.

### R17 · A second look at update build scripts · P3 · Sarina, Agora

- From CodeStrike: before an Agora or AUR update is built, a local model reads the diff of its build script and lists
  what changed in plain words (new sources, new install hooks, new services). Advice only; I decide.

### R18 · Disguise my voice · P3 · Heimdall private mic, Sarina

- **Exists:** the private mic chain (scrub, planned "voice only" and "my voice only"); Sarina's speaker check.
- **Phases:** 1. Light DSP disguise in `rust/scrub`'s chain: pitch and formant shift with a little jitter (Pi-capable).
  Honest label: this hides casual recognition, a determined listener can partly undo it. 2. Optional neural conversion
  to one of a few **synthetic** voices that belong to nobody (desktop GPU). Never a target taken from a real person.
- **Notes:** Sarina's speaker check reads the signal before the disguise. Off by default.

### R19 · Identifier privacy extras · P1 · Heimdall wifi-guard, audits

- **Exists:** untrusted Wi-Fi gets a stable random address per network, no host name in DHCP, no mDNS or LLMNR;
  scans use random addresses.
- **Add:** the same for untrusted wired networks (`ethernet.cloned-mac-address=stable`); IPv6 privacy addresses
  (`ip6-privacy=2`) and a stable-privacy interface ID; DHCP client ID derived from the random address; a hardening check
  that BlueZ uses address privacy. Each with a test like the wifi-guard tests.
- **Notes:** random addresses use the locally administered bit, as phones do; I never borrow another maker's prefix.

### R20 · "What does this compile to" · P3 · Sarina tool

- A Sarina tool that runs `rustc --emit=asm -C opt-level=3` on a snippet in bubblewrap (no network, private /tmp) and
  returns the assembly for one function. No compiler website, nothing new installed. For keeping hot paths light.

### R21 · Council perspectives · P3 · Sarina council

- One local model asked the same question in turn under a few fixed lenses (security, privacy, performance, the person
  using it, the sceptic); the local synthesiser lists agreement and disagreement. Runs one at a time on the GPU. Lenses,
  not personality types.

### R22 · Learning by doing in Yggdrasil · P4 · Yggdrasil

- A small skill tree in Settings' tips: shortcuts, privacy switches, backups. Progress stays on the computer.

## 3. Lessons for privacy, anonymity and security

| Lesson (from) | Turned into |
| --- | --- |
| **Detection is data.** Rules, scenarios and checks are files that can be read, tested and versioned (Zeek scripts, CrowdSec scenarios, Wazuh rules and SCA policies, YARA) | R10, R13; every data file owner-signed so it cannot be planted; a `replay` test before any rule goes live |
| **Share signals, not observations.** CrowdSec's community list works because many machines report; the price is that each reports what it saw | receive-only public lists (as today); sharing only inside my own mesh (R12); OTX pull-only (R6); never a per-indicator question by default (R5) |
| **Name what you see, keep it briefly** (Zeek's DNS and TLS names) | R11: names in memory only, gone with the connection |
| **Analysis must not reveal the analyst** (cloudproxy's egress separation) | R8: sinkhole first; real egress only through a throwaway tunnel, kill switch on; a check that no Limbo analysis profile can route via the home connection |
| **Identifiers leak** (SpoofMAC, OUI databases) | R19: random wired and Wi-Fi addresses, IPv6 privacy, no host name, Bluetooth privacy check; R3 shows when a device on my network uses a private address |
| **Vendor data needs a clean source** (OUI-Master-Database mixes licences) | R3: IEEE files only; every data file in a repo has its licence line in REFERENCES.md |
| **A voice is an identifier** (voice conversion) | R18: disguise on the private mic; with location sounds already scrubbed, the voice is the next leak |
| **Visibility per record** (DataWave) | R9: labels on alerts, evidence and vault notes; missing label = owner only |
| **Approval gates and playbooks** (Fusion, Foundry, Paperclip, CodeStrike) | R7: fixed verbs, propose by default, replay test, my approval; R16 budgets; R17 a second look before an update builds |
| **Least privilege per step** (n8n credentials per node) | R7: each playbook verb runs through the narrow root request queue, never a general shell |
| **Learn what normal is** (TALON, Netdata) | R14 hysteresis; R15 known / variant / novel |
| **Say what is covered, and what is not** (ATT&CK, mitreattack-python) | R1, R2: tags and a gap list, never a "fully covered" claim |
| **Old engines get replaced** (YARA → YARA-X) | R4; REFERENCES.md rows get a Checked date and a maintenance note |

Checks to add to `RELEASE_CHECKS.md` when the items land: no analysis profile reaches the home connection (R8); every data
file has its licence and notice (R1, R3); no feature sends an observed address, name or hash off the machine unless its
switch is on (R5, R6, R11).

## 4. Roadmap: the top 10, in order

1. **R19** identifier privacy extras (wired random address, IPv6 privacy, Bluetooth check): small, all existing code.
2. **R3** MAC-vendor crate with all IEEE registries; Settings uses it.
3. **R1** ATT&CK technique table and detector tags (with MITRE's notice).
4. **R4** phase 1: Rust lint for rule drafts; then the YARA-X decision.
5. **R11** names beside addresses in the connection log, intel match at sight.
6. **R5** URLhaus ClamAV URL signatures (download only).
7. **R7** response playbooks, propose-only, with the replay test.
8. **R9** visibility labels on alerts and evidence, then the vault.
9. **R8** phase 1: Limbo sinkhole network for sample runs and app vetting.
10. **R13** configuration checks as data, then the cross-view check.

Later: R2, R6, R10, R12, R14 to R18, R20 to R22.

## 5. What I decided (2026-11-18)

1. **YARA-X: yes.** The `yara-x` crate (BSD-3) does the exact compile check and its slow-rule warnings in the Rust daemon,
   after the own lint; Limbo's analysis image moves to YARA-X's `yr`. ClamAV's YARA subset stays the install target.
2. **ATT&CK data: yes.** The trimmed Enterprise table (Linux and Windows) ships in Heimdall with MITRE's notice.
3. **IEEE downloads: no.** hwdata (Arch's package) stays the only vendor source; nothing fetches the IEEE files. The crate
   still reads `/var/lib/heimdall-ieee` if I ever put the files there by hand.
4. **URLhaus ClamAV signatures: yes, on by default** (download only, CC0).
5. **OTX: yes**, an optional account key, pull-only; absent key = off.
6. **Playbooks: yes**, both restrict-only verbs may run on `auto`: `limbo-restrict` and `app-network-block` for a set time.
   Each playbook still starts as `propose`; I switch `auto` on per playbook.
7. **Limbo: yes**, running a sample is in scope. Sinkhole first (R8 phase 1). The egress (VPN account or throwaway
   server) and who pays for it is chosen when phase 2 is built; until then a sample run has no route out.
8. **Voice disguise: yes, build it.** Phase 1 (DSP) first; the synthetic voices of phase 2 are made for the purpose,
   belong to nobody and are picked when phase 2 is built.
9. **Wired networks: yes**, a random address on untrusted wired networks by default.
10. **Labels: yes**, these defaults (a note's own `visibility:` field can only narrow them; a damaged field means `owner`):

    | Vault folder | Default | Why |
    | --- | --- | --- |
    | Talks, Entities, Inbox, Sources, Reflections | `local-model` | about me and the people I know; never leaves the computer |
    | Daily, Security | `local-model` | this computer's status and findings |
    | Decisions, Projects | `local-model` | may name private plans |
    | Lessons, Guides, Playbooks, Study, Jokes | `online` | general know-how; still sent only after my yes on each call, cleaned |

    No folder is `export` by default. Evidence and alerts follow R9 phase 1 (personal paths and captures: `owner`).
