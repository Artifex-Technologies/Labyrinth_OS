//! labyrinth-quarantine: a safe place for files the malware scan is confident about, and the way back (the Python script of the
//! same name, with the same commands, files, output and exit codes; its help text is src/doc/labyrinth-quarantine.txt).
//!
//!   add PATH --signature NAME   move a regular file you own into quarantine (made private and not executable)
//!   list [--json]               what is in quarantine, newest first
//!   restore ID                  put it back (never over an existing file) and remember its content in scan-allow
//!   delete ID                   delete it for good
//!   allowed SHA256              exit 0 if that content was restored ("keep this file")
//!
//! ~/.local/share/labyrinth/quarantine/<id>/{payload,info.json} and ~/.config/labyrinth/scan-allow. No root.
//!
//! Errors the script caught (ValueError, OSError, which includes text that is not UTF-8) are printed as
//! "labyrinth-quarantine: <message>" with exit code 1; what it did not catch (a broken info.json: not an object, no "when") is
//! a Crash, exit code 1 too.

use crate::py::{self, Crash, Map, Value, R};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::os::unix::fs::MetadataExt;

pub const DOC: &str = include_str!("doc/labyrinth-quarantine.txt");

/// Where things are, from the environment (as the script read it at its start).
pub struct Paths {
    pub data: String,
    pub conf: String,
    pub allow: String,
}

impl Paths {
    pub fn from_env() -> Paths {
        let home = py::home();
        let data = py::join_all(&[&py::env_or("XDG_DATA_HOME", &py::join_all(&[&home, ".local", "share"])), "labyrinth", "quarantine"]);
        let conf = py::join(&py::env_or("XDG_CONFIG_HOME", &py::join(&home, ".config")), "labyrinth");
        let allow = py::join(&conf, "scan-allow");
        Paths { data, conf, allow }
    }
}

/// A failure the script reported with its message (a ValueError or an OSError).
#[derive(Debug)]
pub enum Fail {
    Said(String),
    Crash(Crash),
}

impl From<Crash> for Fail {
    fn from(c: Crash) -> Fail {
        Fail::Said(c.0)
    }
}

type F<T> = Result<T, Fail>;

fn said(m: &str) -> Fail {
    Fail::Said(m.to_string())
}

/// The sha256 of a file's content, as lowercase hex.
pub fn sha256_file(path: &str) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// ^[0-9a-f]{12}$ as Python's re.match reads it ($ also matches before a final newline).
pub fn valid_id(s: &str) -> bool {
    let core = s.strip_suffix('\n').unwrap_or(s);
    core.len() == 12 && core.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether a content was restored before: a line of scan-allow (up to a #, stripped) is that hash.
pub fn allowed(p: &Paths, digest: &str) -> F<bool> {
    match py::read_text(&p.allow) {
        Ok(text) => Ok(py::lines(&text).iter().any(|l| py::strip(py::partition(l, "#").0) == digest)),
        Err(py::ReadError::Os(_)) => Ok(false),
        Err(py::ReadError::Decode(m)) => Err(Fail::Said(m)),
    }
}

/// Quarantine a file; its id. Nothing is moved when it is not a regular file of yours.
pub fn add(p: &Paths, path: &str, signature: &str, source: &str) -> F<String> {
    let st = std::fs::symlink_metadata(path).map_err(|e| Fail::Said(py::oserror(&e, Some(path))))?;
    if !py::isfile(path) || py::islink(path) {
        return Err(said("not a regular file"));
    }
    if st.uid() != py::getuid() {
        return Err(said("not your file"));
    }
    let digest = sha256_file(path).map_err(|e| Fail::Said(py::oserror(&e, Some(path))))?;
    let qid = py::random_hex12();
    let d = py::join(&p.data, &qid);
    py::makedirs_mode(&d, 0o700, false)?;
    let mut info = Map::new();
    info.insert("id".into(), Value::String(qid.clone()));
    info.insert("original".into(), Value::String(py::abspath(path)));
    info.insert("signature".into(), Value::String(signature.to_string()));
    info.insert("sha256".into(), Value::String(digest));
    info.insert("size".into(), Value::from(st.size()));
    info.insert("mode".into(), Value::from(st.mode() & 0o7777));
    info.insert("when".into(), Value::String(py::strftime("%Y-%m-%d %H:%M:%S")));
    info.insert("source".into(), Value::String(source.to_string()));
    let payload = py::join(&d, "payload");
    if let Err(e) = py::move_file(path, &payload) {
        py::rmtree_quiet(&d);
        return Err(Fail::Said(format!("cannot move it: {}", e.0)));
    }
    py::chmod(&payload, 0o600)?;
    py::write_file(&py::join(&d, "info.json"), &py::pyjson::dumps_indent(&Value::Object(info), 1))?;
    Ok(qid)
}

/// Python's < between two values json.load made, for sorting: text with text, numbers (and true/false) with numbers; anything
/// else is a TypeError.
fn comparable(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::String(_), Value::String(_)))
        || matches!((a, b), (Value::Number(_) | Value::Bool(_), Value::Number(_) | Value::Bool(_)))
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Bool(b) => *b as i32 as f64,
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// What is in quarantine, newest first (by "when"; the same "when" in the folders' name order).
pub fn items(p: &Paths) -> R<Vec<Map<String, Value>>> {
    let mut names: Vec<String> = match std::fs::read_dir(&p.data) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
        Err(_) => return Ok(vec![]),
    };
    names.sort();
    let mut out = vec![];
    for n in names {
        if !valid_id(&n) {
            continue;
        }
        let info = match py::load_json(&py::join_all(&[&p.data, &n, "info.json"])) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match info {
            Value::Object(m) => {
                if m.get("id") == Some(&Value::String(n.clone())) {
                    out.push(m);
                }
            }
            other => {
                let kind = match other {
                    Value::Array(_) => "list",
                    Value::String(_) => "str",
                    Value::Bool(_) => "bool",
                    Value::Null => "NoneType",
                    Value::Number(ref x) if x.is_f64() => "float",
                    _ => "int",
                };
                return Err(Crash(format!("AttributeError: '{kind}' object has no attribute 'get'")));
            }
        }
    }
    for i in &out {
        if !i.contains_key("when") {
            return Err(Crash("KeyError: 'when'".into()));
        }
    }
    if out.len() >= 2 {
        let first = &out[0]["when"];
        for i in &out {
            if !comparable(first, &i["when"]) || !comparable(&i["when"], &i["when"]) {
                return Err(Crash("TypeError: '<' not supported between the \"when\" values of info.json files".into()));
            }
        }
    }
    out.sort_by(|a, b| {
        let (x, y) = (&b["when"], &a["when"]);
        match (x, y) {
            (Value::String(s), Value::String(t)) => s.cmp(t),
            _ => num(x).partial_cmp(&num(y)).unwrap_or(std::cmp::Ordering::Equal),
        }
    });
    Ok(out)
}

fn find(p: &Paths, qid: &str) -> F<Map<String, Value>> {
    if !valid_id(qid) {
        return Err(said("bad id"));
    }
    for i in items(p).map_err(Fail::Crash)? {
        if i.get("id") == Some(&Value::String(qid.to_string())) {
            return Ok(i);
        }
    }
    Err(said("no such item"))
}

fn text_of(info: &Map<String, Value>, key: &str) -> F<String> {
    match info.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(Fail::Crash(Crash(format!("TypeError: info.json's {key:?} is not text")))),
        None => Err(Fail::Crash(Crash(format!("KeyError: {}", py::repr(key))))),
    }
}

/// Put an item back where it was (or beside it, as NAME.restored, NAME.restored2 ...); where it went.
pub fn restore(p: &Paths, qid: &str) -> F<String> {
    let info = find(p, qid)?;
    let d = py::join(&p.data, qid);
    let original = text_of(&info, "original")?;
    let mut target = original.clone();
    if py::lexists(&target) {
        target.push_str(".restored");
    }
    let mut n = 1;
    while py::lexists(&target) {
        n += 1;
        target = format!("{original}.restored{n}");
    }
    py::makedirs(&py::dirname(&target))?;
    py::move_file(&py::join(&d, "payload"), &target)?;
    let mode = match info.get("mode") {
        None => 0o644,
        Some(Value::Number(n)) if n.is_i64() || n.is_u64() => n.as_i64().unwrap_or(0o644),
        Some(Value::Bool(b)) => *b as i64,
        Some(_) => return Err(Fail::Crash(Crash("TypeError: info.json's \"mode\" is not a whole number".into()))),
    };
    py::chmod(&target, ((mode & !0o111) as u32) & 0o7777)?;
    py::makedirs(&p.conf)?;
    let sha = py::pyjson::py_str(info.get("sha256").ok_or_else(|| Fail::Crash(Crash("KeyError: 'sha256'".into())))?);
    py::append_file(&p.allow, &format!("{sha}  # restored {}: {}\n", py::strftime("%Y-%m-%d"), py::basename(&original)))?;
    py::rmtree_quiet(&d);
    Ok(target)
}

/// Delete an item for good.
pub fn delete(p: &Paths, qid: &str) -> F<()> {
    find(p, qid)?;
    py::rmtree(&py::join(&p.data, qid))?;
    Ok(())
}

fn str_of(info: &Map<String, Value>, key: &str) -> R<String> {
    info.get(key).map(py::pyjson::py_str).ok_or_else(|| Crash(format!("KeyError: {}", py::repr(key))))
}

fn run(argv: &[String], p: &Paths) -> F<i32> {
    let cmd = argv[1].as_str();
    if cmd == "add" && argv.len() >= 3 {
        let sig = match argv.iter().position(|a| a == "--signature") {
            Some(i) => argv.get(i + 1).cloned().ok_or_else(|| Fail::Crash(Crash("IndexError: list index out of range".into())))?,
            None => "unknown".into(),
        };
        println!("{}", add(p, &argv[2], &sig, "scan")?);
    } else if cmd == "list" {
        let lst = items(p).map_err(Fail::Crash)?;
        if argv.iter().any(|a| a == "--json") {
            let v = Value::Array(lst.into_iter().map(Value::Object).collect());
            println!("{}", py::pyjson::dumps(&v));
        } else {
            for i in &lst {
                let row = (|| -> R<String> {
                    Ok(format!("{}  {}  {}  {}", str_of(i, "id")?, str_of(i, "when")?, str_of(i, "signature")?, str_of(i, "original")?))
                })()
                .map_err(Fail::Crash)?;
                println!("{row}");
            }
        }
    } else if cmd == "restore" && argv.len() == 3 {
        println!("{}", restore(p, &argv[2])?);
    } else if cmd == "delete" && argv.len() == 3 {
        delete(p, &argv[2])?;
    } else if cmd == "allowed" && argv.len() == 3 {
        return Ok(if allowed(p, &argv[2])? { 0 } else { 1 });
    } else {
        eprintln!("labyrinth-quarantine: bad arguments (see --help)");
        return Ok(2);
    }
    Ok(0)
}

/// The program: argv[0] is its name.
pub fn main(argv: &[String]) -> i32 {
    if argv.len() < 2 || argv[1] == "-h" || argv[1] == "--help" {
        println!("{DOC}");
        return if argv.len() >= 2 { 0 } else { 2 };
    }
    let p = Paths::from_env();
    match run(argv, &p) {
        Ok(rc) => rc,
        Err(Fail::Said(m)) => {
            eprintln!("labyrinth-quarantine: {m}");
            1
        }
        Err(Fail::Crash(c)) => py::crashed("labyrinth-quarantine", &c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_as_the_pattern_reads_them() {
        assert!(valid_id("0123456789ab"));
        assert!(valid_id("0123456789ab\n"), "Python's $ matches before a final newline");
        for bad in ["../../etc", "zzzz", "", "123456789012x", "0123456789AB", "0123456789ab\n\n"] {
            assert!(!valid_id(bad), "{bad:?}");
        }
    }
}
