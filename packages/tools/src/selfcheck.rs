//! labyrinth-selfcheck: is the desktop in a sane state (the session against its packages' upgrades, the kernel, the screens and
//! the panel, the login screen, who serves notifications, Labyrinth's services, Heimdall and its file-integrity list)? Read-only.
//! The Python script of the same name with the same option (--quiet), checks, order, output and exit codes (1 on a FAIL, 2 on a
//! warning); its help text is src/doc/labyrinth-selfcheck.txt.
//!
//! Not 1:1: a tool's output that is not UTF-8 has its bad bytes replaced (the script stopped); kernel module folders whose names
//! are not UTF-8 have their bad bytes replaced; JSON with NaN, Infinity or a lone surrogate counts as not JSON.

use crate::py::{self, Crash, Value, R};
use crate::pycompat::decimal;
use std::os::unix::fs::MetadataExt;
use std::time::Duration;

pub const DOC: &str = include_str!("doc/labyrinth-selfcheck.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

impl Status {
    fn word(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Fail => "FAIL",
        }
    }
}

/// subprocess.run(cmd, capture_output=True, text=True, timeout=10): (exit code, standard output); (127, "") when it cannot start
/// or runs past 10 s.
fn run(cmd: &[&str]) -> (i32, String) {
    let o = py::Run { inherit_stdin: true, timeout: Some(Duration::from_secs(10)), ..Default::default() };
    match py::run_opts(cmd, o) {
        Ok(out) => (out.code, out.stdout),
        Err(_) => (127, String::new()),
    }
}

/// One field of time.strptime's "%m", "%d", "%H", "%M" or "%S" (two characters here): its value if strptime's own pattern takes
/// both characters (`first` the characters allowed first, `second` the ones allowed after each first; None: any digit), else None.
fn two(field: &str, allowed: &[(char, Option<&str>)]) -> Option<u32> {
    let mut cs = field.chars();
    let (a, b) = (cs.next()?, cs.next()?);
    for (first, second) in allowed {
        if a != *first {
            continue;
        }
        let ok = match second {
            Some(set) => set.contains(b),
            None => decimal(b).is_some(),
        };
        if ok {
            return Some(decimal(a)? * 10 + decimal(b)?);
        }
    }
    None
}

fn days_in(year: i64, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// time.mktime(time.strptime(stamp, "%Y-%m-%dT%H:%M:%S")) for the stamp pacman's log has (the pattern already took 4-2-2T2:2:2
/// digits): a date strptime refuses stops the program, as it stopped the script (a ValueError).
fn stamp_epoch(stamp: &str) -> R<f64> {
    let bad = || Crash(format!("ValueError: time data {} does not match format '%Y-%m-%dT%H:%M:%S'", py::repr(stamp)));
    let parts: Vec<&str> = stamp.split(['-', 'T', ':']).collect();
    let year = parts[0].chars().try_fold(0i64, |v, c| decimal(c).map(|d| v * 10 + d as i64)).ok_or_else(bad)?;
    let month = two(parts[1], &[('1', Some("012")), ('0', Some("123456789"))]).ok_or_else(bad)?;
    let day = two(parts[2], &[('3', Some("01")), ('1', None), ('2', None), ('0', Some("123456789"))]).ok_or_else(bad)?;
    let hour = two(parts[3], &[('2', Some("0123")), ('0', None), ('1', None)]).ok_or_else(bad)?;
    let min = two(parts[4], &[('0', None), ('1', None), ('2', None), ('3', None), ('4', None), ('5', None)]).ok_or_else(bad)?;
    let sec = two(parts[5], &[('6', Some("01")), ('0', None), ('1', None), ('2', None), ('3', None), ('4', None), ('5', None)])
        .ok_or_else(bad)?;
    if year == 0 {
        return Err(Crash("ValueError: year 0 is out of range".into()));
    }
    if day > days_in(year, month) {
        return Err(Crash("ValueError: day is out of range for month".into()));
    }
    Ok(py::mktime(year as i32, month as i32, day as i32, hour as i32, min as i32, sec as i32))
}

/// The stamp of a pacman log line that the script's pattern
/// `^\[(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)([+-]\d{4})\] \[ALPM\] (upgraded|installed|reinstalled) PACKAGE ` takes (\d: a decimal digit of any
/// script, as in Python), written out so the program carries no regular-expression engine.
fn log_stamp<'a>(line: &'a str, package: &str) -> Option<&'a str> {
    // one character at `at` that `want` takes, and the position after it
    fn take(line: &str, at: &mut usize, want: impl Fn(char) -> bool) -> Option<()> {
        let c = line[*at..].chars().next().filter(|&c| want(c))?;
        *at += c.len_utf8();
        Some(())
    }
    let digit = |c: char| decimal(c).is_some();
    let mut at = 0;
    take(line, &mut at, |c| c == '[')?;
    for part in ["dddd-", "dd-", "ddT", "dd:", "dd:", "dd"] {
        for p in part.chars() {
            if p == 'd' {
                take(line, &mut at, digit)?;
            } else {
                take(line, &mut at, |c| c == p)?;
            }
        }
    }
    let stamp_end = at;
    take(line, &mut at, |c| c == '+' || c == '-')?;
    for _ in 0..4 {
        take(line, &mut at, digit)?;
    }
    take(line, &mut at, |c| c == ']')?;
    let rest = line[at..].strip_prefix(" [ALPM] ")?;
    let rest = ["upgraded ", "installed ", "reinstalled "].iter().find_map(|w| rest.strip_prefix(w))?;
    rest.strip_prefix(package)?.strip_prefix(' ')?;
    Some(&line[1..stamp_end])
}

/// The script's `^\s*command\s*=\s*"(.*)"\s*$` (multi-line, \s as str.isspace()) found in a text: at a line's start, after
/// blanks (also over lines), `command`, `=` and `"`, then on that line a closing `"` with only blanks after it.
fn has_greeter_command(conf: &str) -> bool {
    let ws = |s: &str| s.len() - s.trim_start_matches(crate::pycompat::isspace).len();
    let mut start = 0;
    loop {
        let mut i = start + ws(&conf[start..]);
        let found = conf[i..].strip_prefix("command").and_then(|r| {
            i = conf.len() - r.len();
            i += ws(&conf[i..]);
            conf[i..].strip_prefix('=')
        });
        if let Some(r) = found {
            let mut j = conf.len() - r.len();
            j += ws(&conf[j..]);
            if let Some(r) = conf[j..].strip_prefix('"') {
                let line = r.split('\n').next().unwrap_or("");
                if crate::pycompat::rstrip(line).ends_with('"') {
                    return true;
                }
            }
        }
        match conf[start..].find('\n') {
            Some(k) => start += k + 1,
            None => return false,
        }
    }
}

/// When pacman last installed or upgraded `package` (epoch seconds), or None.
pub fn last_upgrade_epoch(package: &str, log: &str) -> R<Option<f64>> {
    let text = match py::read_text_lossy(log) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let mut best = None;
    for line in py::lines(&text) {
        if let Some(stamp) = log_stamp(line, package) {
            best = Some(stamp_epoch(stamp)?);
        }
    }
    Ok(best)
}

/// When the first process named `name` started (its /proc folder's time), or None.
fn process_start_epoch(name: &str) -> R<Option<f64>> {
    let (rc, out) = run(&["pgrep", "-x", name]);
    let pids = py::split(&out);
    if rc != 0 || pids.is_empty() {
        return Ok(None);
    }
    if pids[0].contains('\0') {
        return Err(Crash("ValueError: embedded null byte".into()));
    }
    Ok(std::fs::metadata(format!("/proc/{}", pids[0])).ok().map(|m| m.mtime() as f64 + m.mtime_nsec() as f64 * 1e-9))
}

/// The program started before its package was upgraded (more than `slack` seconds before).
pub fn stale_session(started: Option<f64>, upgraded: Option<f64>, slack: f64) -> bool {
    matches!((started, upgraded), (Some(s), Some(u)) if s + slack < u)
}

/// A version's parts for sorting: its runs of digits as numbers, the rest as text (so 7.10 comes after 7.9).
fn version_key(v: &str) -> Vec<(String, u128)> {
    let mut out = vec![];
    let mut text = String::new();
    let mut chars = v.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut n: u128 = c.to_digit(10).unwrap() as u128;
            while let Some(d) = chars.peek().and_then(|d| d.to_digit(10).filter(|_| d.is_ascii_digit())) {
                n = n.saturating_mul(10).saturating_add(d as u128);
                chars.next();
            }
            out.push((std::mem::take(&mut text), n));
        } else {
            text.push(c);
        }
    }
    out.push((text, 0));
    out
}

/// Kernel module folders sorted by version (numbers compared as numbers).
pub fn version_sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort_by(|a, b| version_key(a).cmp(&version_key(b)).then_with(|| a.cmp(b)));
    names
}

/// The installed kernels' module folders, sorted by version.
pub fn kernel_module_dirs(base: &str) -> R<Vec<String>> {
    if !py::isdir(base) {
        return Ok(vec![]);
    }
    let rd = std::fs::read_dir(base).map_err(|e| py::os_err(e, base))?;
    let mut names = vec![];
    for e in rd {
        let e = e.map_err(|e| py::os_err(e, base))?;
        names.push(e.file_name().to_string_lossy().into_owned());
    }
    Ok(version_sorted(names))
}

/// What is wrong with the login screen, or None. `display_manager` is where display-manager.service points (empty when it
/// points nowhere), `greetd_config` the text of /etc/greetd/config.toml.
pub fn login_screen_problem(display_manager: &str, greetd_config: &str) -> Option<String> {
    if display_manager.is_empty() {
        return Some("no login screen is enabled; enable it: sudo systemctl enable greetd".into());
    }
    let base = py::basename(display_manager);
    if base != "greetd.service" {
        return Some(format!("the login screen is {base}, not greetd; switch: sudo systemctl enable -f greetd"));
    }
    if !has_greeter_command(greetd_config) {
        return Some("/etc/greetd/config.toml has no greeter command".into());
    }
    None
}

/// The checks in order: (status, name, detail).
pub fn checks() -> R<Vec<(Status, String, String)>> {
    let (ok, warn, fail) = (Status::Ok, Status::Warn, Status::Fail);
    let mut out: Vec<(Status, String, String)> = vec![];
    let mut add = |s: Status, name: &str, detail: &str| out.push((s, name.to_string(), detail.to_string()));
    let log = "/var/log/pacman.log";

    // the session predates an upgrade of the compositor or of Yggdrasil's own pieces
    let started = process_start_epoch("Hyprland")?;
    let mut stale = vec![];
    for pkg in ["hyprland", "yggdrasil-session", "yggdrasil-lock", "yggdrasil-panel"] {
        if stale_session(started, last_upgrade_epoch(pkg, log)?, 60.0) {
            stale.push(pkg);
        }
    }
    if !stale.is_empty() {
        add(
            warn,
            "session older than its packages",
            &format!("{} upgraded after the session started: log out and in (or reboot) soon.", stale.join(", ")),
        );
    } else if started.is_none() {
        add(warn, "Hyprland", "Hyprland is not running (run this inside the desktop)");
    } else {
        add(ok, "session matches the installed packages", "");
    }

    // kernel
    let rel = uname_release();
    let flavour = rel.rsplit('-').next().unwrap_or("").to_string();
    let mods: Vec<String> = kernel_module_dirs("/usr/lib/modules")?.into_iter().filter(|m| m.ends_with(&flavour)).collect();
    if !mods.is_empty() && !mods.contains(&rel) {
        add(warn, "running kernel is not installed any more", &format!("running {rel}; installed: {}. Reboot.", mods.join(", ")));
    } else if !mods.is_empty() && *mods.last().unwrap() != rel {
        add(warn, "a newer kernel is installed", &format!("running {rel}, installed {}. Reboot when convenient.", mods.last().unwrap()));
    } else {
        add(ok, "kernel", &rel);
    }

    // screens and the panel
    let (_, js) = run(&["hyprctl", "-j", "monitors"]);
    let outs = screens(&js)?;
    if outs.is_empty() {
        let started_true = started.map(|s| s != 0.0).unwrap_or(false);
        add(if started_true { fail } else { warn }, "no screen reported by Hyprland", "hyprctl -j monitors listed none");
    } else {
        add(ok, "screens", &outs.join(", "));
    }
    let (rc, _) = run(&["pgrep", "-x", "yggdrasil-panel"]);
    add(if rc == 0 { ok } else { fail }, "yggdrasil-panel running", if rc == 0 { "" } else { "log out and in to start it again" });

    // the login screen (a broken one blocks every login)
    let dmf = "/etc/systemd/system/display-manager.service";
    let dm = if py::lexists(dmf) { py::realpath(dmf) } else { String::new() };
    let conf = match py::read_text("/etc/greetd/config.toml") {
        Result::Ok(t) => t,
        Err(py::ReadError::Os(_)) => String::new(),
        Err(e) => return Err(Crash(e.message("/etc/greetd/config.toml"))),
    };
    let mut bad = login_screen_problem(&dm, &conf);
    if bad.is_none() && !py::exists("/usr/bin/yggdrasil-greeter") {
        bad = Some("yggdrasil-greeter is not installed (package yggdrasil-lock)".into());
    }
    match &bad {
        Some(b) => add(fail, "login screen", b),
        None => add(ok, "login screen", "greetd"),
    }

    // notification service owner
    let (_, o) = run(&["busctl", "--user", "list", "--no-legend"]);
    let mut owner = vec![];
    for l in py::splitlines(&o) {
        if l.starts_with("org.freedesktop.Notifications ") {
            match py::split(l).get(2) {
                Some(w) => owner.push(w.to_string()),
                None => return Err(Crash("IndexError: list index out of range".into())),
            }
        }
    }
    match owner.first() {
        Some(w) if w == "yggdrasil-panel" => add(ok, "notifications are served by the Labyrinth panel", ""),
        Some(w) => add(
            warn,
            &format!("notifications are served by {w}, not the Labyrinth panel"),
            "stop it (systemctl --user stop mako) and the panel takes over within 10 s",
        ),
        None => add(warn, "nobody serves notifications", "restart the panel"),
    }

    // services
    for unit in ["ollama", "limbo", "heimdall", "sarina-service", "yggdrasil-powerlimit"] {
        let svc = format!("{unit}.service");
        let (_, st) = run(&["systemctl", "is-active", &svc]);
        let st = py::strip(&st);
        if st == "active" {
            add(ok, unit, "");
        } else if run(&["systemctl", "cat", &svc]).0 != 0 {
            add(ok, unit, "not installed here");
        } else {
            add(warn, unit, &format!("is {}", if st.is_empty() { "unknown" } else { st }));
        }
    }
    let (_, failed) = run(&["systemctl", "--failed", "--no-legend", "--plain"]);
    let names: Vec<&str> = py::splitlines(&failed).into_iter().filter(|l| !py::strip(l).is_empty()).map(|l| py::split(l)[0]).collect();
    if names.is_empty() {
        add(ok, "no failed system units", "");
    } else {
        add(warn, "failed units", &names.join(", "));
    }

    // heimdall
    let (_, body) = run(&["curl", "-s", "-m", "3", "http://127.0.0.1:8930/healthz"]);
    let answers = body.contains("ok");
    add(if answers { ok } else { warn }, "Heimdall answers", if answers { "" } else { "no answer on 127.0.0.1:8930" });
    if let Some(fim) = fim_paths("/etc/heimdall/config.json")? {
        let (_, foreign) = run(&["pacman", "-Qmq"]);
        let mut missing = vec![];
        for pkg in py::split(&foreign) {
            if let Some(stem) = pkg.strip_suffix("craft-bin") {
                let exe = format!("/usr/bin/{stem}craft");
                if py::exists(&exe) && !fim.contains(&exe) {
                    missing.push(exe);
                }
            }
        }
        if missing.is_empty() {
            add(ok, "Heimdall watches the AUR apps' binaries", "");
        } else {
            add(warn, "not in Heimdall's file-integrity list", &missing.join(", "));
        }
    }
    Ok(out)
}

fn uname_release() -> String {
    let mut u: libc::utsname = unsafe { std::mem::zeroed() };
    unsafe { libc::uname(&mut u) };
    unsafe { std::ffi::CStr::from_ptr(u.release.as_ptr()) }.to_string_lossy().into_owned()
}

/// The enabled screens hyprctl listed, as "NAME WxH"; output that is not JSON (or not a list) is none, as in the script.
fn screens(js: &str) -> R<Vec<String>> {
    let v = match py::pyjson::loads(js) {
        Result::Ok(v) => v,
        Err(_) => return Ok(vec![]),
    };
    let items: Vec<Value> = match v {
        Value::Array(a) => a,
        Value::Object(m) if m.is_empty() => vec![],
        Value::String(s) if s.is_empty() => vec![],
        Value::Object(_) | Value::String(_) => return Err(Crash("AttributeError: 'str' object has no attribute 'get'".into())),
        _ => return Ok(vec![]), // a number or null: a TypeError, caught
    };
    let mut out = vec![];
    for o in items {
        let m = match o {
            Value::Object(m) => m,
            _ => return Err(Crash("AttributeError: object has no attribute 'get'".into())),
        };
        if m.get("disabled").map(crate::appnet::truthy).unwrap_or(false) {
            continue;
        }
        let s = |k: &str| m.get(k).map(py::pyjson::py_str).unwrap_or_else(|| "None".into());
        out.push(format!("{} {}x{}", s("name"), s("width"), s("height")));
    }
    Ok(out)
}

/// The paths in Heimdall's file-integrity list ("fim_paths"), as a set of text (what else a set could hold never equals a path);
/// None when the file is missing or not JSON.
fn fim_paths(path: &str) -> R<Option<Vec<String>>> {
    let v = match py::load_json(path) {
        Result::Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let m = match v {
        Value::Object(m) => m,
        _ => return Err(Crash("AttributeError: object has no attribute 'get'".into())),
    };
    let items: Vec<Value> = match m.get("fim_paths") {
        None => vec![],
        Some(Value::Array(a)) => a.clone(),
        Some(Value::String(s)) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        Some(Value::Object(o)) => o.keys().map(|k| Value::String(k.clone())).collect(),
        Some(_) => return Err(Crash("TypeError: object is not iterable".into())),
    };
    let mut out = vec![];
    for x in items {
        match x {
            Value::String(s) => out.push(s),
            Value::Array(_) | Value::Object(_) => return Err(Crash("TypeError: unhashable type".into())),
            _ => {}
        }
    }
    Ok(Some(out))
}

/// The exit status: 1 when anything failed, else 2 when anything warned, else 0.
pub fn worst(results: &[(Status, String, String)]) -> i32 {
    if results.iter().any(|r| r.0 == Status::Fail) {
        1
    } else if results.iter().any(|r| r.0 == Status::Warn) {
        2
    } else {
        0
    }
}

pub fn main(argv: &[String]) -> i32 {
    let quiet = argv.iter().skip(1).any(|a| a == "--quiet");
    let results = match checks() {
        Result::Ok(r) => r,
        Err(c) => return py::crashed("labyrinth-selfcheck", &c),
    };
    let mut text = String::new();
    for (status, name, detail) in &results {
        if quiet && *status == Status::Ok {
            continue;
        }
        let detail = if detail.is_empty() { String::new() } else { format!(": {detail}") };
        text.push_str(&format!("{:<5} {name}{detail}\n", status.word()));
    }
    let w = worst(&results);
    if !quiet && w == 0 {
        text.push_str("everything looks fine\n");
    }
    use std::io::Write;
    let _ = std::io::stdout().write_all(text.as_bytes());
    w
}

#[cfg(test)]
mod tests {
    //! tests/test_labyrinth_selfcheck.py, carried over, and the two fixes of 2026-11-11.
    use super::*;
    use crate::testutil::{env_lock, TempDir};

    const LOG: &str = "[2026-10-14T10:00:00-0600] [ALPM] installed hyprland (0.56.1-1)
[2026-10-16T06:32:12-0600] [ALPM] upgraded hyprland (0.56.1-1 -> 0.56.2-1)
[2026-10-16T06:32:12-0600] [ALPM] upgraded yggdrasil-lock (0.1.0-1 -> 0.1.0-2)
[2026-10-16T07:00:00-0600] [ALPM] upgraded hyprland-extra (1 -> 2)
";

    #[test]
    fn last_upgrade_picks_the_latest_and_only_that_package() {
        let _e = env_lock();
        let d = TempDir::new();
        let f = d.p("pacman.log");
        std::fs::write(&f, LOG).unwrap();
        let want = py::mktime(2026, 10, 16, 6, 32, 12);
        assert_eq!(last_upgrade_epoch("hyprland", &f).unwrap(), Some(want)); // not hyprland-extra, not the older install
        assert_eq!(last_upgrade_epoch("nothing", &f).unwrap(), None);
        assert_eq!(last_upgrade_epoch("hyprland", "/nonexistent").unwrap(), None);
    }

    #[test]
    fn a_date_strptime_refuses_stops_as_the_script_did() {
        let d = TempDir::new();
        let f = d.p("pacman.log");
        std::fs::write(&f, "[2026-02-30T10:00:00-0600] [ALPM] installed x (1)\n").unwrap();
        assert!(last_upgrade_epoch("x", &f).is_err());
        std::fs::write(&f, "[2026-13-01T10:00:00-0600] [ALPM] installed x (1)\n").unwrap();
        assert!(last_upgrade_epoch("x", &f).is_err());
        std::fs::write(&f, "[2028-02-29T23:59:61-0600] [ALPM] installed x (1)\n").unwrap();
        assert!(last_upgrade_epoch("x", &f).unwrap().is_some());
    }

    #[test]
    fn stale_session_() {
        assert!(stale_session(Some(1000.0), Some(5000.0), 60.0));
        assert!(!stale_session(Some(5000.0), Some(1000.0), 60.0)); // started after the upgrade
        assert!(!stale_session(Some(4990.0), Some(5000.0), 60.0)); // within the slack
        assert!(!stale_session(None, Some(5000.0), 60.0));
        assert!(!stale_session(Some(1000.0), None, 60.0));
    }

    #[test]
    fn login_screen() {
        let conf = "vt = 7\n[default_session]\ncommand = \"cage -s -- /usr/bin/yggdrasil-greeter\"\nuser = \"greeter\"\n";
        assert_eq!(login_screen_problem("/usr/lib/systemd/system/greetd.service", conf), None);
        assert!(login_screen_problem("", conf).unwrap().contains("enable"));
        assert!(login_screen_problem("/usr/lib/systemd/system/sddm.service", conf).unwrap().contains("sddm.service"));
        assert!(login_screen_problem("/usr/lib/systemd/system/greetd.service", "vt = 7\n").unwrap().contains("no greeter"));
    }

    #[test]
    fn the_patterns_written_out() {
        assert!(has_greeter_command("command\n=\n\"x\"\n"));
        assert!(has_greeter_command("command = \"a\"b\"  \r\n"));
        assert!(!has_greeter_command("command = \"x\" # c\n"));
        assert!(!has_greeter_command("  xcommand = \"x\"\n"));
        assert_eq!(log_stamp("[2026-10-16T06:32:12-0600] [ALPM] upgraded hyprland (1)\n", "hyprland"), Some("2026-10-16T06:32:12"));
        assert_eq!(log_stamp("[2026-10-16T06:32:12-0600] [ALPM] upgraded hyprland-extra (1)\n", "hyprland"), None);
        assert_eq!(log_stamp("[2026-10-16T06:32:12-0600] [ALPM] removed hyprland (1)\n", "hyprland"), None);
    }

    /// Fixed 2026-11-11: kernel versions were sorted as text, so a leftover 7.9 module folder counted as newer than a running 7.10.
    #[test]
    fn kernels_sort_by_version() {
        let v = version_sorted(vec!["7.9.2-1-cachyos".into(), "7.10.0-1-cachyos".into(), "7.10.0-2-cachyos".into()]);
        assert_eq!(v, vec!["7.9.2-1-cachyos", "7.10.0-1-cachyos", "7.10.0-2-cachyos"]);
    }

    /// Fixed 2026-11-11: a warning before a failure left the exit status at 2 (a warning); a failure is 1 wherever it comes.
    #[test]
    fn a_failure_after_a_warning_is_still_a_failure() {
        let r = |s| (s, String::new(), String::new());
        assert_eq!(worst(&[r(Status::Warn), r(Status::Fail)]), 1);
        assert_eq!(worst(&[r(Status::Fail), r(Status::Warn)]), 1);
        assert_eq!(worst(&[r(Status::Ok), r(Status::Warn)]), 2);
        assert_eq!(worst(&[r(Status::Ok)]), 0);
    }
}
