# REA and Ghidra for Sarina (installed 2026-10-17)

**What:** REA (`rea-agents` on npm, github.com/morluto/rea, MIT) is a reverse-engineering toolkit: a CLI and MCP server that drives a decompiler (here Ghidra 12.1.4) and many
inspectors (ELF, PE, Mach-O, Go and Rust binaries, firmware, APKs, packet captures...). It is a Node.js program, not Windows-only. It is installed as an app in your home, not in
the Labyrinth repo: `~/.local/share/labyrinth/apps/rea` (REA 6.5.0, pinned, installed with `--ignore-scripts` and without its optional native add-ons) and `.../apps/ghidra`.
Vetting before installing: the registry package has no install scripts, 146 dependencies of the usual kind, one maintainer, 237 thousand downloads in the last month, no unexpected network addresses in
its code; Ghidra's zip was checked against the SHA-256 GitHub publishes. REA's own `rea setup` (which edits AI tools' configuration files) was **not** run.

**How it is used safely: `labyrinth-rea`** runs REA inside bubblewrap: no network, no access to your home or files, a private /tmp, a read-only system, killed with its parent. It sees only
`~/Documents/Samples` (read-only, as /samples) and the malware scan's quarantine (read-only, as /quarantine), and writes only to `~/.local/share/labyrinth/rea-work`. It is a sandbox, not a
virtual machine: REA's runtime features (a debugger running the sample) would run it on this kernel inside the sandbox, so samples that could attack the kernel belong in Limbo.
`labyrinth-rea-setup` installs everything again on another machine (needs a JDK 21: `pacman -S jdk21-openjdk`, Node.js, bubblewrap).

**Sarina:** the tool `reverse_engineer` (`Sarina/go/reverse`, on when `SARINA_REVERSE=on`, which `~/.config/labyrinth/sarina.env` now has). Static commands only (`analyze`, `functions`,
`decompile`, `xrefs`, `search`, `search-bytes`, the `inspect-*` family); the debugger, capture and device commands are not offered; paths outside the analysis folder and the quarantine are
refused; every result ends with a reminder to report only what the output shows. Evals (group "reverse"): she finds the rule of the sample program (6 characters, starts with L, ends with 7)
3 times out of 3 after the tool listed functions for her (before that she invented a rule: `analyze` alone does not show logic), and she refuses `/etc/passwd`. Her playbook is in her vault:
`Playbooks/Reverse engineering playbook.md` (and `Pentest.md`); the Playbooks folder is owner-only, she cannot change it.

**Heimdall (built 2026-11-17):** Heimdall does not call REA by itself; it asks Sarina. `sudo heimdalld immunity analyze --sarina SHA256`
lends her sandbox a read-only copy of a held file (`/var/lib/heimdall-sarina-share`, seen by her service account as `/quarantine/<sha256>.sample`),
opens a session on her integration service whose only tool is `reverse_engineer` (nothing is sent unless the service confirms exactly that one
tool), and keeps her answer with the held file as her analysis: untrusted text shown labelled as hers, which can never release, delete or run
anything. A YARA rule in it only becomes a rule draft that root tests and you approve. The copy is taken back on every path, one analysis runs at a
time, none while Sarina is contained, and her session is in her session folder, which Heimdall watches. "Heimdall may ask" is the standing
permission `sarina_analysis` in `/etc/heimdall/config.json`: `ask` (the default: only when you ask; a new held file's notification offers it),
`auto` (every new held file ClamAV does not know, at once), `off`. Asking without waiting in a critical case is not built: holds carry no severity,
and the most critical one (containment) is when Sarina must not be asked. Setting up her service account (REA and Ghidra visible to it,
`SARINA_REVERSE=on` in `/etc/sarina/service.env`, its quarantine folder linked to the share) is done once; Heimdall `docs/IMMUNITY.md`,
"Sarina's analysis", has the steps. Why this is worth doing: drivers and new malware have no write-ups; reading what they do is how a
signature, a detection rule or a block gets written.

**Not done:** Ghidra BSim (finding known functions in a new sample; next); REA's MCP server for other tools; Hopper/IDA/JEB providers (commercial); `functions` needs a symbol table (stripped programs: use `search` and `xrefs`);
speed (Ghidra takes about 8 s for a tiny program, minutes for a large driver: the tool waits up to 6 minutes).
