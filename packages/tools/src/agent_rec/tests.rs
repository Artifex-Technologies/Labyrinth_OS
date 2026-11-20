//! The recorder on stand-ins: sudo, logger and the commands are small shell scripts in a scratch folder, the record folder and
//! the login uid file are in it too, and the caller's streams are buffers. Nothing here needs root or touches the system.

use super::*;
use crate::testutil::TempDir;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::sync::MutexGuard;

/// The sessions share the handler's idea of the running command (CHILD): one at a time.
fn serial() -> MutexGuard<'static, ()> {
    static S: Mutex<()> = Mutex::new(());
    S.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Buf {
    fn get(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.get()).into_owned()
    }
}

fn stub(dir: &str, name: &str, body: &str) -> String {
    let p = format!("{dir}/{name}");
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}

struct World {
    t: TempDir,
    setup: Setup,
}

impl World {
    /// sudo answers with `sudo_exit`, and writes what it was asked to sudo-args.
    fn new(sudo_exit: i32) -> World {
        let t = TempDir::new();
        let d = t.p("");
        let d = d.trim_end_matches('/').to_string();
        fs::create_dir(format!("{d}/bin")).unwrap();
        let sudo = stub(&d, "sudo", &format!("printf '%s\\n' \"$@\" > '{d}/sudo-args'\nexit {sudo_exit}"));
        let logger = stub(&d, "logger", &format!("printf '%s\\n' \"$*\" >> '{d}/journal'"));
        fs::write(format!("{d}/sessionid"), "77\n").unwrap();
        let setup = Setup {
            log_dir: format!("{d}/records"),
            dir_owner: None,
            sudo,
            logger,
            loginuid: format!("{d}/loginuid"),
            sessionid: format!("{d}/sessionid"),
            path: vec![format!("{d}/nothing"), format!("{d}/bin")],
            cap: CAP,
        };
        World { t, setup }
    }

    fn p(&self, rel: &str) -> String {
        self.t.p(rel)
    }

    fn cmd(&self, name: &str, body: &str) {
        stub(&self.p("bin"), name, body);
    }

    fn run(&self, argv: &[&str], input: &[u8]) -> (i32, Buf, Buf) {
        let (out, err) = (Buf::default(), Buf::default());
        let who = Invoker { name: "claude-agent".into(), uid: 1234 };
        let cmd: Vec<OsString> = argv.iter().map(OsString::from).collect();
        let streams =
            Streams { stdin: Box::new(io::Cursor::new(input.to_vec())), stdout: Box::new(out.clone()), stderr: Box::new(err.clone()) };
        (session(&self.setup, &who, &cmd, streams), out, err)
    }

    fn records(&self) -> Vec<PathBuf> {
        match fs::read_dir(&self.setup.log_dir) {
            Ok(rd) => rd.map(|e| e.unwrap().path()).collect(),
            Err(_) => vec![],
        }
    }

    fn only_record(&self) -> (PathBuf, Vec<Value>) {
        let r = self.records();
        assert_eq!(r.len(), 1, "{r:?}");
        let lines = fs::read_to_string(&r[0]).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        (r[0].clone(), lines)
    }
}

fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

// ---- small pieces

#[test]
fn base64_both_ways() {
    for (plain, enc) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("foobar", "Zm9vYmFy")] {
        assert_eq!(b64(plain.as_bytes()), enc);
        assert_eq!(unb64(enc).unwrap(), plain.as_bytes());
    }
    let all: Vec<u8> = (0..=255u8).chain((0..=255u8).rev()).collect();
    for n in 0..all.len() {
        assert_eq!(unb64(&b64(&all[..n])).unwrap(), &all[..n]);
    }
    for bad in ["Zg=", "Z===", "Zg==Zg==", "Zm9*", "Zm9v\n"] {
        assert!(unb64(bad).is_none(), "{bad}");
    }
}

#[test]
fn times_in_utc() {
    let at = |s: u64, ms: u64| UNIX_EPOCH + Duration::from_millis(s * 1000 + ms);
    assert_eq!(iso(at(0, 0)), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso(at(951782400, 5)), "2000-02-29T00:00:00.005Z");
    assert_eq!(iso(at(1709251199, 999)), "2024-02-29T23:59:59.999Z");
    assert_eq!(iso(at(1795641784, 123)), "2026-11-25T21:23:04.123Z");
    assert_eq!(session_id(at(1795641784, 9), 4242), "20261125T212304.009Z-4242");
    assert!(id_ok("20261125T212304.009Z-4242") && id_ok("20261125T212304.009Z-4294967295"));
    for bad in [
        "",
        "../x",
        "a",
        "2026/1",
        "20261125T212304.009Z-",
        "20261125T212304.009Z-4242.jsonl",
        "20261125T212304Z-4242",
        "2026112xT212304.009Z-1",
        "20261125T212304.009Z-12345678901",
    ] {
        assert!(!id_ok(bad), "{bad}");
    }
}

#[test]
fn names_and_showing() {
    assert_eq!(rules_user("claude-agent"), "claude-agent-rules");
    assert_eq!(show(b"pacman"), "pacman");
    assert_eq!(show(b"/usr/bin/x"), "/usr/bin/x");
    assert_eq!(show(b""), "''");
    assert_eq!(show(b"a b"), "'a b'");
    assert_eq!(show(b"it's"), "'it'\\''s'");
    assert_eq!(show(b"a\nb"), "'a\\x0ab'");
    assert_eq!(show(b"\xffx"), "'\\xffx'");
    assert_eq!(show_argv(&["ls", "-l", "a b"]), "ls -l 'a b'");
}

#[test]
fn commands_are_found_as_sudo_finds_them() {
    let w = World::new(0);
    w.cmd("tool", "true");
    let path = w.setup.path.clone();
    let here = w.p("");
    let cwd = Path::new(&here);
    assert_eq!(resolve(OsStr::new("tool"), cwd, &path), Some(PathBuf::from(w.p("bin/tool"))));
    assert_eq!(resolve(OsStr::new("missing"), cwd, &path), None);
    assert_eq!(resolve(OsStr::new(""), cwd, &path), None);
    assert_eq!(resolve(OsStr::new("bin/tool"), cwd, &path), Some(PathBuf::from(w.p("bin/tool"))));
    assert_eq!(resolve(OsStr::new(&w.p("bin/tool")), Path::new("/"), &path), Some(PathBuf::from(w.p("bin/tool"))));
    // not runnable, or a folder: not found (the next folder of the path is looked in)
    fs::create_dir_all(w.p("first")).unwrap();
    fs::write(w.p("first/tool"), "x").unwrap();
    fs::create_dir(w.p("first/sub")).unwrap();
    let path2 = vec![w.p("first"), w.p("bin")];
    assert_eq!(resolve(OsStr::new("tool"), cwd, &path2), Some(PathBuf::from(w.p("bin/tool"))));
    assert_eq!(resolve(OsStr::new("sub"), cwd, &path2), None);
}

#[test]
fn sudo_callers() {
    let me = unsafe { libc::getuid() };
    let name = {
        let pw = unsafe { libc::getpwuid(me) };
        unsafe { std::ffi::CStr::from_ptr((*pw).pw_name) }.to_string_lossy().into_owned()
    };
    let o = |s: &str| Some(OsString::from(s));
    let w = invoker(o(&name), o(&me.to_string())).unwrap();
    assert_eq!((w.name.as_str(), w.uid), (name.as_str(), me));
    assert!(invoker(None, o("1")).is_err());
    assert!(invoker(o(&name), None).is_err());
    assert!(invoker(o(&name), o(&(me + 1).to_string())).is_err());
    assert!(invoker(o("no such user here"), o("1")).is_err());
    assert!(invoker(o(&name), o("-1")).is_err());
}

// ---- sessions

#[test]
fn refused_when_the_rules_do_not_allow_it() {
    let _s = serial();
    let w = World::new(1);
    w.cmd("tool", &format!("touch '{}'", w.p("ran")));
    let (code, out, err) = w.run(&["tool", "--flag", "a b"], b"");
    assert_eq!(code, 1);
    assert!(out.get().is_empty());
    assert_eq!(err.text(), format!("labyrinth-agent-rec: not allowed by the agent's rules: {} --flag 'a b'\n", w.p("bin/tool")));
    assert!(!sh::exists(&w.p("ran")), "nothing may run");
    assert!(w.records().is_empty() && !sh::exists(&w.p("loginuid")));
    // sudo was asked by argv about the rules account, for exactly this command line
    assert_eq!(sh::cat(&w.p("sudo-args")).unwrap(), format!("-n\n-l\n-U\nclaude-agent-rules\n--\n{}\n--flag\na b", w.p("bin/tool")));
    let j = sh::cat(&w.p("journal")).unwrap();
    assert!(j.starts_with("-t labyrinth-agent-rec -p auth.warning -- refused for claude-agent (uid 1234): not allowed by claude-agent-rules: tool --flag 'a b'"), "{j}");
}

#[test]
fn refused_when_not_found() {
    let _s = serial();
    let w = World::new(0);
    let (code, _, err) = w.run(&["missing"], b"");
    assert_eq!(code, 1);
    assert_eq!(err.text(), "labyrinth-agent-rec: missing: command not found\n");
    assert!(!sh::exists(&w.p("sudo-args")) && w.records().is_empty());
    let (code, _, _) = w.run(&[], b"");
    assert_eq!(code, 2);
}

#[test]
fn a_session_is_passed_through_and_recorded() {
    let _s = serial();
    let w = World::new(0);
    w.cmd("tool", "printf 'out 1\\n'; printf 'err 1\\n' >&2; cat; printf 'out 2' ; exit 3");
    let (code, out, err) = w.run(&["tool", "x"], b"typed\n");
    assert_eq!(code, 3);
    assert_eq!(out.text(), "out 1\ntyped\nout 2");
    assert_eq!(err.text(), "err 1\n");
    assert_eq!(sh::cat(&w.p("loginuid")).unwrap(), "1234");
    let (path, lines) = w.only_record();
    let m = fs::metadata(&path).unwrap();
    assert_eq!(m.mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(&w.setup.log_dir).unwrap().mode() & 0o777, 0o700);
    let h = &lines[0];
    let id = path.file_stem().unwrap().to_string_lossy().into_owned();
    assert!(id_ok(&id) && id.ends_with(&format!("-{}", std::process::id())));
    assert_eq!(h["id"], json!(id));
    assert_eq!(h["invoker"], json!("claude-agent"));
    assert_eq!(h["uid"], json!(1234));
    assert_eq!(h["argv"], json!(["tool", "x"]));
    assert_eq!(h["abs"], json!(w.p("bin/tool")));
    assert_eq!(h["loginuid"], json!(1234));
    assert_eq!(h["audit_session"], json!(77));
    assert_eq!(h["cwd"], json!(std::env::current_dir().unwrap().to_string_lossy()));
    assert!(h["start"].as_str().unwrap().ends_with('Z'));
    let f = lines.last().unwrap();
    assert_eq!(f["exit"], json!(3));
    assert_eq!(f["bytes"], json!({"in": 6, "out": 17, "err": 6}));
    assert_eq!(f["truncated"], json!(false));
    assert!(f.get("signal").is_none() && f.get("journal").is_none());
    // the data, in order of each stream
    let data = read_data(&path).unwrap();
    let joined = |s: usize| data.iter().filter(|d| d.1 == s).flat_map(|d| d.2.clone()).collect::<Vec<u8>>();
    assert_eq!(joined(0), b"typed\n");
    assert_eq!(joined(1), b"out 1\ntyped\nout 2");
    assert_eq!(joined(2), b"err 1\n");
    assert!(data.windows(2).all(|p| p[0].0 <= p[1].0));
    // replay --fast: the same output and errors; --input adds the input to the output
    let (ro, re) = (Buf::default(), Buf::default());
    replay(&w.setup.log_dir, &id, true, false, &mut ro.clone(), &mut re.clone()).unwrap();
    assert_eq!((ro.get(), re.get()), (out.get(), err.get()));
    let ri = Buf::default();
    replay(&w.setup.log_dir, &format!("{id}.jsonl"), true, true, &mut ri.clone(), &mut io::sink()).unwrap();
    assert_eq!(ri.text().len(), out.text().len() + 6);
    assert!(replay(&w.setup.log_dir, "../etc", true, false, &mut io::sink(), &mut io::sink()).is_err());
    assert!(replay(&w.setup.log_dir, "20000101T000000.000Z-1", true, false, &mut io::sink(), &mut io::sink()).is_err());
    // the journal: the start, and the end with the record's SHA-256
    let j = sh::cat(&w.p("journal")).unwrap();
    let jl: Vec<&str> = j.lines().collect();
    assert_eq!(jl.len(), 2, "{j}");
    assert!(jl[0].starts_with(&format!("-t labyrinth-agent-rec -p auth.notice -- session {id} by claude-agent (uid 1234) in ")));
    assert!(jl[0].ends_with(": tool x"));
    let sum = sha256_hex(&fs::read(&path).unwrap());
    assert_eq!(
        jl[1],
        format!("-t labyrinth-agent-rec -p auth.notice -- session {id} ended: exit 3; record {} sha256 {sum}", path.display())
    );
    // list
    let e = entries(&w.setup.log_dir);
    assert_eq!(
        e,
        vec![Entry {
            id: id.clone(),
            start: h["start"].as_str().unwrap().into(),
            invoker: "claude-agent".into(),
            exit: Some(3),
            command: "tool x".into()
        }]
    );
}

#[test]
fn binary_data_passes_unchanged() {
    let _s = serial();
    let w = World::new(0);
    w.cmd("tool", "cat; cat >&2 < \"$0.bin\"");
    let all: Vec<u8> = (0..=255u8).cycle().take(300_000).collect();
    fs::write(w.p("bin/tool.bin"), &all[..1000]).unwrap();
    let (code, out, err) = w.run(&["tool"], &all);
    assert_eq!(code, 0);
    assert_eq!(out.get(), all);
    assert_eq!(err.get(), &all[..1000]);
    let (path, _) = w.only_record();
    let (ro, re) = (Buf::default(), Buf::default());
    let id = path.file_stem().unwrap().to_string_lossy().into_owned();
    replay(&w.setup.log_dir, &id, true, false, &mut ro.clone(), &mut re.clone()).unwrap();
    assert_eq!(ro.get(), all);
    assert_eq!(re.get(), &all[..1000]);
}

#[test]
fn the_record_is_cut_at_the_cap_and_the_data_still_flows() {
    let _s = serial();
    let mut w = World::new(0);
    w.setup.cap = 10;
    w.cmd("tool", "printf '0123456'; printf 'abcdefghij' >&2; printf 'rest'");
    let (code, out, err) = w.run(&["tool"], b"");
    assert_eq!(code, 0);
    assert_eq!(out.text(), "0123456rest");
    assert_eq!(err.text(), "abcdefghij");
    let (path, lines) = w.only_record();
    let f = lines.last().unwrap();
    assert_eq!(f["truncated"], json!(true));
    assert_eq!(f["recorded"], json!(10));
    assert_eq!(f["bytes"], json!({"in": 0, "out": 11, "err": 10}));
    let n: usize = read_data(&path).unwrap().iter().map(|d| d.2.len()).sum();
    assert_eq!(n, 10);
}

#[test]
fn exit_codes_and_signals() {
    let _s = serial();
    let w = World::new(0);
    w.cmd("seven", "exit 7");
    w.cmd("killed", "kill -TERM $$");
    assert_eq!(w.run(&["seven"], b"").0, 7);
    assert_eq!(w.run(&["killed"], b"").0, 128 + libc::SIGTERM);
    let killed = w.records().into_iter().map(|p| fs::read_to_string(p).unwrap()).find(|t| t.contains("\"signal\":15")).unwrap();
    assert!(killed.contains("\"exit\":143"));
}

#[test]
fn signals_are_passed_on_to_the_command() {
    let _s = serial();
    let w = World::new(0);
    w.cmd("waits", "sleep 30");
    let t0 = Instant::now();
    let sender = std::thread::spawn(|| {
        while CHILD.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(100));
        forward(libc::SIGTERM);
    });
    let (code, _, _) = w.run(&["waits"], b"");
    sender.join().unwrap();
    assert_eq!(code, 128 + libc::SIGTERM);
    assert!(t0.elapsed() < Duration::from_secs(20));
}

#[test]
fn a_login_uid_that_cannot_be_set_is_noted() {
    let _s = serial();
    let mut w = World::new(0);
    w.setup.loginuid = w.p("no/such/folder/loginuid");
    w.setup.sessionid = w.p("no/such/sessionid");
    w.cmd("tool", "echo hi");
    let (code, out, _) = w.run(&["tool"], b"");
    assert_eq!((code, out.text().as_str()), (0, "hi\n"));
    let (_, lines) = w.only_record();
    assert!(lines[0]["loginuid"]["error"].as_str().unwrap().contains("No such file"));
    assert_eq!(lines[0]["audit_session"], Value::Null);
    // nor can the journal line: the footer says so
    let mut w = World::new(0);
    w.setup.logger = w.p("no-logger");
    w.cmd("tool", "true");
    assert_eq!(w.run(&["tool"], b"").0, 0);
    assert_eq!(w.only_record().1.last().unwrap()["journal"], json!(false));
}

#[test]
fn nothing_runs_without_a_record() {
    let _s = serial();
    let mut w = World::new(0);
    fs::write(w.p("file"), "").unwrap();
    w.setup.log_dir = w.p("file/records");
    w.cmd("tool", &format!("touch '{}'", w.p("ran")));
    let (code, _, err) = w.run(&["tool"], b"");
    assert_eq!(code, 1);
    assert!(err.text().contains("nothing was run"));
    assert!(!sh::exists(&w.p("ran")));
}

#[test]
fn list_is_newest_first() {
    let t = TempDir::new();
    let d = t.p("r");
    fs::create_dir(&d).unwrap();
    let rec = |id: &str, start: &str, footer: bool| {
        let mut s = json!({"start": start, "id": id, "invoker": "claude-agent", "argv": ["pacman", "-S", "a b"]}).to_string() + "\n";
        s += &(json!({"t": 1, "s": "out", "b64": b64(b"x")}).to_string() + "\n");
        if footer {
            s += &(json!({"end": start, "exit": 0}).to_string() + "\n");
        }
        fs::write(format!("{d}/{id}.jsonl"), s).unwrap();
    };
    rec("20261119T100000.000Z-5", "2026-11-19T10:00:00.000Z", true);
    rec("20261119T120000.000Z-3", "2026-11-19T12:00:00.000Z", false);
    rec("20261119T110000.500Z-9", "2026-11-19T11:00:00.500Z", true);
    fs::write(format!("{d}/notes.txt"), "x").unwrap();
    fs::write(format!("{d}/bad-name.jsonl"), "{}").unwrap();
    let ids: Vec<String> = entries(&d).into_iter().map(|e| e.id).collect();
    assert_eq!(ids, ["20261119T120000.000Z-3", "20261119T110000.500Z-9", "20261119T100000.000Z-5"]);
    let out = Buf::default();
    list(&d, 2, &mut out.clone()).unwrap();
    let text = out.text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("20261119T120000.000Z-3") && lines[0].contains("  -     pacman -S 'a b'"), "{text}");
    assert!(lines[1].contains("claude-agent    0     pacman"), "{text}");
    assert!(entries(&t.p("none")).is_empty());
}
