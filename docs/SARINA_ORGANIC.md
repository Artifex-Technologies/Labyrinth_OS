# Making Sarina feel more organic (plan 2026-10-17; status at the end)

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

## Status, 2026-10-17

- **Built: tap/hold conversation** (`SARINA_VOICE.md`): tap Super+S for a back-and-forth that listens until you stop talking and listens again after each answer; hold for one question.
- **Built, and the biggest speed-up: a non-thinking voice model.** Her usual `qwen3:8b` thinks before it answers (2.8 s to the first word for "what is a heat pump", 4.5 s when it used a
  vault tool). `qwen3-voice:8b` is the same weights (no extra disk) with Qwen's own "non-thinking" template (the answer starts after an empty think block): first word in 0.3 s, whole answer in
  0.8 s, and 0.8 s / 1.2 s with a vault tool. The cost: it reasons less, so a retrieval answer can be a little sloppier (it said the lesson was "in file.md"). It is used only for spoken
  questions (`VOICE_MODEL=` in `~/.config/labyrinth/sarina.env`); typed questions keep the thinking model. The two models do not share a loaded copy: alternating between voice and typed costs a reload.
- **Streaming her answer into the voice (item 1): measured, not built.** With the fast model the whole answer is written in under a second, so cutting it into sentences saves only about
  half a second; the first sentence of speech takes about 1.2 s anyway. It becomes worth it for long answers.
- **Built: a Persona note** (`Persona.md` in her vault, owner-only: she reads it at the start of each spoken question and her vault tools refuse to change it, or `Home.md`).
  The effect on her wording is modest so far.
- **Not built:** the small "mm / let me check" sounds (with the fast model most answers arrive before they would play), tone-aware listening (needs your yes: it is derived from your voice),
  a wake word, natural variation in the speech.

## Status, 2026-10-17 (later): tone-awareness and delivery

- **Tone-awareness: built** (`Sarina/go/tone`, command `sarina-tone`, tested with synthetic speech). From each spoken question's recording (deleted afterwards) it measures loudness, pitch and
  pitch movement, speaking pace and pauses, compares them with this speaker's own running averages and, only for a clear difference, gives Sarina a private style hint ("sounds rushed: be
  brief", "sounds low or tired: gentle, no jokes unless they make one", "animated: matching the energy is fine"). **No recordings are needed**: it learns from ordinary questions, says nothing until it has
  heard 12 of them, and follows slow changes (the averages move with a floor on the update weight). Kept: one private file (`~/.local/state/labyrinth/tone-baseline.json`, mode 600, a few numbers).
  Settings > Sound: "Notice how I sound" (on) and "Forget how I sound". She is told never to mention the hint. The hint text does appear in her session file (Heimdall reads those). It reads how a voice differs from
  its own usual, not emotions; expect it to be wrong sometimes, which is why it only nudges style.
- **Companion talk: built** (2026-10-22). Each heard question also gets a mood (usual, tense, upbeat, low) kept in `~/.local/state/labyrinth/mood-history.jsonl` (a time and one word; no audio, no words), and `sarina-tone --pattern` says when a mood stands out over days ("low most evenings this week"). When what is said is not about the computer, `labyrinth-ask-sarina` gives her what a friend would know: the time of day, that pattern, and earlier talks and inside jokes that match by meaning (inside jokes always when a callback is invited). Talk is kept in the vault (`Talks/YYYY-MM.md`, no one-word replies, nothing that looks like a secret); laughter in the recording turns her line before into an inside joke (`Entities/Inside jokes`); a retold joke is caught and replaced once; closing help-desk offers and bracketed asides are dropped. `LABYRINTH_TALK_LOG=0` keeps nothing; `sarina-tone --forget` forgets the voice. Ideas, not code, from psy-supabase (emotion trajectory, recurring themes), Personality-Illusion (show personality in behaviour, test behaviour), synthetic-conversation-generation (multi-turn evals) and RealtimeVoiceChat (still to do: streaming and interruption).
- **Delivery and small sounds: built.** She may start a sentence with `[joking]`, `[unsure]`, `[urgent]`, `[gentle]` or `[serious]` (the voice prompt and her Persona note say when, and to be sparing).
  The voice service then changes how steady or lively the voice is (sampling temperature 0.92 / 0.85 / 0.72 / 0.62 / 0.55), the pauses around the sentence, and sometimes adds a small sound: a soft chuckle after a
  joke (60 %), a hesitation before an unsure sentence ("Hmm...", "Um...", "Well..."), a quiet sigh before comfort (35 %); urgent ends with "!"; serious adds nothing. Any other `[tag]` in her text is dropped, so she cannot
  make the voice cough or laugh by accident. Tested: the text handling (`tests/test_voiced_text.py`) and valid audio for every style; played once: not judged by the assistant.
- **Fixed on the way:** the half-precision speech encoder produced NaN features for the reference voice on the graphics card (the whole sentence came out as a pop); the conditioning is now made once in full precision
  on the processor.
- Still not built: a wake word, streaming sentences into the voice, a spoken "mm" while she thinks.
