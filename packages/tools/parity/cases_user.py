"""The cases of the shell scripts' ports that run as the user (loaded by compare.py): labyrinth-security-setting and, below, the
rest. They run in cases_root.py's world (a root folder of their own in a user and mount namespace; the home is /home/tester
there), so that the system files they read (/usr/share/labyrinth, /etc/pacman.conf, /proc ...) are the world's, and every
program they start is a stand-in that writes down its call. Nothing of this computer's home, settings or services is touched."""
import os

import __main__ as P
import cases_root as C

W = C.W
HOME = "/home/tester"
UENV = {"HOME": HOME, "USER": "tester", "XDG_RUNTIME_DIR": "/run/user/1001"}


def user_case(prog, name, steps, files=None, stubs=(), fakes=(), env=None, differs=(), norm_more=None, extra=None):
    f = {HOME + "/": ("dir", 0o755), "/run/user/1001/": ("dir", 0o700)}
    f.update(files or {})
    setup = C.world(files=f, stubs=stubs, fakes=list(fakes), passwd="tester:x:1001:1001::/home/tester:/bin/bash\n", group="tester:x:1001:\n",
                    extra=extra)
    C.case(prog, name, steps, setup, env=dict(UENV, **(env or {})), differs=differs, norm_more=norm_more)


# ---------------------------------------------------------------- labyrinth-security-setting

SS = "labyrinth-security-setting"


def sec(name, steps, files=None, env=None, differs=(), fakes=()):
    user_case(SS, name, steps, files=files, stubs=("heimdalld",), env=env, differs=differs, fakes=fakes)


sec("usage", [{"args": []}, {"args": ["bogus"]}, {"args": ["scan"]}, {"args": ["scan", ""]}, {"args": ["auto-quarantine"]},
              {"args": ["auto-quarantine", "maybe"]}])
sec("scan a folder", [{"args": ["scan", HOME + "/Downloads"]}, {"args": ["scan", "rel/dir", "extra"]}])
sec("scan a folder named like an option (fixed: ./-x)", [{"args": ["scan", "--quarantine"]}, {"args": ["scan", "-"]}])
sec("scan: heimdalld fails", [{"args": ["scan", "/x"]}], fakes=[("heimdalld", {"rc": 3, "out": "found 1\n", "err": "bad\n"})])
sec("scan: XDG_STATE_HOME", [{"args": ["scan", "/x"]}], env={"XDG_STATE_HOME": HOME + "/st"})
sec("auto-quarantine: on, status, off, status", [{"args": ["auto-quarantine", "status"]}, {"args": ["auto-quarantine", "off"]},
                                                 {"args": ["auto-quarantine", "status"]}, {"args": ["auto-quarantine", "on"]},
                                                 {"args": ["auto-quarantine", "status"]}])
sec("auto-quarantine status: written by hand", [{"args": ["auto-quarantine", "status"]}],
    files={HOME + "/.config/labyrinth/scan.conf": "# action = notify\n  action\t=  notifyx\n"})
sec("auto-quarantine: XDG_CONFIG_HOME", [{"args": ["auto-quarantine", "off"]}], env={"XDG_CONFIG_HOME": HOME + "/cfg"})
sec("auto-quarantine: the folder cannot be made", [{"args": ["auto-quarantine", "on"]}], files={HOME + "/.config": "a file\n"},
    differs=(("err", 0),))
sec("HOME unset", [{"args": ["auto-quarantine", "status"]}], env={"HOME": None}, differs=(("err", 0),))


def user_norm(t):
    import re
    t = re.sub(r"/tmp/tmp\.[A-Za-z0-9]{10}", "<TMP>", t)
    return t


# ---------------------------------------------------------------- labyrinth-gaming

GM = "labyrinth-gaming"
STEAM = HOME + "/.local/share/Steam"
PKGS = ("steam", "lib32-nvidia-utils", "nvidia-utils", "lib32-vulkan-icd-loader", "vulkan-icd-loader", "gamemode", "mangohud", "proton-cachyos",
        "protonup-qt", "protontricks", "umu-launcher")
VDF_ON = '"InstallConfigStore"\n{\n\t"CompatToolMapping"\n\t{\n\t\t"0"\n\t\t{\n\t\t\t"name"\t\t"proton_9"\n\t\t}\n\t}\n}\n'
VDF_OFF = '"InstallConfigStore"\n{\n\t"CompatToolMapping"\n\t{\n\t\t"620"\n\t\t{\n\t\t}\n\t}\n}\n'


def gaming(name, steps=None, installed=PKGS, files=None, stubs=("pacman", "sudo", "nvidia-smi", "vulkaninfo"), fakes=(), steam_running=False):
    f = {"/etc/pacman.conf": "[options]\n[core]\n[multilib]\nInclude = x\n", "/usr/share/steam/compatibilitytools.d/.keep": "",
         "/usr/bin/steam_playall": (C.FAKE.replace("@W@", W), 0o755), "/sys/module/nvidia_drm/parameters/modeset": "Y\n"}
    f.update(files or {})
    fk = [("pacman_-Q", {"rc": 1})] + [("pacman_-Q_" + p, {"rc": 0}) for p in installed]
    fk += [("nvidia-smi_-L", {"out": "GPU 0: NVIDIA GeForce RTX made-up (UUID: GPU-made-up)\nGPU 1: second\n"}),
           ("vulkaninfo_--summary", {"out": "Devices:\n\tdeviceName = NVIDIA GeForce RTX made-up\n"})] + list(fakes)

    def norm(t):
        import re
        return re.sub(r"(nvidia-smi \[-L\]\n)+", r"\1", t)  # the script asked twice, the port once
    procs = [W + "/ov/steam"] if steam_running else []
    setup = C.world(files=f, stubs=stubs, fakes=fk, passwd="tester:x:1001:1001::/home/tester:/bin/bash\n", group="tester:x:1001:\n", procs=procs, pidns=True)
    C.case(GM, name, steps or [{"args": []}, {"args": ["check"]}], setup, env=UENV, norm_more=norm, pidns=True)


gaming("nothing set up: no Steam yet", installed=())
gaming("all there, Steam Play on", files={STEAM + "/steamapps/common/Proton 9.0/x": "", STEAM + "/steamapps/common/Proton - Experimental/x": "",
                                          STEAM + "/steamapps/common/Other/x": "", STEAM + "/compatibilitytools.d/GE-Proton9-1/x": "",
                                          "/usr/share/steam/compatibilitytools.d/proton-cachyos-10/x": "", STEAM + "/config/config.vdf": VDF_ON})
gaming("Steam Play off, no Proton, no extra tools, ~/.steam/steam", files={HOME + "/.steam/steam/steamapps/x": "",
                                                                          HOME + "/.steam/steam/config/config.vdf": VDF_OFF})
gaming("config.vdf: \\r\\n, not UTF-8, written oddly", steps=[{"args": ["check"]}],
       files={STEAM + "/steamapps/x": "", STEAM + "/config/config.vdf": VDF_ON.replace("\n", "\r\n")})
gaming("config.vdf not UTF-8", steps=[{"args": ["check"]}], files={STEAM + "/steamapps/x": "", STEAM + "/config/config.vdf": b"\xff" + VDF_ON.encode()})
gaming("no NVIDIA tools, no multilib, modeset off", stubs=("pacman", "sudo"), files={"/etc/pacman.conf": "#[multilib]\n",
                                                                                   "/sys/module/nvidia_drm/parameters/modeset": "N\n"})
gaming("nvidia-smi fails, Vulkan without NVIDIA", fakes=[("nvidia-smi_-L", {"rc": 9, "err": "NVIDIA-SMI has failed\n"}),
                                                        ("vulkaninfo_--summary", {"out": "deviceName = llvmpipe\nNVIDIA elsewhere\n"})])
gaming("Steam is running", files={STEAM + "/steamapps/x": "", STEAM + "/config/config.vdf": VDF_OFF}, steam_running=True,
       steps=[{"args": ["check"]}, {"args": ["setup"]}])
gaming("setup: installs what is missing, Steam Play with Valve's Proton", steps=[{"args": ["setup"]}], installed=("steam",),
       files={STEAM + "/steamapps/x": "", STEAM + "/config/config.vdf": VDF_OFF})
gaming("setup: the system's Proton-CachyOS (fixed: chosen)", steps=[{"args": ["setup"]}],
       files={STEAM + "/steamapps/x": "", STEAM + "/config/config.vdf": VDF_OFF, "/usr/share/steam/compatibilitytools.d/proton-cachyos-10.0-20261101/x": ""})
gaming("setup: yours first", steps=[{"args": ["setup"]}],
       files={STEAM + "/steamapps/x": "", STEAM + "/config/config.vdf": VDF_OFF, "/usr/share/steam/compatibilitytools.d/proton-cachyos-10/x": "",
              STEAM + "/compatibilitytools.d/proton-cachyos-9/x": "", STEAM + "/compatibilitytools.d/proton-cachyos-11/x": ""})
gaming("setup: pacman fails, no Steam yet, no helper", steps=[{"args": ["setup"]}], installed=(), fakes=[("sudo", {"rc": 1})],
       files={"/usr/bin/steam_playall": ("x", 0o644)})
gaming("setup: no config.vdf", steps=[{"args": ["setup"]}], files={STEAM + "/steamapps/x": ""})
gaming("usage", steps=[{"args": ["bogus"]}, {"args": [""]}])

# ---------------------------------------------------------------- labyrinth-sarina

SA = "labyrinth-sarina"
SARINA_SH = 'echo "  OPENAI_MODEL=${OPENAI_MODEL-unset} OPENAI_BASE_URL=$OPENAI_BASE_URL OPENAI_API_KEY=$OPENAI_API_KEY" >> "$W/calls"\n'
CURL_UP_AFTER = 'n=$(cat "$W/fake/up.n" 2>/dev/null || echo 0); n=$((n + 1)); echo $n > "$W/fake/up.n"; [ "$n" -ge "$(cat "$W/fake/up.at.out")" ] || exit 7\n'


def sarina(name, steps, up_at=1, stubs=("sarina", "ollama", "curl", "sudo", "lspci", "xdg-open"), fakes=(), files=None, env=None):
    fk = [("sarina", {"sh": SARINA_SH}), ("curl", {"sh": CURL_UP_AFTER}), ("ollama_list", {"out": "NAME ID SIZE\nqwen3:8b abc 5 GB\n"}),
          ("ollama_serve", {"sh": 'echo "  OLLAMA_CONTEXT_LENGTH=$OLLAMA_CONTEXT_LENGTH" >> "$W/calls"; echo serving\n'}), ("up.at", {"out": "%d\n" % up_at})]
    setup = user_case_world(files, stubs, fk + list(fakes))
    C.case(SA, name, steps, setup, env=dict(UENV, **(env or {})), fastclock=True, sort_calls=True, settle=0.3)


def user_case_world(files, stubs, fakes, **k):
    f = {HOME + "/": ("dir", 0o755), "/run/user/1001/": ("dir", 0o700)}
    f.update(files or {})
    return C.world(files=f, stubs=stubs, fakes=fakes, passwd="tester:x:1001:1001::/home/tester:/bin/bash\n", group="tester:x:1001:\n", **k)


sarina("chat: the server answers, a model is there", [{"args": []}, {"args": ["chat"]}])
sarina("gui: opens the console", [{"args": ["gui"]}])
sarina("model: the first model chosen", [{"args": ["model"]}], env={"OPENAI_MODEL": "set-before", "OPENAI_API_KEY": "mine"})
sarina("OPENAI_MODEL already set", [{"args": ["chat"]}], env={"OPENAI_MODEL": "llama3"})
sarina("the server is started and answers on the third look", [{"args": ["chat"]}], up_at=3)
sarina("the server never answers", [{"args": ["chat"]}], up_at=1000)
sarina("no model: download it, or not", [{"args": ["chat"], "stdin": "y\n"}, {"args": ["chat"], "stdin": "n\n"}, {"args": ["chat"]}],
       fakes=[("ollama_list", {"out": "NAME ID SIZE\n"})])
sarina("no model, the download fails", [{"args": ["chat"], "stdin": "yes\n"}], fakes=[("ollama_list", {"out": ""}), ("ollama_pull", {"rc": 1})])
sarina("no sarina", [{"args": []}], stubs=("ollama", "curl"))
sarina("no ollama: install it, or not, or the install fails", [{"args": ["chat"], "stdin": "Y\n"}, {"args": ["chat"], "stdin": "\n"}],
       stubs=("sarina", "curl", "sudo", "lspci"), fakes=[("lspci", {"out": "01:00.0 VGA compatible controller: NVIDIA Corporation TU102\n"})])
sarina("no ollama, the install fails", [{"args": [], "stdin": "y\n"}], stubs=("sarina", "curl", "sudo", "lspci"), fakes=[("sudo", {"rc": 1})])
sarina("usage", [{"args": ["bogus"]}])

# ---------------------------------------------------------------- labyrinth-sarina-setting

SST = "labyrinth-sarina-setting"
VOICES = HOME + "/.local/share/labyrinth/tts/voices/"


def setting(name, steps, files=None, fakes=(), env=None):
    f = {"/usr/share/labyrinth/labyrinth-sarina-checkup.service": "[Unit]\n", "/usr/share/labyrinth/labyrinth-sarina-checkup.timer": "[Timer]\n",
         "/usr/share/labyrinth/labyrinth-sarina-held.service": ("[Unit]\n", 0o600), "/usr/share/labyrinth/labyrinth-sarina-held.timer": "[Timer]\n",
         "/usr/share/labyrinth/labyrinth-sarina-reflect.service": "[Unit]\n", "/usr/share/labyrinth/labyrinth-sarina-reflect.timer": "[Timer]\n"}
    f.update(files or {})
    setup = user_case_world(f, ("systemctl", "labyrinth-sarina-speak", "ghostty"), list(fakes))
    C.case(SST, name, steps, setup, env=dict(UENV, **(env or {})), sort_calls=True, settle=0.2)


setting("usage", [{"args": []}, {"args": ["bogus"]}, {"args": ["voice"]}, {"args": ["voice", "x"]}, {"args": ["tone"]}, {"args": ["forge", "x"]},
                  {"args": ["checkup", ""]}, {"args": ["learn", "maybe"]}])
setting("voices: list, get, set", [{"args": ["voice", "list"]}, {"args": ["voice", "get"]}, {"args": ["voice", "set", "en_GB-jenny_dioco-medium"]},
                                   {"args": ["voice", "get"]}, {"args": ["voice", "set", "missing"]}, {"args": ["voice", "set", "../x"]},
                                   {"args": ["voice", "set", ""]}],
        files={VOICES + "en_US-amy-medium.onnx": "x", VOICES + "en_GB-jenny_dioco-medium.onnx": "x", VOICES + "b.onnx.onnx": "x",
               VOICES + "en_US-amy-medium.onnx.json": "x", VOICES + ".hidden.onnx": "x", VOICES + "dir.onnx/": ("dir", 0o755)})
setting("voice get: an empty file", [{"args": ["voice", "get"]}], files={HOME + "/.config/labyrinth/sarina-voice": "\nsecond\n"})
setting("hear", [{"args": ["hear"]}])
setting("tone", [{"args": ["tone", "status"]}, {"args": ["tone", "off"]}, {"args": ["tone", "status"]}, {"args": ["tone", "on"]}, {"args": ["tone", "status"]}])
setting("forge keeps the other settings, no newline at the end", [{"args": ["forge", "status"]}, {"args": ["forge", "on"]}, {"args": ["forge", "status"]},
                                                                  {"args": ["forge", "off"]}, {"args": ["forge", "status"]}],
        files={HOME + "/.config/labyrinth/sarina.env": ("OPENAI_API_KEY=secret\nSARINA_FORGE=off\nSARINA_VAULT=/v", 0o644)})
setting("checkup and learn", [{"args": ["checkup", "status"]}, {"args": ["checkup", "on"]}, {"args": ["learn", "on"]}, {"args": ["learn", "status"]},
                              {"args": ["checkup", "off"]}, {"args": ["learn", "off"]}],
        fakes=[("systemctl_--user_is-enabled_labyrinth-sarina-checkup.timer", {"rc": 1})])
setting("checkup: a timer of mine is kept, daemon-reload fails", [{"args": ["checkup", "on"]}],
        files={HOME + "/.config/systemd/user/labyrinth-sarina-checkup.timer": "mine\n"}, fakes=[("systemctl_--user_daemon-reload", {"rc": 1})])
setting("checkup: enable fails", [{"args": ["checkup", "on"]}], fakes=[("systemctl_--user_enable", {"rc": 4})])
setting("learn: the unit is missing", [{"args": ["learn", "on"]}], files={"/usr/share/labyrinth/labyrinth-sarina-reflect.timer": None},
        )
setting("agents", [{"args": ["agents"]}])
setting("XDG_CONFIG_HOME and XDG_DATA_HOME", [{"args": ["tone", "off"]}, {"args": ["voice", "list"]}, {"args": ["learn", "on"]}],
        env={"XDG_CONFIG_HOME": HOME + "/cfg", "XDG_DATA_HOME": HOME + "/data"}, files={HOME + "/data/labyrinth/tts/voices/v1.onnx": "x"})

# ---------------------------------------------------------------- labyrinth-rea, labyrinth-rea-setup

RE, RS = "labyrinth-rea", "labyrinth-rea-setup"
APPS = HOME + "/.local/share/labyrinth/apps"
REA_MJS = APPS + "/rea/node_modules/rea-agents/scripts/rea.mjs"


def rea(name, steps, files=None, stubs=("bwrap",), env=None, differs=()):
    f = {REA_MJS: "// rea\n", "/usr/lib/jvm/.keep": "", "/etc/ssl/x": ""}
    f.update(files or {})
    C.case(RE, name, steps, user_case_world(f, stubs, []), env=dict(UENV, **(env or {})), differs=differs)


rea("help", [{"args": ["-h"]}, {"args": ["--help", "x"]}, {"args": ["--net", "--help"]}])
rea("a REA command, nothing else there", [{"args": ["decompile", "/samples/a.sys", "--json"]}, {"args": []}])
rea("everything: a JDK, Ghidra, /etc/java*, the quarantine, the network", [{"args": ["--net", "--samples", HOME + "/s", "--work", HOME + "/w", "mcp"]}],
    files={"/usr/lib/jvm/java-25-openjdk/release": "", "/usr/lib/jvm/java-21-openjdk/release": "", "/usr/lib/jvm/java-17-openjdk/release": "",
           "/usr/lib/jvm/java-21-openjdk-x/release": "", APPS + "/ghidra/ghidra_12.1.4_PUBLIC/x": "", APPS + "/ghidra/ghidra_11.0_PUBLIC/x": "",
           "/etc/java-openjdk/x": "", "/etc/java21/x": "", "/etc/fonts/x": "", "/etc/ca-certificates/x": "",
           HOME + "/.local/share/labyrinth/quarantine/x/payload": ""})
rea("--exec", [{"args": ["--exec", "ls", "/opt"]}, {"args": ["--samples", HOME + "/s", "--exec"]}])
rea("options after the command are the command's", [{"args": ["mcp", "--net"]}])
rea("the environment's folders", [{"args": ["x"]}], env={"LABYRINTH_REA_SAMPLES": HOME + "/S", "LABYRINTH_REA_WORK": HOME + "/W",
                                                           "LABYRINTH_APPS": HOME + "/apps"}, files={HOME + "/apps/rea/node_modules/rea-agents/scripts/rea.mjs": ""})
rea("--samples without a folder", [{"args": ["--samples"]}], differs=(("err", 0), ("rc", 0)))
rea("not installed, no bubblewrap", [{"args": ["x"]}], files={REA_MJS: None})
rea("no bubblewrap", [{"args": ["x"]}], stubs=())
rea("a samples folder that cannot be made", [{"args": ["--samples", "/proc/nope", "x"]}], differs=(("err", 0),))

GZIP = b"fake ghidra zip\n"


def rea_setup(name, steps, files=None, stubs=("node", "npm", "curl", "unzip", "java"), fakes=(), env=None):
    import hashlib
    fk = [("node", {"out": "rea-agents 6.5.0\n"}), ("npm_init", {"sh": 'echo "{}" > package.json\n'}),
          ("curl_-fsSL_-o", {"sh": "printf 'fake ghidra zip\\n' > \"$3\"\n"}), ("unzip", {"sh": 'mkdir -p "$4/ghidra_12.1.4_PUBLIC"\n'}),
          ("java_-version", {"err": 'openjdk version "21.0.5" 2024-10-15\nOpenJDK Runtime Environment\n'})] + list(fakes)
    e = {"GHIDRA_SHA256": hashlib.sha256(GZIP).hexdigest()}
    e.update(env or {})
    C.case(RS, name, steps, user_case_world(files, stubs, fk), env=dict(UENV, **e), norm_more=user_norm)


rea_setup("--check: nothing installed", [{"args": ["--check"]}], stubs=())
rea_setup("install everything, then --check", [{"args": []}, {"args": ["--check"]}])
rea_setup("again: package.json and Ghidra there", [{"args": []}], files={APPS + "/rea/package.json": "{}\n", APPS + "/ghidra/ghidra_x/y": ""})
rea_setup("Ghidra's hash does not match", [{"args": []}], env={"GHIDRA_SHA256": "0" * 64})
rea_setup("the download fails", [{"args": []}], fakes=[("curl_-fsSL_-o", {"rc": 22, "err": "curl: (22) 404\n"})])
rea_setup("npm fails", [{"args": []}], fakes=[("npm_install", {"rc": 1, "err": "npm ERR!\n"})])
rea_setup("no node", [{"args": []}], stubs=("npm", "curl", "unzip"))
rea_setup("odd settings from the environment (fixed: refused)", [{"args": ["--check"], "env": {"REA_VERSION": "git+https://example.org/x"}},
                                                                {"args": [], "env": {"GHIDRA_TAG": "../../x"}}, {"args": [], "env": {"GHIDRA_ZIP": "a?b"}},
                                                                {"args": [], "env": {"GHIDRA_SHA256": "xyz"}}, {"args": [], "env": {"GHIDRA_ZIP": ".."}}])

# ---------------------------------------------------------------- labyrinth-claude-code

CC = "labyrinth-claude-code"
INSTALLER = ("#!/bin/bash\necho 'installing Claude Code'\nmkdir -p \"$HOME/.local/bin\"\n"
             "printf '#!/bin/sh\\necho \"2.0.0 (Claude Code)\"\\n' > \"$HOME/.local/bin/claude\"\nchmod 755 \"$HOME/.local/bin/claude\"\n")


def claude(name, steps, files=None, stubs=("curl", "git", "node", "ssh"), fakes=()):
    fk = [("curl_-fsSL_-o", {"sh": "cat \"$W/fake/installer.out\" > \"$3\"\n"}), ("installer", {"out": INSTALLER})] + list(fakes)
    C.case(CC, name, steps, user_case_world(files, stubs, fk), env=UENV, norm_more=user_norm)


claude("install, then status, then update", [{"args": []}, {"args": ["status"]}, {"args": ["update"]}])
claude("already installed", [{"args": ["install"]}, {"args": ["update"]}],
       files={HOME + "/.local/bin/claude": ("#!/bin/sh\necho \"$*\" >> /home/tester/claude.log\n[ \"$1\" = update ] && exit 3\necho 1.9\n", 0o755)})
claude("the download fails (fixed: said)", [{"args": []}], fakes=[("curl_-fsSL_-o", {"rc": 6, "err": "curl: (6) no network\n"})])
claude("the installer fails", [{"args": []}], fakes=[("installer", {"out": "#!/bin/bash\necho half\nexit 1\n"})])
claude("update when not installed, usage", [{"args": ["update"]}, {"args": ["bogus"]}, {"args": [""]}], stubs=("curl",),
       fakes=[("installer", {"out": "exit 0\n"})])

# ---------------------------------------------------------------- labyrinth-sarina-reflect, -warm, -voice-setup

SR, SW, SV = "labyrinth-sarina-reflect", "labyrinth-sarina-warm", "labyrinth-sarina-voice-setup"
ENVF = HOME + "/.config/labyrinth/sarina.env"
VAULT = HOME + "/Vault"
REFLECT_SH = ('echo "  nice=$(nice) SARINA_VAULT=$SARINA_VAULT SARINA_TEMPERATURE=$SARINA_TEMPERATURE X=${X-unset} LD_PRELOAD=${LD_PRELOAD-unset}'
              ' BASH_ENV=${BASH_ENV-unset} OPENAI_MODEL=${OPENAI_MODEL-unset} A=${A-unset}" >> "$W/calls"\n')


def reflect(name, steps=None, env_text="SARINA_VAULT=" + VAULT + "\n", files=None, stubs=("sarina", "nvidia-smi", "curl"), fakes=(), env=None):
    f = {VAULT + "/Talks/x.md": "talk\n"}
    if env_text is not None:
        f[ENVF] = env_text
    f.update(files or {})
    fk = [("sarina", {"sh": REFLECT_SH}), ("nvidia-smi", {"out": "3\n"})] + list(fakes)
    C.case(SR, name, steps or [{"args": []}], user_case_world(f, stubs, fk), env=dict(UENV, **(env or {})))


reflect("a talk to reflect on, nothing in the Inbox")
reflect("the Inbox has files, settings from sarina.env", env_text="SARINA_VAULT=" + VAULT + "\nX=from file=\nA=b=c=\nOPENAI_MODEL=m\nSARINA_TEMPERATURE=0.9\n",
        files={VAULT + "/Inbox/note.pdf": "x", VAULT + "/Inbox/.hidden": "x"})
reflect("loader variables in sarina.env are skipped (fixed)", env_text="SARINA_VAULT=" + VAULT + "\nLD_PRELOAD=/tmp/evil.so\nBASH_ENV=/tmp/x\nUID=0\n")
reflect("what the environment has wins", env_text="SARINA_VAULT=/elsewhere\nOPENAI_MODEL=file\nSARINA_TEMPERATURE=\n", env={"SARINA_VAULT": VAULT, "OPENAI_MODEL": "env"})
reflect("only an empty Inbox, only hidden files", files={VAULT + "/Inbox/.x": ""})
reflect("no vault", env_text="X=1\n")
reflect("the vault is not a folder", env_text="SARINA_VAULT=" + VAULT + "/Talks/x.md\n")
reflect("no sarina.env", env_text=None)
reflect("Sarina is not installed", stubs=("nvidia-smi", "curl"))
reflect("the graphics card is busy", fakes=[("nvidia-smi", {"out": "40\n12\n"})])
reflect("the card's answer is odd or missing", steps=[{"args": []}], fakes=[("nvidia-smi", {"out": "N/A\n", "rc": 9})])
reflect("no nvidia-smi, the server is down", stubs=("sarina", "curl"), fakes=[("curl", {"rc": 7})])
reflect("sarina fails in both steps", files={VAULT + "/Inbox/a": ""}, fakes=[("sarina_reflect", {"rc": 3}), ("sarina_vault-ingest", {"rc": 4})])
reflect("XDG_CONFIG_HOME", env_text=None, files={HOME + "/cfg/labyrinth/sarina.env": "SARINA_VAULT=" + VAULT + "\n"}, env={"XDG_CONFIG_HOME": HOME + "/cfg"})


def warm(name, env_text="", files=None, env=None, listener=False):
    f = {ENVF: env_text}
    f.update(files or {})
    extra = None
    k = {}
    if listener:
        extra = start_voiced
        k = {"settle": 0.3}
    C.case(SW, name, [{"args": []}], user_case_world(f, ("curl",), [], extra=extra), env=dict(UENV, **(env or {})), **k)


VOICED = {}


def start_voiced():
    """A stand-in for labyrinth-voiced: a socket in the world's /run/user/1001 that writes down what it is sent and answers."""
    import socket
    import threading
    path = C.R + "/run/user/1001/labyrinth-sarina/voiced.sock"
    os.makedirs(os.path.dirname(path), exist_ok=True)
    s = socket.socket(socket.AF_UNIX)
    s.bind(path)
    s.listen(4)
    log = os.path.join(W, "voiced.log")

    def serve():
        while True:
            try:
                c, _ = s.accept()
            except OSError:
                return
            with c:
                c.settimeout(5)
                data = b""
                while not data.endswith(b"\n"):
                    chunk = c.recv(200)
                    if not chunk:
                        break
                    data += chunk
                with open(log, "ab") as f:
                    f.write(b"voiced got: " + data)
                c.sendall(b'{"ok": true}\n')
    threading.Thread(target=serve, daemon=True).start()
    VOICED["sock"] = s


warm("the voice model from sarina.env", env_text="OPENAI_MODEL=qwen3:14b\nVOICE_MODEL=qwen3:4b\nVOICE_MODEL=other\n")
warm("the chat model when there is no voice model", env_text="X=1\nOPENAI_MODEL=qwen3:14b\n")
warm("LABYRINTH_VOICE_MODEL wins", env_text="VOICE_MODEL=qwen3:4b\n", env={"LABYRINTH_VOICE_MODEL": "llama3.1:8b"})
warm("nothing set: the default", env_text="")
warm("a model name that is no name (fixed: not sent)", env_text='VOICE_MODEL=x","keep_alive":"-1\n')
warm("labyrinth-voiced is running", env_text="", listener=True)
warm("the voice socket is a plain file", env_text="", files={"/run/user/1001/labyrinth-sarina/voiced.sock": "not a socket\n"})


def voice(name, steps, files=None, stubs=("python3",), fakes=(), env=None):
    venv_sh = ('mkdir -p "$3/bin"; for n in python pip; do cp "$W/bin/python3" "$3/bin/$n"; done\n')
    fk = [("python3_-m_venv", {"sh": venv_sh}), ("python_-m_piper.download_voices", {"out": "a\nb\nc\nd\n", "err": "downloaded\n",
                                                                                        "sh": 'for v; do :; done; touch "$4/$v.onnx" "$4/$v.onnx.json"\n'})] + list(fakes)
    f = {"/usr/share/labyrinth/voice/piper-requirements.txt": "piper-tts==1.3.0 --hash=sha256:00\n"}
    f.update(files or {})
    C.case(SV, name, steps, user_case_world(f, stubs, fk), env=dict(UENV, **(env or {})))


voice("the defaults", [{"args": []}])
voice("one voice, the venv there already", [{"args": ["en_US-ryan-high"]}], files={HOME + "/.local/share/labyrinth/tts/venv/bin/python": ("#!/bin/sh\n", 0o755),
                                                                                    HOME + "/.local/share/labyrinth/tts/venv/bin/pip": ("#!/bin/sh\n", 0o755)})
voice("LABYRINTH_TTS_DIR", [{"args": ["a", "b"]}], env={"LABYRINTH_TTS_DIR": HOME + "/tts"})
voice("names that are no voice names (fixed: refused)", [{"args": ["--download-dir", "/tmp/x"]}, {"args": ["ok", "a/b"]}, {"args": [""]}])
voice("the venv cannot be made", [{"args": []}], fakes=[("python3_-m_venv", {"rc": 1, "err": "no venv\n"})])
voice("pip refuses", [{"args": []}], fakes=[("pip", {"rc": 1, "err": "hash mismatch\n"})])
voice("the download fails", [{"args": ["x"]}], fakes=[("python_-m_piper.download_voices", {"rc": 2, "out": "1\n2\n3\n4\nfailed"})])

# ---------------------------------------------------------------- labyrinth-install, labyrinth-notify-activate

IN, NA = "labyrinth-install", "labyrinth-notify-activate"
C.case(IN, "the installer is there", [{"args": []}, {"args": ["--x", "y z"]}], user_case_world(None, ("yggdrasil-installer",), [("yggdrasil-installer", {"rc": 3})]),
       env=UENV)
C.case(IN, "the installer is missing", [{"args": []}], user_case_world(None, ("notify-send",), []), env=UENV)
C.case(IN, "the installer is missing, no notifications", [{"args": []}], user_case_world(None, ("notify-send",), [("notify-send", {"rc": 1, "err": "no\n"})]),
       env=UENV)
C.case(IN, "the installer is missing, no notify-send", [{"args": []}], user_case_world(None, (), []), env=UENV)

DBUS_AT = ('n=$(cat "$W/fake/dbus.n" 2>/dev/null || echo 0); n=$((n + 1)); echo $n > "$W/fake/dbus.n"\n'
           'if [ "$n" -ge "$(cat "$W/fake/dbus.at.out")" ]; then echo "method return"; echo "   boolean true"; else echo "   boolean false"; fi\n')


def notify(name, at, running, fastclock=False):
    f = {"/usr/bin/yggdrasil-panel": None}
    procs = ["/usr/bin/yggdrasil-panel"] if running else []
    setup = user_case_world(None, ("dbus-send",), [("dbus-send", {"sh": DBUS_AT}), ("dbus.at", {"out": "%d\n" % at})], procs=procs, pidns=True)
    if not running:
        def go(paths, setup=setup):
            setup(paths)
            import shutil
            dst = C.host("/usr/bin/yggdrasil-panel")
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copy(C.linger(), dst)
        setup = go
    del f
    C.case(NA, name, [{"args": []}], setup, env=dict(UENV, LINGER_LOG=W + "/calls"), pidns=True, fastclock=fastclock, sort_calls=True, settle=0.3)


notify("the panel runs and has the name", 1, True)
notify("the panel runs, takes the name on the third look", 3, True)
notify("the panel is started, then takes the name", 2, False)
notify("the name never comes", 10000, True, fastclock=True)
