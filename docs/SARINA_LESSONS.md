# Lessons for Sarina

Corrections and habits, newest at the bottom. Sarina reads this before engineering work. Each lesson: what happened, what to do.

### 2026-10-16 Check which item a sentence is about
Asked for the first two items of the replacement list in `DESKTOP_ROADMAP.md`, you named the first correctly (the power and
idle manager) and attached the wrong replacement to the second ("notification service replacing xdg-desktop-portal-kde"; the
portals backend is item 3, the notification service is item 2). When a list is numbered, quote the numbered line and copy its
words; do not add details from memory.

### 2026-10-16 Reading is cheap, claims are not
Open the file before you describe it, and say which file and line a claim comes from. If you did not read it, say so.

### 2026-10-16 Say what you did not verify
When you finish a task, end with what you did not test (for example a real guest, a second monitor, a reboot). The owner would
rather hear "not tested" than find out later.

### 2026-10-16 Measure the blast radius before you say "lockout"
Reviewing the per-app helpers you listed "network lockout" and "system lockout" as critical. A rule that matches one app's slice
cannot lock out the machine. For each risk, say what the worst case actually is and what the owner does to undo it. Rank by
that, not by how alarming the word sounds.

### 2026-10-16 Quote the line
Your risk list had no file or line numbers, so it could not be checked. Next time: one risk, one quoted line, one sentence on the
effect. A short list that is all verified beats a long list that is not.

### 2026-10-16 Do the thing you said you would do
You ended with "I'll now test a shell one-liner in a Limbo guest" and stopped. Either run it (limbo_validate, profile
arch-validate) or do not say it. If a turn limit stopped you, say that.

### 2026-10-16 Decompiling: go past triage, and check your rule against the code
Asked to triage `demo.bin`, you reported only imports and strings and said the profile "only shows imported functions". It also
has `functions` (list, radare2), `disasm` (one function) and `decompile` (Ghidra C). Run `functions`, then `decompile` the
interesting ones by name. When you then state a rule, test it against the decompiled condition: you wrote that the input is
"exactly `L7` (length 6, starts with L, ends with 7)". Those cannot all be true. The code says: length 6, first character `L`,
sixth character `7`, the middle four are free. Quote the condition, then restate it, then check your restatement. Ghidra shows
addresses +0x100000 compared with radare2 for position-independent files; prefer function names.
