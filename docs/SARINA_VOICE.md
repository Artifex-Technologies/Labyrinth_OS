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
