//! labyrinth-appnet-ask-agent: shows the questions of labyrinth-appnet-ask as notifications (a user service started at login):
//! "<App> wants to connect to <name or address>" with Allow / Allow all / Deny, and sends the answer back. The Python script of
//! the same name with the same socket protocol, notification and environment (LABYRINTH_ASK_SOCKET, LABYRINTH_NOTIFY,
//! LABYRINTH_ASK_TIMEOUT); its help text is src/doc/labyrinth-appnet-ask-agent.txt.
//!
//! Not 1:1: the host name of an address is asked of the C library with getnameinfo() (the script used gethostbyaddr(); both read
//! /etc/hosts and the resolver, and give the same name); a line from the service that is not UTF-8 is skipped (it stopped the
//! script with a traceback).

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
        libc::getnameinfo(&storage as *const _ as *const libc::sockaddr, len as libc::socklen_t, host.as_mut_ptr(), host.len() as libc::socklen_t, std::ptr::null_mut(), 0, libc::NI_NAMEREQD)
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
        Ok(Q { id: m.get("id").cloned().unwrap_or(Value::Null), app: text(m.get("app"), "app")?, ip: text(m.get("ip"), "ip")?, port, proto: text(m.get("proto"), "proto")? })
    }
}

/// The notification's title and text.
pub fn message(q: &Q, name: &str) -> (String, String) {
    let where_ = if name.is_empty() { q.ip.clone() } else { name.to_string() };
    let port = if q.port != 0 { format!(" port {}", q.port) } else { String::new() };
    let detail = if name.is_empty() { format!("{}{port}", q.proto) } else { format!("{}{port} ({})", q.ip, q.proto) };
    (format!("{} wants to connect to {where_}", app_title(&q.app)), detail)
}

/// Show the notification; the chosen key ("allow", "all", "deny"), or "" when it was closed or timed out.
pub fn ask_user(q: &Q, name: &str, notify: &str, timeout: &str) -> String {
    let (title, body) = message(q, name);
    let mut cmd: Vec<String> = ["timeout", timeout, notify, "-a", "Firewall", "-u", "normal", "-t", "0", "-i", "network-error"].iter().map(|s| s.to_string()).collect();
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
    let key = ask_user(q, &name, notify, timeout);
    answer(sock, q, &name, &key);
}

fn run() -> R<i32> {
    let sock = py::env_get("LABYRINTH_ASK_SOCKET", "/run/labyrinth/appnet-ask.sock");
    let notify = py::env_get("LABYRINTH_NOTIFY", "notify-send");
    let t = py::env_get("LABYRINTH_ASK_TIMEOUT", "300");
    let timeout = crate::pycompat::int(&t).ok_or_else(|| Crash(format!("ValueError: invalid literal for int() with base 10: {}", py::repr(&t))))?.to_string();
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
        assert_eq!(message(&q, "example.com"), ("Pdfcraft wants to connect to example.com".to_string(), "1.2.3.4 port 443 (TCP)".to_string()));
        assert_eq!(message(&q, ""), ("Pdfcraft wants to connect to 1.2.3.4".to_string(), "TCP port 443".to_string()));
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
