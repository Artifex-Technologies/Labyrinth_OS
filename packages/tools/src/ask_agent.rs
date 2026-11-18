//! labyrinth-appnet-ask-agent: shows the questions of labyrinth-appnet-ask as notifications (a user service started at login):
//! "<App> wants to connect to <name or address>" with Allow / Allow all / Deny, and sends the answer back. The Python script of
//! the same name with the same socket protocol, notification and environment (LABYRINTH_ASK_SOCKET, LABYRINTH_NOTIFY,
//! LABYRINTH_ASK_TIMEOUT); its help text is src/doc/labyrinth-appnet-ask-agent.txt.
//!
//! Not 1:1: the host name of an address is asked of the C library with getnameinfo() (the script used gethostbyaddr(); both read
//! /etc/hosts and the resolver, and give the same name); a line from the service that is not UTF-8 is skipped (it stopped the
//! script with a traceback). Since 2026-11-15 (the privacy pack, docs/PRIVACY_PACK.md): the app is named as its desktop entry
//! names it (the script made a title from the id; that is still the fallback), the body says which program really runs, and a
//! name or host name that imitates another (lookalike.rs: mixed alphabets, look-alike letters, hidden or text-turning
//! characters) gets a warning title and the reasons, unless "lookalike_warnings" is false in ~/.config/heimdall/privacy.json.
//! Without a desktop entry for the app and with ordinary names, the notification is the script's.

use crate::py::{self, Crash, Map, Value, R};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub const DOC: &str = include_str!("doc/labyrinth-appnet-ask-agent.txt");
const ACTIONS: &[(&str, &str)] = &[("allow", "Allow"), ("all", "Allow all"), ("deny", "Deny")];

/// A readable name for an app id ("com.mitchellh.ghostty" -> "Ghostty").
pub fn app_title(app_id: &str) -> String {
    let last = if app_id.matches('.').count() >= 2 { app_id.rsplit('.').next().unwrap_or("") } else { app_id };
    let t = py::title(py::strip(&last.replace(['-', '_'], " ")));
    if t.is_empty() {
        app_id.to_string()
    } else {
        t
    }
}

/// What the notification says about an app: the name people see (its desktop entry's Name, else `app_title` of the id) and
/// the program that really runs ("" when there is no desktop entry).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppFacts {
    pub name: String,
    pub program: String,
}

/// The app's desktop entry, looked up as labyrinth-launch finds it.
pub fn app_facts(app_id: &str) -> AppFacts {
    let mut f = AppFacts { name: app_title(app_id), program: String::new() };
    if let Some(e) = crate::launch::find_desktop(app_id).and_then(|p| crate::launch::parse_desktop(&p).ok()) {
        let n = py::strip(crate::launch::get(&e, "Name"));
        if !n.is_empty() {
            f.name = n.chars().take(200).collect();
        }
        f.program = program_of(crate::launch::get(&e, "Exec"), crate::launch::get(&e, "X-Flatpak"));
    }
    f
}

/// The program an Exec line runs: its first word (after `env` and its settings) found on PATH, with links followed; a
/// Flatpak app by its id.
pub fn program_of(exec: &str, flatpak: &str) -> String {
    if !py::strip(flatpak).is_empty() {
        return format!("Flatpak app {}", py::strip(flatpak));
    }
    let words = py::shlex_split(exec).unwrap_or_default();
    let mut rest = words.iter().map(String::as_str);
    let mut first = rest.next();
    if first.is_some_and(|w| py::basename(w) == "env") {
        first = rest.by_ref().find(|w| !w.contains('=') && !w.starts_with('-'));
    }
    let Some(w) = first else { return String::new() };
    if py::basename(w) == "flatpak" {
        if let Some(id) = rest.skip_while(|x| *x != "run").skip(1).find(|x| !x.starts_with('-')) {
            return format!("Flatpak app {id}");
        }
    }
    let found = if w.contains('/') { Some(w.to_string()) } else { py::which(w) };
    match found {
        Some(p) => py::realpath(&p),
        None => w.to_string(),
    }
}

/// Whether look-alike warnings are on (~/.config/heimdall/privacy.json "lookalike_warnings"; anything but false is on).
pub fn lookalike_warnings_on() -> bool {
    let base = py::env("XDG_CONFIG_HOME").filter(|s| !s.is_empty()).unwrap_or_else(|| py::join(&py::home(), ".config"));
    match py::load_json(&py::join_all(&[&base, "heimdall", "privacy.json"])) {
        Ok(Value::Object(m)) => m.get("lookalike_warnings").and_then(Value::as_bool).unwrap_or(true),
        _ => true,
    }
}

/// Text for a notification: notification servers read <, > and & as markup, so they are written as entities.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The host name of an address, if the resolver answers within 1.5 s; "" otherwise.
pub fn reverse_name(ip: &str) -> String {
    let ip = ip.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(lookup(&ip));
    });
    rx.recv_timeout(Duration::from_millis(1500)).unwrap_or_default()
}

fn lookup(ip: &str) -> String {
    let (addr, scope) = match ip.split_once('%') {
        Some((a, s)) => (a, Some(s)),
        None => (ip, None),
    };
    let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let len;
    if let Ok(v4) = addr.parse::<std::net::Ipv4Addr>() {
        let sa = unsafe { &mut *(&mut storage as *mut _ as *mut libc::sockaddr_in) };
        sa.sin_family = libc::AF_INET as libc::sa_family_t;
        sa.sin_addr.s_addr = u32::from_ne_bytes(v4.octets());
        len = std::mem::size_of::<libc::sockaddr_in>();
    } else if let Ok(v6) = addr.parse::<std::net::Ipv6Addr>() {
        let sa = unsafe { &mut *(&mut storage as *mut _ as *mut libc::sockaddr_in6) };
        sa.sin6_family = libc::AF_INET6 as libc::sa_family_t;
        sa.sin6_addr.s6_addr = v6.octets();
        if let Some(s) = scope {
            let c = std::ffi::CString::new(s).unwrap_or_default();
            sa.sin6_scope_id = unsafe { libc::if_nametoindex(c.as_ptr()) };
        }
        len = std::mem::size_of::<libc::sockaddr_in6>();
    } else {
        return String::new();
    }
    let mut host = vec![0 as libc::c_char; 1025];
    let rc = unsafe {
        libc::getnameinfo(
            &storage as *const _ as *const libc::sockaddr,
            len as libc::socklen_t,
            host.as_mut_ptr(),
            host.len() as libc::socklen_t,
            std::ptr::null_mut(),
            0,
            libc::NI_NAMEREQD,
        )
    };
    if rc != 0 {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(host.as_ptr()) }.to_string_lossy().into_owned()
}

/// The question's fields the agent uses.
#[derive(Debug, Clone)]
pub struct Q {
    pub id: Value,
    pub app: String,
    pub ip: String,
    pub port: i128,
    pub proto: String,
}

fn text(v: Option<&Value>, key: &str) -> R<String> {
    match v {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Ok(py::pyjson::py_str(other)),
        None => Err(Crash(format!("KeyError: {}", py::repr(key)))),
    }
}

impl Q {
    pub fn from(m: &Map<String, Value>) -> R<Q> {
        let port = match m.get("port") {
            Some(Value::Number(n)) => n.as_i64().map(|x| x as i128).unwrap_or_else(|| n.as_f64().unwrap_or(0.0).trunc() as i128),
            Some(Value::Bool(b)) => *b as i128,
            _ => 0,
        };
        Ok(Q {
            id: m.get("id").cloned().unwrap_or(Value::Null),
            app: text(m.get("app"), "app")?,
            ip: text(m.get("ip"), "ip")?,
            port,
            proto: text(m.get("proto"), "proto")?,
        })
    }
}

/// The notification's title and text. `name` is the address's host name ("" when it has none); `warn`: look-alike warnings
/// are on.
pub fn message(q: &Q, name: &str, app: &AppFacts, warn: bool) -> (String, String) {
    use crate::lookalike::{check, visible};
    let where_ = if name.is_empty() { q.ip.clone() } else { name.to_string() };
    let port = if q.port != 0 { format!(" port {}", q.port) } else { String::new() };
    let detail = if name.is_empty() { format!("{}{port}", q.proto) } else { format!("{}{port} ({})", q.ip, q.proto) };
    let prog_name = py::basename(&app.program).to_string();
    let (a, h, p) = (check(&app.name), check(name), check(&prog_name));
    if warn && (a.suspicious() || h.suspicious() || p.suspicious()) {
        let what = if a.suspicious() || p.suspicious() { "Look-alike name" } else { "Look-alike address" };
        let shown_where = if name.is_empty() { q.ip.clone() } else { esc(&visible(name)) };
        let mut lines = vec![];
        for (r, whose) in [(&a, "the app's name"), (&p, "the program's name"), (&h, "the address's name")] {
            if r.suspicious() {
                lines.push(format!("Careful: {whose} may imitate another. {}", esc(&r.words())));
            }
        }
        let prog = if app.program.is_empty() { format!("not known (app id {})", q.app) } else { esc(&visible(&app.program)) };
        lines.push(format!("Real program: {prog}"));
        lines.push(detail);
        return (format!("{what}: {} wants to connect to {shown_where}", esc(&visible(&app.name))), lines.join("\n"));
    }
    let body = if app.program.is_empty() { detail } else { format!("{detail}\nProgram: {}", app.program) };
    (format!("{} wants to connect to {where_}", app.name), body)
}

/// Show the notification; the chosen key ("allow", "all", "deny"), or "" when it was closed or timed out.
pub fn ask_user(q: &Q, name: &str, app: &AppFacts, warn: bool, notify: &str, timeout: &str) -> String {
    let (title, body) = message(q, name, app, warn);
    let mut cmd: Vec<String> = ["timeout", timeout, notify, "-a", "Firewall", "-u", "normal", "-t", "0", "-i", "network-error"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for (k, label) in ACTIONS {
        cmd.push("-A".into());
        cmd.push(format!("{k}={label}"));
    }
    cmd.push(title);
    cmd.push(body);
    match py::run_opts(&cmd, py::Run { inherit_stdin: true, ..Default::default() }) {
        Ok(p) => {
            let a = py::strip(&p.stdout);
            if ACTIONS.iter().any(|(k, _)| *k == a) {
                a.to_string()
            } else {
                String::new()
            }
        }
        Err(_) => String::new(),
    }
}

/// Send the answer to the service (nothing happens when it cannot be reached).
pub fn answer(sock: &str, q: &Q, name: &str, key: &str) {
    let verdict = match key {
        "allow" => "allow",
        "all" => "allow-all",
        "deny" => "deny",
        _ => "ignore",
    };
    let mut m = Map::new();
    m.insert("op".into(), Value::from("decide"));
    m.insert("id".into(), q.id.clone());
    m.insert("verdict".into(), Value::from(verdict));
    m.insert("name".into(), Value::from(name));
    if let Ok(mut s) = UnixStream::connect(sock) {
        if s.write_all(format!("{}\n", py::pyjson::dumps(&Value::Object(m))).as_bytes()).is_ok() {
            let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
        }
    }
}

/// One question: its host name, the notification, the answer.
pub fn handle(sock: &str, notify: &str, timeout: &str, q: &Q) {
    let name = reverse_name(&q.ip);
    let key = ask_user(q, &name, &app_facts(&q.app), lookalike_warnings_on(), notify, timeout);
    answer(sock, q, &name, &key);
}

fn run() -> R<i32> {
    let sock = py::env_get("LABYRINTH_ASK_SOCKET", "/run/labyrinth/appnet-ask.sock");
    let notify = py::env_get("LABYRINTH_NOTIFY", "notify-send");
    let t = py::env_get("LABYRINTH_ASK_TIMEOUT", "300");
    let timeout = crate::pycompat::int(&t)
        .ok_or_else(|| Crash(format!("ValueError: invalid literal for int() with base 10: {}", py::repr(&t))))?
        .to_string();
    loop {
        let mut seen: Vec<Value> = vec![]; // ids start from 1 again when the service restarts
        if let Ok(mut s) = UnixStream::connect(&sock) {
            if s.write_all(b"{\"op\":\"subscribe\"}\n").is_ok() {
                let mut r = BufReader::new(s);
                let mut buf = vec![];
                loop {
                    buf.clear();
                    match r.read_until(b'\n', &mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                    let q: Value = match std::str::from_utf8(&buf).ok().and_then(|t| serde_json::from_str(t).ok()) {
                        Some(v) => v,
                        None => continue,
                    };
                    let m = match q {
                        Value::Object(m) => m,
                        other => return Err(Crash(format!("AttributeError: {} has no attribute 'get'", py::pyjson::py_repr(&other)))),
                    };
                    if m.get("type") == Some(&Value::from("ask")) {
                        let id = m.get("id").cloned().ok_or_else(|| Crash("KeyError: 'id'".into()))?;
                        if matches!(id, Value::Array(_) | Value::Object(_)) {
                            return Err(Crash("TypeError: unhashable type".into()));
                        }
                        if !seen.contains(&id) {
                            seen.push(id);
                            let (sock, notify, timeout) = (sock.clone(), notify.clone(), timeout.clone());
                            std::thread::spawn(move || match Q::from(&m) {
                                Ok(q) => handle(&sock, &notify, &timeout, &q),
                                Err(e) => eprintln!("labyrinth-appnet-ask-agent: a question could not be shown: {}", e.0),
                            });
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

/// The program: argv[0] is its name.
pub fn main(argv: &[String]) -> i32 {
    if argv.len() > 1 {
        println!("{DOC}");
        return 0;
    }
    match run() {
        Ok(rc) => rc,
        Err(e) => py::crashed("labyrinth-appnet-ask-agent", &e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_message() {
        // tests/test_labyrinth_appnet_ask.py: AgentTests
        assert_eq!(app_title("com.mitchellh.ghostty"), "Ghostty");
        assert_eq!(app_title("spotify-launcher"), "Spotify Launcher");
        let q = Q { id: Value::from(1), app: "pdfcraft".into(), ip: "1.2.3.4".into(), port: 443, proto: "TCP".into() };
        let f = AppFacts { name: app_title("pdfcraft"), program: String::new() };
        assert_eq!(
            message(&q, "example.com", &f, true),
            ("Pdfcraft wants to connect to example.com".to_string(), "1.2.3.4 port 443 (TCP)".to_string())
        );
        assert_eq!(message(&q, "", &f, true), ("Pdfcraft wants to connect to 1.2.3.4".to_string(), "TCP port 443".to_string()));
    }

    fn q() -> Q {
        Q { id: Value::from(1), app: "vivaldi-evil".into(), ip: "192.0.2.1".into(), port: 443, proto: "TCP".into() }
    }

    #[test]
    fn the_desktop_name_and_the_real_program_are_shown() {
        let f = AppFacts { name: "Vivaldi".into(), program: "/opt/vivaldi/vivaldi".into() };
        let (t, b) = message(&q(), "example.com", &f, true);
        assert_eq!(t, "Vivaldi wants to connect to example.com");
        assert_eq!(b, "192.0.2.1 port 443 (TCP)\nProgram: /opt/vivaldi/vivaldi");
    }

    #[test]
    fn a_look_alike_app_name_gets_a_warning_and_the_real_program() {
        let f = AppFacts { name: "Viv\u{0430}ldi".into(), program: "/home/u/.local/bin/v".into() };
        let (t, b) = message(&q(), "example.com", &f, true);
        assert_eq!(t, "Look-alike name: Viv\u{0430}ldi wants to connect to example.com");
        assert!(b.starts_with("Careful: the app's name may imitate another. It mixes Latin and Cyrillic letters"), "{b}");
        assert!(b.contains("\"Vivaldi\""));
        assert!(b.contains("Real program: /home/u/.local/bin/v\n192.0.2.1 port 443 (TCP)"));
        // switched off: the plain notification
        assert_eq!(message(&q(), "example.com", &f, false).0, "Viv\u{0430}ldi wants to connect to example.com");
    }

    #[test]
    fn a_name_that_turns_the_text_is_shown_with_the_character_written_out() {
        let f = AppFacts { name: "Pay\u{202E}lapyaP".into(), program: String::new() };
        let (t, b) = message(&q(), "", &f, true);
        assert_eq!(t, "Look-alike name: Pay&lt;U+202E&gt;lapyaP wants to connect to 192.0.2.1");
        assert!(b.contains("Real program: not known (app id vivaldi-evil)"));
        assert!(!t.contains('\u{202E}') && !b.contains('\u{202E}'));
    }

    #[test]
    fn a_look_alike_host_name_or_program_is_caught_too() {
        let f = AppFacts { name: "Vivaldi".into(), program: "/usr/bin/vivaldi".into() };
        let (t, b) = message(&q(), "p\u{0430}ypal.com", &f, true);
        assert_eq!(t, "Look-alike address: Vivaldi wants to connect to p\u{0430}ypal.com");
        assert!(b.contains("the address's name may imitate another"));
        let f = AppFacts { name: "Vivaldi".into(), program: "/usr/bin/viv\u{0430}ldi".into() };
        assert!(message(&q(), "", &f, true).0.starts_with("Look-alike name: "));
    }

    #[test]
    fn programs_of_exec_lines() {
        assert_eq!(program_of("sh -c true", ""), py::realpath(&py::which("sh").unwrap()));
        assert_eq!(program_of("env FOO=1 BAR=2 sh %U", ""), py::realpath(&py::which("sh").unwrap()));
        assert_eq!(
            program_of("/usr/bin/flatpak run --branch=stable --command=vlc org.videolan.VLC %U", ""),
            "Flatpak app org.videolan.VLC"
        );
        assert_eq!(program_of("anything", "org.gnome.Loupe"), "Flatpak app org.gnome.Loupe");
        assert_eq!(program_of("no-such-program-here %f", ""), "no-such-program-here");
        assert_eq!(program_of("", ""), "");
    }

    #[test]
    fn the_pref_file_switches_warnings() {
        let _g = crate::testutil::env_lock();
        let d = crate::testutil::TempDir::new();
        let old = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", d.p(""));
        assert!(lookalike_warnings_on(), "no file: on");
        std::fs::create_dir_all(d.p("heimdall")).unwrap();
        std::fs::write(d.p("heimdall/privacy.json"), r#"{"lookalike_warnings": false}"#).unwrap();
        assert!(!lookalike_warnings_on());
        std::fs::write(d.p("heimdall/privacy.json"), r#"{"lookalike_warnings": "no"}"#).unwrap();
        assert!(lookalike_warnings_on(), "not a boolean: on");
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    #[test]
    fn titles_of_odd_ids() {
        assert_eq!(app_title("a.b"), "A.B");
        assert_eq!(app_title("x.y.__"), "x.y.__");
        assert_eq!(app_title("org.gnome.text_editor-2"), "Text Editor 2");
    }

    #[test]
    fn loopback_has_a_name() {
        assert_eq!(reverse_name("127.0.0.1"), "localhost");
    }
}
