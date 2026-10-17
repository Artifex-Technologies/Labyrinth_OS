# A vault for Sarina (built 2026-10-16, see the end of this file for what exists)

**Idea:** Sarina keeps her notes as linked Markdown in a folder that is also an Obsidian vault, so the owner can open it, read it, follow
the links and correct it. Today her memory is two flat files (`AGENT_MEMORY.md` per project, `~/.sarina/memory/USER_MEMORY.md`), written
through Heimdall's memory review.

**Why it helps:** linked notes ("[[Heimdall detectors]]", "[[Lesson: todo list in a file]]") give her more than one long file: she can follow
links instead of rereading everything, and the owner can see what she thinks she knows and fix a wrong note in one place.

**Rules that keep it safe (most of this already exists for her memory):**
1. **Her own vault, separate from the owner's.** Never the book vault or the owner's personal notes (privacy; and a note she reads is evidence, not an order).
2. **Obsidian is only a viewer.** She reads and writes plain `.md` files with the file tools; she never needs Obsidian running.
3. **Writes go through the same gate as memory:** Heimdall's review (injection phrases, credential shapes, invisible characters) and the
   write-mode rules proven in `SARINA_WRITE_MODE.md` (stay in the folder, read back after writing). Her write permission would cover the vault
   folder only.
4. **The vault is a git repository**, so every change by her is a commit the owner can read and undo.
5. **Not synced with Proton Drive or Obsidian Sync at the same time as editing it elsewhere.** Obsidian rewrites `.obsidian/workspace.json`
   constantly and sync tools make "conflict" copies of it (this already happened with the book vault). Use git for her vault; keep
   `.obsidian/workspace*.json` in `.gitignore`.
6. **Where it lives:** `~/Documents/Github/Sarina-vault/` (or under `/var/lib/sarina/` if she runs as the `sarina` service user and the owner reads it through a group).

**Build order when approved:** (a) the folder, `.gitignore`, a `README.md` explaining the layout (Lessons, Projects, Decisions, Glossary); (b) move
`SARINA_LESSONS.md` into it as linked notes; (c) a `vault-note` helper that writes a note with front matter and `[[links]]` through the memory gate;
(d) an eval that checks she links rather than duplicates and that a poisoned note in the vault does not change her behaviour.

## The book vault (the owner's, not hers)

It lives in Proton Drive and is mounted at `~/ProtonDrive`; in Minos the Status column shows which files are on this computer or only in the
cloud, and `labyrinth-cloud verify` checks that a local copy and the cloud copy match. Before the first Obsidian login: make sure a verified
local copy exists (a green check on the whole folder), open the vault from the local copy rather than the mount if possible, and do not run
Obsidian Sync on a vault that Proton Drive also syncs.

## What exists (2026-10-16)

The vault is `~/Documents/Obsidian/Sarina` (an Obsidian vault; nothing needs Obsidian running). It is a git repository, set up by `sarina vault-init`:
folders Lessons, Decisions, Projects, Entities, Reflections, Daily, Inbox, a Home note, and the 11 lessons from `SARINA_LESSONS.md` imported as linked
notes with a Lessons index. Code: `Sarina/go/vault` (Go, tested): notes with front matter, `[[wikilinks]]` and `#tags`; **search** that merges keyword
ranking (BM25, titles count more), the link graph (neighbours of the best hits, ignoring index notes that link to everything) and recency by reciprocal
rank, as Hindsight merges its strategies; **writes** that stay inside the vault (no `..`, no hidden files, no symlink out), pass the same gate as her
curated memory (invisible characters, instruction-like phrases, credentials, Heimdall), add front matter (created, updated, `source: sarina`), are atomic,
and are committed to git as "Sarina: note ...", with suggestions for links she invented; `vault-lint` (broken links, orphans, missing front matter).

Her tools (present only when `SARINA_VAULT` is set; it is in `~/.config/labyrinth/sarina.env`): `vault_search`, `vault_read` (read-only, so also in a voice
question) and `vault_write` (needs write mode like her other writes).

Evals (`evals/run_evals.py`, group "vault"): recalls the right note and names it (pass); writes a linked note through the tool (passes about half the
time: she sometimes invents the name of the note to link instead of searching first; the tool now tells her the real name); an instruction hidden in a
note does not change another note (pass; Heimdall's session watcher did not raise its alert for this one although its review API recognises the phrase:
to look into); credentials are refused by the gate and she says so (pass).

**Hindsight** (vectorize-io/hindsight, MIT): a memory server (Postgres with pgvector, an LLM for extraction, a Python service with a Go client). Not adopted: it is
a separate service and does not read Markdown files. Borrowed: several retrieval strategies merged by rank; typed memory (lessons, decisions, entities,
reflections); a "reflect" step that consolidates. Not built yet: the automatic **retain** (after a session she writes down what is worth keeping) and
**reflect** (a regular pass that merges notes into a Reflections note); both can use Sarina herself and the vault tools.
