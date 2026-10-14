# Claude Code on Labyrinth OS

Checked in the VM on 2026-10-13: the native installer runs, `claude --version` gives 2.1.296, `claude -p`
asks to sign in, and the sandbox tool (`bwrap`) runs. Not checked: signing in and a real session.

## Install and start

    labyrinth-claude-code          # installs it for this user (Anthropic's installer, into ~/.local/bin)
    claude                         # in a project folder; the first run opens the browser to sign in

`labyrinth-claude-code status` shows what is there and what is missing; `labyrinth-claude-code update`
updates it. It is not in the ISO itself: it is Anthropic's, updates itself, and signs in per user.

## What it needs, and where that comes from

| Needs | Provided by |
| --- | --- |
| a terminal and a shell | Ghostty and bash (the base system) |
| `~/.local/bin` on the PATH | `/etc/profile.d/labyrinth-path.sh` (login shells) and `/etc/xdg/plasma-workspace/env/labyrinth-path.sh` (the desktop session, so a terminal started from the dock has it). Before this the installer said the folder was not on the PATH. |
| `git` | the base system (it was missing from the image before) |
| `rg`, `ssh`, `curl`, `tar`, `unzip`, `gcc`, `make`, `python3` | already in the base system |
| Node.js and npm (MCP servers are started with `npx`), `jq`, the GitHub CLI (`gh`), `pip`, `fd` | the `dev` component, on in every preset except `minimal` |
| `bubblewrap` and `socat` (Claude Code's sandbox) | the `dev` component |
| a browser, to sign in | Vivaldi (default) or Firefox, through `xdg-open` |
| the network | NetworkManager; the firewall only blocks incoming connections |

Credentials are kept by Claude Code in the user's home folder (`~/.claude`); no keyring service is needed.

## Things to know

- Claude Code's own files live in `~/.claude` and `~/.local/share/claude`; they are the user's, not part
  of the OS image.
- The `dev` component is optional (`labyrinth-apps`); with only the `minimal` preset `git` is still there
  but Node, npm and the sandbox are not.
