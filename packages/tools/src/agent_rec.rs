//! labyrinth-agent-rec: the recorder of the engineering agent's root sessions. sudo-rs has no input/output recording (GNU sudo's
//! log_input, log_output, iolog_dir), so this program does it, as root: the agent's one sudo rule is this program
//! (/etc/sudoers.d/50-labyrinth-agent, written by labyrinth-agent-setup), and `claude-sudo CMD...` is
//! `sudo -n -u AGENT sudo -n /usr/bin/labyrinth-agent-rec -- CMD...`. Its help is src/doc/labyrinth-agent-rec.txt.
//!
//! What it does with a command:
//!   - the caller is SUDO_USER (SUDO_UID must be that account's); what the caller may run as root is written in sudoers for a
//!     second account, `{caller}-rules`, that nobody can become. CMD is found on sudo's secure_path and the question is put to
//!     sudo itself, by argv: `sudo -n -l -U {caller}-rules -- /ABS/CMD ARGS...` (exit 0: those rules allow exactly this command
//!     line as root). Anything else is refused, in one line and in the journal, and nothing runs;
//!   - the session's login uid (/proc/self/loginuid) is set to the caller's, so the kernel's audit log attributes the session
//!     and everything it starts to the agent (labyrinth-agent-setup's audit rule matches on it);
//!   - the command runs with no shell, in a session of its own (no controlling terminal), its standard input, output and errors
//!     through pipes: each is copied through to the caller and into the record as it flows, byte for byte;
//!   - the record is one file per session in /var/log/labyrinth-agent (0700, root's; the file 0600, made new): JSON lines, a
//!     header, the data (ms since the start, the stream, base64) and a footer; at most 64 MiB of data is recorded per session
//!     (what comes after still passes through, and the footer says it was cut);
//!   - the journal (tag labyrinth-agent-rec, facility auth) gets one line at the start (the session and the command) and one at
//!     the end (the exit code and the SHA-256 of the finished record), so a record changed afterwards no longer matches;
//!   - SIGINT, SIGTERM and SIGHUP are passed on to the command's process group; the exit code is the command's (128 + N when
//!     signal N ended it).
//!
//! `list` and `replay` read the records; only root may, and not an agent (a caller with a rules account).
//!
//! Known limitation: the command never has a terminal, also when the caller's standard input is one (no pseudo-terminal yet).

use crate::pycompat::json::{json, Value};
use crate::sh;
use sha2::{Digest, Sha256};
use std::ffi::{CString, OsStr, OsString};
use std::fs;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const DOC: &str = include_str!("doc/labyrinth-agent-rec.txt");
/// Where the labyrinth-tools package installs it (the agent's sudo rule names this path).
pub const REC: &str = "/usr/bin/labyrinth-agent-rec";
pub const LOG_DIR: &str = "/var/log/labyrinth-agent";
/// sudo's secure_path (the package's /etc/sudoers), where a command without a slash is looked for.
pub const SECURE_PATH: [&str; 3] = ["/usr/local/sbin", "/usr/local/bin", "/usr/bin"];
/// The most data recorded per session.
pub const CAP: u64 = 64 << 20;
const TAG: &str = "labyrinth-agent-rec";
/// A journal line's command is cut here (as the gate cuts its lines).
const LOG_MAX: usize = 2000;
const STREAMS: [&str; 3] = ["in", "out", "err"];

/// The account that holds an agent's root rules in sudoers: nobody can become it, it is only asked about.
pub fn rules_user(agent: &str) -> String {
    format!("{agent}-rules")
}

fn err(msg: &str, code: i32) -> i32 {
    eprintln!("{TAG}: {msg}");
    code
}

// ---------------------------------------------------------------- where things are (fixed in the program, others in tests)

pub struct Setup {
    pub log_dir: String,
    /// The owner the record folder is given (root's; None leaves it as it is).
    pub dir_owner: Option<(u32, u32)>,
    pub sudo: String,
    pub logger: String,
    pub loginuid: String,
    pub sessionid: String,
    pub path: Vec<String>,
    pub cap: u64,
}

impl Setup {
    pub fn system() -> Setup {
        Setup {
            log_dir: LOG_DIR.into(),
            dir_owner: Some((0, 0)),
            sudo: "/usr/bin/sudo".into(),
            logger: "/usr/bin/logger".into(),
            loginuid: "/proc/self/loginuid".into(),
            sessionid: "/proc/self/sessionid".into(),
            path: SECURE_PATH.iter().map(|s| s.to_string()).collect(),
            cap: CAP,
        }
    }
}

/// Who sudo ran this for.
pub struct Invoker {
    pub name: String,
    pub uid: u32,
}

/// The caller's standard input, output and errors (the process's own in the program).
pub struct Streams {
    pub stdin: Box<dyn Read + Send>,
    pub stdout: Box<dyn Write + Send>,
    pub stderr: Box<dyn Write + Send>,
}

// ---------------------------------------------------------------- small pieces

/// A regular file that may be run.
fn runnable(p: &Path) -> bool {
    let Ok(c) = CString::new(p.as_os_str().as_bytes()) else { return false };
    fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
        && unsafe { libc::faccessat(libc::AT_FDCWD, c.as_ptr(), libc::X_OK, libc::AT_EACCESS) == 0 }
}

/// The command as sudo finds it: a name with a slash as it is (relative to the folder we are in), any other in the folders of
/// the path, the first that may be run.
pub fn resolve(cmd: &OsStr, cwd: &Path, path: &[String]) -> Option<PathBuf> {
    let b = cmd.as_bytes();
    if b.is_empty() {
        return None;
    }
    if b.contains(&b'/') {
        let p = if b[0] == b'/' { PathBuf::from(cmd) } else { cwd.join(cmd) };
        return runnable(&p).then_some(p);
    }
    path.iter().map(|d| Path::new(d).join(cmd)).find(|p| runnable(p))
}

/// An argument as the journal and `list` show it: as it is when it is plain, else in single quotes, with control characters
/// and bytes that are not UTF-8 as \xNN.
pub fn show(arg: &[u8]) -> String {
    let plain = |c: u8| c.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&c);
    if !arg.is_empty() && arg.iter().all(|&c| plain(c)) {
        return String::from_utf8_lossy(arg).into_owned();
    }
    let mut out = String::from("'");
    let text = String::from_utf8_lossy(arg);
    let utf8 = std::str::from_utf8(arg).is_ok();
    if utf8 {
        for ch in text.chars() {
            match ch {
                '\'' => out.push_str("'\\''"),
                c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
                c => out.push(c),
            }
        }
    } else {
        for &c in arg {
            match c {
                b'\'' => out.push_str("'\\''"),
                c if !(0x20..0x7f).contains(&c) => out.push_str(&format!("\\x{c:02x}")),
                c => out.push(c as char),
            }
        }
    }
    out.push('\'');
    out
}

pub fn show_argv<S: AsRef<OsStr>>(argv: &[S]) -> String {
    argv.iter().map(|a| show(a.as_ref().as_bytes())).collect::<Vec<_>>().join(" ")
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn b64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn unb64(text: &str) -> Option<Vec<u8>> {
    let t = text.as_bytes();
    if !t.len().is_multiple_of(4) {
        return None;
    }
    let val = |c: u8| B64.iter().position(|&x| x == c).map(|v| v as u32);
    let mut out = Vec::with_capacity(t.len() / 4 * 3);
    for (k, q) in t.chunks(4).enumerate() {
        let last = k == t.len() / 4 - 1;
        let pad = q.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for &c in &q[..4 - pad] {
            n = n << 6 | val(c)?;
        }
        n <<= 6 * pad as u32;
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..3 - pad]);
    }
    Some(out)
}

/// Year, month, day, hour, minute, second of a time in UTC (days to the civil date: H. Hinnant's algorithm).
fn utc(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400) as u32);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d, rem / 3600, rem / 60 % 60, rem % 60)
}

/// 2026-11-19T21:03:04.123Z
pub fn iso(t: SystemTime) -> String {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let (y, mo, da, h, mi, s) = utc(d.as_secs() as i64);
    format!("{y:04}-{mo:02}-{da:02}T{h:02}:{mi:02}:{s:02}.{:03}Z", d.subsec_millis())
}

/// A session's id: its start in UTC to the millisecond and the recorder's pid (20261119T210304.123Z-4242), also the record's name.
pub fn session_id(t: SystemTime, pid: u32) -> String {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let (y, mo, da, h, mi, s) = utc(d.as_secs() as i64);
    format!("{y:04}{mo:02}{da:02}T{h:02}{mi:02}{s:02}.{:03}Z-{pid}", d.subsec_millis())
}

/// What an id may be: exactly that shape (never a path).
pub fn id_ok(id: &str) -> bool {
    let b = id.as_bytes();
    let digits = |r: std::ops::Range<usize>| b.get(r).is_some_and(|x| x.iter().all(u8::is_ascii_digit));
    (22..=31).contains(&b.len())
        && digits(0..8)
        && b[8] == b'T'
        && digits(9..15)
        && b[15] == b'.'
        && digits(16..19)
        && &b[19..21] == b"Z-"
        && digits(21..b.len())
}

fn journal(setup: &Setup, prio: &str, msg: &str) -> bool {
    let mut m = msg.to_string();
    if m.len() > LOG_MAX {
        let mut cut = LOG_MAX;
        while !m.is_char_boundary(cut) {
            cut -= 1;
        }
        let more = m.len() - cut;
        m.truncate(cut);
        m.push_str(&format!(" [{more} bytes more]"));
    }
    Command::new(&setup.logger)
        .args(["-t", TAG, "-p", prio, "--", &m])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Does sudo let the rules account run exactly this command line as root?
pub fn allowed(setup: &Setup, rules: &str, abs: &Path, args: &[OsString]) -> bool {
    Command::new(&setup.sudo)
        .args(["-n", "-l", "-U", rules, "--"])
        .arg(abs)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.code() == Some(0))
        .unwrap_or(false)
}

// ---------------------------------------------------------------- the record

pub struct Rec {
    file: fs::File,
    hash: Sha256,
    start: Instant,
    cap: u64,
    recorded: u64,
    bytes: [u64; 3],
    truncated: bool,
    closed: bool,
    error: Option<String>,
    /// The journal line at the start could not be written (the footer says so).
    pub unlogged: bool,
}

impl Rec {
    pub fn new(file: fs::File, cap: u64) -> Rec {
        Rec {
            file,
            hash: Sha256::new(),
            start: Instant::now(),
            cap,
            recorded: 0,
            bytes: [0; 3],
            truncated: false,
            closed: false,
            error: None,
            unlogged: false,
        }
    }

    fn line(&mut self, v: &Value) {
        if self.error.is_some() {
            return;
        }
        let mut s = v.to_string();
        s.push('\n');
        match self.file.write_all(s.as_bytes()) {
            Ok(()) => self.hash.update(s.as_bytes()),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn header(&mut self, v: Value) {
        self.line(&v);
    }

    /// Data of stream s (0 in, 1 out, 2 err), recorded while there is room.
    pub fn data(&mut self, s: usize, b: &[u8]) {
        if self.closed {
            return;
        }
        self.bytes[s] += b.len() as u64;
        let room = self.cap.saturating_sub(self.recorded).min(b.len() as u64) as usize;
        if room < b.len() {
            self.truncated = true;
        }
        if room == 0 {
            return;
        }
        self.recorded += room as u64;
        let t = self.start.elapsed().as_millis() as u64;
        self.line(&json!({"t": t, "s": STREAMS[s], "b64": b64(&b[..room])}));
    }

    /// The footer; nothing is recorded after it. The SHA-256 of all that was written, and the write error if there was one.
    pub fn finish(&mut self, exit: i32, signal: Option<i32>) -> (String, Option<String>) {
        self.closed = true;
        let mut f = json!({
            "end": iso(SystemTime::now()),
            "exit": exit,
            "bytes": {"in": self.bytes[0], "out": self.bytes[1], "err": self.bytes[2]},
            "recorded": self.recorded,
            "truncated": self.truncated,
        });
        if let Some(s) = signal {
            f["signal"] = json!(s);
        }
        if self.unlogged {
            f["journal"] = json!(false);
        }
        self.line(&f);
        if let Err(e) = self.file.sync_all() {
            self.error.get_or_insert(e.to_string());
        }
        let hex = self.hash.clone().finalize().iter().map(|b| format!("{b:02x}")).collect();
        (hex, self.error.clone())
    }
}

fn create_record(setup: &Setup, id: &str) -> io::Result<(PathBuf, fs::File)> {
    if fs::symlink_metadata(&setup.log_dir).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return Err(io::Error::other("the record folder is a link"));
    }
    sh::install_dir(&setup.log_dir, 0o700, setup.dir_owner)?;
    let p = Path::new(&setup.log_dir).join(format!("{id}.jsonl"));
    let f = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC).open(&p)?;
    Ok((p, f))
}

// ---------------------------------------------------------------- signals

static CHILD: AtomicI32 = AtomicI32::new(0);
static PENDING: AtomicI32 = AtomicI32::new(0);

fn send(pid: i32, sig: i32) {
    // the command leads its own process group (setsid); right after the fork it may not yet
    unsafe {
        if libc::kill(-pid, sig) != 0 {
            libc::kill(pid, sig);
        }
    }
}

/// What the handler does: pass the signal on to the command, or keep it until there is one.
pub fn forward(sig: i32) {
    let pid = CHILD.load(Ordering::SeqCst);
    if pid > 0 {
        send(pid, sig);
    } else {
        PENDING.store(sig, Ordering::SeqCst);
    }
}

extern "C" fn on_signal(sig: libc::c_int) {
    forward(sig);
}

/// SIGINT, SIGTERM and SIGHUP are passed on to the command (the program sets this; the tests call `forward`).
pub fn forward_signals() {
    for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as usize;
            sa.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(sig, &sa, std::ptr::null_mut());
        }
    }
}

// ---------------------------------------------------------------- a session

/// Copy one stream through and into the record until it ends, or until it cannot be passed on (then the reading side is
/// closed, as a pipe to a reader that went away is). Gives the writer back.
fn pump<R: Read, W: Write>(mut from: R, mut to: W, rec: &Mutex<Rec>, s: usize) -> W {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match from.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        if let Ok(mut r) = rec.lock() {
            r.data(s, &buf[..n]);
        }
        if to.write_all(&buf[..n]).and_then(|_| to.flush()).is_err() {
            break;
        }
    }
    to
}

fn loginuid(setup: &Setup, uid: u32) -> Value {
    match fs::write(&setup.loginuid, uid.to_string()) {
        Ok(()) => json!(uid),
        Err(e) => json!({"error": e.to_string()}),
    }
}

/// Check, record and run one command for the caller. The exit code.
pub fn session(setup: &Setup, who: &Invoker, cmd: &[OsString], io: Streams) -> i32 {
    let Streams { stdin, stdout, mut stderr } = io;
    let mut say = |m: &str| {
        let _ = writeln!(stderr, "{TAG}: {m}");
    };
    let Some(name) = cmd.first() else {
        say("no command (labyrinth-agent-rec -- CMD ARGS...)");
        return 2;
    };
    let shown = show_argv(cmd);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let Some(abs) = resolve(name, &cwd, &setup.path) else {
        say(&format!("{}: command not found", show(name.as_bytes())));
        journal(setup, "auth.warning", &format!("refused for {} (uid {}): not found: {shown}", who.name, who.uid));
        return 1;
    };
    let rules = rules_user(&who.name);
    if !allowed(setup, &rules, &abs, &cmd[1..]) {
        let line = format!("not allowed by the agent's rules: {} {}", show(abs.as_os_str().as_bytes()), show_argv(&cmd[1..]));
        say(line.trim_end());
        journal(setup, "auth.warning", &format!("refused for {} (uid {}): not allowed by {rules}: {shown}", who.name, who.uid));
        return 1;
    }
    // before any thread: the kernel lets a process set only its own login uid
    let luid = loginuid(setup, who.uid);
    let ses = fs::read_to_string(&setup.sessionid).ok().and_then(|s| s.trim().parse::<u64>().ok());
    let started = SystemTime::now();
    let id = session_id(started, std::process::id());
    let (path, file) = match create_record(setup, &id) {
        Ok(x) => x,
        Err(e) => {
            say(&format!("cannot make the record in {}: {e}; nothing was run", setup.log_dir));
            journal(setup, "auth.warning", &format!("refused for {} (uid {}): no record ({e}): {shown}", who.name, who.uid));
            return 1;
        }
    };
    let mut rec = Rec::new(file, setup.cap);
    rec.header(json!({
        "start": iso(started),
        "id": id,
        "invoker": who.name,
        "uid": who.uid,
        "cwd": cwd.to_string_lossy(),
        "argv": cmd.iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "abs": abs.to_string_lossy(),
        "loginuid": luid,
        "audit_session": ses,
    }));
    let start_line = format!("session {id} by {} (uid {}) in {}: {shown}", who.name, who.uid, show(cwd.as_os_str().as_bytes()));
    rec.unlogged = !journal(setup, "auth.notice", &start_line);
    let rec = Arc::new(Mutex::new(rec));

    let mut c = Command::new(&abs);
    c.arg0(name).args(&cmd[1..]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => {
            let code = if e.kind() == io::ErrorKind::NotFound { 127 } else { 126 };
            let _ = writeln!(stderr, "{TAG}: {}: {e}", show(abs.as_os_str().as_bytes()));
            return end(setup, &rec, &id, &path, code, None, stderr);
        }
    };
    CHILD.store(child.id() as i32, Ordering::SeqCst);
    let pending = PENDING.swap(0, Ordering::SeqCst);
    if pending != 0 {
        send(child.id() as i32, pending);
    }
    let (cin, cout, cerr) = (child.stdin.take(), child.stdout.take(), child.stderr.take());
    if let Some(cin) = cin {
        let r = rec.clone();
        // not waited for: the caller's input may never end; it stops being recorded with the footer
        std::thread::spawn(move || {
            pump(stdin, cin, &r, 0);
        });
    }
    let out = cout.map(|o| {
        let r = rec.clone();
        std::thread::spawn(move || {
            pump(o, stdout, &r, 1);
        })
    });
    let errt = cerr.map(|e| {
        let r = rec.clone();
        std::thread::spawn(move || pump(e, stderr, &r, 2))
    });
    let status = child.wait();
    CHILD.store(0, Ordering::SeqCst);
    if let Some(t) = out {
        let _ = t.join();
    }
    let stderr: Box<dyn Write + Send> = match errt.map(|t| t.join()) {
        Some(Ok(w)) => w,
        _ => Box::new(io::sink()),
    };
    let (code, sig) = match status {
        Ok(st) => match (st.code(), st.signal()) {
            (Some(c), _) => (c, None),
            (None, Some(s)) => (128 + s, Some(s)),
            _ => (1, None),
        },
        Err(_) => (1, None),
    };
    end(setup, &rec, &id, &path, code, sig, stderr)
}

fn end(setup: &Setup, rec: &Mutex<Rec>, id: &str, path: &Path, code: i32, sig: Option<i32>, mut stderr: Box<dyn Write + Send>) -> i32 {
    let (hex, error) = match rec.lock() {
        Ok(mut r) => r.finish(code, sig),
        Err(_) => (String::new(), Some("the record's lock was poisoned".into())),
    };
    let mut msg = format!("session {id} ended: exit {code}; record {} sha256 {hex}", path.display());
    if let Some(e) = &error {
        msg.push_str(&format!(" (the record is incomplete: {e})"));
        let _ = writeln!(stderr, "{TAG}: the record {} is incomplete: {e}", path.display());
    }
    journal(setup, "auth.notice", &msg);
    code
}

// ---------------------------------------------------------------- reading the records

/// One session as `list` shows it.
#[derive(Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub start: String,
    pub invoker: String,
    pub exit: Option<i64>,
    pub command: String,
}

fn first_line(p: &Path) -> Option<Value> {
    let f = fs::File::open(p).ok()?;
    let mut line = String::new();
    io::BufReader::new(f).take(1 << 20).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

/// The footer, when the record has one (the last line; a session still running, or cut off, has none).
fn footer(p: &Path) -> Option<Value> {
    let mut f = fs::File::open(p).ok()?;
    let len = f.metadata().ok()?.len();
    let from = len.saturating_sub(4096);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut tail = Vec::new();
    f.read_to_end(&mut tail).ok()?;
    let body = tail.strip_suffix(b"\n")?;
    let last = body.rsplit(|&c| c == b'\n').next()?;
    let v: Value = serde_json::from_slice(last).ok()?;
    v.get("end").is_some().then_some(v)
}

/// The sessions in the folder, newest first.
pub fn entries(dir: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let id = name.strip_suffix(".jsonl")?.to_string();
                if !id_ok(&id) {
                    return None;
                }
                let h = first_line(&e.path())?;
                let f = footer(&e.path());
                let s = |k: &str| h.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let argv: Vec<String> = h
                    .get("argv")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str()).map(String::from).collect())
                    .unwrap_or_default();
                Some(Entry {
                    start: s("start"),
                    invoker: s("invoker"),
                    exit: f.and_then(|f| f.get("exit")?.as_i64()),
                    command: show_argv(&argv),
                    id,
                })
            })
            .collect(),
        Err(_) => vec![],
    };
    out.sort_by(|a, b| b.start.cmp(&a.start).then_with(|| b.id.cmp(&a.id)));
    out
}

pub fn list(dir: &str, n: usize, out: &mut dyn Write) -> io::Result<()> {
    for e in entries(dir).into_iter().take(n) {
        let exit = e.exit.map(|c| c.to_string()).unwrap_or_else(|| "-".into());
        writeln!(out, "{}  {}  {}  {}  {}", sh::pad(&e.id, 24), e.start, sh::pad(&e.invoker, 14), sh::pad(&exit, 4), e.command)?;
    }
    Ok(())
}

/// The data of a record, in order: (ms since the start, stream 0 in / 1 out / 2 err, bytes).
pub fn read_data(p: &Path) -> io::Result<Vec<(u64, usize, Vec<u8>)>> {
    let f = fs::File::open(p)?;
    let mut out = vec![];
    for line in io::BufReader::new(f).lines() {
        let line = line?;
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let (Some(s), Some(b)) = (v.get("s").and_then(|x| x.as_str()), v.get("b64").and_then(|x| x.as_str())) else { continue };
        let Some(s) = STREAMS.iter().position(|x| *x == s) else { continue };
        let data = unb64(b).ok_or_else(|| io::Error::other("a data line that is not base64"))?;
        out.push((v.get("t").and_then(|x| x.as_u64()).unwrap_or(0), s, data));
    }
    Ok(out)
}

/// The output and errors written back (with --input, the input too, on the output), with their timing unless fast.
pub fn replay(dir: &str, id: &str, fast: bool, input: bool, out: &mut dyn Write, errw: &mut dyn Write) -> Result<(), String> {
    let id = id.strip_suffix(".jsonl").unwrap_or(id);
    if !id_ok(id) {
        return Err(format!("not a session id: {}", show(id.as_bytes())));
    }
    let p = Path::new(dir).join(format!("{id}.jsonl"));
    let data = read_data(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let start = Instant::now();
    for (t, s, b) in data {
        if s == 0 && !input {
            continue;
        }
        if !fast {
            let at = Duration::from_millis(t);
            let now = start.elapsed();
            if at > now {
                std::thread::sleep(at - now);
            }
        }
        let w: &mut dyn Write = if s == 2 { errw } else { out };
        w.write_all(&b).and_then(|_| w.flush()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------------------------------------------------------------- the program

/// The caller, from sudo's variables: SUDO_USER an account name, SUDO_UID that account's uid.
pub fn invoker(user: Option<OsString>, uid: Option<OsString>) -> Result<Invoker, String> {
    let (Some(user), Some(uid)) = (user, uid) else { return Err("runs through sudo (SUDO_USER and SUDO_UID are not set)".into()) };
    let name = user.to_str().filter(|n| sh::account_name(n)).ok_or("SUDO_USER is not an account name")?.to_string();
    let uid: u32 = uid.to_str().filter(|u| sh::digits(u)).and_then(|u| u.parse().ok()).ok_or("SUDO_UID is not a number")?;
    match sh::user_id(&name) {
        Some((u, _)) if u == uid => Ok(Invoker { name, uid }),
        _ => Err("SUDO_USER and SUDO_UID are not the same account".into()),
    }
}

/// Is the one who ran sudo an agent (an account with a rules account)? Agents may not read the records.
fn caller_is_agent() -> bool {
    match std::env::var("SUDO_USER") {
        Ok(u) if sh::account_name(&u) => sh::user_id(&rules_user(&u)).is_some(),
        Ok(u) => !u.is_empty(),
        Err(_) => false,
    }
}

fn reader_check() -> Result<(), i32> {
    if sh::euid() != 0 {
        return Err(err("only root can read the records: sudo labyrinth-agent-rec list|replay", 1));
    }
    if caller_is_agent() {
        return Err(err("the agent may not read its own records", 1));
    }
    Ok(())
}

pub fn main(args: &[OsString]) -> i32 {
    sh::set_prog(TAG);
    let a: Vec<&OsStr> = args.iter().skip(1).map(|s| s.as_os_str()).collect();
    let word = |i: usize| a.get(i).and_then(|s| s.to_str());
    match word(0) {
        None => {
            eprint!("{DOC}");
            return 2;
        }
        Some("-h" | "--help") => {
            print!("{DOC}");
            return 0;
        }
        Some("list") => {
            let n = match word(1) {
                None => 20,
                Some(n) if sh::digits(n) && n.len() <= 6 => n.parse().unwrap_or(20),
                Some(_) => return err("list [N]: N is a number", 2),
            };
            if a.len() > 2 {
                return err("list [N]", 2);
            }
            if let Err(c) = reader_check() {
                return c;
            }
            return match list(LOG_DIR, n, &mut io::stdout()) {
                Ok(()) => 0,
                Err(e) => err(&e.to_string(), 1),
            };
        }
        Some("replay") => {
            let (mut id, mut fast, mut input) = (None, false, false);
            for x in &a[1..] {
                match x.to_str() {
                    Some("--fast") => fast = true,
                    Some("--input") => input = true,
                    Some(s) if id.is_none() && !s.starts_with('-') => id = Some(s.to_string()),
                    _ => return err("replay ID [--fast] [--input]", 2),
                }
            }
            let Some(id) = id else { return err("replay ID [--fast] [--input]", 2) };
            if let Err(c) = reader_check() {
                return c;
            }
            return match replay(LOG_DIR, &id, fast, input, &mut io::stdout(), &mut io::stderr()) {
                Ok(()) => 0,
                Err(e) => err(&e, 1),
            };
        }
        _ => {}
    }
    let cmd: Vec<OsString> = if word(0) == Some("--") {
        a[1..].iter().map(|s| s.to_os_string()).collect()
    } else {
        a.iter().map(|s| s.to_os_string()).collect()
    };
    if sh::euid() != 0 {
        return err("runs as root through sudo (claude-sudo CMD...)", 1);
    }
    let who = match invoker(std::env::var_os("SUDO_USER"), std::env::var_os("SUDO_UID")) {
        Ok(w) => w,
        Err(e) => return err(&e, 1),
    };
    let rules = rules_user(&who.name);
    match sh::user_id(&rules) {
        Some((u, _)) if u != 0 && u != who.uid => {}
        _ => return err(&format!("{rules} is not an account: {} is not an agent set up by labyrinth-agent-setup", who.name), 1),
    }
    forward_signals();
    session(
        &Setup::system(),
        &who,
        &cmd,
        Streams { stdin: Box::new(io::stdin()), stdout: Box::new(io::stdout()), stderr: Box::new(io::stderr()) },
    )
}

#[cfg(test)]
mod tests;
