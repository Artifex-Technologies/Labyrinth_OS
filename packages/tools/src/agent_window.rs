//! labyrinth-agent-window [HOURS] | --close | --status: the owner lets the engineering agent use the broad root commands for a
//! few hours (a sudoers drop-in that a timer removes). The bash script of the same name (profile/airootfs/usr/local/sbin) with
//! the same arguments, output, journal lines, files, calls and exit codes.
//!
//! logger, visudo, systemctl and systemd-run are still those programs; the rules are written whole under a name sudo skips,
//! checked by visudo there and renamed into place (src/agent_setup.rs's write_sudoers).
//!
//! Fixed in both (script and port):
//!   - when systemd-run could not set the timer (a leftover unit of the same name makes it fail: open, close and open again
//!     quickly), the rules stayed in place with nothing to remove them, for good (a sudoers file outlives a restart). They are
//!     removed again now, and it says so (exit code 1);
//!   - SUDO_USER went into the rules' comment line unchecked; it must be an account's name now.

use crate::sh::{self, Io};

const FILE: &str = "/etc/sudoers.d/51-labyrinth-agent-window";
const AGENT: &str = "claude-agent";
const UNIT: &str = "labyrinth-agent-window-close";

fn err(msg: &str, code: i32) -> i32 {
    eprintln!("{msg}");
    code
}

/// Written to the journal (Heimdall reads it), then said. Err: logger's exit code (set -e stopped there).
fn note(msg: &str) -> Result<(), i32> {
    let code = sh::status(&["logger", "-t", "labyrinth-agent", "-p", "auth.warning", "--", msg]);
    if code != 0 {
        return Err(code);
    }
    println!("{msg}");
    Ok(())
}

pub fn rules(user: &str, hours: &str) -> String {
    format!(
        "# Written by labyrinth-agent-window for {user}: removed by the timer {UNIT} after {hours} h, or by --close.\n\
         Cmnd_Alias AGENT_WINDOW = /usr/bin/pacman, /usr/bin/systemctl, /usr/bin/journalctl, /usr/bin/loginctl, /usr/bin/systemd-run, \\\n    \
         /usr/bin/heimdalld maintenance *, /usr/bin/limbo, /usr/bin/limbo *, /usr/bin/ufw *, /usr/local/bin/labyrinth-*, /usr/bin/labyrinth-*, \\\n    \
         /usr/bin/install, /usr/bin/mkdir, /usr/bin/chown, /usr/bin/chmod\n\
         {AGENT} ALL=(root) NOPASSWD: AGENT_WINDOW\n"
    )
}

/// awk '{print "closes " $1, $2, $3}' on systemctl's list of the timer.
pub fn closes(list: &[u8]) -> String {
    let text = String::from_utf8_lossy(list);
    let mut out = String::new();
    for line in sh::lines(text.as_bytes()) {
        let line = String::from_utf8_lossy(line);
        let f: Vec<&str> = line.split([' ', '\t']).filter(|w| !w.is_empty()).collect();
        let field = |i: usize| f.get(i).copied().unwrap_or("");
        out.push_str(&format!("closes {} {} {}\n", field(0), field(1), field(2)));
    }
    sh::subst(out.as_bytes())
}

/// ^([1-9]|1[0-2])$
pub fn hours_ok(h: &str) -> bool {
    matches!(h, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11" | "12")
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-agent-window");
    if sh::euid() != 0 {
        return err("run it with sudo: sudo labyrinth-agent-window HOURS", 1);
    }
    let user = std::env::var("SUDO_USER").unwrap_or_default();
    if user.is_empty() || user == AGENT {
        return err("only the owner opens the window (not the agent itself)", 1);
    }
    if !sh::account_name(&user) {
        return err("SUDO_USER is not an account name", 1);
    }
    let arg = match args.get(1) {
        Some(a) if !a.is_empty() => a.as_str(),
        _ => "4",
    };
    let r = match arg {
        "--status" => {
            if sh::is_file(FILE) {
                let (_, out) = sh::capture(&["systemctl", "list-timers", "--all", "--no-legend", &format!("{UNIT}.timer")], Io::Null);
                println!("open: {}", closes(&out));
            } else {
                println!("closed");
            }
            Ok(())
        }
        "--close" => close(&user),
        hours => open(&user, hours),
    };
    match r {
        Ok(()) => 0,
        Err(c) => c,
    }
}

fn close(user: &str) -> Result<(), i32> {
    if let Err(e) = sh::rm_f(FILE) {
        return Err(err(&format!("rm: cannot remove '{FILE}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0))), 1));
    }
    sh::run(&["systemctl", "stop", &format!("{UNIT}.timer")], Io::Inherit, Io::Null);
    note(&format!("agent window closed by {user}"))
}

fn open(user: &str, hours: &str) -> Result<(), i32> {
    if !hours_ok(hours) {
        return Err(err("hours: 1 to 12", 2));
    }
    crate::agent_setup::write_sudoers(FILE, &rules(user, hours), "the window's rules did not validate; nothing changed")?;
    sh::run(&["systemctl", "stop", &format!("{UNIT}.timer")], Io::Inherit, Io::Null);
    let unit = format!("--unit={UNIT}");
    let after = format!("--on-active={hours}h");
    if sh::status(&["systemd-run", "--quiet", &unit, &after, "--timer-property=AccuracySec=1min", "/usr/bin/rm", "-f", FILE]) != 0 {
        let _ = sh::rm_f(FILE);
        return Err(err("could not set the timer that closes the window, so it was not opened", 1));
    }
    note(&format!("agent window opened by {user} for {hours} h"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hours_and_list() {
        for ok in ["1", "9", "10", "12"] {
            assert!(hours_ok(ok));
        }
        for bad in ["0", "13", "01", "1 ", "+1", "١", ""] {
            assert!(!hours_ok(bad));
        }
        assert_eq!(
            closes(b"Wed 2026-11-11 20:00:00 MST 3h left - - labyrinth-agent-window-close.timer x\n"),
            "closes Wed 2026-11-11 20:00:00"
        );
        assert_eq!(closes(b""), "");
        assert_eq!(closes(b"  a\n\n"), "closes a  \ncloses   ");
    }

    #[test]
    fn the_rules() {
        let r = rules("owner", "4");
        assert!(r.starts_with(
            "# Written by labyrinth-agent-window for owner: removed by the timer labyrinth-agent-window-close after 4 h, or by --close.\n"
        ));
        assert!(r.contains("/usr/bin/systemd-run, \\\n    /usr/bin/heimdalld maintenance *"));
        assert!(r.ends_with("claude-agent ALL=(root) NOPASSWD: AGENT_WINDOW\n"));
    }
}
