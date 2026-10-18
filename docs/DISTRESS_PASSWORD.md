# Distress password

A second password that does something quietly different when it is typed where the real one is asked for.
Settings > Security > Distress password. Opening the page needs your account password; turning it on asks "are you sure" and the
password again; nothing is stored until **Save**.

## Rules
- Typed **once**, or **N times within M seconds**. Fewer than that behaves exactly like a wrong password.
- Works **forever** or for a day, week or month.
- It must differ from the real password. It is stored only as a yescrypt hash in `/etc/labyrinth/distress.json` (root, 0600).

## Section 1: saving needs no extra password
- **Silent login**: the screen unlocks as normal; a hidden event is written (root-only log, and the journal as `labyrinth-distress`).
- **Decoy session**: lock the real account and open a decoy one. *Not live yet*: it needs Labyrinth's own session manager
  (`/usr/lib/labyrinth/decoy-session`). Until that exists nothing is changed and the account is never locked.

## Section 2: saving asks for the account password a second time, after a warning
Works with Section 1 or on its own. One of: run commands; wipe keys (LUKS keyslots, SSH/GPG/keyrings) and/or shut down;
factory reset (remove accounts and data, restart into first-time setup); wipe the whole system (destroy partition tables and
encryption headers of every internal disk, power off). "Show" lists exactly what each would do on this computer.

## Where it works
- **Labyrinth's lock screen** (`labyrinth-lock`): asks the helper only after the real password was wrong, and only if the feature is set up.
  Not live until Labyrinth runs on a compositor with session-lock.
- **Terminals and other prompts**: needs a PAM module, not built yet (the option is shown switched off).

## Parts
`packages/distress` (`labyrinth-distress`, root, through pkexec, action `org.labyrinth.distress`): commands `open`, `save`, `check`,
`plan`. Every change re-checks the account password through PAM. Dry run: `LABYRINTH_DISTRESS_DRY=1` (only honoured when the helper is
run directly; pkexec clears the environment). The destructive steps are unit-tested as plans and dry runs only, never executed here:
try them in a Limbo VM before relying on them.
