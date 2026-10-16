# The engineering agent's own account (Claude) and Heimdall's maintenance window

Decided with the owner 2026-10-16: the engineering agent (Claude) works under the owner's instructions and is not invisible to
Heimdall. To tell its actions apart from the owner's (and Sarina's), and to avoid passing the owner's password around, it gets its own
audited account. This is for attribution and review, not containment.

## Built, in the repository (nothing below is active on this machine until the owner runs it)
- `profile/airootfs/usr/local/bin/labyrinth-agent-setup` (read it first): creates `claude-agent` (password locked), writes
  `/etc/sudoers.d/50-labyrinth-agent` (validated with visudo), installs `claude-sudo`, tells Heimdall who may open a maintenance
  window. `sudo labyrinth-agent-setup --owner zlothra`; undo the rules with `--remove`.
  - the agent may run as root, without a password: pacman; systemctl, journalctl, loginctl, systemd-run; `heimdalld maintenance`, limbo, ufw;
    `labyrinth-*`; install, mkdir, chown, chmod. pacman, systemd-run and install are in effect root: the safeguard is the record, not a fence.
  - every use is logged by sudo under the agent's name with its terminal input and output (`sudoreplay -d /var/log/sudo-io/claude-agent -l`).
  - the owner can run commands as the agent without a password (`sudo -u claude-agent ...`); `claude-sudo CMD` is shorthand for
    `sudo -u claude-agent sudo -n CMD`.
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

## To switch it on (the owner, or Claude once its tool permissions allow)
```
cd ~/Documents/Github/Heimdall && go build -o /tmp/heimdalld ./cmd/heimdalld
sudo install -m755 /tmp/heimdalld /usr/bin/heimdalld && sudo systemctl restart heimdall
sudo labyrinth-agent-setup --owner zlothra      # after reading it
```
and add the ArtCraft binaries to `fim_paths` in /etc/heimdall/config.json (/usr/bin/pdfcraft, pdfcraft-cli, and the same for
cadcraft, deckcraft, designcraft, filmcraft, gridcraft, photocraft, soundcraft, vectorcraft, wordcraft).
Heimdall's integrity detector will flag the swapped /usr/bin/heimdalld once (expected; open a window first to see it softened:
`sudo heimdalld maintenance on --for 30m --reason "deploying Heimdall update"` needs the NEW binary, so on this one deploy it will simply alert).

## Why this design
- Not an exemption: the content checks apply to everyone, and the log keeps every softened alert.
- A window ends by itself, is capped, needs a reason, and announces itself.
- Separate account: sudo's log says who did what; the owner's password is not needed by the agent (and need not be shared).
