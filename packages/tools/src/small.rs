//! Three small programs, each the bash or sh script of the same name with the same output, calls and exit codes:
//!
//!   labyrinth-claude-code [install|update|status]  installs, updates or checks Claude Code for this user (Anthropic's own
//!                                                  installer, into ~/.local/bin)
//!   labyrinth-install                              starts the installer from the live desktop (yggdrasil-installer), or says
//!                                                  it is missing
//!   labyrinth-notify-activate                      started by D-Bus when a notification is sent and nobody owns
//!                                                  org.freedesktop.Notifications: starts the panel (which serves them) and
//!                                                  waits until it has taken the name
//!
//! curl, bash (Claude Code's installer is a bash script), claude, yggdrasil-installer, notify-send, dbus-send and the panel are
//! still those programs; `command -v`, the process list (the script's pgrep -f) and the waits are done here.
//!
//! Fixed in both (scripts and ports): labyrinth-claude-code ran `curl ... | bash`: a download cut short ran as far as it came, and
//! a failed download was never reported (bash read nothing and succeeded). The installer is downloaded whole first now, and run
//! only when the download succeeded. And `update` said "not installed yet" when the update itself failed.

use crate::sh::{self, Io};

// ---- labyrinth-claude-code

fn claude_status() {
    match sh::which("claude") {
        Some(p) => {
            let (_, out) = sh::capture(&["claude", "--version"], Io::Null);
            println!("claude:   {} ({p})", sh::subst(out.split(|&c| c == b'\n').next().unwrap_or(&[])));
        }
        None => println!("claude:   not installed"),
    }
    for c in ["git", "rg", "node", "npm", "gh", "jq", "ssh"] {
        println!("{} {}", sh::pad(&format!("{c}:"), 9), sh::which(c).unwrap_or_else(|| "missing".into()));
    }
    println!("(git is needed; node and npm for MCP servers; gh for GitHub; the others are conveniences)");
}

pub fn claude_code(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-claude-code");
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{home}/.local/bin:{path}"));
    match args.get(1).map(|s| s.as_str()).filter(|s| !s.is_empty()).unwrap_or("install") {
        "status" => {
            claude_status();
            0
        }
        "update" => {
            if !sh::have("claude") {
                println!("Claude Code is not installed yet: run labyrinth-claude-code");
                return 1;
            }
            sh::status(&["claude", "update"])
        }
        "install" => {
            if sh::have("claude") {
                println!("Claude Code is already installed.");
            } else {
                let tmpdir = std::env::var("TMPDIR").ok().filter(|t| !t.is_empty()).unwrap_or_else(|| "/tmp".into());
                let ok = match sh::mkstemp(&format!("{tmpdir}/tmp.XXXXXXXXXX")) {
                    Ok((tmp, f)) => {
                        drop(f);
                        let ok = sh::status(&["curl", "-fsSL", "-o", &tmp, "https://claude.ai/install.sh"]) == 0
                            && sh::status(&["bash", &tmp]) == 0;
                        let _ = std::fs::remove_file(&tmp);
                        ok
                    }
                    Err(_) => false,
                };
                if !ok {
                    println!("The installer did not finish. Check the network and try again.");
                    return 1;
                }
            }
            claude_status();
            println!();
            println!("Next: open a terminal in your project and run: claude   (it opens the browser to sign in the first time)");
            0
        }
        _ => {
            println!("usage: labyrinth-claude-code [install|update|status]");
            2
        }
    }
}

// ---- labyrinth-install

pub fn install(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-install");
    if sh::have("yggdrasil-installer") {
        let mut argv = vec!["yggdrasil-installer".to_string()];
        argv.extend(args.iter().skip(1).cloned());
        return sh::exec(&argv);
    }
    let msg = "The installer (yggdrasil-installer) is missing from this medium.";
    if sh::run(&["notify-send", "Labyrinth installer", msg], Io::Inherit, Io::Null) != 0 {
        eprintln!("{msg}");
    }
    1
}

// ---- labyrinth-notify-activate

const PANEL: &str = "/usr/bin/yggdrasil-panel";

/// Does a process run whose whole command line is exactly the panel's path (pgrep -f '^/usr/bin/yggdrasil-panel$')?
pub fn panel_running() -> bool {
    sh::processes().iter().any(|pid| {
        std::fs::read(format!("/proc/{pid}/cmdline"))
            .map(|c| {
                let joined: Vec<u8> = c.strip_suffix(b"\0").unwrap_or(&c).iter().map(|&b| if b == 0 { b' ' } else { b }).collect();
                joined == PANEL.as_bytes()
            })
            .unwrap_or(false)
    })
}

pub fn notify_activate() -> i32 {
    sh::set_prog("labyrinth-notify-activate");
    if !panel_running() {
        let _ = sh::detached(&[PANEL], &[], "/dev/null");
    }
    for _ in 0..100 {
        let (_, out) = sh::capture(
            &[
                "dbus-send",
                "--session",
                "--dest=org.freedesktop.DBus",
                "--print-reply",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus.NameHasOwner",
                "string:org.freedesktop.Notifications",
            ],
            Io::Null,
        );
        if sh::contains(&out, "true") {
            return 0;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    1
}
