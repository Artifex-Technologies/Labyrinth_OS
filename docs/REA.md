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

**Heimdall (design, not built):** Heimdall does not call REA by itself. The path would be: Heimdall asks Sarina's integration service to examine a quarantined file (the file is already
visible read-only as /quarantine/...), Sarina runs `reverse_engineer`, and the finding comes back to Heimdall as evidence with the analysis recorded in her session (which he watches). "Heimdall
may ask" is a standing permission you can switch off. In a high-risk case (a critical alert on an unknown file) he may ask without waiting, but the analysis stays static and sandboxed; nothing
is ever run. Why this is worth doing: drivers and new malware have no write-ups; reading what they do is how a signature, a detection rule or a block gets written.

**Not done:** Heimdall's request path; REA's MCP server for other tools; Hopper/IDA/JEB providers (commercial); `functions` needs a symbol table (stripped programs: use `search` and `xrefs`);
speed (Ghidra takes about 8 s for a tiny program, minutes for a large driver: the tool waits up to 6 minutes).
