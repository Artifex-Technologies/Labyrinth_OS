//! labyrinth-security-setting: Labyrinth's security settings as Yggdrasil's Settings asks for them (declared in
//! /usr/share/yggdrasil/integrations/labyrinth-security.conf; the way of asking is Yggdrasil's docs/INTEGRATIONS.md):
//!   scan FOLDER                      scan one folder now with Heimdall's scanner (the scan's memory of unchanged files)
//!   auto-quarantine status|on|off    certain malware to quarantine by itself (labyrinth-scan-downloads reads scan.conf)
//! The bash script of the same name with the same arguments, output, files, calls and exit codes.
//!
//! Fixed in both: a FOLDER starting with `-` reached heimdalld as an option; it is passed as ./FOLDER now.
//! Not 1:1: with HOME unset (and no XDG_CONFIG_HOME/XDG_STATE_HOME) this says so (bash stopped on the unbound variable); both
//! exit with code 1.

use crate::sh;

const USAGE: &str = "usage: labyrinth-security-setting scan FOLDER | auto-quarantine status|on|off";

fn xdg(var: &str, below_home: &str) -> Option<String> {
    match std::env::var(var) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => std::env::var("HOME").ok().map(|h| format!("{h}/{below_home}")),
    }
}

/// ^[[:space:]]*action[[:space:]]*=[[:space:]]*notify
pub fn says_notify(line: &[u8]) -> bool {
    let l = sh::lstrip(line);
    let Some(l) = l.strip_prefix(b"action") else { return false };
    let Some(l) = sh::lstrip(l).strip_prefix(b"=") else { return false };
    sh::lstrip(l).starts_with(b"notify")
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-security-setting");
    let (Some(conf), Some(state)) = (xdg("XDG_CONFIG_HOME", ".config"), xdg("XDG_STATE_HOME", ".local/state")) else {
        eprintln!("labyrinth-security-setting: HOME is not set");
        return 1;
    };
    let (conf, state) = (format!("{conf}/labyrinth"), format!("{state}/labyrinth"));
    let arg = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
    match arg(1) {
        "scan" => {
            if arg(2).is_empty() {
                return 2;
            }
            if let Err(e) = sh::mkdir_p(&state) {
                eprintln!("mkdir: cannot create directory '{state}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
            }
            let folder = if arg(2).starts_with('-') { format!("./{}", arg(2)) } else { arg(2).to_string() };
            sh::exec(&["heimdalld", "scan-files", "--state", &format!("{state}/scan-state.json"), &folder])
        }
        "auto-quarantine" => {
            let file = format!("{conf}/scan.conf");
            let set = |line: &str| match sh::mkdir_p(&conf) {
                Ok(()) => match sh::write(&file, line.as_bytes()) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("labyrinth-security-setting: {file}: {e}");
                        1
                    }
                },
                Err(e) => {
                    eprintln!("mkdir: cannot create directory '{conf}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
                    1
                }
            };
            match arg(2) {
                "status" => {
                    println!("{}", if sh::grep(&file, says_notify) { "off" } else { "on" });
                    0
                }
                "on" => set("action = quarantine\n"),
                "off" => set("action = notify\n"),
                _ => 2,
            }
        }
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn notify_lines() {
        assert!(super::says_notify(b"action = notify"));
        assert!(super::says_notify(b"  action\t=notifyx"));
        assert!(!super::says_notify(b"# action = notify"));
        assert!(!super::says_notify(b"action = quarantine"));
        assert!(!super::says_notify(b"actions = notify"));
    }
}
