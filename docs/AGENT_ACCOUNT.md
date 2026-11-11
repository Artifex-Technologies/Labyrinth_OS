# The engineering agent's own account (Claude) and Heimdall's maintenance window

Decided with the owner 2026-10-16: the engineering agent (Claude) works under the owner's instructions and is not invisible to
Heimdall. To tell its actions apart from the owner's (and Sarina's), and to avoid passing the owner's password around, it gets its own
audited account. This is for attribution and review, not containment.

## Where it stands (2026-11-07)

Three things happened, in this order:

1. **2026-10-16, broad rules.** The first `labyrinth-agent-setup` let `claude-agent` run pacman, systemctl, journalctl,
   loginctl, systemd-run, `heimdalld maintenance`, limbo, ufw, `labyrinth-*`, install, mkdir, chown and chmod as root, without a
   password. pacman, systemd-run and install are in effect root: the safeguard was the record, not a fence.
2. **2026-10-27, narrowed (commit 63d68d4).** A security audit found that any process running as the owner reached that root
   access silently. `labyrinth-agent-setup` now writes `/etc/sudoers.d/50-labyrinth-agent`, which lets the agent run one program
   as root: `/usr/local/sbin/labyrinth-agent`. It checks each action and logs it for Heimdall:
   - `pkg-install FILE...`: packages built here, the ecosystem's own names only (yggdrasil-*, labyrinth-*, heimdall, sarina,
     limbo, liki, agora);
   - `pkg-add NAME...`: packages from the signed official repositories, never remote-access tools, tunnels or servers;
   - `service ACTION UNIT...`: the ecosystem's own services (and ollama);
   - `logs ...`: the journal, read-only options only;
   - `maintenance on|off|status`: Heimdall's maintenance window;
   - `limbo up|down|doctor|status`: manifests from the agent's folders only;
   - `rollback list|status|dry-run N`, `firewall-status`;
   - `deploy SRC DEST`: one file into a user-side place of the desktop, never anything root runs.

   Anything broader needs the owner: `sudo labyrinth-agent-window HOURS` opens the old broad list for 1 to 12 hours
   (`/etc/sudoers.d/51-labyrinth-agent-window`), and a timer closes it. `--owner-login` makes the owner's own session reach the
   agent account only after the owner's password (once per boot, kept 12 hours).
3. **2026-11-02, broad rules kept on my PC.** The narrowed rules were applied on my PC without my asking, and they stopped
   work I need done. My choice: claude-agent keeps the broad list, permanently, in `/etc/sudoers.d/52-claude-agent-broad`, while
   the work continues. `labyrinth-agent` stays installed beside it. That file is written by a script outside this repository
   (on my Desktop), not by `labyrinth-agent-setup`; removing it puts the narrowed rules back in force. Nobody narrows or widens
   these rules again without asking me first.

So today, on my PC, both are live: `labyrinth-agent` and the broad list. What the repository ships (`labyrinth-agent-setup`)
is the narrowed design. In both cases every use is logged by sudo under the agent's name, with its terminal input and output
(`sudoreplay -d /var/log/sudo-io/claude-agent -l`), and `claude-sudo CMD` is shorthand for `sudo -n -u claude-agent sudo -n CMD`.

## 2026-11-11: holes closed, and a Rust port

Porting `labyrinth-agent` to Rust (packages/tools, bin `labyrinth-agent`; the Python script and the port were compared on 796
command lines in a namespace of their own, with every program they would start checked against an allow-list) found holes in
the gate. They are closed in the script and the port alike; the list is in packages/tools/README.md ("labyrinth-agent: holes
found while porting"). The ones that mattered:

- `logs -n --vacuum-time=1s` (and `--rotate`, `--flush`, `--setup-keys`, `--cursor-file=PATH`) reached journalctl as root:
  `-n` takes its value only when it is a count, and the gate passed any word as that value. The journal could be wiped.
- `deploy` read any file as root and wrote it readable to all (/etc/shadow, my ssh keys); the source is read as the agent's
  account now. It could also replace /usr/bin/yggdrasil-spell and yggdrasil-gametime, which root services run; programs root is
  told to run are refused now, and so are the graphics card unlock's support table, the optional apps' build recipes and the boot
  menu theme's folder.
- The agent could stop the timer that closes my window (`service stop labyrinth-agent-window-close.timer`), and stop or disable
  Heimdall. The window's units are mine now; Heimdall can be started and restarted by the agent, not stopped or disabled.
- `pkg-add` checked only the names given (`pkg-add sshfs` brings openssh); what pacman would install is checked now.
- The programs it starts get a fixed environment (PATH=/usr/bin, HOME=/root, sudo's own variables) instead of relying on sudo's
  secure_path and env_reset.

The switch to the Rust gate (sudo's rule pointing at /usr/bin/labyrinth-agent from the labyrinth-tools package) is prepared on the
branch `agent-gate-rust`, not merged.

## Built, in the repository

- `profile/airootfs/usr/local/bin/labyrinth-agent-setup` (read it first): creates `claude-agent` (password locked), writes
  `/etc/sudoers.d/50-labyrinth-agent` (validated with visudo), installs `claude-sudo`, and tells Heimdall who may open a
  maintenance window. `sudo labyrinth-agent-setup --owner USER [--owner-login]`; undo the rules with `--remove`.
- `profile/airootfs/usr/local/sbin/labyrinth-agent` and `labyrinth-agent-window` (above).
- Heimdall (commit "Maintenance window..."): 
  - **Maintenance window:** `sudo heimdalld maintenance on --for 2h --reason "why"`, `off`, `status`. At most 4 hours, a reason is required,
    root only, and only the users listed in `maintenance_actors` (empty = any root user). The file lives in /etc/heimdall (Heimdall only reads
    it, so nothing it watches can open one). While open, findings from file-integrity, resource-pressure, baseline-drift, network-exposure
    and connection-watch are written as info with "[maintenance: reason]" in front. **Not softened:** prompt injection, credentials, hidden
    characters, escalation, session bursts, and everything about Limbo guests (so quarantine is unaffected). Opening and closing a window is
    itself a warning-level alert (who, why, until when), so the owner always sees it.
  - **Package-aware file-integrity:** a changed watched file is looked up in pacman's database. It now matching what the package shipped
    (SHA-256 from the package's mtree) is info ("a package update"); a package-owned file that differs, or a file no package owns, is
    critical. The old behaviour (every change critical) is kept for the plain detector.
  - The ArtCraft binaries are added to the integrity list by the deploy steps below.

## To switch it on (the owner)
```
sudo install -m755 profile/airootfs/usr/local/sbin/labyrinth-agent{,-window} /usr/local/sbin/
sudo labyrinth-agent-setup --owner USER --owner-login      # after reading it
```
and add the ArtCraft binaries to `fim_paths` in /etc/heimdall/config.json (/usr/bin/pdfcraft, pdfcraft-cli, and the same for
cadcraft, deckcraft, designcraft, filmcraft, gridcraft, photocraft, soundcraft, vectorcraft, wordcraft).

## Why this design
- Not an exemption: the content checks apply to everyone, and the log keeps every softened alert.
- A window ends by itself, is capped, needs a reason, and announces itself.
- Separate account: sudo's log says who did what; the owner's password is not needed by the agent (and need not be shared).
