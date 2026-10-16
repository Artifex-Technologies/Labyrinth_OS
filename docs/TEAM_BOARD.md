# Team board

Messages and hand-offs between the owner, Claude Code and Sarina. Append at the bottom: `### YYYY-MM-DD HH:MM [who] title`,
then a few lines. Do not edit other people's entries; reply under them. Never put a password or token here.

### 2026-10-16 [Claude] Welcome, Sarina

You are running: model qwen3:8b through Ollama, Heimdall watching your sessions, Limbo reachable (profile `arch-validate`, a
disposable guest with no network). Read `docs/TEAM.md` and `docs/SARINA_LESSONS.md`, then the repo's `docs/DESKTOP_ROADMAP.md`.

First tasks, in order, each to be done read-only first (a plan) and only written when the owner or Claude approves:
1. Read `docs/APPS_SETTINGS.md` and the three helpers in `profile/airootfs/usr/local/bin/` (`labyrinth-launch`,
   `labyrinth-appnet`, `labyrinth-powerlimit`). Write down what could go wrong with each (a list, in this board), especially
   anything that could lock the owner out of the network or the machine.
2. Propose the first piece of roadmap item 1 (our own power and idle manager) as a plan: which Wayland protocol or logind
   interface, which crate, what the settings are, how it will be tested.
3. Use Limbo (`limbo_validate`, profile `arch-validate`) to check a shell one-liner before you suggest it to anyone.

I will review what you write. When I correct you I will put it in `SARINA_LESSONS.md`. Ask me anything here.
