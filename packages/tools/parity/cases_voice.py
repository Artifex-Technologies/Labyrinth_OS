"""The cases of Sarina's two voice scripts (loaded by compare.py): labyrinth-sarina-ask and labyrinth-sarina-speak. They run in
cases_root.py's world as the user does (cases_user.py's home, /home/tester). whisper-cli, curl, sarina, sarina-tone,
labyrinth-ask-sarina, Piper (the venv's python), pw-play, pkill and the voice service's client (/usr/bin/sarina-voice, laid over
this computer's) are stand-ins that write down their calls: nothing records from a microphone and nothing is played. The
loudness the speak script worked out with the venv's Python is worked out here by this computer's python3 from the same
recording, so the port's own numbers are compared with Python's."""
import os
import socket
import struct

import cases_root as C
import cases_user as U

W = C.W
HOME = U.HOME
RUN = "/run/user/1001/labyrinth-sarina"
CFG = HOME + "/.config/labyrinth"
WAV = HOME + "/q.wav"
MODEL = HOME + "/.local/share/labyrinth/stt/ggml-base.en.bin"
RULES = "/usr/share/labyrinth/sarina-voice-rules.txt"

# ---------------------------------------------------------------- labyrinth-sarina-ask

SA = "labyrinth-sarina-ask"
# what labyrinth-ask-sarina was given besides its arguments (the timing, a number that changes, as <S>)
ASK_ENV = ('ws=${LABYRINTH_VOICE_WHISPER_S-unset}; case $ws in [0-9]*.[0-9][0-9]) ws="<S>";; esac\n'
           'echo "  OPENAI_MODEL=${OPENAI_MODEL-unset} SARINA_FALLBACK_MODELS=${SARINA_FALLBACK_MODELS-unset} LABYRINTH_TONE=${LABYRINTH_TONE-unset}'
           ' WHISPER_S=$ws RULES=${LABYRINTH_VOICE_RULES-unset}" >> "$W/calls"\n')
ANSWER = "## **It is** noon.\n<think>hmm\nstill thinking</think> Late `lunch`?\n\n   \n<think>x</think>\nBye now"


def ask(name, steps=None, heard=" What time is it?\n", answer=ANSWER, files=None, stubs=("curl", "whisper-cli", "sarina", "sarina-tone", "labyrinth-ask-sarina"),
        fakes=(), env=None):
    f = {WAV: "RIFF", MODEL: "model", RULES: "Spoken talk: short.\n", CFG + "/sarina.env": "OPENAI_MODEL=big\nVOICE_MODEL=small\nSARINA_OWNER_NAME=Alex\n",
         CFG + "/stt-words": "Zed, Quux\nnot this line\n"}
    for k, v in (files or {}).items():
        f[k] = v
    fk = [("whisper-cli", {"out": heard, "err": "whisper's log\n"}), ("sarina-tone", {"out": "calm, slow\nsecond line\n", "err": "tone log\n"}),
          ("labyrinth-ask-sarina", {"sh": ASK_ENV, "out": answer, "err": "ask's log\n"})] + list(fakes)
    C.case(SA, name, steps or [{"args": [WAV]}], U.user_case_world({k: v for k, v in f.items() if v is not None}, stubs, fk),
           env=dict(U.UENV, **(env or {})))


ask("a question, answered sentence by sentence")
ask("no recording, a recording that is not there, a folder", [{"args": []}, {"args": [HOME + "/none.wav"]}, {"args": [HOME]}])
ask("Sarina is not installed", stubs=("curl", "whisper-cli"))
ask("the model server is down", fakes=[("curl", {"rc": 7})])
ask("no speech model", files={MODEL: None})
ask("no whisper-cli", stubs=("curl", "sarina", "labyrinth-ask-sarina"))
ask("LABYRINTH_WHISPER and LABYRINTH_STT_MODEL, the language", env={"LABYRINTH_WHISPER": "my-whisper", "LABYRINTH_STT_MODEL": HOME + "/m.bin",
                                                                   "LABYRINTH_STT_LANG": "de"},
    files={HOME + "/m.bin": "m"}, stubs=("curl", "my-whisper", "sarina", "sarina-tone", "labyrinth-ask-sarina"), fakes=[("my-whisper", {"out": "Hallo.\n"})])
ask("LABYRINTH_STT_MODEL names nothing", env={"LABYRINTH_STT_MODEL": HOME + "/none.bin"})
ask("the model from /usr/share, in bash's order", files={MODEL: None, "/usr/share/labyrinth/stt/ggml-tiny.bin": "t", "/usr/share/labyrinth/stt/ggml-Base.bin": "b",
                                                        "/usr/share/labyrinth/stt/ggml-.bin": ("dir", 0o755)})
ask("XDG_DATA_HOME and XDG_CONFIG_HOME", env={"XDG_DATA_HOME": HOME + "/data", "XDG_CONFIG_HOME": HOME + "/cfg"},
    files={MODEL: None, HOME + "/data/labyrinth/stt/ggml-a.bin": "a", HOME + "/cfg/labyrinth/sarina.env": "VOICE_MODEL=v\nOPENAI_MODEL=v\n"})
ask("whisper's spaces, tabs and lines", heard="\t  Hello    there\t,\n\n  Sarina \r\n")
ask("silence: whisper's notes only", heard="[BLANK_AUDIO]\n (silence) *music*\n")
for i, said in enumerate(["Thank you.", "  Thanks for watching!", "Bye-bye.", "Bye bye…", "OK", "Thank you, Sarina.", "thank  you", "¿You?"]):
    ask("a stock phrase or not (%d)" % i, heard=said + "\n")
ask("laughter alone: Haha.", heard="[Laughs]\n")
ask("laughter and words", heard="(laughing) That one was good *chuckles*\n")
ask("laughter that is not a note", heard="laughter is the best\n")
ask("tone off in Settings", files={CFG + "/sarina-tone": "off\n\n"})
ask("tone off in Settings, LABYRINTH_TONE given", files={CFG + "/sarina-tone": "off"}, env={"LABYRINTH_TONE": "1"})
ask("LABYRINTH_TONE=0", env={"LABYRINTH_TONE": "0"})
ask("no sarina-tone, sarina-tone says nothing", stubs=("curl", "whisper-cli", "sarina", "labyrinth-ask-sarina"))
ask("sarina-tone fails with no output", fakes=[("sarina-tone", {"rc": 3})])
ask("a conversation (Yggdrasil's name first)", env={"YGGDRASIL_CONVERSATION": "c1", "LABYRINTH_CONVERSATION": "old"})
ask("a conversation (the older panels' name)", env={"LABYRINTH_CONVERSATION": "old"})
ask("LABYRINTH_VOICE_MODEL wins, the fallback kept", env={"LABYRINTH_VOICE_MODEL": "tiny", "SARINA_FALLBACK_MODELS": "mine"})
ask("the voice model is the chat model", files={CFG + "/sarina.env": "VOICE_MODEL=m\nOPENAI_MODEL=m\nVOICE_MODEL=later\n"})
ask("no voice model: nothing set", files={CFG + "/sarina.env": "OPENAI_MODEL=big\n"}, env={"OPENAI_MODEL": "env"})
ask("the owner from the environment, no stt-words", env={"SARINA_OWNER_NAME": "Sam"}, files={CFG + "/stt-words": None})
ask("no sarina.env, no voice rules", files={CFG + "/sarina.env": None, RULES: None, "/usr/share/labyrinth/other": "x"})
ask("it sounded like her own voice", fakes=[("labyrinth-ask-sarina", {"sh": ASK_ENV, "out": "one line\n", "rc": 3})])
ask("she fails before a word", fakes=[("labyrinth-ask-sarina", {"sh": ASK_ENV, "rc": 2})])
ask("she fails after a word", fakes=[("labyrinth-ask-sarina", {"sh": ASK_ENV, "out": "half\n", "rc": 2})])
ask("no answer, only thinking", answer="<think>a\nb</think>\n**\n")
ask("an open think block to the end", answer="Yes.\n<think>never\nclosed\n")
ask("labyrinth-ask-sarina is missing", stubs=("curl", "whisper-cli", "sarina", "sarina-tone"))

# ---------------------------------------------------------------- labyrinth-sarina-speak

SP = "labyrinth-sarina-speak"
TTS = HOME + "/.local/share/labyrinth/tts"
SAMPLE = "/sample.wav"


def wave(rate=22050, n=4000):
    """A short Piper-like recording: 16-bit mono PCM, louder and softer stretches."""
    import math
    samples = [int(12000 * math.sin(i / 7.0) * (1 if (i // 1200) % 2 == 0 else 0.15)) for i in range(n)]
    data = struct.pack("<%dh" % n, *samples)
    return (b"RIFF" + struct.pack("<I", 36 + len(data)) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16) + b"data"
            + struct.pack("<I", len(data)) + data)


# Piper writes the recording (arguments: -P -m piper -m MODEL -f WAV); the venv's python run as `python - WAV` (the loudness) is
# this computer's python3 with the same program
PIPER_SH = 'cp /sample.wav "$7"\n'
ENVELOPE_SH = 'exec /usr/bin/python3 - "$2"\n'
# what the player saw: the loudness file and whether the pid file was there
PLAY_SH = ('{ printf "  env: "; tr "\\n" "|" < %s/speech.env 2>/dev/null; echo; [ -f %s/speaking.pid ] && echo "  pid file there"; } >> "$W/calls"\n'
           % (RUN, RUN))


def norm(t):
    import re
    t = re.sub(r"^(python|py) \[-\] \[[^\]]*\]\n", "", t, flags=re.M)  # the loudness: the venv's Python in the script, done in the port
    t = re.sub(r"speech\.[A-Za-z0-9]{6}\.wav", "speech.<RND>.wav", t)
    t = re.sub(r"\b\d{13}\b", "<MS>", t)
    return t


def make_socket():
    path = C.host(RUN + "/voiced.sock")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    s = socket.socket(socket.AF_UNIX)
    s.bind(path)
    s.close()


def speak(name, steps, files=None, fakes=(), env=None, voiced=False, stubs=("pw-play", "pkill"), differs=()):
    f = {TTS + "/venv/bin/python": (C.FAKE.replace("@W@", W), 0o755), TTS + "/voices/en_US-amy-medium.onnx": "voice",
         "/usr/bin/sarina-voice": (C.FAKE.replace("@W@", W), 0o755), SAMPLE: wave()}
    for k, v in (files or {}).items():
        f[k] = v
    fk = [("python_-P", {"sh": PIPER_SH, "stdin": True}), ("python_-", {"sh": ENVELOPE_SH}), ("pw-play", {"sh": PLAY_SH})] + list(fakes)
    setup = U.user_case_world({k: v for k, v in f.items() if v is not None}, stubs, fk, extra=make_socket if voiced else None)
    C.case(SP, name, steps, setup, env=dict(U.UENV, **(env or {})), norm_more=norm, differs=differs)


speak("said with Piper: marks, links and think blocks out", [{"args": ["<think>plan\nit</think>**Hello** [there](http://x)", "`Sarina`_ #1 "]}])
speak("from standard input, on lines", [{"args": [], "stdin": "First line.\n\nSecond   line.\n\n\n"}])
speak("nothing to say", [{"args": []}, {"args": ["** ` #"]}, {"args": ["<think>only</think>"]}])
speak("long text is cut after 700 characters", [{"args": ["word " * 200]}, {"args": ["a" + "\u00e9" * 800]}])
speak("long text with names", [{"args": ["a" + "\u00e9" * 800]}], files={U.HOME + "/.config/labyrinth/sarina-say.json": '{"x": "y"}'})
speak("names as the owner spelled them", [{"args": ["Siobhan met siobhan's SIOBHAN, not Siobhanna; C++ and c++."]}],
      files={U.HOME + "/.config/labyrinth/sarina-say.json": '{"Siobhan": "Shivawn", "_": "under", " ": "space", "n": 3, "C++": "see plus plus", "Met": 1}'})
speak("names: LABYRINTH_SAY_MAP; a list; not JSON", [{"args": ["Siobhan"], "env": {"LABYRINTH_SAY_MAP": U.HOME + "/m1"}},
                                                    {"args": ["Siobhan"], "env": {"LABYRINTH_SAY_MAP": U.HOME + "/m2"}},
                                                    {"args": ["Siobhan"], "env": {"LABYRINTH_SAY_MAP": U.HOME + "/m3"}}],
      files={U.HOME + "/m1": '{"siobhan": "Shiv"}', U.HOME + "/m2": "[1, 2]", U.HOME + "/m3": "{nope"}, differs=(("err", 1),))
speak("the chosen voice, a voice name that is no name, a missing voice", [{"args": ["Hi"]}, {"args": ["Hi"], "env": {"LABYRINTH_SARINA_VOICE": "../x"}},
                                                                         {"args": ["Hi"], "env": {"LABYRINTH_SARINA_VOICE": "en_GB-alba"}}],
      files={U.HOME + "/.config/labyrinth/sarina-voice": "en_US-ryan-high\nsecond\n", TTS + "/voices/en_US-ryan-high.onnx": "v"})
speak("no Piper", [{"args": ["Hi"]}], files={TTS + "/venv/bin/python": None})
speak("LABYRINTH_TTS_DIR and LABYRINTH_TTS_PYTHON", [{"args": ["Hi"]}], env={"LABYRINTH_TTS_DIR": U.HOME + "/tts", "LABYRINTH_TTS_PYTHON": U.HOME + "/py"},
      files={U.HOME + "/tts/voices/en_US-amy-medium.onnx": "v", U.HOME + "/py": (C.FAKE.replace("@W@", W), 0o755)})
speak("Piper fails", [{"args": ["Hi"]}], fakes=[("python_-P", {"rc": 1, "err": "piper broke\n"})])
speak("to a file instead of playing", [{"args": ["--to", U.HOME + "/out.wav", "Hi"]}, {"args": ["Hi", "--to", U.HOME + "/no/dir/x.wav"]}])
speak("no loudness file: LABYRINTH_NO_ENVELOPE; a recording that is no wave", [{"args": ["Hi"], "env": {"LABYRINTH_NO_ENVELOPE": "1"}},
                                                                              {"args": ["Hi"], "env": {"LABYRINTH_PLAYER": "my-player"}}],
      fakes=[("my-player", {"sh": PLAY_SH})], stubs=("pw-play", "pkill", "my-player"))
speak("a silent recording", [{"args": ["Hi"]}], files={SAMPLE: wave(n=0)})
speak("the voice service speaks", [{"args": ["Hello there"]}], voiced=True)
speak("the voice service fails: Piper speaks", [{"args": ["Hello"]}], voiced=True, fakes=[("sarina-voice", {"rc": 1})])
speak("the voice service is passed over: --to, LABYRINTH_SARINA_ENGINE=piper", [{"args": ["--to", U.HOME + "/o.wav", "Hi"]},
                                                                                {"args": ["Hi"], "env": {"LABYRINTH_SARINA_ENGINE": "piper"}}], voiced=True)
speak("LABYRINTH_VOICED_SOCKET names a plain file", [{"args": ["Hi"], "env": {"LABYRINTH_VOICED_SOCKET": U.HOME + "/sock"}}], files={U.HOME + "/sock": "x"})
speak("--lines: each line as it comes, the last one without a newline left", [{"args": ["--lines"], "stdin": "One.\n\n**Two** `x`\n<think>t</think>\nThree"}])
speak("--lines with the voice service; one line it fails", [{"args": ["--lines"], "stdin": "One.\nTwo.\n"}], voiced=True,
      fakes=[("sarina-voice_speak-voiced_Two.", {"rc": 1})])
speak("--lines with a missing voice: skipped", [{"args": ["--lines"], "stdin": "One.\n", "env": {"LABYRINTH_SARINA_VOICE": "gone"}},
                                                {"args": ["--lines"], "stdin": "One.\n", "env": {"LABYRINTH_SARINA_VOICE": "a b"}}])
speak("--stop: nothing playing, a pid file of another program, of no number", [{"args": ["--stop"]}, {"args": ["--stop", "ignored"]}, {"args": ["--stop"]}],
      files={RUN + "/speaking.pid": "1\n", RUN + "/other": "x"})
speak("--stop: a pid file that is no number", [{"args": ["x", "--stop"]}], files={RUN + "/speaking.pid": "12a\n"})
speak("a run folder that was open to others is closed", [{"args": ["Hi"]}], files={RUN + "/": ("dir", 0o755)})
