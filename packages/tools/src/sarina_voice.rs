//! Sarina's two voice scripts, each the bash script of the same name with the same output, calls and exit codes:
//!
//!   labyrinth-sarina-ask FILE.wav   a spoken question (Super+S): whisper-cli turns the recording into words, sarina-tone notes how
//!                                   it was said, and labyrinth-ask-sarina --voice answers; each sentence is printed as it comes
//!   labyrinth-sarina-speak [TEXT]   says Sarina's answer aloud: with the voice service (labyrinth-voiced) when it runs, else with
//!                                   Piper; --stop, --to FILE.wav, --lines (a sentence per line of standard input)
//!
//! whisper-cli, sarina-tone, labyrinth-ask-sarina, curl, Piper (its venv's python), the voice service's client
//! (/usr/bin/sarina-voice speak-voiced), pw-play, pkill and cp are still those programs; reading sarina.env, the clean-up of what
//! whisper heard, the markdown and think blocks taken out of an answer, the names said as spelled (sarina-say.json; the script ran
//! Python for it) and the loudness of the speech for the panel's voice island (the script ran the venv's Python for it) are done
//! here.
//!
//! The switch: the voice service's client is /usr/bin/sarina-voice (Sarina's package) started as `sarina-voice speak-voiced`; the
//! script ran labyrinth-sarina-speak-voiced from its own folder (/usr/local/bin, a link to the same program).
//!
//! Not 1:1:
//!   - text that is not UTF-8 (from whisper-cli, sarina-tone or labyrinth-ask-sarina) is read with replacement characters;
//!   - the ask: "said something yet" is kept in memory, not in a file made with mktemp in XDG_RUNTIME_DIR (when that folder was
//!     missing the script printed a blank line before every sentence, and errors);
//!   - [[:punct:]] for the stock phrases whisper "hears" in silence is every character that is not a letter, digit, space or
//!     control character (glibc's list differs for characters Unicode has not assigned);
//!   - speak: `--to` as the last argument is a file name of "" (bash looped for ever on its failed `shift 2`);
//!   - speak: a spelling in sarina-say.json is said as written (Python read backslashes in it as escapes and group references,
//!     and a bad one stopped it: nothing was said); the file's JSON is read as JSON (Python also took NaN and Infinity);
//!   - speak: stopped with SIGTERM (--stop, a new answer) it removes its recording and pid file as bash's EXIT trap did; Python
//!     is not needed for the names or the loudness.

use crate::sh::{self, Io};
use std::ffi::{CString, OsString};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::process::{Command, Stdio};
use std::sync::LazyLock;

macro_rules! re {
    ($name:ident, $p:expr) => {
        static $name: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new($p).expect("pattern"));
    };
    ($name:ident, bytes $p:expr) => {
        static $name: LazyLock<regex::bytes::Regex> = LazyLock::new(|| regex::bytes::Regex::new($p).expect("pattern"));
    };
}

fn var(k: &str) -> Option<String> {
    std::env::var_os(k).map(|v| v.to_string_lossy().into_owned())
}

/// `${K:-default}`
fn var_or(k: &str, default: &str) -> String {
    var(k).filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_string())
}

fn home() -> String {
    var("HOME").unwrap_or_default()
}

fn config_home() -> String {
    var_or("XDG_CONFIG_HOME", &format!("{}/.config", home()))
}

fn data_home() -> String {
    var_or("XDG_DATA_HOME", &format!("{}/.local/share", home()))
}

/// `sed -n 's/^KEY=//p' FILE | head -n1`: the first KEY= line's value, "" when none.
fn env_file_value(path: &str, key: &str) -> String {
    let b = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let prefix = format!("{key}=");
    for line in sh::lines(&b) {
        if line.starts_with(prefix.as_bytes()) {
            return sh::subst(&line[prefix.len()..]);
        }
    }
    String::new()
}

/// [[:space:]] in a UTF-8 locale (glibc's iswspace).
pub fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1680}' | '\u{2000}'..='\u{2006}' | '\u{2008}'..='\u{200a}'
        | '\u{2028}' | '\u{2029}' | '\u{205f}' | '\u{3000}')
}

fn strip_space(s: &str) -> &str {
    s.trim_matches(is_space)
}

fn code_of(st: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    st.code().unwrap_or_else(|| 128 + st.signal().unwrap_or(0))
}

// ---------------------------------------------------------------- labyrinth-sarina-ask

/// What whisper printed, as the script's `tr '\n' ' ' | sed` left it: one line, no space at either end, runs of spaces made one.
pub fn heard_line(out: &[u8]) -> String {
    let t = sh::subst(out).replace('\n', " ");
    let t = strip_space(&t);
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        if !(c == ' ' && s.ends_with(' ')) {
            s.push(c);
        }
    }
    s
}

re!(LAUGH, r"(?i-u)[\[(*] *(laugh|laughs|laughing|laughter|chuckl|giggl)[a-z]* *[\])*]");
re!(BRACKETS, r"\[[^\]]*\]");
re!(STARS, r"\*[^*]*\*");
re!(PARENS, r"\([^)]*\)");

/// Laughter written as [laughs], (laughing) or *chuckles*.
pub fn laughed(heard: &str) -> bool {
    LAUGH.is_match(heard)
}

/// What was said, without whisper's notes in brackets, stars and parentheses ([BLANK_AUDIO], (silence)).
pub fn clean(heard: &str) -> String {
    let t = BRACKETS.replace_all(heard, "");
    let t = STARS.replace_all(&t, "");
    let t = PARENS.replace_all(&t, "");
    strip_space(&t).to_string()
}

/// A stock phrase whisper "hears" in silence or noise: not a question.
pub fn stock_phrase(clean: &str) -> bool {
    let lower: String =
        clean.chars().map(|c| c.to_ascii_lowercase()).filter(|&c| c.is_alphanumeric() || is_space(c) || c.is_control()).collect();
    matches!(
        strip_space(&lower),
        "you"
            | "thank you"
            | "thanks"
            | "thanks for watching"
            | "thank you for watching"
            | "thanks for listening"
            | "please subscribe"
            | "subscribe"
            | "see you next time"
            | "see you"
            | "bye"
            | "bye bye"
            | "goodbye"
            | "okay"
            | "ok"
            | "uh"
            | "um"
            | "hmm"
            | "oh"
            | "yeah"
            | ""
    )
}

re!(THINK_LINE, r"<think>.*</think>");
re!(HEADING, r"^#+ *");

/// One line of her answer as the panel gets it; the state says whether a think block is open. None: nothing of it is shown.
pub fn answer_line(line: &str, thinking: &mut bool) -> Option<String> {
    let mut line = line.to_string();
    if *thinking {
        let i = line.find("</think>")?;
        *thinking = false;
        line = line[i + "</think>".len()..].to_string();
    }
    if line.contains("<think>") && !line.contains("</think>") {
        *thinking = true;
        line.truncate(line.find("<think>").unwrap_or(line.len()));
    }
    let t = THINK_LINE.replace_all(&line, "");
    let t = t.replace("**", "").replace('`', "");
    let t = HEADING.replace(&t, "");
    let t = strip_space(&t);
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn say(m: &str) {
    println!("{m}");
}

/// The first readable model file of a glob, in bash's order.
fn first_model(dir: &str) -> Option<String> {
    sh::glob(dir, |n| n.starts_with("ggml-") && n.ends_with(".bin") && n.len() >= "ggml-.bin".len()).into_iter().find(|p| sh::is_file(p))
}

pub fn ask(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-sarina-ask");
    let wav = args.get(1).cloned().unwrap_or_default();
    if !sh::is_file(&wav) {
        say("There is no recording to send.");
        return 1;
    }
    if !sh::have("sarina") {
        say("Sarina is not installed on this computer yet.");
        return 1;
    }
    // her model server (Ollama) has to be running already; nothing is started or downloaded here
    if !sh::quiet(&["curl", "-fsS", "-m", "2", "http://127.0.0.1:11434/api/tags"]) {
        say("Sarina's model server is not running.");
        return 1;
    }

    // ---- 1. speech to text
    let mut model = var("LABYRINTH_STT_MODEL").unwrap_or_default();
    if model.is_empty() {
        model =
            first_model(&format!("{}/labyrinth/stt", data_home())).or_else(|| first_model("/usr/share/labyrinth/stt")).unwrap_or_default();
    }
    let whisper = var_or("LABYRINTH_WHISPER", "whisper-cli");
    if !sh::have(&whisper) || model.is_empty() || !sh::is_file(&model) {
        say("Listening is not set up yet: it needs whisper-cpp and a speech model (see docs/SARINA_VOICE.md).");
        return 1;
    }
    let cfg = format!("{}/labyrinth", config_home());
    let words = sh::head1(&format!("{cfg}/stt-words")).unwrap_or_default();
    let owner = match var("SARINA_OWNER_NAME").filter(|v| !v.is_empty()) {
        Some(o) => o,
        None => env_file_value(&format!("{cfg}/sarina.env"), "SARINA_OWNER_NAME"),
    };
    let mut prompt = String::new();
    if !owner.is_empty() {
        prompt.push_str(&format!("{owner}, "));
    }
    prompt.push_str("Sarina, Labyrinth, Heimdall, Fylgja, Minos, LykanOS, Chymaera, Limbo, Obsidian, Hyprland");
    if !words.is_empty() {
        prompt.push_str(&format!(", {words}"));
    }
    prompt.push('.');
    let lang = var_or("LABYRINTH_STT_LANG", "en");
    let started = std::time::Instant::now();
    let (_, out) = sh::capture(&[&whisper, "-m", &model, "-f", &wav, "-nt", "-np", "-l", &lang, "--prompt", &prompt], Io::Null);
    let heard = heard_line(&out);
    // how long speech to text took, for labyrinth-ask-sarina's timing line (LABYRINTH_VOICE_TIMING=1)
    std::env::set_var("LABYRINTH_VOICE_WHISPER_S", format!("{:.2}", started.elapsed().as_secs_f64()));
    let laughed = laughed(&heard);
    let mut said = clean(&heard);
    if stock_phrase(&said) {
        said.clear();
    }
    if said.is_empty() && laughed {
        said = "Haha.".into();
    }
    if said.is_empty() {
        say("I did not catch that. Try again, a little closer to the microphone.");
        return 1;
    }

    // how it was said, as a private style hint for her (off: Settings > Sarina > "Notice how I sound", or LABYRINTH_TONE=0)
    let mut tone_on = var_or("LABYRINTH_TONE", "1") != "0";
    if sh::cat(&format!("{cfg}/sarina-tone")).as_deref() == Some("off") {
        tone_on = false;
        if std::env::var_os("LABYRINTH_TONE").is_some() {
            std::env::set_var("LABYRINTH_TONE", "0");
        }
    }
    let mut tone = String::new();
    if tone_on && sh::have("sarina-tone") {
        let (_, o) = sh::capture(&["sarina-tone", &wav], Io::Null);
        tone = sh::subst(o.split(|&c| c == b'\n').next().unwrap_or(&[]));
    }

    // what was understood is printed at once (the card shows it while Sarina works on the answer)
    say(&format!("Heard: \"{said}\""));

    // ---- 2. ask Sarina
    let vm = match var("LABYRINTH_VOICE_MODEL").filter(|v| !v.is_empty()) {
        Some(v) => v,
        None => env_file_value(&format!("{cfg}/sarina.env"), "VOICE_MODEL"),
    };
    if !vm.is_empty() {
        // if the voice model is missing or failing, her usual model answers instead of nothing
        let main = env_file_value(&format!("{cfg}/sarina.env"), "OPENAI_MODEL");
        if !main.is_empty() && main != vm {
            std::env::set_var("SARINA_FALLBACK_MODELS", var_or("SARINA_FALLBACK_MODELS", &main));
        }
        std::env::set_var("OPENAI_MODEL", &vm);
    }
    // the rules for a spoken answer go in her system prompt once, so a conversation's history is the questions and answers
    std::env::set_var("LABYRINTH_VOICE_RULES", sh::cat("/usr/share/labyrinth/sarina-voice-rules.txt").unwrap_or_default());
    let mut question = String::new();
    if laughed {
        question.push_str("(They laughed.) ");
    }
    if !tone.is_empty() {
        question.push_str(&format!("(Style hint, never mention it or say how they sound: {tone}) "));
    }
    question.push_str(&said);
    // the desktop names a conversation's turns YGGDRASIL_CONVERSATION (older panels: LABYRINTH_CONVERSATION)
    let conversation = match var("YGGDRASIL_CONVERSATION").filter(|v| !v.is_empty()) {
        Some(c) => c,
        None => var("LABYRINTH_CONVERSATION").unwrap_or_default(),
    };
    let mut argv: Vec<String> = vec!["labyrinth-ask-sarina".into(), "--voice".into()];
    if !conversation.is_empty() {
        argv.extend(["--conversation".into(), conversation]);
    }
    argv.extend(["--cwd".into(), home(), question]);

    // her answer comes a sentence per line as she writes it: each line is passed on at once, without markdown marks
    let _ = std::io::stdout().flush();
    let child = Command::new(&argv[0]).args(&argv[1..]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
    let mut printed = false;
    let rc = match child {
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                127
            } else {
                126
            }
        }
        Ok(mut c) => {
            let mut r = BufReader::new(c.stdout.take().expect("piped"));
            let mut thinking = false;
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match r.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                if buf.last() == Some(&b'\n') {
                    buf.pop();
                }
                buf.retain(|&b| b != 0);
                let line = String::from_utf8_lossy(&buf);
                if let Some(l) = answer_line(&line, &mut thinking) {
                    if !printed {
                        println!();
                    }
                    printed = true;
                    say(&l);
                }
            }
            drop(r);
            c.wait().map(code_of).unwrap_or(1)
        }
    };
    if rc == 3 {
        say("I did not catch that: it sounded like my own voice.");
        return 1;
    }
    if rc != 0 && !printed {
        say("Sarina could not answer (is her service set up? try: sarina doctor).");
        return 1;
    }
    if !printed {
        say("Sarina had no answer.");
        return 1;
    }
    0
}

// ---------------------------------------------------------------- labyrinth-sarina-speak

re!(THINK_ALL, bytes r"(?s)<think>.*</think>");
re!(MARKS, bytes r"[*`#_]+");
re!(LINK, bytes r"\[([^\]]*)\]\([^)]*\)");
re!(SPACES, bytes r"  +");

/// The script's plain(): no think blocks, no markdown marks, links as their text, one line, at most 700 characters; the names
/// as the owner spelled them out. Bytes in, as `printf '%s' | sed | tr | sed | cut` passed them.
pub fn plain(text: &[u8], map: &str) -> Vec<u8> {
    let t = THINK_ALL.replace_all(text, &b""[..]);
    let t: Vec<u8> = t.iter().map(|&c| if c == b'\n' { b' ' } else { c }).collect();
    let t = MARKS.replace_all(&t, &b""[..]);
    let t = LINK.replace_all(&t, &b"$1"[..]);
    let t = SPACES.replace_all(&t, &b" "[..]);
    let mut t: &[u8] = &t;
    while t.first() == Some(&b' ') {
        t = &t[1..];
    }
    while t.last() == Some(&b' ') {
        t = &t[..t.len() - 1];
    }
    say_names(first_chars(t, 700).to_vec(), map)
}

/// `cut -c1-N` in a UTF-8 locale: the first N characters, a byte that is not part of one counting as one.
pub fn first_chars(t: &[u8], n: usize) -> &[u8] {
    let (mut count, mut end) = (0, 0);
    for chunk in t.utf8_chunks() {
        for c in chunk.valid().chars() {
            if count == n {
                return &t[..end];
            }
            count += 1;
            end += c.len_utf8();
        }
        for _ in chunk.invalid() {
            if count == n {
                return &t[..end];
            }
            count += 1;
            end += 1;
        }
    }
    &t[..end]
}

/// Names said the way sarina-say.json spells them ({"Siobhan": "Shivawn"}; whole words, any case).
pub fn say_names(text: Vec<u8>, map: &str) -> Vec<u8> {
    if !sh::is_file(map) {
        return text;
    }
    // Python read the text as UTF-8 and stopped (nothing out) when it was not
    let mut s = match String::from_utf8(text) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let names: serde_json::Value = match std::fs::read(map).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
        Some(v) => v,
        None => return s.into_bytes(),
    };
    let obj = match names.as_object() {
        Some(o) => o,
        // a list or a number: Python stopped on names.items()
        None => return vec![],
    };
    for (word, said) in obj {
        let Some(said) = said.as_str() else { continue };
        if word.trim().is_empty() || word == "_" {
            continue;
        }
        if let Ok(r) = regex::Regex::new(&format!(r"(?i)\b{}\b", regex::escape(word))) {
            s = r.replace_all(&s, regex::NoExpand(said)).into_owned();
        }
    }
    s.into_bytes()
}

/// The loudness of the speech, one number (0 to 100) per 50 ms, relative to her loudest moment; None when the file cannot be
/// read as the script's Python read it (the wave module: PCM, the samples taken as 16-bit).
pub fn levels(wav: &[u8]) -> Option<String> {
    let u16le = |b: &[u8], i: usize| -> Option<u32> { Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?) as u32) };
    let u32le = |b: &[u8], i: usize| -> Option<u32> { Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?)) };
    if wav.get(0..4)? != b"RIFF" || wav.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut pos, mut fmt, mut data) = (12usize, None, None);
    while pos + 8 <= wav.len() {
        let id = &wav[pos..pos + 4];
        let size = u32le(wav, pos + 4)? as usize;
        let body = pos + 8;
        if id == b"fmt " {
            let tag = u16le(wav, body)?;
            let channels = u16le(wav, body + 2)?;
            let rate = u32le(wav, body + 4)?;
            let bits = u16le(wav, body + 14)?;
            let pcm = tag == 1 || (tag == 0xfffe && wav.get(body + 24..body + 26) == Some(&[1, 0][..]));
            if !pcm || channels == 0 || bits == 0 {
                return None;
            }
            fmt = Some((channels as usize, rate as usize, bits.div_ceil(8) as usize));
        } else if id == b"data" {
            let (ch, _, width) = fmt?;
            let end = (body + size).min(wav.len());
            let frame = ch * width;
            let n = (end - body) / frame;
            data = Some(&wav[body..body + n * frame]);
            break;
        }
        pos = body + size + (size & 1);
    }
    let (_, rate, _) = fmt?;
    let data = data?;
    if data.len() % 2 != 0 {
        return None;
    }
    let samples: Vec<i64> = data.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c) as i64).collect();
    let step = (rate / 20).max(1);
    let rms: Vec<f64> =
        samples.chunks(step).map(|c| ((c.iter().map(|v| v * v).sum::<i64>() as f64) / (c.len() as f64)).sqrt() / 32768.0).collect();
    let peak = rms.iter().cloned().fold(None, |m: Option<f64>, r| Some(m.map_or(r, |m| if r > m { r } else { m }))).unwrap_or(0.0);
    let out: Vec<String> = rms
        .iter()
        .map(|&r| if peak > 1e-4 && r > 0.04 * peak { (((r / peak).powf(0.9)).min(1.0) * 100.0).trunc().to_string() } else { "0".into() })
        .collect();
    Some(out.join(" "))
}

/// The files a SIGTERM removes, as bash's EXIT trap did (the recording; the pid file of --lines).
static CLEANUP: std::sync::Mutex<Vec<CString>> = std::sync::Mutex::new(Vec::new());

extern "C" fn on_term(_: libc::c_int) {
    if let Ok(list) = CLEANUP.try_lock() {
        for p in list.iter() {
            unsafe { libc::unlink(p.as_ptr()) };
        }
    }
    unsafe { libc::_exit(128 + libc::SIGTERM) };
}

fn cleanup(paths: &[&str]) {
    if let Ok(mut l) = CLEANUP.lock() {
        l.clear();
        l.extend(paths.iter().filter_map(|p| CString::new(*p).ok()));
    }
    unsafe { libc::signal(libc::SIGTERM, on_term as extern "C" fn(libc::c_int) as libc::sighandler_t) };
}

struct Speak {
    dir: String,
    run: String,
    pidfile: String,
    to: String,
}

impl Speak {
    /// Stop whatever is being said now: the speaking process (when it is this program) and what it started.
    fn stop(&self) {
        if !sh::is_file(&self.pidfile) {
            return;
        }
        let p = sh::cat(&self.pidfile).unwrap_or_default();
        if !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()) {
            let cmdline = std::fs::read(format!("/proc/{p}/cmdline")).unwrap_or_default();
            let cmdline: Vec<u8> = cmdline.iter().map(|&c| if c == 0 { b' ' } else { c }).collect();
            if String::from_utf8_lossy(&cmdline).contains("labyrinth-sarina-speak") {
                sh::run(&["pkill", "-TERM", "-P", &p], Io::Inherit, Io::Null);
                if let Ok(pid) = p.parse::<i32>() {
                    unsafe { libc::kill(pid, libc::SIGTERM) };
                }
            }
        }
        let _ = sh::rm_f(&self.pidfile);
    }

    fn make_run(&self) -> bool {
        sh::mkdir_p(&self.run).is_ok() && {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.run, std::fs::Permissions::from_mode(0o700));
            true
        }
    }

    fn write_pid(&self) {
        if let Err(e) = sh::write(&self.pidfile, format!("{}\n", std::process::id()).as_bytes()) {
            eprintln!("{}: {}: {}", sh::prog(), self.pidfile, crate::py::strerror(e.raw_os_error().unwrap_or(0)));
        }
    }

    /// The voice service's client, when the service is up (not for --to; LABYRINTH_SARINA_ENGINE=piper forces Piper).
    fn voiced(&self, line: &[u8]) -> bool {
        let sock = var_or("LABYRINTH_VOICED_SOCKET", &format!("{}/voiced.sock", self.run));
        if !self.to.is_empty() || var("LABYRINTH_SARINA_ENGINE").as_deref() == Some("piper") || !sh::is_socket(&sock) {
            return false;
        }
        let text = OsString::from_vec(line.to_vec());
        sh::status(&[OsString::from("/usr/bin/sarina-voice"), OsString::from("speak-voiced"), text]) == 0
    }

    /// The voice and Piper: (python, model), or why not.
    fn piper(&self) -> Result<(String, String), String> {
        let voice = match var("LABYRINTH_SARINA_VOICE").filter(|v| !v.is_empty()) {
            Some(v) => v,
            None => sh::head1(&format!("{}/labyrinth/sarina-voice", config_home())).unwrap_or_default(),
        };
        let voice = if voice.is_empty() { "en_US-amy-medium".to_string() } else { voice };
        if !voice.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-')) {
            return Err("bad voice name".into());
        }
        let model = format!("{}/voices/{voice}.onnx", self.dir);
        let py = var_or("LABYRINTH_TTS_PYTHON", &format!("{}/venv/bin/python", self.dir));
        if !(sh::executable(&py) && sh::is_file(&model)) {
            return Err(format!("Speaking is not set up: Piper or the voice {voice} is missing (see docs/SARINA_VOICE.md)."));
        }
        Ok((py, model))
    }

    fn temp_wav(&self) -> String {
        let mut b = CString::new(format!("{}/speech.XXXXXX.wav", self.run)).unwrap_or_default().into_bytes_with_nul();
        let fd = unsafe { libc::mkostemps(b.as_mut_ptr() as *mut libc::c_char, 4, libc::O_CLOEXEC) };
        if fd < 0 {
            eprintln!(
                "mktemp: failed to create file via template '{}/speech.XXXXXX.wav': {}",
                self.run,
                crate::py::strerror(std::io::Error::last_os_error().raw_os_error().unwrap_or(0))
            );
            return String::new();
        }
        unsafe { libc::close(fd) };
        b.pop();
        String::from_utf8_lossy(&b).into_owned()
    }
}

/// `printf '%s' TEXT | PY -P -m piper -m MODEL -f WAV >/dev/null 2>&1`
fn run_piper(py: &str, model: &str, wav: &str, text: &[u8]) -> bool {
    let _ = std::io::stdout().flush();
    let child = Command::new(py)
        .args(["-P", "-m", "piper", "-m", model, "-f", wav])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mut c = match child {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut i) = c.stdin.take() {
        let _ = i.write_all(text);
    }
    c.wait().map(|s| s.success()).unwrap_or(false)
}

fn play(wav: &str) {
    let player = var_or("LABYRINTH_PLAYER", "pw-play");
    sh::run(&[player.as_str(), wav], Io::Null, Io::Null);
}

pub fn speak(args: &[OsString]) -> i32 {
    sh::set_prog("labyrinth-sarina-speak");
    let dir = var_or("LABYRINTH_TTS_DIR", &format!("{}/labyrinth/tts", data_home()));
    // never a shared /tmp folder another account could make first
    let run = format!("{}/labyrinth-sarina", var_or("XDG_RUNTIME_DIR", &format!("/run/user/{}", unsafe { libc::getuid() })));
    let mut s = Speak { dir, pidfile: format!("{run}/speaking.pid"), run, to: String::new() };
    let mut lines = false;
    let mut rest: Vec<Vec<u8>> = vec![];
    let mut i = 1;
    while i < args.len() {
        let a = args[i].clone().into_vec();
        match &a[..] {
            b"--stop" => {
                s.stop();
                return 0;
            }
            b"--to" => {
                s.to = args.get(i + 1).map(|v| v.to_string_lossy().into_owned()).unwrap_or_default();
                i += 2;
            }
            b"--lines" => {
                lines = true;
                i += 1;
            }
            _ => {
                rest.push(a);
                i += 1;
            }
        }
    }
    let map = var_or("LABYRINTH_SAY_MAP", &format!("{}/labyrinth/sarina-say.json", config_home()));

    if lines {
        // each line as it arrives: one voice at a time, this process holds the floor until the input ends
        s.make_run();
        s.stop();
        s.write_pid();
        cleanup(&[&s.pidfile]);
        let stdin = std::io::stdin();
        let mut r = stdin.lock();
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match r.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            // read stops at a last line without a newline, as bash's `while read` did
            if buf.last() != Some(&b'\n') {
                break;
            }
            buf.pop();
            buf.retain(|&b| b != 0);
            let line = plain(&buf, &map);
            if line.is_empty() || s.voiced(&line) {
                continue;
            }
            let Ok((py, model)) = s.piper() else { continue };
            let wav = s.temp_wav();
            cleanup(&[&s.pidfile, &wav]);
            if run_piper(&py, &model, &wav, &line) {
                play(&wav);
            }
            let _ = sh::rm_f(&wav);
            cleanup(&[&s.pidfile]);
        }
        let _ = sh::rm_f(&s.pidfile);
        return 0;
    }

    let mut text: Vec<u8> = rest.join(&b' ');
    if text.is_empty() {
        let mut all = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut all);
        text = sh::subst(&all).into_bytes();
        // $(cat) keeps bytes as they are; subst made them text: take the raw bytes without trailing newlines and NULs
        let mut raw: Vec<u8> = all.into_iter().filter(|&c| c != 0).collect();
        while raw.last() == Some(&b'\n') {
            raw.pop();
        }
        text = if raw.is_empty() { text } else { raw };
    }
    let text = plain(&text, &map);
    if text.is_empty() {
        return 0;
    }

    // the voice service when it is up: it makes speech a sentence at a time and plays it; if it fails, Piper speaks
    let sock = var_or("LABYRINTH_VOICED_SOCKET", &format!("{}/voiced.sock", s.run));
    if s.to.is_empty() && var("LABYRINTH_SARINA_ENGINE").as_deref() != Some("piper") && sh::is_socket(&sock) {
        s.make_run();
        s.stop();
        s.write_pid();
        if s.voiced(&text) {
            let _ = sh::rm_f(&s.pidfile);
            return 0;
        }
        let _ = sh::rm_f(&s.pidfile);
    }

    let (py, model) = match s.piper() {
        Ok(v) => v,
        Err(why) => {
            say(&why);
            return 1;
        }
    };
    s.make_run();
    let wav = s.temp_wav();
    cleanup(&[&wav]);
    let done = |code: i32| {
        let _ = sh::rm_f(&wav);
        code
    };
    if !run_piper(&py, &model, &wav, &text) {
        say("The voice could not make that sound.");
        return done(1);
    }
    if !s.to.is_empty() {
        sh::status(&["cp", wav.as_str(), s.to.as_str()]);
        return done(0);
    }

    s.stop(); // one voice at a time
    s.write_pid();
    cleanup(&[&wav, &s.pidfile]);
    // the loudness for the panel's voice island: first line, when playing starts (ms since 1970); second, the numbers
    let env = format!("{}/speech.env", s.run);
    if var("LABYRINTH_NO_ENVELOPE").unwrap_or_default().is_empty() {
        if let Some(levels) = std::fs::read(&wav).ok().and_then(|b| levels(&b)).filter(|l| !l.is_empty()) {
            let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) + 120;
            if let Err(e) = sh::write(&env, format!("{ms}\n{levels}\n").as_bytes()) {
                eprintln!("{}: {env}: {}", sh::prog(), crate::py::strerror(e.raw_os_error().unwrap_or(0)));
            }
        }
    }
    play(&wav);
    let _ = sh::rm_f(&s.pidfile);
    let _ = sh::rm_f(&env);
    done(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_output_is_one_clean_line() {
        assert_eq!(heard_line(b"  Hello   there\n Sarina.\n\n"), "Hello there Sarina.");
        assert_eq!(heard_line(b"\t[BLANK_AUDIO]\n"), "[BLANK_AUDIO]");
    }

    #[test]
    fn notes_and_stock_phrases() {
        assert!(laughed("[Laughs] okay") && laughed("*chuckles*") && laughed("( laughing )") && !laughed("laugh"));
        assert_eq!(clean("[BLANK_AUDIO] (silence) *music* What time is it? "), "What time is it?");
        assert!(stock_phrase("Thank you.") && !stock_phrase("Bye-bye!") && stock_phrase("Thanks for watching!"));
        assert!(stock_phrase("Thank you\u{2026}") && !stock_phrase("Thank you, Sarina."));
    }

    #[test]
    fn answer_lines_skip_think_blocks_and_marks() {
        let mut t = false;
        assert_eq!(answer_line("## **Hi** `there` ", &mut t).as_deref(), Some("Hi there"));
        assert_eq!(answer_line("a <think>x", &mut t).as_deref(), Some("a"));
        assert!(t);
        assert_eq!(answer_line("still thinking", &mut t), None);
        assert_eq!(answer_line("done</think> b", &mut t).as_deref(), Some("b"));
        assert!(!t);
        assert_eq!(answer_line("<think>x</think>   ", &mut t), None);
    }

    #[test]
    fn plain_speech() {
        let t = plain(b"<think>a\nb</think>**Bold** [link](http://x) and `code`\nnext  line_ # ", "/nonexistent");
        assert_eq!(t, b"Bold link and code next line");
        assert_eq!(plain(&[b'a'; 800], "/nonexistent").len(), 700);
        assert_eq!(first_chars("é".repeat(800).as_bytes(), 700).len(), 1400);
        assert_eq!(first_chars(b"a\xffb\xc3", 3), b"a\xffb");
    }

    #[test]
    fn names_as_spelled() {
        let d = std::env::temp_dir().join(format!("sarina-say-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let m = d.join("say.json");
        std::fs::write(&m, r#"{"Siobhan": "Shivawn", "_": "x", " ": "y", "n": 3, "c++": "see plus plus"}"#).unwrap();
        let m = m.to_string_lossy().into_owned();
        assert_eq!(say_names(b"siobhan and SIOBHANS, c++".to_vec(), &m), b"Shivawn and SIOBHANS, c++");
        std::fs::write(&m, "[1]").unwrap();
        assert_eq!(say_names(b"x".to_vec(), &m), b"");
        std::fs::write(&m, "not json").unwrap();
        assert_eq!(say_names(b"x".to_vec(), &m), b"x");
        assert_eq!(say_names(vec![0xff], &m), b"");
        std::fs::remove_dir_all(&d).unwrap();
    }

    fn wav(rate: u32, samples: &[i16]) -> Vec<u8> {
        let mut b = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(rate.to_le_bytes());
        b.extend((rate * 2).to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend(((samples.len() * 2) as u32).to_le_bytes());
        for s in samples {
            b.extend(s.to_le_bytes());
        }
        b
    }

    #[test]
    fn loudness() {
        let mut s = vec![0i16; 100];
        s.extend(vec![16000i16; 100]);
        s.extend(vec![1000i16; 100]);
        assert_eq!(levels(&wav(2000, &s)).as_deref(), Some("0 100 8"));
        assert_eq!(levels(&wav(2000, &[])).as_deref(), Some(""));
        assert_eq!(levels(b"not a wave"), None);
    }

    #[test]
    fn stop_ends_a_speaking_process_and_removes_the_pid_file() {
        let d = std::env::temp_dir().join(format!("sarina-stop-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let mut c = Command::new("bash").args(["-c", "exec -a labyrinth-sarina-speak sleep 30"]).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let pf = d.join("speaking.pid").to_string_lossy().into_owned();
        std::fs::write(&pf, format!("{}\n", c.id())).unwrap();
        let s = Speak { dir: String::new(), run: d.to_string_lossy().into_owned(), pidfile: pf.clone(), to: String::new() };
        s.stop();
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(c.wait().unwrap().signal(), Some(libc::SIGTERM));
        assert!(!sh::exists(&pf));
        // a pid file naming another program: left running, the file removed
        let mut other = Command::new("sleep").arg("30").spawn().unwrap();
        std::fs::write(&pf, format!("{}\n", other.id())).unwrap();
        s.stop();
        assert!(other.try_wait().unwrap().is_none() && !sh::exists(&pf));
        other.kill().unwrap();
        let _ = other.wait();
        std::fs::remove_dir_all(&d).unwrap();
    }
}
