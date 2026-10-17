# Making Sarina feel more organic (plan, 2026-10-17: nothing here is built yet)

Prompted by `x4nth055/emotion-recognition-using-speech` (MIT, Python 3.6 era, 2019): classical models (SVM, random forest, MLP, a small Keras RNN) on MFCC, chroma,
mel, contrast and tonnetz features, trained on three small ACTED datasets (RAVDESS, TESS, EMO-DB). Its own numbers are 77 to 90 % on three to five emotions on a single
split (the SVC example scores 1.0 on training data and 0.81 on test: overfitting). Acted speech by a few dozen actors does not carry over to someone talking normally
to a computer, the dependencies are years old, and the project looks dormant. So: **do not port the models; take the idea.**

## What would make the biggest difference (in the order I would build it)

1. **Speak while she is still thinking.** `sarina chat --stream` already prints the reply as it is produced. Cut it into sentences as they arrive and hand each to the voice
   service at once: the first words come about a second and a half after you let go instead of after the whole answer. Needs one long-lived speaker (the panel's island follows
   it for the whole answer) instead of one process per answer. No privacy cost.
2. **Small human noises, in her voice:** a quick "mm" or "let me check" played only when an answer takes longer than about a second and a half, from short clips made once with
   the voice and cached. Never for quick answers. No privacy cost.
3. **Tone-aware listening, quietly (opt-in, local, never stored).** A coarse read of how you sound (calm, tense, low, upbeat) from loudness, speaking rate, pauses and pitch
   movement (computed in Go; plain signal processing, no model) and, if it proves worth it, a modern speech-emotion model run through the same onnxruntime the voice already uses
   (a model trained on real speech, not actors). It is used only as a nudge to her style (shorter and gentler when you sound rushed or low); she never announces it ("you sound sad"
   is both presumptuous and often wrong), and it is shown only if you ask "how did I sound?". It is voice-derived personal data: nothing is saved, nothing leaves the machine, and
   Heimdall treats it like a recording. Expect modest accuracy.
4. **A follow-up window** (opt-in): for a few seconds after she finishes, a second hold of the shortcut is not needed to continue the conversation. Always-listening or a wake word
   (an on-device detector such as openWakeWord via onnxruntime) is a bigger privacy decision: not without your say-so.
5. **A persona note in her vault** (how she talks: warm, a little shy, short, honest about what she did not check), read at the start of every session, and what she learns about how
   you like answers written down as notes (`Entities/`, `Lessons/`), so she is consistent between days.
6. **Natural variation in the speech**: sentence-type aware sampling (a question rises, an apology is softer), within what the ONNX Chatterbox exposes.

## Go or other
Signal features (loudness, rate, pauses, pitch by autocorrelation) and the model runner belong in `Sarina/go/voice` (Go, next to the Piper and Chatterbox code). The streaming
glue is shell and Python today (`labyrinth-sarina-ask`, `labyrinth-sarina-speak`, the voice service) and can move to Go once it is settled.
