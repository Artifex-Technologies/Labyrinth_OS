//! How the Python scripts met the outside, so the ports meet it the same way: os.path's answers (join, dirname, basename,
//! expanduser, realpath, abspath, isdir...), files read as text (strictly UTF-8, universal newlines), commands run with their
//! output captured as text, the local time as time.strftime writes it, shutil's move and rmtree, shutil.which, shlex, and
//! what happened where a script stopped with an error (a traceback and exit code 1): a Crash.

use std::ffi::{CStr, CString, OsStr, OsString};
use std::io::{self, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

pub use crate::pycompat::json::{self as pyjson, Map, Value};
pub use crate::pycompat::{repr, repr_list, split, splitlines, strip};

/// Where a script would have stopped with a traceback: its exit code is 1, its message is on standard error.
#[derive(Debug, Clone, PartialEq)]
pub struct Crash(pub String);

impl From<io::Error> for Crash {
    fn from(e: io::Error) -> Crash {
        Crash(oserror(&e, None))
    }
}

pub type R<T> = Result<T, Crash>;

/// Prints what a script's traceback would have ended with and gives its exit code.
pub fn crashed(prog: &str, c: &Crash) -> i32 {
    eprintln!("{prog}: unexpected error (the Python script stopped here with a traceback): {}", c.0);
    1
}

/// str(OSError) as Python writes it: "[Errno 2] No such file or directory: 'path'".
pub fn oserror(e: &io::Error, path: Option<&str>) -> String {
    let code = e.raw_os_error().unwrap_or(0);
    let text = if code != 0 { strerror(code) } else { e.to_string() };
    match path {
        Some(p) => format!("[Errno {code}] {text}: {}", repr(p)),
        None => format!("[Errno {code}] {text}"),
    }
}

/// str(OSError) for a call on two paths (os.rename): "[Errno 18] Invalid cross-device link: 'a' -> 'b'".
pub fn oserror2(e: &io::Error, a: &str, b: &str) -> String {
    format!("{} -> {}", oserror(e, Some(a)), repr(b))
}

/// The C library's text for an error number (what Python puts in an OSError).
pub fn strerror(code: i32) -> String {
    unsafe { CStr::from_ptr(libc::strerror(code)).to_string_lossy().into_owned() }
}

/// An OSError with the path it was about.
pub fn os_err(e: io::Error, path: &str) -> Crash {
    Crash(oserror(&e, Some(path)))
}

// ---- text

/// Text read in universal-newline mode: \r\n and a lone \r become \n.
pub fn universal(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// The lines of a text as `for line in f` gives them: each with its \n (the last one may have none).
pub fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// Why a file could not be read as text: an OSError, or bytes that are not UTF-8 (a UnicodeDecodeError, which Python counts
/// as a ValueError).
#[derive(Debug)]
pub enum ReadError {
    Os(io::Error),
    Decode(String),
}

impl ReadError {
    /// The message Python's exception would have (str(e)).
    pub fn message(&self, path: &str) -> String {
        match self {
            ReadError::Os(e) => oserror(e, Some(path)),
            ReadError::Decode(m) => m.clone(),
        }
    }
}

/// str(UnicodeDecodeError) for bytes that are not UTF-8: "'utf-8' codec can't decode byte 0xff in position 3: invalid start
/// byte". (Python counts positions within the 8 KB piece it was decoding; for a longer file the position may differ.)
pub fn decode_error(bytes: &[u8], e: &std::str::Utf8Error) -> String {
    let at = e.valid_up_to();
    match e.error_len() {
        None => {
            let end = bytes.len() - 1;
            if end == at {
                format!("'utf-8' codec can't decode byte 0x{:02x} in position {at}: unexpected end of data", bytes[at])
            } else {
                format!("'utf-8' codec can't decode bytes in position {at}-{end}: unexpected end of data")
            }
        }
        Some(_) => {
            let b = bytes[at];
            let why = if (0x80..=0xc1).contains(&b) || b >= 0xf5 { "invalid start byte" } else { "invalid continuation byte" };
            format!("'utf-8' codec can't decode byte 0x{b:02x} in position {at}: {why}")
        }
    }
}

/// UTF-8 text from bytes, strictly (the Err is str(UnicodeDecodeError)).
pub fn utf8(bytes: Vec<u8>) -> Result<String, String> {
    match String::from_utf8(bytes) {
        Ok(t) => Ok(t),
        Err(e) => Err(decode_error(e.as_bytes(), &e.utf8_error())),
    }
}

/// open(path).read(): strictly UTF-8, universal newlines.
pub fn read_text(path: &str) -> Result<String, ReadError> {
    let bytes = std::fs::read(path).map_err(ReadError::Os)?;
    utf8(bytes).map(|t| universal(&t)).map_err(ReadError::Decode)
}

/// open(path, errors="replace").read(): bad bytes become U+FFFD, universal newlines.
pub fn read_text_lossy(path: &str) -> io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(universal(&String::from_utf8_lossy(&bytes)))
}

/// json.load(open(path)): Err(None) for an OSError, Err(Some(message)) for text that is not UTF-8 or not JSON (both
/// ValueErrors in Python).
pub fn load_json(path: &str) -> Result<Value, Option<String>> {
    let text = match read_text(path) {
        Ok(t) => t,
        Err(ReadError::Os(_)) => return Err(None),
        Err(ReadError::Decode(m)) => return Err(Some(m)),
    };
    pyjson::loads(&text).map_err(Some)
}

/// text[:n] in characters.
pub fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// str.partition(sep)
pub fn partition<'a>(s: &'a str, sep: &str) -> (&'a str, bool, &'a str) {
    match s.find(sep) {
        Some(i) => (&s[..i], true, &s[i + sep.len()..]),
        None => (s, false, ""),
    }
}

/// str.rpartition(sep): when sep is not there, the whole text is the last part.
pub fn rpartition<'a>(s: &'a str, sep: &str) -> (&'a str, bool, &'a str) {
    match s.rfind(sep) {
        Some(i) => (&s[..i], true, &s[i + sep.len()..]),
        None => ("", false, s),
    }
}

/// str.title(): the first letter of each run of letters upper (title) case, the rest lower case.
pub fn title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_cased = false;
    for c in s.chars() {
        if prev_cased {
            out.extend(c.to_lowercase());
        } else {
            out.push_str(&titlecase(c));
        }
        prev_cased = c.is_lowercase() || c.is_uppercase() || is_titlecase(c);
    }
    out
}

fn is_titlecase(c: char) -> bool {
    matches!(c, '\u{1c5}' | '\u{1c8}' | '\u{1cb}' | '\u{1f2}' | '\u{1f88}'..='\u{1f8f}' | '\u{1f98}'..='\u{1f9f}' | '\u{1fa8}'..='\u{1faf}' | '\u{1fbc}' | '\u{1fcc}' | '\u{1ffc}')
}

fn titlecase(c: char) -> String {
    // the few letters whose title case is not their upper case (Unicode's titlecase mappings)
    match c {
        '\u{1c4}'..='\u{1c6}' => "\u{1c5}".into(),
        '\u{1c7}'..='\u{1c9}' => "\u{1c8}".into(),
        '\u{1ca}'..='\u{1cc}' => "\u{1cb}".into(),
        '\u{1f1}'..='\u{1f3}' => "\u{1f2}".into(),
        '\u{df}' => "Ss".into(),
        '\u{fb00}' => "Ff".into(),
        '\u{fb01}' => "Fi".into(),
        '\u{fb02}' => "Fl".into(),
        '\u{fb03}' => "Ffi".into(),
        '\u{fb04}' => "Ffl".into(),
        '\u{fb05}' | '\u{fb06}' => "St".into(),
        '\u{587}' => "\u{535}\u{582}".into(),
        _ => c.to_uppercase().collect(),
    }
}

// ---- the environment

/// os.environ.get(name) as text (None when unset; set but empty stays empty).
pub fn env(name: &str) -> Option<String> {
    std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}

/// os.environ.get(name) or default: an empty value counts as unset.
pub fn env_or(name: &str, default: &str) -> String {
    match env(name) {
        Some(v) if !v.is_empty() => v,
        _ => default.to_string(),
    }
}

/// os.environ.get(name, default): only an unset variable takes the default.
pub fn env_get(name: &str, default: &str) -> String {
    env(name).unwrap_or_else(|| default.to_string())
}

/// bool(os.environ.get(name)): set and not empty.
pub fn env_set(name: &str) -> bool {
    std::env::var_os(name).map(|v| !v.is_empty()).unwrap_or(false)
}

pub fn getuid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn geteuid() -> u32 {
    unsafe { libc::geteuid() }
}

/// A user's home folder from the password database.
fn pw_home(name: Option<&str>) -> Option<String> {
    unsafe {
        let pw = match name {
            None => libc::getpwuid(libc::getuid()),
            Some(n) => {
                let c = CString::new(n).ok()?;
                libc::getpwnam(c.as_ptr())
            }
        };
        if pw.is_null() || (*pw).pw_dir.is_null() {
            return None;
        }
        Some(CStr::from_ptr((*pw).pw_dir).to_string_lossy().into_owned())
    }
}

/// os.path.expanduser(path)
pub fn expanduser(path: &str) -> String {
    if !path.starts_with('~') {
        return path.to_string();
    }
    let i = path[1..].find('/').map(|i| i + 1).unwrap_or(path.len());
    let home = if i == 1 {
        match env("HOME") {
            Some(h) => h,
            None => match pw_home(None) {
                Some(h) => h,
                None => return path.to_string(),
            },
        }
    } else {
        match pw_home(Some(&path[1..i])) {
            Some(h) => h,
            None => return path.to_string(),
        }
    };
    let home = home.trim_end_matches('/');
    let out = format!("{home}{}", &path[i..]);
    if out.is_empty() {
        "/".into()
    } else {
        out
    }
}

/// os.path.expanduser("~")
pub fn home() -> String {
    expanduser("~")
}

// ---- os.path

/// os.path.join(a, b...)
pub fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        b.to_string()
    } else if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

pub fn join_all(parts: &[&str]) -> String {
    let mut out = parts[0].to_string();
    for p in &parts[1..] {
        out = join(&out, p);
    }
    out
}

/// os.path.dirname(p)
pub fn dirname(p: &str) -> String {
    match p.rfind('/') {
        None => String::new(),
        Some(i) => {
            let head = &p[..i + 1];
            let trimmed = head.trim_end_matches('/');
            if trimmed.is_empty() {
                head.to_string()
            } else {
                trimmed.to_string()
            }
        }
    }
}

/// os.path.basename(p)
pub fn basename(p: &str) -> &str {
    match p.rfind('/') {
        None => p,
        Some(i) => &p[i + 1..],
    }
}

/// os.path.split(p)
pub fn split_path(p: &str) -> (String, String) {
    (dirname(p), basename(p).to_string())
}

pub fn isabs(p: &str) -> bool {
    p.starts_with('/')
}

pub fn getcwd() -> String {
    std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// os.path.abspath(p)
pub fn abspath(p: &str) -> String {
    if isabs(p) {
        crate::pycompat::normpath(p)
    } else {
        crate::pycompat::normpath(&join(&getcwd(), p))
    }
}

/// os.path.exists(p): follows links; any error is False.
pub fn exists(p: impl AsRef<Path>) -> bool {
    std::fs::metadata(p).is_ok()
}

/// os.path.lexists(p)
pub fn lexists(p: impl AsRef<Path>) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

/// os.path.isdir(p)
pub fn isdir(p: impl AsRef<Path>) -> bool {
    std::fs::metadata(p).map(|m| m.is_dir()).unwrap_or(false)
}

/// os.path.isfile(p)
pub fn isfile(p: impl AsRef<Path>) -> bool {
    std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

/// os.path.islink(p)
pub fn islink(p: impl AsRef<Path>) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

/// os.path.realpath(p) (strict=False): links resolved as far as they go, the parts that do not exist kept as they are.
pub fn realpath(p: &str) -> String {
    let mut seen: Vec<(String, Option<String>)> = Vec::new();
    let (path, _) = joinrealpath(String::new(), p, &mut seen);
    abspath(&path)
}

fn joinrealpath(mut path: String, rest: &str, seen: &mut Vec<(String, Option<String>)>) -> (String, bool) {
    let mut rest = rest.to_string();
    if isabs(&rest) {
        rest = rest[1..].to_string();
        path = "/".into();
    }
    while !rest.is_empty() {
        let (name, _, r) = partition(&rest, "/");
        let (name, r) = (name.to_string(), r.to_string());
        rest = r;
        if name.is_empty() || name == "." {
            continue;
        }
        if name == ".." {
            if !path.is_empty() {
                let (head, tail) = split_path(&path);
                path = head;
                if tail == ".." {
                    path = join(&join(&path, ".."), "..");
                }
            } else {
                path = "..".into();
            }
            continue;
        }
        let newpath = join(&path, &name);
        let is_link = std::fs::symlink_metadata(&newpath).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if !is_link {
            path = newpath;
            continue;
        }
        if let Some((_, known)) = seen.iter().find(|(k, _)| *k == newpath) {
            match known {
                Some(resolved) => {
                    path = resolved.clone();
                    continue;
                }
                None => return (join(&newpath, &rest), false),
            }
        }
        seen.push((newpath.clone(), None));
        let target = match std::fs::read_link(&newpath) {
            Ok(t) => t.to_string_lossy().into_owned(),
            Err(_) => {
                path = newpath;
                continue;
            }
        };
        let (p, ok) = joinrealpath(path, &target, seen);
        if !ok {
            return (join(&p, &rest), false);
        }
        for (k, v) in seen.iter_mut() {
            if *k == newpath {
                *v = Some(p.clone());
            }
        }
        path = p;
    }
    (path, true)
}

/// os.makedirs(path, exist_ok=True): the folders that are missing are made (0o777 less the umask).
pub fn makedirs(path: &str) -> R<()> {
    makedirs_mode(path, 0o777, true)
}

/// os.makedirs(path, mode, exist_ok): `mode` for the last folder only, as Python does since 3.7.
pub fn makedirs_mode(path: &str, mode: u32, exist_ok: bool) -> R<()> {
    let (head, tail) = split_path(path);
    let (head, tail) = if tail.is_empty() { split_path(&head) } else { (head, tail) };
    if !head.is_empty() && !tail.is_empty() && !exists(&head) {
        match makedirs_mode(&head, 0o777, true) {
            Ok(()) => {}
            Err(_) if lexists(&head) => {}
            Err(e) => return Err(e),
        }
    }
    use std::os::unix::fs::DirBuilderExt;
    match std::fs::DirBuilder::new().mode(mode).create(path) {
        Ok(()) => Ok(()),
        Err(e) => {
            if exist_ok && isdir(path) {
                Ok(())
            } else {
                Err(os_err(e, path))
            }
        }
    }
}

/// os.replace(src, dst)
pub fn replace(src: &str, dst: &str) -> R<()> {
    std::fs::rename(src, dst).map_err(|e| Crash(oserror2(&e, src, dst)))
}

/// os.chmod(path, mode)
pub fn chmod(path: &str, mode: u32) -> R<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|e| os_err(e, path))
}

/// open(path, "w").write(text) (0o666 less the umask for a new file).
pub fn write_file(path: &str, text: &str) -> R<()> {
    std::fs::write(path, text).map_err(|e| os_err(e, path))
}

/// open(path, "a").write(text)
pub fn append_file(path: &str, text: &str) -> R<()> {
    let mut f = std::fs::OpenOptions::new().append(true).create(true).open(path).map_err(|e| os_err(e, path))?;
    f.write_all(text.as_bytes()).map_err(|e| os_err(e, path))
}

/// shutil.copystat's part that matters here, after the content: the times (to the nanosecond), the mode, the extended
/// attributes (those the target cannot take are skipped, as shutil does).
fn copystat(src: &str, dst: &str) -> R<()> {
    let st = std::fs::metadata(src).map_err(|e| os_err(e, src))?;
    let c = CString::new(dst).map_err(|_| Crash(format!("embedded null byte: {}", repr(dst))))?;
    let times = [
        libc::timespec { tv_sec: st.atime() as _, tv_nsec: st.atime_nsec() as _ },
        libc::timespec { tv_sec: st.mtime() as _, tv_nsec: st.mtime_nsec() as _ },
    ];
    if unsafe { libc::utimensat(libc::AT_FDCWD, c.as_ptr(), times.as_ptr(), 0) } != 0 {
        return Err(os_err(io::Error::last_os_error(), dst));
    }
    copy_xattrs(src, dst)?;
    chmod(dst, st.mode() & 0o7777)
}

fn copy_xattrs(src: &str, dst: &str) -> R<()> {
    let s = CString::new(src).unwrap_or_default();
    let d = CString::new(dst).unwrap_or_default();
    let n = unsafe { libc::listxattr(s.as_ptr(), std::ptr::null_mut(), 0) };
    if n <= 0 {
        return Ok(());
    }
    let mut names = vec![0u8; n as usize];
    let n = unsafe { libc::listxattr(s.as_ptr(), names.as_mut_ptr() as *mut libc::c_char, names.len()) };
    if n <= 0 {
        return Ok(());
    }
    for name in names[..n as usize].split(|b| *b == 0).filter(|x| !x.is_empty()) {
        let cname = CString::new(name).unwrap_or_default();
        let len = unsafe { libc::getxattr(s.as_ptr(), cname.as_ptr(), std::ptr::null_mut(), 0) };
        if len < 0 {
            continue;
        }
        let mut val = vec![0u8; len as usize];
        let len = unsafe { libc::getxattr(s.as_ptr(), cname.as_ptr(), val.as_mut_ptr() as *mut libc::c_void, val.len()) };
        if len < 0 {
            continue;
        }
        if unsafe { libc::setxattr(d.as_ptr(), cname.as_ptr(), val.as_ptr() as *const libc::c_void, len as usize, 0) } != 0 {
            let e = io::Error::last_os_error();
            match e.raw_os_error() {
                Some(libc::EPERM) | Some(libc::ENOTSUP) | Some(libc::ENODATA) | Some(libc::EINVAL) | Some(libc::EACCES) => {}
                _ => return Err(os_err(e, dst)),
            }
        }
    }
    Ok(())
}

/// shutil.copy2(src, dst) for a file: the content, then its times, extended attributes and mode.
pub fn copy2(src: &str, dst: &str) -> R<()> {
    let mut f = std::fs::File::open(src).map_err(|e| os_err(e, src))?;
    let mut g = std::fs::File::create(dst).map_err(|e| os_err(e, dst))?;
    io::copy(&mut f, &mut g).map_err(|e| Crash(oserror2(&e, src, dst)))?;
    drop(g);
    copystat(src, dst)
}

/// shutil.move(src, dst) for a file and a target that is not a folder: a rename; when that fails for any reason (another file
/// system, a folder that cannot be written), a copy with its times and mode, then the original removed.
pub fn move_file(src: &str, dst: &str) -> R<()> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    copy2(src, dst)?;
    std::fs::remove_file(src).map_err(|e| os_err(e, src))
}

/// shutil.rmtree(path) as Python 3.14 does it: in each folder the files are removed first (in the folder's own order), then its
/// sub-folders, the last found first; the error names the folder being emptied; something already gone below the top is no
/// error.
pub fn rmtree(path: &str) -> R<()> {
    enum Step {
        Walk(String, bool),
        Rmdir(String, bool),
    }
    let mut stack = vec![Step::Walk(path.to_string(), true)];
    while let Some(step) = stack.pop() {
        match step {
            Step::Rmdir(p, top) => match std::fs::remove_dir(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound && !top => {}
                Err(e) => return Err(os_err(e, &p)),
            },
            Step::Walk(p, top) => {
                let gone = |e: &io::Error| e.kind() == io::ErrorKind::NotFound && !top;
                match std::fs::symlink_metadata(&p) {
                    Ok(m) if m.file_type().is_symlink() => return Err(Crash(format!("[Errno None] None: {}", repr(&p)))),
                    Ok(_) => {}
                    Err(e) if gone(&e) => continue,
                    Err(e) => return Err(os_err(e, &p)),
                }
                let rd = match std::fs::read_dir(&p) {
                    Ok(r) => r,
                    Err(e) if gone(&e) => continue,
                    Err(e) => return Err(os_err(e, &p)),
                };
                stack.push(Step::Rmdir(p.clone(), top));
                let mut entries = vec![];
                for e in rd {
                    entries.push(e.map_err(|e| os_err(e, &p))?);
                }
                for e in entries {
                    match e.file_type() {
                        Ok(t) if t.is_dir() => {
                            stack.push(Step::Walk(join(&p, &e.file_name().to_string_lossy()), false));
                            continue;
                        }
                        Err(er) if er.kind() == io::ErrorKind::NotFound => continue,
                        _ => {}
                    }
                    match std::fs::remove_file(e.path()) {
                        Ok(()) => {}
                        Err(er) if er.kind() == io::ErrorKind::NotFound => {}
                        Err(er) => return Err(os_err(er, &p)),
                    }
                }
            }
        }
    }
    Ok(())
}

/// shutil.rmtree(path, ignore_errors=True)
pub fn rmtree_quiet(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

/// shutil.which(name): a name with a slash is checked itself, else each folder of PATH (os.defpath when it is unset).
pub fn which(name: &str) -> Option<String> {
    fn ok(p: &str) -> bool {
        let c = match CString::new(p) {
            Ok(c) => c,
            Err(_) => return false,
        };
        exists(p) && !isdir(p) && unsafe { libc::access(c.as_ptr(), libc::X_OK) } == 0
    }
    if name.contains('/') {
        return if ok(name) { Some(name.to_string()) } else { None };
    }
    let path = env("PATH").unwrap_or_else(|| "/bin:/usr/bin".into());
    if path.is_empty() {
        return None;
    }
    let mut seen: Vec<String> = vec![];
    for d in path.split(':') {
        let norm = d.to_string();
        if seen.contains(&norm) {
            continue;
        }
        seen.push(norm);
        let p = join(d, name);
        if ok(&p) {
            return Some(p);
        }
    }
    None
}

// ---- time

extern "C" {
    fn tzset();
}

pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// time.strftime(fmt) in local time.
pub fn strftime(fmt: &str) -> String {
    strftime_at(fmt, now() as i64)
}

pub fn strftime_at(fmt: &str, secs: i64) -> String {
    let t: libc::time_t = secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        tzset();
        libc::localtime_r(&t, &mut tm);
    }
    let c = CString::new(fmt).unwrap_or_default();
    let mut buf = vec![0u8; 256];
    let n = unsafe { libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), c.as_ptr(), &tm) };
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// time.mktime(time.strptime(text, fmt)) for "%Y-%m-%dT%H:%M:%S" already split into numbers: the local time's epoch seconds,
/// daylight saving time left for the C library to decide (tm_isdst = -1), as Python does.
pub fn mktime(year: i32, mon: i32, day: i32, h: i32, m: i32, s: i32) -> f64 {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = year - 1900;
    tm.tm_mon = mon - 1;
    tm.tm_mday = day;
    tm.tm_hour = h;
    tm.tm_min = m;
    tm.tm_sec = s;
    tm.tm_isdst = -1;
    unsafe {
        tzset();
        libc::mktime(&mut tm) as f64
    }
}

/// Six random bytes as twelve hex digits (what uuid.uuid4().hex[:12] gives: those digits of a version-4 id are all random).
pub fn random_hex12() -> String {
    let mut b = [0u8; 6];
    let mut got = 0;
    while got < b.len() {
        let n = unsafe { libc::getrandom(b[got..].as_mut_ptr() as *mut libc::c_void, b.len() - got, 0) };
        if n > 0 {
            got += n as usize;
        }
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// ---- commands

/// What subprocess.run(..., capture_output=True, text=True) gives back.
#[derive(Debug, Default, Clone)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// How a command could not finish: it could not start (an OSError), or it ran past its time (TimeoutExpired).
#[derive(Debug)]
pub enum RunError {
    Os(String),
    Timeout(String),
}

impl RunError {
    pub fn message(&self) -> &str {
        match self {
            RunError::Os(m) | RunError::Timeout(m) => m,
        }
    }
}

fn status_code(st: std::process::ExitStatus) -> i32 {
    st.code().unwrap_or_else(|| -(st.signal().unwrap_or(1)))
}

fn start_error(e: &io::Error, prog: &OsStr) -> String {
    oserror(e, Some(&prog.to_string_lossy()))
}

/// The options of a command the scripts ran.
#[derive(Default)]
pub struct Run<'a> {
    /// written to its standard input (else it gets none: /dev/null)
    pub input: Option<&'a str>,
    /// keep this process's standard input (Python's default when no input was given)
    pub inherit_stdin: bool,
    /// subprocess's timeout=
    pub timeout: Option<Duration>,
    /// how Python wrote the timeout in its message ("1800")
    pub timeout_text: Option<String>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
}

/// subprocess.run(argv, capture_output=True, text=True, ...): standard output and error as text (bytes that are not UTF-8
/// replaced, universal newlines). A program that cannot start is RunError::Os with Python's message; one that runs past its
/// timeout is killed and is RunError::Timeout.
pub fn run_opts<S: AsRef<OsStr>>(argv: &[S], o: Run) -> Result<Output, RunError> {
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..]).stdout(Stdio::piped()).stderr(Stdio::piped());
    c.stdin(if o.input.is_some() {
        Stdio::piped()
    } else if o.inherit_stdin {
        Stdio::inherit()
    } else {
        Stdio::null()
    });
    if let Some(d) = &o.cwd {
        c.current_dir(d);
    }
    for (k, v) in &o.env {
        c.env(k, v);
    }
    let mut child = c.spawn().map_err(|e| RunError::Os(start_error(&e, argv[0].as_ref())))?;
    if let Some(text) = o.input {
        let mut si = child.stdin.take().unwrap();
        let text = text.to_string();
        std::thread::spawn(move || {
            let _ = si.write_all(text.as_bytes());
        });
    }
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let t1 = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = so.read_to_end(&mut b);
        b
    });
    let t2 = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = se.read_to_end(&mut b);
        b
    });
    let status = match o.timeout {
        None => child.wait().map_err(|e| RunError::Os(oserror(&e, None)))?,
        Some(limit) => {
            // a thread waits for it, so nothing here wakes up until it ends or its time is up
            let pid = child.id() as libc::pid_t;
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(child.wait());
            });
            match rx.recv_timeout(limit) {
                Ok(st) => st.map_err(|e| RunError::Os(oserror(&e, None)))?,
                Err(_) => {
                    unsafe { libc::kill(pid, libc::SIGKILL) };
                    let _ = rx.recv();
                    let shown: Vec<String> = argv.iter().map(|a| a.as_ref().to_string_lossy().into_owned()).collect();
                    let secs = o.timeout_text.clone().unwrap_or_else(|| format!("{}", limit.as_secs()));
                    return Err(RunError::Timeout(format!("Command {} timed out after {secs} seconds", repr_list(&shown))));
                }
            }
        }
    };
    let out = t1.join().unwrap_or_default();
    let err = t2.join().unwrap_or_default();
    Ok(Output {
        code: status_code(status),
        stdout: universal(&String::from_utf8_lossy(&out)),
        stderr: universal(&String::from_utf8_lossy(&err)),
    })
}

/// subprocess.run(argv, capture_output=True[, text=True]): a program that cannot start is a Crash (the scripts did not catch
/// that OSError).
pub fn run<S: AsRef<OsStr>>(argv: &[S]) -> R<Output> {
    run_opts(argv, Run { inherit_stdin: true, ..Default::default() }).map_err(|e| Crash(e.message().to_string()))
}

/// subprocess.run(argv).returncode: the command's output goes where ours does.
pub fn call<S: AsRef<OsStr>>(argv: &[S]) -> R<i32> {
    let st = Command::new(&argv[0]).args(&argv[1..]).status().map_err(|e| Crash(start_error(&e, argv[0].as_ref())))?;
    Ok(status_code(st))
}

/// Children started and not waited for (Popen without wait): reaped now and then, as Python's subprocess does, so they do not
/// stay behind as zombies in a long-running program.
static BACKGROUND: Mutex<Vec<Child>> = Mutex::new(Vec::new());

/// Reap the background children that have ended.
pub fn reap() {
    if let Ok(mut v) = BACKGROUND.lock() {
        v.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
    }
}

/// subprocess.Popen(argv, stdout=DEVNULL, stderr=DEVNULL, start_new_session=True[, env=...]): started in a session of its own,
/// not waited for. Standard input is ours (Popen's default).
pub fn spawn_detached<S: AsRef<OsStr>>(argv: &[S], env: &[(&str, &str)]) -> R<()> {
    reap();
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..]).stdout(Stdio::null()).stderr(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let child = c.spawn().map_err(|e| Crash(start_error(&e, argv[0].as_ref())))?;
    if let Ok(mut v) = BACKGROUND.lock() {
        v.push(child);
    }
    Ok(())
}

/// The folder this program was started from (the scripts' os.path.dirname(os.path.abspath(__file__))).
pub fn here() -> String {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned())).unwrap_or_default()
}

/// The program's arguments as text (bytes that are not UTF-8 replaced).
pub fn args() -> Vec<String> {
    std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect()
}

/// The program's arguments as given.
pub fn args_os() -> Vec<OsString> {
    std::env::args_os().collect()
}

pub fn os_bytes(s: &OsStr) -> &[u8] {
    s.as_bytes()
}

pub fn os_from_bytes(b: Vec<u8>) -> OsString {
    OsString::from_vec(b)
}

// ---- shlex

/// shlex.quote(s)
pub fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    let safe = s.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if safe {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

/// shlex.split(s) (POSIX mode, no comments). Err: Python's ValueError ("No closing quotation", "No escaped character").
pub fn shlex_split(s: &str) -> Result<Vec<String>, String> {
    // the state machine of shlex.shlex.read_token with posix=True, whitespace_split=True, commenters=""
    #[derive(PartialEq, Clone, Copy)]
    enum St {
        Space,
        Word,
        Quote(char),
        Escape(char),
    }
    let ws = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
    let mut out = vec![];
    let mut state = St::Space;
    let mut token = String::new();
    let mut chars = s.chars();
    loop {
        let next = chars.next();
        match state {
            St::Space => match next {
                None => break,
                Some(c) if ws(c) => {}
                Some('\\') => {
                    state = St::Escape('a');
                }
                Some(c) if c == '\'' || c == '"' => {
                    state = St::Quote(c);
                }
                Some(c) => {
                    token.push(c);
                    state = St::Word;
                }
            },
            St::Word => match next {
                None => {
                    out.push(std::mem::take(&mut token));
                    break;
                }
                Some(c) if ws(c) => {
                    out.push(std::mem::take(&mut token));
                    state = St::Space;
                }
                Some('\\') => state = St::Escape('a'),
                Some(c) if c == '\'' || c == '"' => state = St::Quote(c),
                Some(c) => token.push(c),
            },
            St::Quote(q) => match next {
                None => return Err("No closing quotation".into()),
                Some(c) if c == q => {
                    state = St::Word;
                }
                Some('\\') if q == '"' => state = St::Escape(q),
                Some(c) => token.push(c),
            },
            St::Escape(from) => match next {
                None => return Err("No escaped character".into()),
                Some(c) => {
                    // inside double quotes only a quote or a backslash is escaped; elsewhere the backslash just goes
                    if from == '"' && c != '"' && c != '\\' {
                        token.push('\\');
                    }
                    token.push(c);
                    state = if from == 'a' { St::Word } else { St::Quote(from) };
                }
            },
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shlex_as_python() {
        assert_eq!(shlex_split("a 'b c' \"d \\\" e\" f\\ g").unwrap(), vec!["a", "b c", "d \" e", "f g"]);
        assert_eq!(shlex_split("''").unwrap(), vec![""]);
        assert_eq!(shlex_split("x\"\"").unwrap(), vec!["x"]);
        assert_eq!(shlex_split("\"a\\b\"").unwrap(), vec!["a\\b"]);
        assert_eq!(shlex_split("  ").unwrap(), Vec::<String>::new());
        assert!(shlex_split("'a").is_err());
        assert!(shlex_split("a\\").is_err());
        assert_eq!(shlex_quote("a b"), "'a b'");
        assert_eq!(shlex_quote("it's"), "'it'\"'\"'s'");
        assert_eq!(shlex_quote("/usr/bin/x-1"), "/usr/bin/x-1");
        assert_eq!(shlex_quote(""), "''");
    }

    #[test]
    fn paths_as_python() {
        assert_eq!(dirname("/a/b"), "/a");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(dirname("a"), "");
        assert_eq!(basename("/a/b/"), "");
        assert_eq!(join("/a", "/b"), "/b");
        assert_eq!(realpath("/nonexistent-x/../y"), "/y");
        assert_eq!(rpartition("a: b: c", ": "), ("a: b", true, "c"));
        assert_eq!(rpartition("abc", ": "), ("", false, "abc"));
        assert_eq!(title("spotify launcher"), "Spotify Launcher");
        assert_eq!(title("pdfcraft2go x"), "Pdfcraft2Go X");
    }

    #[test]
    fn decode_errors_as_python() {
        let b = b"ab\xffc".to_vec();
        assert_eq!(utf8(b).unwrap_err(), "'utf-8' codec can't decode byte 0xff in position 2: invalid start byte");
        assert_eq!(
            utf8(b"a\xe2\x28".to_vec()).unwrap_err(),
            "'utf-8' codec can't decode byte 0xe2 in position 1: invalid continuation byte"
        );
        assert_eq!(utf8(b"a\xe2\x82".to_vec()).unwrap_err(), "'utf-8' codec can't decode bytes in position 1-2: unexpected end of data");
    }
}
