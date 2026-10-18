# Distress password

A second password that does something quietly different when it is typed where the real one is asked for.
Settings > Security > Distress password. Opening the page needs your account password. Nothing is stored until **Save**.

## All the options

### Turning it on
| Option | What it does |
| --- | --- |
| **Use a distress password** (switch) | Off by default. Turning it on asks "Turn on the distress password?" with a warning and your account password. Turning it off and saving disables it (the settings are kept). |

### The distress password
| Option | What it does |
| --- | --- |
| **New distress password** / **Type it again** | At least 6 characters, must be different from your real password, both boxes must match. Leave empty to keep the one already set. Only a yescrypt hash is stored, in `/etc/labyrinth/distress.json` (root only, mode 0600). |

### When it counts
| Option | What it does |
| --- | --- |
| **How many times: Once** | A single entry fires it. |
| **How many times: Several times** | It fires only after it has been typed **Entries needed** times within **Within** seconds. Fewer than that behaves exactly like a wrong password. |
| **Entries needed** | 2 to 10 (default 3). |
| **Within** | 10 to 600 seconds (default 60). |
| **How long it works** | *Keep as it is*, *Forever*, *One day*, *One week* or *One month* (counted from Save). After that the password is ordinary again. The page shows how long it still has. |

### Section 1: saving needs no extra password
Pick **one** of these.

| Option | What it does |
| --- | --- |
| **Nothing** | No Section 1 action. Use this when you only want something from Section 2. |
| **Silent login** | You are let in as normal; nobody is told. A hidden event is written to a root-only log and to the journal (tag `labyrinth-distress`). |
| **Decoy session** | Locks your real account and opens a decoy one. *Not live yet:* it needs Labyrinth's own session manager (`/usr/lib/labyrinth/decoy-session`). Until that exists, choosing it changes nothing and your account is never locked. |
| **Also in terminals and other password prompts** (switch) | Makes the distress password also work at sudo, the login screen, su and administrator questions, through the PAM module (see below). Off by default. |

### Section 2: saving shows a warning and asks for your password again
Works with Section 1 or on its own (with Section 1 set to Nothing, this is all it does). Pick **one**; the actions run a moment after the
answer, in the order listed, and a step that fails does not stop the next one.

| Option | What it does |
| --- | --- |
| **None** | No Section 2 action. |
| **Run commands** | The commands you type (one per line) run in order, as the administrator. |
| **Wipe keys** | Two switches, both on by default. **Wipe the keys**: deletes `~/.ssh`, `~/.gnupg`, the keyrings (`~/.local/share/keyrings`, `kwalletd`) and the Labyrinth vault of every normal account, and erases the encryption keys (LUKS keyslots) of every encrypted partition, so that data can never be read again. **Shut down**: powers the computer off immediately. |
| **Factory reset** | Removes every normal account and its home folder, resets `/etc/labyrinth`, `/var/lib/labyrinth`, the account list, saved network connections, Bluetooth pairings, Flatpak data and caches, marks the computer for first-time setup, and restarts. |
| **Wipe system** | Erases the encryption keys of every encrypted partition, destroys the partition table and the first 64 MiB of every internal, non-removable disk (USB sticks and the live medium are left alone), discards the blocks, and powers off. No operating system is left. |
| **See exactly what it would do on this computer: Show** | Lists the real steps for the chosen option on this machine (accounts, partitions and disks found). Nothing is run. |

### Saving
| Step | What happens |
| --- | --- |
| **Save** (Section 1 only) | Saves with the account password you opened the page with. |
| **Save** (Section 2 chosen) | Shows **Save section 2?**, a warning that fits the option, and asks for your account password once more. **I understand. Save** saves; **Cancel** goes back. |
| **Discard changes** | Puts every box back to what is saved. |

### What it has done
A list of every change and every time it fired, newest first.

## Where it works
- **Labyrinth's lock screen** (`labyrinth-lock`): asks the helper only after the real password was wrong, and only if the feature is set up.
  Not live until Labyrinth runs on a compositor with session-lock.
- **Terminals and other prompts** (sudo, login, su, administrator questions): the PAM module `pam_labyrinth_distress.so`
  (`packages/distress-pam`). Turning on "Also in terminals and other password prompts" and saving adds one line to
  `/etc/pam.d/system-auth` (a copy of the original is kept as `system-auth.labyrinth-original`; turning it off removes the line):
  `-auth [success=done default=ignore] pam_labyrinth_distress.so`. The leading `-` makes PAM skip it silently if the file is ever
  missing, so it cannot lock anyone out. It does nothing at all (not even a prompt) until a distress password is set up. Only a
  "silent login" lets you in; every other choice acts and the prompt still says the password was wrong. Tested on a throw-away
  PAM service; installed on this system, not yet switched on.

## Parts
`packages/distress` (`labyrinth-distress`, root, through pkexec, action `org.labyrinth.distress`): commands `open`, `save`, `check`,
`plan`, plus `check-for`, `pam-enable` and `pam-disable` for the PAM module (root only, never through pkexec). Every change re-checks
the account password through PAM. The helper ignores environment variables in a normal build (it also runs as root from PAM); test
builds (`--features test-paths`) can use a dry run (`LABYRINTH_DISTRESS_DRY=1`). The destructive steps are unit-tested as plans and
dry runs only, never executed here: try them in a Limbo VM before relying on them.
