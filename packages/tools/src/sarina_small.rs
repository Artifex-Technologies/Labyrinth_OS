//! Three small Sarina helpers, each the bash or sh script of the same name with the same output, calls and exit codes:
//!
//!   labyrinth-sarina-reflect      once a day (a user timer), Sarina reads the talks in her vault and updates what she has
//!                                 learned about you, and turns files dropped in the vault's Inbox/ into notes; skipped (exit 75,
//!                                 the unit tries again) while a game uses the graphics card or the model server is down
//!   labyrinth-sarina-warm         asks Ollama (and labyrinth-voiced, when it runs) to load Sarina's models now, so a spoken
//!                                 question finds them ready (the panel runs it when Super+S starts recording)
//!   labyrinth-sarina-voice-setup  gives Sarina a voice: Piper in a venv of its own (pinned, hash-checked wheels) and the voices
//!
//! nvidia-smi, curl, sarina, python3 (the venv) and its pip are still those programs; reading sarina.env, the niceness (nice),
//! the socket to labyrinth-voiced (the script ran Python for it) and counting the voices are done here.
//!
//! Fixed in both (scripts and ports):
//!   - reflect: every KEY=VALUE of sarina.env that was not set already went into the environment of everything it starts,
//!     LD_PRELOAD and the interpreters' own variables too (a line there would run in nvidia-smi, curl and sarina); those are
//!     skipped now (the loader's LD_*, BASH_ENV, ENV, GLIBC_TUNABLES, GCONV_PATH, LOCPATH, NLSPATH, PYTHONPATH, PYTHONHOME,
//!     PYTHONSTARTUP, NODE_OPTIONS, PERL5LIB, PERL5OPT, RUBYOPT);
//!   - warm: the model's name went unchecked into the JSON it sends (a `"` in it changed the request); a name that is not a
//!     model's is not sent now (the script exits 0 as always);
//!   - voice-setup: a voice name starting with `-` reached piper.download_voices as an option; voice names must be names now.
//!
//! Not 1:1: warm waits for labyrinth-voiced's answer (5 s at most) after Ollama's instead of leaving it to a Python process in
//! the background; with XDG_RUNTIME_DIR set but empty it looks in /tmp (the Python part looked in the current folder).

use crate::sh::{self, Io};

fn config_home() -> String {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{}/.config", std::env::var("HOME").unwrap_or_default()))
}

fn env_nonempty(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

// ---- labyrinth-sarina-reflect

/// Variables a settings file may not give: the loader's and the interpreters' (fixed), and bash's own (set in the script, so
/// it never exported them).
pub fn denied(k: &str) -> bool {
    const LOADER: [&str; 13] = [
        "BASH_ENV",
        "ENV",
        "GLIBC_TUNABLES",
        "GCONV_PATH",
        "LOCPATH",
        "NLSPATH",
        "PYTHONPATH",
        "PYTHONHOME",
        "PYTHONSTARTUP",
        "NODE_OPTIONS",
        "PERL5LIB",
        "PERL5OPT",
        "RUBYOPT",
    ];
    const BASH: [&str; 27] = [
        "BASHOPTS",
        "BASHPID",
        "COMP_WORDBREAKS",
        "DIRSTACK",
        "EPOCHREALTIME",
        "EPOCHSECONDS",
        "EUID",
        "GROUPS",
        "HISTCMD",
        "HOSTNAME",
        "HOSTTYPE",
        "IFS",
        "LINENO",
        "MACHTYPE",
        "OPTERR",
        "OPTIND",
        "OSTYPE",
        "PPID",
        "PS4",
        "PWD",
        "RANDOM",
        "SECONDS",
        "SHELL",
        "SHELLOPTS",
        "SHLVL",
        "SRANDOM",
        "UID",
    ];
    k.starts_with("LD_") || k.starts_with("BASH") || k == "_" || LOADER.contains(&k) || BASH.contains(&k)
}

/// ^[A-Z_][A-Z0-9_]*$
fn key_ok(k: &str) -> bool {
    let b = k.as_bytes();
    !b.is_empty()
        && (b[0].is_ascii_uppercase() || b[0] == b'_')
        && b.iter().all(|&c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

/// sarina.env's KEY=VALUE lines as `IFS='=' read -r k v` splits them: the value is the rest of the line, less one '=' at its end
/// when that is its only one (bash's rule for the last field).
pub fn settings(text: &[u8]) -> Vec<(String, String)> {
    let mut out = vec![];
    for line in sh::lines(text) {
        let l = String::from_utf8_lossy(line);
        let Some((k, rest)) = l.split_once('=') else { continue };
        if !key_ok(k) {
            continue;
        }
        let v = if rest.ends_with('=') && rest.matches('=').count() == 1 { &rest[..rest.len() - 1] } else { rest };
        out.push((k.to_string(), v.to_string()));
    }
    out
}

pub fn reflect() -> i32 {
    sh::set_prog("labyrinth-sarina-reflect");
    let envf = format!("{}/labyrinth/sarina.env", config_home());
    for (k, v) in settings(&std::fs::read(&envf).unwrap_or_default()) {
        if !denied(&k) && env_nonempty(&k).is_none() {
            std::env::set_var(&k, &v);
        }
    }
    let vault = match env_nonempty("SARINA_VAULT") {
        Some(v) if sh::is_dir(&v) => v,
        _ => {
            println!("no vault set (SARINA_VAULT in {envf})");
            return 0;
        }
    };
    if !sh::have("sarina") {
        println!("Sarina is not installed");
        return 0;
    }
    // never take the graphics card while a game uses it: try again in an hour (the unit restarts on 75)
    let (_, out) = sh::capture(&["nvidia-smi", "--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"], Io::Null);
    let u = sh::subst(out.split(|&c| c == b'\n').next().unwrap_or(&[]));
    if sh::digits(&u) && u.parse::<u64>().map(|n| n >= 40).unwrap_or(true) {
        println!("the graphics card is busy; trying again later");
        return 75;
    }
    if !sh::quiet(&["curl", "-fsS", "-m", "2", "http://127.0.0.1:11434/api/tags"]) {
        println!("the model server is not running; trying again later");
        return 75;
    }
    if env_nonempty("SARINA_TEMPERATURE").is_none() {
        std::env::set_var("SARINA_TEMPERATURE", "0.6");
    }
    let mut rc = sh::nice(10, &["sarina", "reflect", "--path", &vault]);
    // files dropped in the vault's Inbox/ become linked notes under Sources/ (the originals stay)
    if !sh::glob(&format!("{vault}/Inbox"), |_| true).is_empty() {
        let c = sh::nice(10, &["sarina", "vault-ingest", "--path", &vault]);
        if c != 0 {
            rc = c;
        }
    }
    rc
}

// ---- labyrinth-sarina-warm

/// The first VOICE_MODEL= (or OPENAI_MODEL=) line's value in sarina.env (sed -n 's/^KEY=//p' | head -n1).
pub fn first_value(text: &[u8], key: &str) -> String {
    let p = format!("{key}=");
    sh::lines(text)
        .into_iter()
        .find(|l| l.starts_with(p.as_bytes()))
        .map(|l| String::from_utf8_lossy(&l[p.len()..]).into_owned())
        .unwrap_or_default()
}

pub fn warm() -> i32 {
    sh::set_prog("labyrinth-sarina-warm");
    let conf = std::fs::read(format!("{}/labyrinth/sarina.env", config_home())).unwrap_or_default();
    let mut model = env_nonempty("LABYRINTH_VOICE_MODEL").unwrap_or_else(|| first_value(&conf, "VOICE_MODEL"));
    if model.is_empty() {
        model = first_value(&conf, "OPENAI_MODEL");
    }
    if model.is_empty() {
        model = "qwen3:8b".into();
    }
    // the voice too (labyrinth-voiced, if it runs): its models load while the question is being said
    let sock = format!("{}/labyrinth-sarina/voiced.sock", env_nonempty("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()));
    let voiced = sh::is_socket(&sock).then(|| {
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let t = Some(std::time::Duration::from_secs(5));
            if let Ok(mut s) = std::os::unix::net::UnixStream::connect(&sock) {
                let _ = s.set_read_timeout(t);
                let _ = s.set_write_timeout(t);
                if s.write_all(b"{\"op\":\"warm\"}\n").is_ok() {
                    let mut b = [0u8; 200];
                    let _ = s.read(&mut b);
                }
            }
        })
    });
    if crate::ecosystem::model_ok(&model) {
        let mut body = String::from("{\"model\":");
        crate::pycompat::json::string(&mut body, &model);
        body.push_str(",\"prompt\":\"\",\"keep_alive\":\"10m\"}");
        sh::quiet(&["curl", "-fsS", "-m", "30", "http://127.0.0.1:11434/api/generate", "-d", &body]);
    }
    if let Some(t) = voiced {
        let _ = t.join();
    }
    0
}

// ---- labyrinth-sarina-voice-setup

const DEFAULT_VOICES: [&str; 3] = ["en_US-amy-medium", "en_GB-jenny_dioco-medium", "en_US-lessac-medium"];

/// A voice's name: ^[A-Za-z0-9_.-]{1,80}$, not starting with `-` (else it is an option of the downloader's).
pub fn voice_name(v: &str) -> bool {
    crate::sarina_setting::voice_ok(v) && !v.starts_with('-')
}

pub fn voice_setup(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-sarina-voice-setup");
    let voices: Vec<&str> = if args.len() > 1 { args[1..].iter().map(|s| s.as_str()).collect() } else { DEFAULT_VOICES.to_vec() };
    if let Some(bad) = voices.iter().find(|v| !voice_name(v)) {
        eprintln!("not a voice name: {bad}");
        return 2;
    }
    let data = env_nonempty("XDG_DATA_HOME").unwrap_or_else(|| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let dir = env_nonempty("LABYRINTH_TTS_DIR").unwrap_or_else(|| format!("{data}/labyrinth/tts"));
    if let Err(e) = sh::mkdir_p(&format!("{dir}/voices")) {
        eprintln!("mkdir: cannot create directory '{dir}/voices': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
        return 1;
    }
    if !sh::executable(&format!("{dir}/venv/bin/python")) {
        let c = sh::status(&["python3", "-m", "venv", &format!("{dir}/venv")]);
        if c != 0 {
            return c;
        }
    }
    // exactly the pinned files, checked by hash, wheels only (no package's own build code runs), nothing upgraded on its own
    let pip = format!("{dir}/venv/bin/pip");
    let c = sh::status(&[
        &pip,
        "install",
        "--quiet",
        "--require-hashes",
        "--only-binary",
        ":all:",
        "--no-deps",
        "-r",
        "/usr/share/labyrinth/voice/piper-requirements.txt",
    ]);
    if c != 0 {
        return c;
    }
    let python = format!("{dir}/venv/bin/python");
    let voices_dir = format!("{dir}/voices");
    let mut argv = vec![python.as_str(), "-m", "piper.download_voices", "--download-dir", &voices_dir];
    argv.extend(&voices);
    let (c, out) = sh::combined(&argv);
    use std::io::Write;
    let _ = std::io::stdout().write_all(sh::tail(&out, 3));
    if c != 0 {
        return c;
    }
    let n = sh::glob(&voices_dir, |n| n.ends_with(".onnx")).len();
    println!("Done. Voices in {voices_dir}: {n}");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_lines() {
        let s = settings(b"SARINA_VAULT=/v\nA=b=\nB=b=c=\nC==\nlower=x\n1X=y\nD= spaced \nnot a line\n");
        let want: Vec<(String, String)> = [("SARINA_VAULT", "/v"), ("A", "b"), ("B", "b=c="), ("C", ""), ("D", " spaced ")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        assert_eq!(s, want);
        assert!(
            denied("LD_PRELOAD")
                && denied("BASH_ENV")
                && denied("UID")
                && denied("BASH_VERSION")
                && !denied("SARINA_VAULT")
                && !denied("OPENAI_MODEL")
        );
        assert_eq!(first_value(b"X=1\nVOICE_MODEL=a\nVOICE_MODEL=b\n", "VOICE_MODEL"), "a");
        assert!(voice_name("en_US-amy-medium") && !voice_name("--download-dir") && !voice_name("a/b"));
    }
}
