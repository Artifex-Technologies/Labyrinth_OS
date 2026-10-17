# Asking Sarina by voice

Press **Super+S**: Fylgja (the notch) drops down, the microphone starts recording, and a card shows
"Listening…" with the seconds. Press **Super+S** again (or click **Send**) to send the question.
**Esc**, or a click outside the card, cancels it and throws the recording away. Sarina's answer (or the
reason there is none) appears in the same card; Esc or a click outside closes it.

It is a press-to-start, press-to-send toggle, not hold-to-talk: KDE's global shortcuts only report the
press, not the release.

## What is built (Labyrinth's side)

- `labyrinth-panel` serves `org.labyrinth.Panel` on the session bus with `AskSarina`; the shortcut
  runs `busctl --user call org.labyrinth.Panel /Panel org.labyrinth.Panel AskSarina`
  (`labyrinth-sarina-ask.desktop`, bound to Meta+S in `kglobalshortcutsrc`).
- Recording uses `pw-record` (PipeWire): mono, 16 kHz, 16-bit, into a private folder
  (`$XDG_RUNTIME_DIR/labyrinth-sarina/`, mode 700, in memory-backed storage). The file is deleted as soon
  as the question has been handed over, or cancelled.
- The question goes to `labyrinth-sarina-ask FILE.wav`, which prints Sarina's answer on stdout. A
  non-zero exit shows its printed line as the reason. It does not start her model server or download
  anything.
- Nothing is sent over the network by Labyrinth. Whether Sarina does so is up to Sarina (she is meant to
  be local).

## Updated 2026-10-16: how the question becomes an answer (no audio was played or recorded while doing this)

`labyrinth-sarina-ask` used to call `sarina ask --audio`, which does not exist in Sarina (her real voice mode is the separate
terminal command `sarina voice-chat`, which needs a Python speech library). It now does the two steps itself, all local:

1. **Speech to text** with whisper.cpp: `whisper-cli -m MODEL -f recording.wav`. The model is looked for in
   `$LABYRINTH_STT_MODEL`, `~/.local/share/labyrinth/stt/ggml-*.bin`, then `/usr/share/labyrinth/stt/ggml-*.bin`.
2. **The words go to Sarina** as a normal read-only question through `labyrinth-ask-sarina`, so her session is in the folder
   Heimdall watches. She is asked for at most three short plain sentences (the card is small). The card shows what she
   heard (`Heard: "..."`) above her answer, so a misheard question is obvious. Markdown and `<think>` blocks are removed.

Without whisper.cpp and a model, Super+S says "Listening is not set up yet..." on the card instead of an obscure error.

**Set up on this machine 2026-10-16:** whisper-cpp installed, `ggml-base.en.bin` (checksum verified) in `~/.local/share/labyrinth/stt/`. Tested only with a silent file (no sound): the pipeline runs end to end in ~6 s; on silence whisper says "you", which the script now treats as no speech. Not tested: a real voice, the microphone, or the card. **To set it up elsewhere:** `sudo pacman -S whisper-cpp`, then put
`ggml-base.en.bin` (or `ggml-small.en.bin` for better accuracy) from the whisper.cpp model list into
`~/.local/share/labyrinth/stt/`. Then Super+S once, speaking, and check the "Heard:" line.

**Speed (measured 2026-10-16):** whisper.cpp base.en transcribes in about 0.8 s (3 s of audio); Sarina's answer is the rest: about 2 s
once the model is loaded, plus about 2 s more if Ollama had unloaded it. Pressing Super+S now also runs `labyrinth-sarina-warm`, which
loads the model while you are still speaking (it unloads again 10 minutes later, so it does not hold the graphics card's memory
while gaming). So a short question is about 3 s from Send to answer. **Smaller model for voice: tried, not worth it.** qwen3:4b averaged ~3.6 s per short question against ~2.7 s for qwen3:8b on this machine (the
8b already runs fast on the graphics card, and the 4b was a little chattier). The setting stays (`VOICE_MODEL=` in
`~/.config/labyrinth/sarina.env` or `$LABYRINTH_VOICE_MODEL`, also used by the warm-up) for a weaker computer; it is not set here, and the 4b
was removed. `ggml-tiny.en.bin` was not tried (it mishears more and the speech step is only 0.8 s).
**The card shows what she heard while she is still working:** the script prints `Heard: "..."` the moment the words are known and the panel
shows it under "Sarina is thinking…".

**Speaking the answer back: BUILT 2026-10-16** (see the section at the end). The old note follows, kept for history: `piper` (the program) is installed but there is no voice model, and Sarina's own
`voice-chat` uses a Python piper. A later step: `piper --model VOICE.onnx --output-raw | pw-play` for the answer, behind a setting,
off by default.

Tests: `tests/test_labyrinth_sarina_ask.py` runs the script with stand-ins (no microphone, no model, no sound).

## What is NOT known yet

- **Sarina's interface.** Her repository was not available when this was written. The script assumes
  `sarina ask --audio FILE` prints the answer as text. That line is a proposal; change it (or Sarina's
  CLI) so the two agree. If Sarina only takes text, the speech-to-text step has to live somewhere: her
  Python voice helper, or a small local speech model run before she is called.
- Whether she can speak the answer back (not built; the card shows text).
- Microphone access, levels, the right input device, and whether recordings are clear enough, all need
  testing on the real machine.

## Tested so far (in the VM, no microphone, no Sarina)

- With no microphone the recording ends at once and the card says so.
- With a stand-in for `pw-record`: the Listening card with its seconds, Send, and the hand-over to
  `labyrinth-sarina-ask`; the card showed its "model server is not running" message.
- NOT tested: real audio, a real answer on the card, a long answer, closing during "thinking".


## Sarina speaks her answers (2026-10-16)

After a question you SPOKE (Super+S), the panel runs `labyrinth-sarina-speak` with the answer (not the `Heard:` line): Piper, a local neural
voice, makes a wav in the private runtime folder and `pw-play` plays it; the wav is deleted. Settings > Sound > "Sarina's voice" has the switch
(`sarina_speaks` in panel.conf, on by default) and a voice list with "Hear it" (the choice is one line in `~/.config/labyrinth/sarina-voice`).
She stops when you press Super+S again, Esc, or click outside the card.

- **Set up here:** Piper 1.8.0 in `~/.local/share/labyrinth/tts/venv` (from PyPI, no root) and three voices (en_US-amy-medium default,
  en_GB-jenny_dioco-medium, en_US-lessac-medium). On another computer: `labyrinth-sarina-voice-setup`. The system package named `piper` in Arch is a
  gaming-mouse tool, not this; the AUR has `piper-tts` too, but the venv avoids root and AUR builds.
- **Speed:** about 1.7 s to make a sentence on this machine, so the answer card appears first and the voice follows a moment later.
- **Tested:** the script and the cleaning of the text with a stand-in for Piper (`tests/test_sarina_speak.py`), and the real voices writing real wav
  files (no playback). **Not tested:** hearing it, the panel starting it after a real answer, Esc stopping it. Voice downloads were not checksum-verified
  by us (Hugging Face serves them over HTTPS); the sha256 of the three at download time: amy 469c630d..., see `sha256sum` on the files.
- **Not built:** speaking while she is still writing (it waits for the whole answer), a different voice for warnings, speaking typed questions
  (it is only for spoken ones, on purpose: she does not talk at you when you typed).


## Hold to talk, and the voice island (2026-10-16)

- **Hold Super+S, talk, let go.** No card opens and there is no Send button. The panel registers the shortcut itself with KDE's global-shortcut
  daemon (`org.kde.KGlobalAccel`, component `labyrinth-panel`, action `ask-sarina`; change it in System Settings > Shortcuts) because only
  registered actions report the key coming UP (`globalShortcutReleased`). A tap shorter than a third of a second is not a question: it just stops her talking.
  The old desktop-file shortcut (`labyrinth-sarina-ask.desktop`) is unregistered at startup. The D-Bus call `AskSarina` still works as a press-to-start,
  press-to-send fallback.
- **The island:** while you talk, while she thinks, and while she answers, the notch grows downwards into a black pill with a glowing wave in it
  (`packages/panel/src/voicewave.rs`, the iOS 9 Siri wave: sine curves squeezed at the ends, added together so crossings glow). The wave follows
  the microphone (the newest 120 ms of the recording as it is written) and, while she speaks, her voice: `labyrinth-sarina-speak` writes the
  loudness of the speech to `$XDG_RUNTIME_DIR/labyrinth-sarina/speech.env` (one number per 50 ms) just before it plays, and the panel reads it by the clock.
  Colours: copper (like the ribbon picture), Siri, or Labyrinth, in Settings > Sound > Sarina's voice.
- **No answer card.** She answers aloud. If speaking is off, the answer is a notification; failures ("I did not catch that") are a notification and are
  said aloud too.
- **Tested:** the wave and levels (unit tests and pictures), the key press and release through the real shortcut daemon (a synthetic Super+S held for
  1.3 s showed the island, released it, and the island went away after the answer path). **Not tested by a person:** talking, hearing the answer, how the wave feels.

## Graphics-card memory (2026-10-16, RTX 2080 Ti, 11 GB)

Measured with `qwen3:8b` at 16k context: the model used 7272 MiB (about 5.2 GB of weights and 2.2 GB of conversation memory), the desktop and
browsers about 0.9 GB. Flash attention plus an 8-bit key/value cache (`OLLAMA_FLASH_ATTENTION=1`, `OLLAMA_KV_CACHE_TYPE=q8_0`, a drop-in in
`/etc/systemd/system/ollama.service.d/labyrinth-vram.conf`, installed and Ollama restarted) brought it to 6188 MiB: **1.08 GB freed, speed 90 to 88
tokens/s** (a 2 % difference, same ten-line test). A smaller context (8k) would free about 0.5 GB more; the voice answer model is unloaded 10
minutes after the last question. A bigger card is not needed for a voice model of about 350M parameters: it should fit in what is left (about 4 GB).

## Chatterbox Turbo, first test (2026-10-16)

Chosen as the next voice: MIT licence, copies a voice from about 10 seconds of audio, English. Resemble publishes it as four ONNX graphs
(`ResembleAI/chatterbox-turbo-ONNX`: speech encoder, token embeddings, an autoregressive language model with a KV cache, and a conditional
decoder, in fp32/fp16/q8/q4/q4f16; fp16 is about 1.7 GB, q4f16 about 0.6 GB in total), so it needs no PyTorch: `scripts/voice/chatterbox_onnx_test.py`
runs it with onnxruntime. Files are in `~/.local/share/labyrinth/tts/chatterbox/` (fp32 downloaded, 3.1 GB), a separate venv `cb-venv`, and three reference
clips cut from the owner's real recordings (`refs/ref1-3.wav`, 9 s each, 24 kHz).

Things the short ONNX example in the model card does not tell you (found while testing):
- Encode the text WITHOUT special tokens and add exactly one `<|endoftext|>` (50256): the embedding graph reads the last token of its first input as the speech-start slot. Two of them, or none, breaks it.
- Always taking the most likely token (the example's `argmax`) makes the voice stall and leave long silences (19 s of audio for 7 s of words). Sampling with
  temperature 0.8, top-k 1000, top-p 0.95 (Chatterbox's own defaults) plus the repetition penalty 1.2 gives sensible lengths (about 8.6 s for the test sentence).
- Cut the digital silence at the end.

Speed on the CPU in fp32 (no GPU build of onnxruntime installed): about 1.4x real time (7 to 9 s to make 7 to 9 s of speech), plus 6 s to load. Not usable
for conversation yet; the GPU or the quantised graphs should be several times faster. Samples to compare (not heard by the assistant): `~/Desktop/Sarina-voice-samples/chatterbox-*.wav`.

**Go or Rust?** Not for the model itself: the Python is only a short driver (tokenize, loop over the language model, sample, run the decoder); the model is the ONNX files.
That driver is about 150 lines and ports to Go on top of `go/voice` (same onnxruntime, plus a GPT-2 style tokenizer and the sampling loop), which is the
plan once the voice is chosen. Rewriting the network itself in Go or Rust would be weeks of work for no gain over onnxruntime.

## Misheard names, spoken paths and pronunciation (2026-10-16)

A spoken "create a file in slash home slash zlothra slash documents slash obsidian slash sarina" was written by the speech step as `home/lothra/document/obsidian/serina`
and she tried that as a real path ("invalid", then offered to create the folder). Fixes: whisper is given the names to expect (`--prompt "Zlothra, Sarina, Labyrinth,
Heimdall, ..."`; add more, one comma-separated line, in `~/.config/labyrinth/stt-words`); a spoken question may now use read tools and her vault tools only (`labyrinth-ask-sarina --voice`:
`read_file, list_dir, glob_search, grep_search, vault_search, vault_read, vault_write`, with write permission but no other write tool), and the prompt tells her never to use a path
from the spoken words (vault tools take a name inside the vault, like `Inbox/Voice test`); the vault tools explain that when a path is refused.
**Pronunciation:** the voice reads letters, so names are respelled before speaking: `Sarina` becomes `Suhreenuh` (suh-REE-nuh), `Zlothra` becomes `Zlawthruh` (the owner's "Zlaw-thr-uh", Sloth + Mothra), `Fylgja` `Filyah`, `Chymaera` `Kymeeruh`, `Heimdall` `Haymdawl`. Add or change entries
in `~/.config/labyrinth/pronunciations` (`word = how to spell it so it is said right`). Tested with a copy of the vault: a spoken, misheard path is now turned into a note
`Inbox/Voice test.md` (committed to git). Not tested: the pronunciation by ear.
