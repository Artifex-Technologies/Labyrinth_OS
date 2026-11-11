//! What the ports of the shell scripts share: programs started by argv (never a command line, so no quoting or word splitting),
//! the coreutils steps the scripts used done in-process (`install -d`, `sed -i` on whole lines, `>`, `>>`, `rm -f`, `$(cat)`,
//! `head -n1`), bash's order for the names a glob gives (strcoll, as bash, ls and sort use it), `mktemp`, and `date`'s time
//! stamps. Programs that do real system work (mount, btrfs, systemctl, pacman, cp -a ...) are still those programs.

use std::ffi::{CString, OsStr};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

static PROG: OnceLock<&'static str> = OnceLock::new();

/// The program's name, for messages ("labyrinth-rollback: ...").
pub fn set_prog(name: &'static str) {
    let _ = PROG.set(name);
}

pub fn prog() -> &'static str {
    PROG.get().copied().unwrap_or("labyrinth")
}

/// Where a program's output or errors go: as ours, or nowhere (>/dev/null).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Io {
    Inherit,
    Null,
}

fn stdio(i: Io) -> Stdio {
    match i {
        Io::Inherit => Stdio::inherit(),
        Io::Null => Stdio::null(),
    }
}

/// What bash says and returns when a program cannot be started: 127 when it is not there, 126 when it cannot be run.
fn not_started(name: &OsStr, e: &io::Error, err: Io) -> i32 {
    let code = if e.kind() == io::ErrorKind::NotFound { 127 } else { 126 };
    if err == Io::Inherit {
        let why = if code == 127 { "command not found".to_string() } else { e.to_string() };
        eprintln!("{}: {}: {why}", prog(), name.to_string_lossy());
    }
    code
}

fn code_of(st: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    st.code().unwrap_or_else(|| 128 + st.signal().unwrap_or(0))
}

fn command<S: AsRef<OsStr>>(argv: &[S]) -> Command {
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..]);
    c
}

/// Run a program with our standard input; its output and errors where asked. The exit code, as $? would be.
pub fn run<S: AsRef<OsStr>>(argv: &[S], out: Io, err: Io) -> i32 {
    let _ = io::stdout().flush();
    match command(argv).stdout(stdio(out)).stderr(stdio(err)).status() {
        Ok(st) => code_of(st),
        Err(e) => not_started(argv[0].as_ref(), &e, err),
    }
}

/// `cmd` (everything as ours): its exit code.
pub fn status<S: AsRef<OsStr>>(argv: &[S]) -> i32 {
    run(argv, Io::Inherit, Io::Inherit)
}

/// `cmd >/dev/null 2>&1`: did it succeed?
pub fn quiet<S: AsRef<OsStr>>(argv: &[S]) -> bool {
    run(argv, Io::Null, Io::Null) == 0
}

/// `$(cmd)` with its errors where asked: the exit code and the output as it came.
pub fn capture<S: AsRef<OsStr>>(argv: &[S], err: Io) -> (i32, Vec<u8>) {
    let _ = io::stdout().flush();
    let child = command(argv).stdout(Stdio::piped()).stderr(stdio(err)).spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (not_started(argv[0].as_ref(), &e, err), vec![]),
    };
    let mut out = vec![];
    if let Some(mut so) = child.stdout.take() {
        let _ = so.read_to_end(&mut out);
    }
    let code = child.wait().map(code_of).unwrap_or(1);
    (code, out)
}

/// `$(cmd 2>/dev/null)` as text: what bash's command substitution keeps (trailing newlines removed, NUL bytes dropped), with
/// bytes that are not UTF-8 replaced.
pub fn output<S: AsRef<OsStr>>(argv: &[S]) -> String {
    subst(&capture(argv, Io::Null).1)
}

/// What `$(...)` makes of a program's output: trailing newlines removed and NUL bytes dropped (bash drops them, with a warning).
pub fn subst(b: &[u8]) -> String {
    let kept: Vec<u8> = b.iter().copied().filter(|&c| c != 0).collect();
    let mut s = String::from_utf8_lossy(&kept).into_owned();
    while s.ends_with('\n') {
        s.pop();
    }
    s
}

/// `cmd &` in a script: started, not waited for, standard input from /dev/null (bash's rule for background commands without job
/// control), output and errors where asked.
pub fn background<S: AsRef<OsStr>>(argv: &[S], out: Io, err: Io) {
    let _ = io::stdout().flush();
    let _ = command(argv).stdin(Stdio::null()).stdout(stdio(out)).stderr(stdio(err)).spawn();
}

/// `exec cmd`: this process becomes the program (nothing of ours runs after). Returns only when it cannot be started, with
/// bash's exit code for that.
pub fn exec<S: AsRef<OsStr>>(argv: &[S]) -> i32 {
    let _ = io::stdout().flush();
    let e = command(argv).exec();
    not_started(argv[0].as_ref(), &e, Io::Inherit)
}

/// `command -v NAME`: the first file of that name in PATH that may be run (a name with a slash is itself checked).
pub fn which(name: &str) -> Option<String> {
    crate::py::which(name)
}

pub fn have(name: &str) -> bool {
    which(name).is_some()
}

// ---- tests of files, as [[ ]] does them

/// [[ -e P ]]
pub fn exists(p: &str) -> bool {
    fs::metadata(p).is_ok()
}

/// [[ -f P ]]
pub fn is_file(p: &str) -> bool {
    fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

/// [[ -d P ]]
pub fn is_dir(p: &str) -> bool {
    fs::metadata(p).map(|m| m.is_dir()).unwrap_or(false)
}

/// [[ -s P ]]: there, and not empty.
pub fn non_empty(p: &str) -> bool {
    fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false)
}

/// [[ -b P ]]
pub fn is_block(p: &str) -> bool {
    use std::os::unix::fs::FileTypeExt;
    fs::metadata(p).map(|m| m.file_type().is_block_device()).unwrap_or(false)
}

/// [[ -S P ]]
pub fn is_socket(p: &str) -> bool {
    use std::os::unix::fs::FileTypeExt;
    fs::metadata(p).map(|m| m.file_type().is_socket()).unwrap_or(false)
}

fn access(p: &str, how: i32) -> bool {
    let c = match CString::new(p) {
        Ok(c) => c,
        Err(_) => return false,
    };
    unsafe { libc::faccessat(libc::AT_FDCWD, c.as_ptr(), how, libc::AT_EACCESS) == 0 }
}

/// [[ -x P ]]
pub fn executable(p: &str) -> bool {
    access(p, libc::X_OK)
}

/// [[ -r P ]]
pub fn readable(p: &str) -> bool {
    access(p, libc::R_OK)
}

/// [[ -t 0 ]]
pub fn stdin_is_terminal() -> bool {
    unsafe { libc::isatty(0) == 1 }
}

pub fn euid() -> u32 {
    unsafe { libc::geteuid() }
}

// ---- reading

/// `$(cat P)` (errors to /dev/null): None when it cannot be read.
pub fn cat(p: &str) -> Option<String> {
    fs::read(p).ok().map(|b| subst(&b))
}

/// `$(head -n1 P)`: the first line; None when it cannot be read.
pub fn head1(p: &str) -> Option<String> {
    let b = fs::read(p).ok()?;
    let first = b.split(|&c| c == b'\n').next().unwrap_or(&[]);
    Some(subst(first))
}

/// The lines of a file as sed, grep and awk see them (split at \n; a last line without one is a line).
pub fn lines(b: &[u8]) -> Vec<&[u8]> {
    if b.is_empty() {
        return vec![];
    }
    let body = b.strip_suffix(b"\n").unwrap_or(b);
    body.split(|&c| c == b'\n').collect()
}

/// `grep -q` with a test on each line (no file: no match).
pub fn grep(p: &str, test: impl Fn(&[u8]) -> bool) -> bool {
    fs::read(p).map(|b| lines(&b).into_iter().any(test)).unwrap_or(false)
}

/// The line starts with these bytes.
pub fn starts(line: &[u8], prefix: &str) -> bool {
    line.starts_with(prefix.as_bytes())
}

/// The bytes are somewhere in the line.
pub fn contains(line: &[u8], what: &str) -> bool {
    let w = what.as_bytes();
    !w.is_empty() && line.windows(w.len()).any(|x| x == w)
}

/// [[:space:]] in the C locale (what grep's ^[[:space:]]* skips in these files).
pub fn space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}

/// The line with the blanks ([[:space:]]) at its start removed.
pub fn lstrip(line: &[u8]) -> &[u8] {
    let n = line.iter().take_while(|&&c| space(c)).count();
    &line[n..]
}

// ---- writing

/// `> P`: created with mode 0666 less the umask (an existing file keeps its mode), emptied, written; through a link at P as the
/// shell does.
pub fn write(p: &str, data: &[u8]) -> io::Result<()> {
    let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o666).open(p)?;
    f.write_all(data)
}

/// `>> P`
pub fn append(p: &str, data: &[u8]) -> io::Result<()> {
    let mut f = fs::OpenOptions::new().append(true).create(true).mode(0o666).open(p)?;
    f.write_all(data)
}

/// `echo LINE >> P`, but on a line of its own: when the file does not end with a newline, one is written first (the scripts'
/// `echo >>` glued the new setting to the end of the last line then).
pub fn append_line(p: &str, line: &str) -> io::Result<()> {
    let ends_open = fs::read(p).map(|b| !b.is_empty() && !b.ends_with(b"\n")).unwrap_or(false);
    let mut data = Vec::new();
    if ends_open {
        data.push(b'\n');
    }
    data.extend_from_slice(line.as_bytes());
    data.push(b'\n');
    append(p, &data)
}

/// A new file written whole in P's folder and renamed onto P: whoever reads P sees the old or the new one, never half of it, and
/// a link at P is replaced, not followed. Mode and owner as given (the umask does not apply).
pub fn replace_file(p: &str, data: &[u8], mode: u32, owner: Option<(u32, u32)>) -> io::Result<()> {
    let dir = match Path::new(p).parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => Path::new(".").to_path_buf(),
    };
    let base = Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (tmp, mut f) = mkstemp(&format!("{}/.{base}.XXXXXX", dir.display()))?;
    let done = (|| {
        f.write_all(data)?;
        if let Some((u, g)) = owner {
            std::os::unix::fs::fchown(&f, Some(u), Some(g))?;
        }
        f.set_permissions(fs::Permissions::from_mode(mode))?;
        f.sync_all()?;
        fs::rename(&tmp, p)
    })();
    if done.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    done
}

/// `sed -i` with a change on whole lines: each line (without its newline) is given to `edit`, which returns the new line or
/// None to keep it. Written as sed -i writes: a new file in the same folder with the old one's mode and owner, renamed onto it
/// (a link there is replaced by the file, as sed does without --follow-symlinks); a last line without a newline stays without.
/// Nothing is written when no line changed. Err: the file could not be read or written.
pub fn sed_i(p: &str, mut edit: impl FnMut(&[u8]) -> Option<Vec<u8>>) -> io::Result<bool> {
    let old = fs::read(p)?;
    let meta = fs::symlink_metadata(p)?;
    let meta = if meta.file_type().is_symlink() { fs::metadata(p)? } else { meta };
    let mut new = Vec::with_capacity(old.len() + 64);
    let mut changed = false;
    for piece in old.split_inclusive(|&c| c == b'\n') {
        let (line, nl) = match piece.strip_suffix(b"\n") {
            Some(l) => (l, true),
            None => (piece, false),
        };
        match edit(line) {
            Some(l) => {
                changed |= l != line;
                new.extend_from_slice(&l);
            }
            None => new.extend_from_slice(line),
        }
        if nl {
            new.push(b'\n');
        }
    }
    if changed {
        replace_file(p, &new, meta.mode() & 0o7777, Some((meta.uid(), meta.gid())))?;
    }
    Ok(changed)
}

/// `rm -f P` (a folder is an error, as for rm).
pub fn rm_f(p: &str) -> io::Result<()> {
    match fs::remove_file(p) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

/// `rm -rf P`: a folder with all it holds, a file, or a link (never what it points to); nothing there is no error.
pub fn rm_rf(p: &str) -> io::Result<()> {
    match fs::symlink_metadata(p) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(m) if m.is_dir() => fs::remove_dir_all(p),
        Ok(_) => fs::remove_file(p),
    }
}

/// `install -d [-m MODE] [-o USER -g GROUP] P`: the folders on the way that are missing are made with mode 0755 (whatever the
/// umask), then P is given MODE and the owner, also when it was already there (as install does).
pub fn install_dir(p: &str, mode: u32, owner: Option<(u32, u32)>) -> io::Result<()> {
    let path = Path::new(p);
    let mut missing = vec![];
    let mut cur = Some(path);
    while let Some(c) = cur {
        if c.as_os_str().is_empty() || fs::metadata(c).is_ok() {
            break;
        }
        missing.push(c.to_path_buf());
        cur = c.parent();
    }
    for d in missing.iter().rev() {
        match fs::create_dir(d) {
            Ok(()) => fs::set_permissions(d, fs::Permissions::from_mode(0o755))?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && is_dir(&d.to_string_lossy()) => {}
            Err(e) => return Err(e),
        }
    }
    if !fs::metadata(p)?.is_dir() {
        return Err(io::Error::from_raw_os_error(libc::ENOTDIR));
    }
    if let Some((u, g)) = owner {
        std::os::unix::fs::chown(p, Some(u), Some(g))?;
    }
    fs::set_permissions(p, fs::Permissions::from_mode(mode))
}

/// `mkdir -p P`: the missing folders made with 0777 less the umask.
pub fn mkdir_p(p: &str) -> io::Result<()> {
    fs::create_dir_all(p)
}

/// `mkdir -p -m MODE P`: the missing folders on the way with 0777 less the umask (and u+wx), P itself with MODE when it is made
/// (a folder already there keeps its mode).
pub fn mkdir_p_mode(p: &str, mode: u32) -> io::Result<()> {
    if is_dir(p) {
        return Ok(());
    }
    if let Some(parent) = Path::new(p).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    match fs::create_dir(p) {
        Ok(()) => fs::set_permissions(p, fs::Permissions::from_mode(mode)),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && is_dir(p) => Ok(()),
        Err(e) => Err(e),
    }
}

/// The ids of an account and a group by name (the system's own lookup, as install -o/-g and chown do).
pub fn user_id(name: &str) -> Option<(u32, u32)> {
    let c = CString::new(name).ok()?;
    let pw = unsafe { libc::getpwnam(c.as_ptr()) };
    if pw.is_null() {
        None
    } else {
        Some(unsafe { ((*pw).pw_uid, (*pw).pw_gid) })
    }
}

pub fn group_id(name: &str) -> Option<u32> {
    let c = CString::new(name).ok()?;
    let gr = unsafe { libc::getgrnam(c.as_ptr()) };
    if gr.is_null() {
        None
    } else {
        Some(unsafe { (*gr).gr_gid })
    }
}

// ---- temporary files

/// `mktemp -d TEMPLATE` (ending in XXXXXX): a new folder only we may enter (0700).
pub fn mkdtemp(template: &str) -> io::Result<String> {
    let mut b = CString::new(template)?.into_bytes_with_nul();
    let r = unsafe { libc::mkdtemp(b.as_mut_ptr() as *mut libc::c_char) };
    if r.is_null() {
        return Err(io::Error::last_os_error());
    }
    b.pop();
    Ok(String::from_utf8_lossy(&b).into_owned())
}

/// `mktemp TEMPLATE`: a new empty file only we may read (0600), open for writing.
pub fn mkstemp(template: &str) -> io::Result<(String, fs::File)> {
    use std::os::fd::FromRawFd;
    let mut b = CString::new(template)?.into_bytes_with_nul();
    let fd = unsafe { libc::mkostemp(b.as_mut_ptr() as *mut libc::c_char, libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    b.pop();
    Ok((String::from_utf8_lossy(&b).into_owned(), unsafe { fs::File::from_raw_fd(fd) }))
}

/// rename(2) that never replaces anything: `mv A B` where B must not be there (mv would move A into a folder B, or over a file B).
pub fn rename_new(from: &str, to: &str) -> io::Result<()> {
    let (a, b) = (CString::new(from)?, CString::new(to)?);
    let r = unsafe { libc::renameat2(libc::AT_FDCWD, a.as_ptr(), libc::AT_FDCWD, b.as_ptr(), libc::RENAME_NOREPLACE) };
    if r == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// n random bytes from the kernel as 2n hex digits (what `openssl rand -hex n` prints).
pub fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    let mut got = 0;
    while got < n {
        let r = unsafe { libc::getrandom(b[got..].as_mut_ptr() as *mut libc::c_void, n - got, 0) };
        if r > 0 {
            got += r as usize;
        } else if io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            // no randomness at all: stop rather than hand out a weak token
            panic!("getrandom failed");
        }
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// ---- order of names

fn collate_init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        libc::setlocale(libc::LC_COLLATE, c"".as_ptr());
    });
}

/// The order of the locale (LC_ALL, LC_COLLATE, LANG), as bash sorts a glob's names and as ls and sort order lines; equal ones
/// by their bytes.
pub fn collate(a: &str, b: &str) -> std::cmp::Ordering {
    collate_init();
    let (ca, cb) = (CString::new(a).unwrap_or_default(), CString::new(b).unwrap_or_default());
    let r = unsafe { libc::strcoll(ca.as_ptr(), cb.as_ptr()) };
    r.cmp(&0).then_with(|| a.as_bytes().cmp(b.as_bytes()))
}

/// `DIR/PATTERN` as bash expands it, the pattern given as a test on each name: the names (not those starting with a dot), in
/// bash's order, as DIR/NAME. A folder that cannot be read gives none.
pub fn glob(dir: &str, test: impl Fn(&str) -> bool) -> Vec<String> {
    let mut names: Vec<String> = match fs::read_dir(if dir.is_empty() { "/" } else { dir }) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.') && test(n))
            .collect(),
        Err(_) => vec![],
    };
    names.sort_by(|a, b| collate(a, b));
    names.into_iter().map(|n| format!("{dir}/{n}")).collect()
}

// ---- time

/// `date +FORMAT` in local time.
pub fn date(fmt: &str) -> String {
    crate::py::strftime(fmt)
}

/// `date -Is`: 2026-11-11T08:30:00-07:00
pub fn date_iso() -> String {
    let z = date("%z");
    let z = if z.len() == 5 { format!("{}:{}", &z[..3], &z[3..]) } else { z };
    format!("{}{z}", date("%Y-%m-%dT%H:%M:%S"))
}

// ---- small checks

/// Only ASCII digits, at least one ([[ $x =~ ^[0-9]+$ ]]).
pub fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

/// An account name that is safe to write into sudoers and to hand to programs: letters, digits, `_` `-` `.`, not starting with
/// `-` or `.`, at most 32 bytes, an optional `$` at the end (machine accounts).
pub fn account_name(s: &str) -> bool {
    let core = s.strip_suffix('$').unwrap_or(s);
    !core.is_empty()
        && s.len() <= 32
        && !core.starts_with('-')
        && !core.starts_with('.')
        && core.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
}

/// printf '%-Ns': padded with spaces to N bytes (bash counts bytes).
pub fn pad(s: &str, n: usize) -> String {
    let mut out = s.to_string();
    while out.len() < n {
        out.push(' ');
    }
    out
}

/// A path's last part (`${p##*/}`).
pub fn base(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// Bytes as a path argument.
pub fn os(b: &[u8]) -> &OsStr {
    OsStr::from_bytes(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_like_sed() {
        assert_eq!(lines(b""), Vec::<&[u8]>::new());
        assert_eq!(lines(b"a\nb"), vec![&b"a"[..], b"b"]);
        assert_eq!(lines(b"a\n\n"), vec![&b"a"[..], b""]);
    }

    #[test]
    fn substitution_strips_trailing_newlines_and_nuls() {
        assert_eq!(subst(b"a\0b\n\n"), "ab");
        assert_eq!(subst(b"\n a \n"), "\n a ");
    }

    #[test]
    fn sed_keeps_a_last_line_without_newline_and_the_mode() {
        let t = crate::testutil::TempDir::new();
        let d = t.p("");
        let d = d.trim_end_matches('/');
        let p = format!("{d}/f");
        fs::write(&p, "A=1\nB=2").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(sed_i(&p, |l| l.starts_with(b"B=").then(|| b"B=3".to_vec())).unwrap());
        assert_eq!(fs::read_to_string(&p).unwrap(), "A=1\nB=3");
        assert_eq!(fs::metadata(&p).unwrap().mode() & 0o777, 0o640);
        assert!(!sed_i(&p, |_| None).unwrap());
    }

    #[test]
    fn append_line_starts_a_new_line() {
        let t = crate::testutil::TempDir::new();
        let d = t.p("");
        let d = d.trim_end_matches('/');
        let p = format!("{d}/f");
        fs::write(&p, "A=1").unwrap();
        append_line(&p, "B=2").unwrap();
        append_line(&p, "C=3").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "A=1\nB=2\nC=3\n");
    }

    #[test]
    fn install_dir_sets_mode_on_an_existing_folder() {
        let t = crate::testutil::TempDir::new();
        let d = t.p("");
        let d = d.trim_end_matches('/');
        let p = format!("{d}/a/b");
        install_dir(&p, 0o750, None).unwrap();
        assert_eq!(fs::metadata(format!("{d}/a")).unwrap().mode() & 0o777, 0o755);
        assert_eq!(fs::metadata(&p).unwrap().mode() & 0o777, 0o750);
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        install_dir(&p, 0o755, None).unwrap();
        assert_eq!(fs::metadata(&p).unwrap().mode() & 0o777, 0o755);
    }

    #[test]
    fn rename_new_never_moves_into_a_folder() {
        let t = crate::testutil::TempDir::new();
        let d = t.p("");
        let d = d.trim_end_matches('/');
        fs::create_dir(format!("{d}/a")).unwrap();
        fs::create_dir(format!("{d}/b")).unwrap();
        assert!(rename_new(&format!("{d}/a"), &format!("{d}/b")).is_err());
        assert!(is_dir(&format!("{d}/a")) && !exists(&format!("{d}/b/a")));
        rename_new(&format!("{d}/a"), &format!("{d}/c")).unwrap();
        assert!(is_dir(&format!("{d}/c")));
    }

    #[test]
    fn account_names() {
        for ok in ["owner", "claude-agent", "a_b", "x1", "m$"] {
            assert!(account_name(ok), "{ok}");
        }
        for bad in ["", "-x", ".x", "a b", "a\nb", "ALL,x", "a:b", "%wheel", "$", &"a".repeat(33)] {
            assert!(!account_name(bad), "{bad:?}");
        }
    }
}
