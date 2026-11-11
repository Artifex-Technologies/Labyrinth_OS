//! labyrinth-sarina-setting: Sarina's settings for Yggdrasil's Settings > Sarina page (the controls declared in sarina-git's
//! integration file; the way of asking is Yggdrasil's docs/INTEGRATIONS.md):
//!   switches:  tone|forge|checkup|learn status|on|off      choices: voice list|get|set NAME      buttons: hear, agents
//! The bash script of the same name with the same arguments, output, files, calls and exit codes.
//!
//! systemctl --user, labyrinth-sarina-speak (in the background) and ghostty are still those programs; the settings files, the
//! voices' list and the timers' copies are read and written here. sarina.env (it holds the AI provider's key) is written whole
//! under a name only this program picks and renamed into place, readable by you alone (the script used a fixed .tmp name).
//!
//! Not 1:1: when a file cannot be copied or written, the message is this program's (cp's or bash's before).

use crate::sh::{self, Io};
use std::fs;

const SPEAK: &str = "Hello. I am Sarina, and this is how I sound when I answer you.";

fn config_home() -> String {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{}/.config", std::env::var("HOME").unwrap_or_default()))
}

/// The last KEY=... line's value in sarina.env (sed -n "s/^KEY=//p" | tail -n1).
pub fn env_get(text: &[u8], key: &str) -> String {
    let p = format!("{key}=");
    sh::lines(text)
        .into_iter()
        .rfind(|l| l.starts_with(p.as_bytes()))
        .map(|l| String::from_utf8_lossy(&l[p.len()..]).into_owned())
        .unwrap_or_default()
}

/// sarina.env without its KEY= lines, then KEY=VALUE when VALUE is not empty (each line ends with a newline, as grep wrote it).
pub fn env_with(text: &[u8], key: &str, value: &str) -> Vec<u8> {
    let p = format!("{key}=");
    let mut out = vec![];
    for l in sh::lines(text).into_iter().filter(|l| !l.starts_with(p.as_bytes())) {
        out.extend_from_slice(l);
        out.push(b'\n');
    }
    if !value.is_empty() {
        out.extend_from_slice(format!("{key}={value}\n").as_bytes());
    }
    out
}

fn env_set(file: &str, key: &str, value: &str) -> i32 {
    let old = fs::read(file).unwrap_or_default();
    match sh::replace_file(file, &env_with(&old, key, value), 0o600, None) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("labyrinth-sarina-setting: {file}: {e}");
            1
        }
    }
}

/// A user timer installed from /usr/share/labyrinth once: status|on|off. Err: exit now with that code.
fn daily(unit: &str, how: &str) -> Result<i32, i32> {
    let timer = format!("{unit}.timer");
    match how {
        "status" => {
            println!("{}", if sh::quiet(&["systemctl", "--user", "is-enabled", &timer]) { "on" } else { "off" });
            Ok(0)
        }
        "on" => {
            let dir = format!("{}/systemd/user", config_home());
            if let Err(e) = sh::mkdir_p(&dir) {
                eprintln!("mkdir: cannot create directory '{dir}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
            }
            for f in [format!("{unit}.service"), timer.clone()] {
                let dst = format!("{dir}/{f}");
                if !sh::exists(&dst) {
                    let src = format!("/usr/share/labyrinth/{f}");
                    if let Err(e) = copy(&src, &dst) {
                        let why = crate::py::strerror(e.raw_os_error().unwrap_or(0));
                        if sh::exists(&src) {
                            eprintln!("cp: cannot create regular file '{dst}': {why}");
                        } else {
                            eprintln!("cp: cannot stat '{src}': {why}");
                        }
                        return Err(1);
                    }
                }
            }
            let code = sh::status(&["systemctl", "--user", "daemon-reload"]);
            if code != 0 {
                return Ok(code);
            }
            Ok(sh::run(&["systemctl", "--user", "enable", "--now", &timer], Io::Null, Io::Null))
        }
        "off" => {
            sh::quiet(&["systemctl", "--user", "disable", "--now", &timer]);
            Ok(0)
        }
        _ => Err(2),
    }
}

/// `cp SRC DST`: the bytes, and SRC's mode less the umask for a new DST.
fn copy(src: &str, dst: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let data = fs::read(src)?;
    let mode = fs::metadata(src)?.permissions().mode() & 0o777;
    let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(mode).open(dst)?;
    f.write_all(&data)
}

/// ^[A-Za-z0-9_.-]{1,80}$
pub fn voice_ok(v: &str) -> bool {
    (1..=80).contains(&v.len()) && v.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-sarina-setting");
    let conf = format!("{}/labyrinth", config_home());
    let env_file = format!("{conf}/sarina.env");
    if let Err(e) = sh::mkdir_p_mode(&conf, 0o700) {
        eprintln!("mkdir: cannot create directory '{conf}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
    }
    let data = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let voices = format!("{data}/labyrinth/tts/voices");
    let arg = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
    let speak = || sh::background(&["labyrinth-sarina-speak", SPEAK], Io::Null, Io::Null);
    let r = match arg(1) {
        "voice" => match arg(2) {
            "list" => {
                let mut names: Vec<String> = sh::glob(&voices, |n| n.ends_with(".onnx")).into_iter().filter(|f| sh::is_file(f)).map(|f| sh::base(&f).strip_suffix(".onnx").unwrap_or("").to_string()).collect();
                names.sort_by(|a, b| sh::collate(a, b));
                for n in names {
                    println!("{n}");
                }
                Ok(0)
            }
            "get" => {
                let v = sh::head1(&format!("{conf}/sarina-voice")).unwrap_or_default();
                println!("{}", if v.is_empty() { "en_US-amy-medium" } else { &v });
                Ok(0)
            }
            "set" => {
                let v = arg(3);
                if !voice_ok(v) || !sh::is_file(&format!("{voices}/{v}.onnx")) {
                    return 1;
                }
                if let Err(e) = sh::write(&format!("{conf}/sarina-voice"), format!("{v}\n").as_bytes()) {
                    eprintln!("labyrinth-sarina-setting: {conf}/sarina-voice: {e}");
                }
                speak();
                Ok(0)
            }
            _ => Err(2),
        },
        "hear" => {
            speak();
            Ok(0)
        }
        // "Notice how I sound": on unless switched off (labyrinth-sarina-ask reads the same file)
        "tone" => match arg(2) {
            "status" => {
                println!("{}", if sh::cat(&format!("{conf}/sarina-tone")).as_deref() == Some("off") { "off" } else { "on" });
                Ok(0)
            }
            "on" => Ok(sh::rm_f(&format!("{conf}/sarina-tone")).map(|_| 0).unwrap_or(1)),
            "off" => Ok(sh::write(&format!("{conf}/sarina-tone"), b"off\n").map(|_| 0).unwrap_or(1)),
            _ => Err(2),
        },
        "forge" => match arg(2) {
            "status" => {
                let on = env_get(&fs::read(&env_file).unwrap_or_default(), "SARINA_FORGE") == "on";
                println!("{}", if on { "on" } else { "off" });
                Ok(0)
            }
            "on" => Ok(env_set(&env_file, "SARINA_FORGE", "on")),
            "off" => Ok(env_set(&env_file, "SARINA_FORGE", "")),
            _ => Err(2),
        },
        // the daily check-up, and with it the 10-minute look at what Heimdall holds
        "checkup" => match daily("labyrinth-sarina-checkup", arg(2)) {
            Ok(0) if arg(2) == "status" => Ok(0),
            Ok(0) => daily("labyrinth-sarina-held", arg(2)),
            other => other,
        },
        "learn" => daily("labyrinth-sarina-reflect", arg(2)),
        // her agents, and approving one, need you at a terminal (it shows every file first)
        "agents" => Ok(sh::exec(&[
            "ghostty",
            "-e",
            "bash",
            "-c",
            "sarina forge-list; echo; echo \"To approve one: sarina forge-approve NAME   To remove one: sarina forge-remove NAME\"; exec bash",
        ])),
        _ => {
            eprintln!("usage: labyrinth-sarina-setting voice|hear|tone|forge|checkup|learn|agents ...");
            Ok(2)
        }
    };
    match r {
        Ok(c) | Err(c) => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_file() {
        let t = b"OPENAI_API_KEY=k\nSARINA_FORGE=on\nX=1\nSARINA_FORGE=off";
        assert_eq!(env_get(t, "SARINA_FORGE"), "off");
        assert_eq!(env_get(b"", "SARINA_FORGE"), "");
        assert_eq!(env_with(t, "SARINA_FORGE", "on"), b"OPENAI_API_KEY=k\nX=1\nSARINA_FORGE=on\n");
        assert_eq!(env_with(t, "SARINA_FORGE", ""), b"OPENAI_API_KEY=k\nX=1\n");
        assert!(voice_ok("en_US-amy-medium") && !voice_ok("../x") && !voice_ok("a b") && !voice_ok(""));
    }
}
