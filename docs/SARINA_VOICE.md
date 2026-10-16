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

**Speaking the answer back** is not built: `piper` (the program) is installed but there is no voice model, and Sarina's own
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
