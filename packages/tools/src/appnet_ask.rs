//! labyrinth-appnet-ask: the "ask" half of Settings > Apps > Network (a root service). It reads the kernel log lines of the ask
//! rules (labyrinth-appnet), turns each new destination of an app into one question for that person's session
//! (labyrinth-appnet-ask-agent) and applies the answer. The Python script of the same name with the same socket protocol, files,
//! nft calls and environment; its help text is src/doc/labyrinth-appnet-ask.txt.
//!
//! /run/labyrinth/appnet-ask.sock, JSON lines; the kernel says who is on the other end (SO_PEERCRED):
//!   {"op":"subscribe"}  {"op":"decide","id":N,"verdict":"allow"|"deny"|"allow-all"|"ignore","name":".."}
//!   {"op":"list"}       {"op":"forget","app":..,"ip":..}
//!
//! Not 1:1: the script served everything from one thread (asyncio); here each connection has a thread and one lock keeps the
//! questions and answers in order as the script did. A subscriber that stops reading for 5 s is dropped (the script would have
//! stopped asking everyone until it read again). Where a connection's handler stopped on an error, the connection is closed as
//! the script closed it; the message on standard error is one line (asyncio printed a traceback). A request line in UTF-16 or
//! UTF-32, or with NaN or Infinity in it, is skipped (Python's json read those).

use crate::appnet::{self, Ask, AskEntry};
use crate::py::{self, Crash, Map, Value, R};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DOC: &str = include_str!("doc/labyrinth-appnet-ask.txt");

/// A dropped connection the kernel logged: (uid, app, ip, port, proto).
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub uid: String,
    pub app: String,
    pub ip: String,
    pub port: i128,
    pub proto: String,
}

pub fn line_regex() -> regex::Regex {
    let p = r"labask:(?P<app>[A-Za-z0-9._+-]+) .*?DST=(?P<dst>[0-9A-Fa-f:.]+) .*?PROTO=(?P<proto>\w+)(?: .*?DPT=(?P<dpt>\d+))?.* UID=(?P<uid>\d+)";
    regex::Regex::new(&crate::pyre::translate(p)).expect("the pattern")
}

/// One kernel log line from the ask rule, or None for anything else.
pub fn parse_log(re: &regex::Regex, msg: &str) -> Option<Event> {
    let m = re.captures(msg)?;
    let port = match m.name("dpt") {
        Some(d) => crate::pycompat::int(d.as_str()).unwrap_or(0),
        None => 0,
    };
    Some(Event { uid: m["uid"].to_string(), app: m["app"].to_string(), ip: m["dst"].to_string(), port, proto: m["proto"].to_string() })
}

/// A question shown to a person.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub id: u64,
    pub uid: String,
    pub app: String,
    pub ip: String,
    pub port: i128,
    pub proto: String,
}

impl Question {
    /// The question as it goes over the socket (without the uid).
    pub fn json(&self) -> String {
        let mut m = Map::new();
        m.insert("type".into(), Value::from("ask"));
        m.insert("id".into(), Value::from(self.id));
        m.insert("app".into(), Value::from(self.app.clone()));
        m.insert("ip".into(), Value::from(self.ip.clone()));
        m.insert("port".into(), Value::from(self.port as i64));
        m.insert("proto".into(), Value::from(self.proto.clone()));
        py::pyjson::dumps(&Value::Object(m))
    }
}

/// How a request could not be answered: an OSError (the script's handler caught it and closed the connection) or another
/// error (its handler stopped; asyncio closed the connection and wrote the error down).
#[derive(Debug)]
pub enum Stop {
    Os(String),
    Other(String),
}

fn os(c: Crash) -> Stop {
    Stop::Os(c.0)
}

fn other(c: Crash) -> Stop {
    Stop::Other(c.0)
}

/// Runs an nft command (a test gives its own).
pub type Nft = Box<dyn FnMut(&[String]) -> Result<(), Stop> + Send>;
/// Loads the rules (labyrinth-appnet apply; a test gives its own).
pub type Apply = Box<dyn FnMut(&appnet::Rules) -> Result<(), Stop> + Send>;

/// The questions and answers. No I/O but through `nft`, `apply` and the two state files.
pub struct Engine {
    pub nft: Nft,
    pub apply: Apply,
    pub now: Box<dyn Fn() -> f64 + Send>,
    pub cooldown_secs: f64,
    pending: Vec<Question>,
    by_key: HashMap<(String, String, String), u64>,
    cooldown: HashMap<(String, String, String), f64>,
    next_id: u64,
}

fn mode_of(rules: &appnet::Rules, uid: &str, app: &str) -> Option<String> {
    rules.iter().find(|(u, _)| u == uid).and_then(|(_, a)| a.iter().find(|(x, _)| x == app)).map(|(_, m)| m.clone())
}

fn entry_of<'a>(ask: &'a Ask, uid: &str, app: &str) -> Option<&'a AskEntry> {
    ask.iter().find(|(u, _)| u == uid).and_then(|(_, a)| a.iter().find(|(x, _)| x == app)).map(|(_, e)| e)
}

fn entry_mut<'a>(ask: &'a mut Ask, uid: &str, app: &str) -> &'a mut AskEntry {
    let ui = match ask.iter().position(|(u, _)| u == uid) {
        Some(i) => i,
        None => {
            ask.push((uid.to_string(), vec![]));
            ask.len() - 1
        }
    };
    let apps = &mut ask[ui].1;
    let ai = match apps.iter().position(|(a, _)| a == app) {
        Some(i) => i,
        None => {
            apps.push((app.to_string(), AskEntry::default()));
            apps.len() - 1
        }
    };
    &mut apps[ai].1
}

fn pop(v: &mut Vec<(String, String)>, key: &str) -> bool {
    match v.iter().position(|(k, _)| k == key) {
        Some(i) => {
            v.remove(i);
            true
        }
        None => false,
    }
}

fn set(v: &mut Vec<(String, String)>, key: &str, val: &str) {
    match v.iter_mut().find(|(k, _)| k == key) {
        Some(e) => e.1 = val.to_string(),
        None => v.push((key.to_string(), val.to_string())),
    }
}

/// The question id a JSON value names, as a Python dict lookup reads it (1, 1.0 and true are all the key 1). Err: a value
/// that cannot be a key (a list or an object: the script's handler stopped).
fn qid_of(v: &Value) -> Result<Option<u64>, Stop> {
    match v {
        Value::Bool(b) => Ok(Some(*b as u64)),
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok(Some(u))
            } else if let Some(f) = n.as_f64() {
                Ok(if f.fract() == 0.0 && (0.0..1.8e19).contains(&f) { Some(f as u64) } else { None })
            } else {
                Ok(None)
            }
        }
        Value::Array(_) | Value::Object(_) => Err(Stop::Other("TypeError: unhashable type".into())),
        _ => Ok(None),
    }
}

impl Engine {
    pub fn new(nft: Nft, apply: Apply, now: Box<dyn Fn() -> f64 + Send>, cooldown_secs: f64) -> Engine {
        Engine { nft, apply, now, cooldown_secs, pending: vec![], by_key: HashMap::new(), cooldown: HashMap::new(), next_id: 1 }
    }

    /// A dropped connection: the new question to show, or None (decided, pending, cooling down, not in ask mode).
    pub fn asked(&mut self, ev: &Event) -> R<Option<Question>> {
        let key = (ev.uid.clone(), ev.app.clone(), ev.ip.clone());
        if mode_of(&appnet::load_state(None)?, &ev.uid, &ev.app).as_deref() != Some("ask") {
            return Ok(None);
        }
        let ask = appnet::load_ask(None)?;
        if let Some(known) = entry_of(&ask, &ev.uid, &ev.app) {
            if known.allow.iter().any(|(ip, _)| *ip == ev.ip) || known.deny.iter().any(|(ip, _)| *ip == ev.ip) {
                return Ok(None);
            }
        }
        if self.by_key.contains_key(&key) {
            return Ok(None);
        }
        if self.cooldown.get(&key).copied().unwrap_or(0.0) > (self.now)() {
            return Ok(None);
        }
        let q = Question { id: self.next_id, uid: ev.uid.clone(), app: ev.app.clone(), ip: ev.ip.clone(), port: ev.port, proto: ev.proto.clone() };
        self.next_id += 1;
        self.pending.push(q.clone());
        self.by_key.insert(key, q.id);
        Ok(Some(q))
    }

    pub fn questions_for(&self, uid: &str) -> Vec<Question> {
        self.pending.iter().filter(|q| q.uid == uid).cloned().collect()
    }

    /// Apply an answer from person `uid`: (ok, message).
    pub fn decide(&mut self, uid: &str, qid: &Value, verdict: &Value, name: &Value) -> Result<(bool, String), Stop> {
        let q = match qid_of(qid)?.and_then(|id| self.pending.iter().find(|q| q.id == id)) {
            Some(q) if q.uid == uid => q.clone(),
            _ => return Ok((false, "no such question".into())),
        };
        let name: String = match name {
            Value::String(s) => s.chars().filter(|c| c.is_ascii_alphanumeric() || "._-".contains(*c)).take(120).collect(),
            v if !appnet::truthy(v) => String::new(),
            _ => return Err(Stop::Other("TypeError: expected string or bytes-like object".into())),
        };
        let key = (uid.to_string(), q.app.clone(), q.ip.clone());
        self.pending.retain(|x| x.id != q.id);
        self.by_key.remove(&key);
        let verdict = verdict.as_str().unwrap_or("");
        if verdict == "ignore" {
            self.cooldown.insert(key, (self.now)() + self.cooldown_secs);
            return Ok((true, "ok".into()));
        }
        let mut ask = appnet::load_ask(None).map_err(other)?;
        match verdict {
            "allow" => {
                let e = entry_mut(&mut ask, uid, &q.app);
                set(&mut e.allow, &q.ip, &name);
                pop(&mut e.deny, &q.ip);
                appnet::save_ask(&ask, None).map_err(os)?;
                self.add_element(uid, &q.app, &q.ip)?;
            }
            "deny" => {
                let e = entry_mut(&mut ask, uid, &q.app);
                set(&mut e.deny, &q.ip, &name);
                pop(&mut e.allow, &q.ip);
                appnet::save_ask(&ask, None).map_err(os)?;
            }
            "allow-all" => {
                let mut rules = appnet::load_state(None).map_err(other)?;
                if let Some(i) = rules.iter().position(|(u, _)| u == uid) {
                    rules[i].1.retain(|(a, _)| *a != q.app);
                    if rules[i].1.is_empty() {
                        rules.remove(i);
                    }
                }
                appnet::save_state(&rules, None).map_err(os)?;
                (self.apply)(&rules)?;
            }
            _ => return Ok((false, "unknown verdict".into())),
        }
        Ok((true, "ok".into()))
    }

    fn element(&mut self, verb: &str, uid: &str, app: &str, ip: &str) -> Result<(), Stop> {
        let fam = if ip.contains(':') { 6 } else { 4 };
        let n = appnet::uid_int(uid).map_err(other)?;
        let args: Vec<String> = [verb, "element", "inet", appnet::TABLE, &appnet::set_name(n, app, fam), "{", ip, "}"].iter().map(|s| s.to_string()).collect();
        (self.nft)(&args)
    }

    fn add_element(&mut self, uid: &str, app: &str, ip: &str) -> Result<(), Stop> {
        self.element("add", uid, app, ip)
    }

    /// What this person decided, for the apps in ask mode or with decisions.
    pub fn listing(&self, uid: &str) -> Result<Value, Stop> {
        let ask = appnet::load_ask(None).map_err(other)?;
        let rules = appnet::load_state(None).map_err(other)?;
        let mut out = Map::new();
        if let Some((_, apps)) = ask.iter().find(|(u, _)| u == uid) {
            for (app, e) in apps {
                if mode_of(&rules, uid, app).as_deref() == Some("ask") || !e.allow.is_empty() || !e.deny.is_empty() {
                    let one: Ask = vec![(uid.to_string(), vec![(app.clone(), e.clone())])];
                    if let Value::Object(m) = appnet::ask_value(&one) {
                        if let Some(Value::Object(a)) = m.get(uid) {
                            out.insert(app.clone(), a[app].clone());
                        }
                    }
                }
            }
        }
        Ok(Value::Object(out))
    }

    /// The destination is asked about again next time.
    pub fn forget(&mut self, uid: &str, app: &str, ip: &str) -> Result<bool, Stop> {
        let mut ask = appnet::load_ask(None).map_err(other)?;
        if entry_of(&ask, uid, app).is_none() || !appnet::valid_ip(ip) {
            return Ok(false);
        }
        let e = entry_mut(&mut ask, uid, app);
        let mut gone = pop(&mut e.allow, ip);
        gone |= pop(&mut e.deny, ip);
        if gone {
            appnet::save_ask(&ask, None).map_err(os)?;
            self.element("delete", uid, app, ip)?;
        }
        Ok(gone)
    }
}

/// `nft ARGS` with a 10-second limit (LABYRINTH_NFT names another program).
pub fn run_nft(nft: &str, args: &[String]) -> Result<(), Stop> {
    let mut argv = vec![nft.to_string()];
    argv.extend(args.iter().cloned());
    let o = py::Run { inherit_stdin: true, timeout: Some(Duration::from_secs(10)), timeout_text: Some("10".into()), ..Default::default() };
    match py::run_opts(&argv, o) {
        Ok(_) => Ok(()),
        Err(py::RunError::Os(m)) => Err(Stop::Os(m)),
        Err(py::RunError::Timeout(m)) => Err(Stop::Other(format!("TimeoutExpired: {m}"))),
    }
}

/// labyrinth-appnet's apply, from inside the service: nft and logger missing are OSErrors, a broken file is not.
pub fn apply_rules(rules: &appnet::Rules) -> Result<(), Stop> {
    let script = appnet::ruleset_now(rules).map_err(other)?;
    let p = py::run_opts(&["nft", "-f", "-"], py::Run { input: Some(&script), ..Default::default() }).map_err(|e| Stop::Os(e.message().to_string()))?;
    if p.code != 0 {
        let s = py::strip(&p.stderr);
        let msg = if s.is_empty() { "nft failed" } else { s };
        eprintln!("{msg}");
        let first = py::splitlines(msg).first().copied().unwrap_or("").to_string();
        py::call(&["logger", "-t", "labyrinth-appnet", "-p", "auth.warning", "--", &format!("per-app network rules did not load, apps are NOT limited: {first}")]).map_err(os)?;
    }
    Ok(())
}

// ---- the service

struct Shared {
    engine: Engine,
    /// uid -> the connections subscribed (a connection may be there more than once, as in the script)
    subs: HashMap<String, Vec<(u64, UnixStream)>>,
}

fn push(sh: &mut Shared, q: &Question) {
    let line = format!("{}\n", q.json());
    if let Some(list) = sh.subs.get_mut(&q.uid) {
        list.retain_mut(|(_, w)| w.write_all(line.as_bytes()).is_ok());
    }
}

fn reply(w: &mut UnixStream, v: Value) -> std::io::Result<()> {
    w.write_all(format!("{}\n", py::pyjson::dumps(&v)).as_bytes())
}

fn result(ok: bool, message: &str) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::from("result"));
    m.insert("ok".into(), Value::Bool(ok));
    m.insert("message".into(), Value::from(message));
    Value::Object(m)
}

fn peer_uid(s: &UnixStream) -> String {
    use std::os::unix::io::AsRawFd;
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe { libc::getsockopt(s.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, &mut cred as *mut _ as *mut libc::c_void, &mut len) };
    if rc != 0 {
        return String::new();
    }
    (cred.uid as i32).to_string()
}

/// One line of a request, at most 64 KiB (asyncio's limit): Ok(None) at the end.
fn read_line(r: &mut BufReader<UnixStream>) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = vec![];
    let n = r.by_ref().take(65537).read_until(b'\n', &mut line)?;
    if n == 0 {
        return Ok(None);
    }
    if line.len() > 65536 || (!line.ends_with(b"\n") && line.len() == 65537) {
        return Err(std::io::Error::other("Separator is not found, and chunk exceed the limit"));
    }
    Ok(Some(line))
}

fn handle(shared: Arc<Mutex<Shared>>, stream: UnixStream, conn: u64) {
    let uid = peer_uid(&stream);
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let _ = writer.set_write_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(stream);
    let mut problem: Option<String> = None;
    loop {
        let line = match read_line(&mut reader) {
            Ok(Some(l)) => l,
            Ok(None) => break,
            Err(e) => {
                problem = Some(format!("ValueError: {e}"));
                break;
            }
        };
        let req: Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let req = match req {
            Value::Object(m) => m,
            other => {
                problem = Some(format!("AttributeError: {} has no attribute 'get'", py::pyjson::py_repr(&other)));
                break;
            }
        };
        let op = req.get("op").cloned().unwrap_or(Value::Null);
        let null = Value::Null;
        let mut sh = shared.lock().unwrap_or_else(|e| e.into_inner());
        let outcome: Result<(), Stop> = match op.as_str() {
            Some("subscribe") => {
                if let Ok(w) = writer.try_clone() {
                    let _ = w.set_write_timeout(Some(Duration::from_secs(5)));
                    sh.subs.entry(uid.clone()).or_default().push((conn, w));
                }
                for q in sh.engine.questions_for(&uid) {
                    push(&mut sh, &q);
                }
                Ok(())
            }
            Some("decide") => {
                let name = req.get("name").cloned().unwrap_or(Value::from(""));
                match sh.engine.decide(&uid, req.get("id").unwrap_or(&null), req.get("verdict").unwrap_or(&null), &name) {
                    Ok((ok, msg)) => reply(&mut writer, result(ok, &msg)).map_err(|e| Stop::Os(e.to_string())),
                    Err(e) => Err(e),
                }
            }
            Some("list") => match sh.engine.listing(&uid) {
                Ok(apps) => {
                    let mut m = Map::new();
                    m.insert("type".into(), Value::from("list"));
                    m.insert("apps".into(), apps);
                    reply(&mut writer, Value::Object(m)).map_err(|e| Stop::Os(e.to_string()))
                }
                Err(e) => Err(e),
            },
            Some("forget") => {
                let app = py::pyjson::py_str(req.get("app").unwrap_or(&null));
                let ip = py::pyjson::py_str(req.get("ip").unwrap_or(&null));
                match sh.engine.forget(&uid, &app, &ip) {
                    Ok(gone) => reply(&mut writer, result(gone, "")).map_err(|e| Stop::Os(e.to_string())),
                    Err(e) => Err(e),
                }
            }
            _ => Ok(()),
        };
        drop(sh);
        match outcome {
            Ok(()) => {}
            Err(Stop::Os(_)) => break,
            Err(Stop::Other(m)) => {
                problem = Some(m);
                break;
            }
        }
    }
    if let Some(p) = problem {
        eprintln!("labyrinth-appnet-ask: a connection ended on an error (the script's asyncio wrote a traceback): {p}");
    }
    let mut sh = shared.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(list) = sh.subs.get_mut(&uid) {
        if let Some(i) = list.iter().position(|(c, _)| *c == conn) {
            list.remove(i);
        }
    }
    let _ = writer.shutdown(std::net::Shutdown::Both);
}

fn serve() -> R<i32> {
    let socket = py::env_get("LABYRINTH_ASK_SOCKET", "/run/labyrinth/appnet-ask.sock");
    let nft = py::env_get("LABYRINTH_NFT", "nft");
    let log_cmd: Vec<String> = py::split(&py::env_get("LABYRINTH_ASK_LOGCMD", "journalctl -k -f -n 0 -o cat")).into_iter().map(String::from).collect();
    let cooldown_text = py::env_get("LABYRINTH_ASK_COOLDOWN", "60");
    let cooldown = py_float(&cooldown_text).ok_or_else(|| Crash(format!("ValueError: could not convert string to float: {}", py::repr(&cooldown_text))))?;
    let start = Instant::now();
    let engine = Engine::new(
        Box::new(move |args: &[String]| run_nft(&nft, args)),
        Box::new(|rules: &appnet::Rules| apply_rules(rules)),
        Box::new(move || start.elapsed().as_secs_f64()),
        cooldown,
    );
    let shared = Arc::new(Mutex::new(Shared { engine, subs: HashMap::new() }));
    py::makedirs(&py::dirname(&socket))?;
    match std::fs::remove_file(&socket) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(py::os_err(e, &socket)),
    }
    let listener = UnixListener::bind(&socket).map_err(|e| py::os_err(e, &socket))?;
    // asyncio (Python 3.13 and later) removes the socket when the server closes, as it does when the service stops on an error
    let gone = |c: Crash| {
        let _ = std::fs::remove_file(&socket);
        c
    };
    py::chmod(&socket, 0o666).map_err(gone)?;
    if log_cmd.is_empty() {
        return Err(gone(Crash("ValueError: no log command".into())));
    }
    let mut child = Command::new(&log_cmd[0])
        .args(&log_cmd[1..])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| gone(Crash(py::oserror(&e, Some(&log_cmd[0])))))?;
    let sock_path = socket.clone();
    let out = child.stdout.take().unwrap();
    let sh = shared.clone();
    std::thread::spawn(move || {
        let re = line_regex();
        let mut r = BufReader::new(out);
        let mut buf = vec![];
        loop {
            buf.clear();
            match r.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let text = String::from_utf8_lossy(&buf);
            if let Some(ev) = parse_log(&re, &text) {
                let mut s = sh.lock().unwrap_or_else(|e| e.into_inner());
                match s.engine.asked(&ev) {
                    Ok(Some(q)) => push(&mut s, &q),
                    Ok(None) => {}
                    Err(c) => {
                        let _ = std::fs::remove_file(&sock_path);
                        std::process::exit(py::crashed("labyrinth-appnet-ask", &c))
                    }
                }
            }
        }
        let _ = child.wait();
    });
    let mut conn = 0u64;
    for s in listener.incoming() {
        let s = match s {
            Ok(s) => s,
            Err(_) => continue,
        };
        conn += 1;
        let sh = shared.clone();
        let id = conn;
        std::thread::spawn(move || handle(sh, s, id));
    }
    Ok(0)
}

/// float(text) as Python reads it: surrounding whitespace, underscores between digits, inf and nan.
pub fn py_float(text: &str) -> Option<f64> {
    let s = py::strip(text);
    let b: Vec<char> = s.chars().collect();
    for (i, c) in b.iter().enumerate() {
        if *c == '_' && !(i > 0 && i + 1 < b.len() && b[i - 1].is_ascii_digit() && b[i + 1].is_ascii_digit()) {
            return None;
        }
    }
    let s: String = b.iter().filter(|c| **c != '_').map(|c| crate::pycompat::decimal(*c).map(|d| char::from(b'0' + d as u8)).unwrap_or(*c)).collect();
    let l = s.to_ascii_lowercase();
    let body = l.trim_start_matches(['+', '-']);
    if body == "inf" || body == "infinity" || body == "nan" {
        return l.parse::<f64>().ok();
    }
    if !body.chars().all(|c| c.is_ascii_digit() || ".e+-".contains(c)) {
        return None;
    }
    s.parse::<f64>().ok()
}

/// The program: argv[0] is its name.
pub fn main(argv: &[String]) -> i32 {
    if argv.len() > 1 {
        println!("{DOC}");
        return 0;
    }
    match serve() {
        Ok(rc) => rc,
        Err(e) => py::crashed("labyrinth-appnet-ask", &e),
    }
}

#[cfg(test)]
mod tests;
