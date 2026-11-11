//! labyrinth-agent: the root actions the engineering agent (account claude-agent) may take without the owner, each one checked.
//! It is a gate: sudo lets the agent run this program and nothing else as root (/etc/sudoers.d/50-labyrinth-agent). The Python
//! script of the same name (was /usr/local/sbin/labyrinth-agent; kept in packages/tools/python/) with the same commands, checks, refusals, journal
//! lines, calls and exit codes; its help is src/doc/labyrinth-agent.txt.
//!
//! Written for a gate:
//!   - no regular-expression engine: each of the script's patterns is spelled out below as code on bytes, matched whole;
//!   - arguments are bytes: a name that is not UTF-8 is refused or passed on unchanged, as the script did (it printed such bytes
//!     as \udcXX, which `show` and `repr` do too);
//!   - every decision is made by one function, `decide`, which asks the system what it needs through `World` (is it a regular
//!     file, what package is in this file, is a name in the repositories, what would pacman install, does root run this
//!     program): the tests and the fuzz target call it with made-up answers and check what comes out against an allow-list of
//!     their own (`audit`);
//!   - the programs it starts are found in /usr/bin only (labyrinth-rollback by its full path) and get a clean environment
//!     (`child_env`).
//!
//! Not 1:1: Ctrl+C while a program it started is running: the script killed that program a quarter of a second later (pacman
//! too, in the middle of a transaction) and stopped with a traceback; here the program gets the Ctrl+C from the terminal as
//! before and this waits for it, then exits with its code.

use crate::py::{self, Crash};
use crate::pycompat;
use std::ffi::{CString, OsStr, OsString};
use std::io::{Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};

pub const DOC: &str = include_str!("doc/labyrinth-agent.txt");

/// Text as the script held it: bytes (an argument, a name from pacman), shown as Python showed it.
pub type B = Vec<u8>;

// ---------------------------------------------------------------- the patterns (each matched whole)

const OURS: [&[u8]; 7] = [b"yggdrasil", b"labyrinth", b"heimdall", b"sarina", b"limbo", b"liki", b"agora"];
/// remote-access and tunnel tools (Heimdall's catalogue and more) and servers: exact names
const NEVER_EXACT: [&[u8]; 29] = [
    b"tailscale",
    b"headscale",
    b"ngrok",
    b"frp",
    b"frpc",
    b"frps",
    b"chisel",
    b"rathole",
    b"bore",
    b"bore-cli",
    b"meshagent",
    b"tor",
    b"openssh",
    b"dropbear",
    b"tinyssh",
    b"tigervnc",
    b"x11vnc",
    b"wayvnc",
    b"xrdp",
    b"gnome-remote-desktop",
    b"krfb",
    b"krdp",
    b"ttyd",
    b"cockpit",
    b"sunshine",
    b"samba",
    b"ksmbd-tools",
    b"nfs-utils",
    b"vsftpd",
];
/// ... names that begin so (zerotier.*, netbird.* ...)
const NEVER_PREFIX: [&[u8]; 6] = [b"zerotier", b"netbird", b"cloudflared", b"anydesk", b"teamviewer", b"rustdesk"];
const UNIT_HEADS: [&[u8]; 8] =
    [b"yggdrasil-", b"labyrinth-", b"heimdall", b"limbo", b"sarina", b"ollama", b"systemd-sysctl", b"greetd-test"];
const SERVICE_ACTIONS: [&[u8]; 13] = [
    b"start",
    b"stop",
    b"restart",
    b"reload",
    b"status",
    b"is-active",
    b"enable",
    b"disable",
    b"enable-now",
    b"disable-now",
    b"user-enable",
    b"user-disable",
    b"daemon-reload",
];
/// Heimdall watches the agent: these are the owner's
const STOPS: [&[u8]; 4] = [b"stop", b"disable", b"disable-now", b"user-disable"];
const LOG_VALUE: [&[u8]; 12] =
    [b"-u", b"--unit", b"-o", b"--output", b"--since", b"--until", b"-p", b"--priority", b"-t", b"--identifier", b"-g", b"--grep"];
const LOG_BARE: [&[u8]; 15] = [
    b"-k",
    b"--dmesg",
    b"-q",
    b"--quiet",
    b"--no-pager",
    b"-f",
    b"--follow",
    b"-r",
    b"--reverse",
    b"-x",
    b"--catalog",
    b"--no-hostname",
    b"-a",
    b"--all",
    b"--case-sensitive",
];
const LOG_FIELDS: [&[u8]; 8] =
    [b"_COMM", b"_PID", b"_UID", b"_TRANSPORT", b"_SYSTEMD_UNIT", b"_SYSTEMD_USER_UNIT", b"SYSLOG_IDENTIFIER", b"PRIORITY"];
pub const LIMBO_DIRS: [&[u8]; 2] = [b"/var/lib/limbo/build-agent/", b"/etc/limbo/validation/"];
/// places among the deploy targets that root uses (docs/GPU_UNLOCK.md; the optional apps' recipes; the boot menu theme)
pub const DEPLOY_NEVER: [&[u8]; 3] =
    [b"/usr/share/yggdrasil/gpu-unlock", b"/usr/share/labyrinth/pkgbuilds", b"/usr/share/labyrinth/grub-evodevo"];
/// the settings Settings changes only with the owner's password (polkit, auth_admin_keep: casefan's fans and pumps, gpufan,
/// powerlimit, tune), and the protections the account helper and Agora's pkg-root read
pub const DEPLOY_GUARDED: [&[u8]; 6] = [
    b"/etc/yggdrasil/case-fans.json",
    b"/etc/yggdrasil/gpu-fan.json",
    b"/etc/yggdrasil/power-limits.json",
    b"/etc/yggdrasil/tuning.json",
    b"/etc/yggdrasil/protected-accounts",
    b"/etc/yggdrasil/protected-packages",
];
/// pkg-install's copies: a fresh folder in here for each install (root's own, 0700), removed afterwards
pub const STAGE: &str = "/var/cache/labyrinth-agent";
/// where root is told to run a program
pub const ROOT_RUNS: [&str; 11] = [
    "/etc/systemd/system",
    "/run/systemd/system",
    "/usr/local/lib/systemd/system",
    "/usr/lib/systemd/system",
    "/etc/pacman.d/hooks",
    "/usr/share/libalpm/hooks",
    "/etc/udev/rules.d",
    "/run/udev/rules.d",
    "/usr/lib/udev/rules.d",
    "/usr/share/polkit-1/actions",
    "/etc/sudoers.d",
];
const KEEP_ENV: [&str; 7] = ["LOGNAME", "USER", "SUDO_COMMAND", "SUDO_GID", "SUDO_HOME", "SUDO_UID", "SUDO_USER"];
pub const LOG_MAX: usize = 4000;
const ROLLBACK: &[u8] = b"/usr/local/bin/labyrinth-rollback";

fn lower_digit(c: u8) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

/// (yggdrasil|labyrinth|heimdall|sarina|limbo|liki|agora)[-a-z0-9.+_]*
pub fn ours(n: &[u8]) -> bool {
    OURS.iter().any(|h| n.starts_with(h) && n[h.len()..].iter().all(|&c| lower_digit(c) || b"-.+_".contains(&c)))
}

/// [a-z0-9@._+-]{1,100}
pub fn pkgname(n: &[u8]) -> bool {
    (1..=100).contains(&n.len()) && n.iter().all(|&c| lower_digit(c) || b"@._+-".contains(&c))
}

/// The script's NEVER_ADD: an exact name, a name that begins with one of the prefixes, or one that ends in -server (none of them
/// over a line break: `.` in a pattern stops at \n).
pub fn never_add(n: &[u8]) -> bool {
    if n.contains(&b'\n') {
        return false;
    }
    NEVER_EXACT.contains(&n) || NEVER_PREFIX.iter().any(|p| n.starts_with(p)) || n.ends_with(b"-server")
}

/// (yggdrasil-|labyrinth-|heimdall|limbo|sarina|ollama|systemd-sysctl|greetd-test)[A-Za-z0-9@._:-]*
pub fn unit(u: &[u8]) -> bool {
    UNIT_HEADS.iter().any(|h| u.starts_with(h) && u[h.len()..].iter().all(|&c| c.is_ascii_alphanumeric() || b"@._:-".contains(&c)))
}

/// -?[0-9]+|[0-9a-f]{32}
pub fn boot_id(s: &[u8]) -> bool {
    let d = s.strip_prefix(b"-").unwrap_or(s);
    (!d.is_empty() && d.iter().all(u8::is_ascii_digit))
        || (s.len() == 32 && s.iter().all(|&c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
}

/// (_COMM|_PID|...|PRIORITY)=[A-Za-z0-9@._:/-]*
pub fn log_match(s: &[u8]) -> bool {
    let Some(i) = s.iter().position(|&c| c == b'=') else { return false };
    LOG_FIELDS.contains(&&s[..i]) && s[i + 1..].iter().all(|&c| c.is_ascii_alphanumeric() || b"@._:/-".contains(&c))
}

/// \+?[0-9]+|all: what journalctl's -n takes as its value
pub fn lines(s: &[u8]) -> bool {
    s == b"all" || digits(s.strip_prefix(b"+").unwrap_or(s))
}

fn digits(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(u8::is_ascii_digit)
}

/// [A-Za-z0-9._+-]{1,64}: a terminal type that names no file
fn term_ok(s: &[u8]) -> bool {
    (1..=64).contains(&s.len()) && s.iter().all(|&c| c.is_ascii_alphanumeric() || b"._+-".contains(&c))
}

/// [A-Za-z0-9._/-] and [A-Za-z0-9._-]
fn path_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b"._/-".contains(&c)
}

fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b"._-".contains(&c)
}

/// X ending in one of the suffixes, X at least one character, each `ok`
fn with_suffix(s: &[u8], suffixes: &[&[u8]], ok: fn(u8) -> bool) -> bool {
    suffixes.iter().any(|x| s.len() > x.len() && s.ends_with(x) && s[..s.len() - x.len()].iter().all(|&c| ok(c)))
}

/// The script's DEPLOY patterns: the mode of a place the agent may write, else None.
pub fn deploy_pattern(d: &[u8]) -> Option<u32> {
    if let Some(r) = d.strip_prefix(b"/usr/bin/".as_slice()) {
        let tail = |t: &[u8]| !t.is_empty() && t.iter().all(|&c| lower_digit(c) || c == b'-');
        let ok = r == b"minos"
            || r.strip_prefix(b"yggdrasil-".as_slice()).is_some_and(tail)
            || r.strip_prefix(b"ygg-".as_slice()).is_some_and(tail);
        return ok.then_some(0o755);
    }
    for p in [b"/usr/share/yggdrasil/".as_slice(), b"/usr/share/labyrinth/"] {
        if let Some(r) = d.strip_prefix(p) {
            if !r.is_empty() && r.iter().all(|&c| path_char(c)) {
                return Some(0o644);
            }
        }
    }
    if d.strip_prefix(b"/usr/share/icons/".as_slice()).is_some_and(|r| with_suffix(r, &[b".svg", b".png"], path_char)) {
        return Some(0o644);
    }
    for p in [b"/usr/share/applications/".as_slice(), b"/usr/share/wayland-sessions/"] {
        if d.strip_prefix(p).is_some_and(|r| with_suffix(r, &[b".desktop"], name_char)) {
            return Some(0o644);
        }
    }
    if d.strip_prefix(b"/etc/yggdrasil/".as_slice()).is_some_and(|r| with_suffix(r, &[b".json", b".conf"], name_char)) {
        return Some(0o644);
    }
    None
}

// ---------------------------------------------------------------- text as the script wrote it

/// A str made from bytes the way Python made sys.argv (UTF-8, each byte that is not part of a character kept as a lone surrogate
/// U+DC80..U+DCFF), as it printed it on standard error (backslashreplace: \udcXX).
pub fn show(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len());
    for chunk in b.utf8_chunks() {
        out.push_str(chunk.valid());
        for &x in chunk.invalid() {
            out.push_str(&format!("\\udc{x:02x}"));
        }
    }
    out
}

/// repr() of such a str.
pub fn repr(b: &[u8]) -> String {
    let quote = if b.contains(&b'\'') && !b.contains(&b'"') { '"' } else { '\'' };
    let mut out = String::from(quote);
    for chunk in b.utf8_chunks() {
        for c in chunk.valid().chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                c if c == quote => {
                    out.push('\\');
                    out.push(c);
                }
                c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
                c if (c as u32) < 0x7f => out.push(c),
                c if pycompat::isprintable(c) => out.push(c),
                c if (c as u32) <= 0xff => out.push_str(&format!("\\x{:02x}", c as u32)),
                c if (c as u32) <= 0xffff => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push_str(&format!("\\U{:08x}", c as u32)),
            }
        }
        for &x in chunk.invalid() {
            out.push_str(&format!("\\udc{x:02x}"));
        }
    }
    out.push(quote);
    out
}

fn cat(parts: &[&[u8]]) -> B {
    parts.concat()
}

fn join(items: &[B], sep: &[u8]) -> B {
    items.join(sep)
}

fn argv(parts: &[&[u8]]) -> Vec<B> {
    parts.iter().map(|p| p.to_vec()).collect()
}

fn basename(p: &[u8]) -> &[u8] {
    p.rsplit(|&c| c == b'/').next().unwrap_or(p)
}

// ---------------------------------------------------------------- the decisions

/// Why nothing (more) is done: a refusal (exit code 2, written to the journal) or where the script stopped with a traceback.
#[derive(Debug, PartialEq)]
pub enum Fail {
    Refused(B),
    Crash(Crash),
}

impl From<Crash> for Fail {
    fn from(c: Crash) -> Fail {
        Fail::Crash(c)
    }
}

fn refused(parts: &[&[u8]]) -> Fail {
    Fail::Refused(cat(parts))
}

/// What is done once the checks have passed.
#[derive(Debug, PartialEq)]
pub enum Plan {
    /// these programs, one after the other, each written to the journal first; the exit code is the last one's
    Runs(Vec<Vec<B>>),
    /// one file put in place (the source is read as the caller then)
    Deploy { src: B, dest: B, mode: u32 },
    /// pkg-install: the journal line saying where the packages were copied, then the runs (heimdalld, pacman -U of the copies)
    Install { note: B, runs: Vec<Vec<B>> },
}

/// What the decisions need to know of the system, asked in the script's order.
pub trait World {
    /// pkg-install's copy of package file number i (read as the caller, never through a link at its end) in the stage folder,
    /// made at the first copy; a refusal where the caller may not read it
    fn copy_in(&mut self, path: &[u8], i: usize) -> Result<B, Fail>;
    /// the stage folder the copies are in
    fn staged(&self) -> B;
    /// None when path and every folder above it are root's alone (owned by root, writable by root only, no link), else why not
    fn root_only(&mut self, path: &[u8]) -> Option<B>;
    /// the package's name in pacman -Qip PATH's "Name :" line, empty when there is none
    fn pkg_name(&mut self, path: &[u8]) -> Result<B, Crash>;
    /// pacman -Si NAME succeeds
    fn in_repos(&mut self, name: &[u8]) -> Result<bool, Crash>;
    /// what pacman -Sp prints for installing names (one name a line), None when it fails
    fn transaction(&mut self, names: &[B]) -> Result<Option<B>, Crash>;
    /// the first file where root is told to run programs that names this one
    fn root_runs(&mut self, name: &[u8]) -> Option<B>;
}

/// The package files the agent may install: each copied first (`World::copy_in`), and the copy's package must be one of ours.
/// The names and the copies: only the copies are installed, so a file changed after it was checked is not what pacman gets.
pub fn check_pkg_files(paths: &[B], w: &mut dyn World) -> Result<(Vec<B>, Vec<B>), Fail> {
    if paths.is_empty() {
        return Err(refused(&[b"no package files"]));
    }
    let (mut names, mut copies) = (vec![], vec![]);
    for (i, p) in paths.iter().enumerate() {
        let parent_part = p.split(|&c| c == b'/').any(|c| c == b"..");
        if !p.starts_with(b"/") || parent_part || !(p.ends_with(b".pkg.tar.zst") || p.ends_with(b".pkg.tar.xz")) {
            return Err(refused(&[p, b": not an absolute path to a package file"]));
        }
        let c = w.copy_in(p, i)?;
        let n = w.pkg_name(&c)?;
        if n.is_empty() || !ours(&n) {
            let shown: &[u8] = if n.is_empty() { b"?" } else { &n };
            return Err(refused(&[p, b": package ", shown, b" is not one of the ecosystem's own"]));
        }
        names.push(n);
        copies.push(c);
    }
    Ok((names, copies))
}

pub fn check_pkg_names(names: &[B]) -> Result<(), Fail> {
    if names.is_empty() {
        return Err(refused(&[b"no package names"]));
    }
    for n in names {
        if !pkgname(n) || n.starts_with(b"-") {
            return Err(refused(&[repr(n).as_bytes(), b": not a package name"]));
        }
        if never_add(n) {
            return Err(refused(&[n, b": remote-access tools and servers need the owner (labyrinth-agent-window)"]));
        }
    }
    Ok(())
}

/// Python's bytes.strip(): the ASCII white space at both ends.
fn strip_ascii(mut b: &[u8]) -> &[u8] {
    let ws = |c: &u8| b" \t\n\r\x0b\x0c".contains(c);
    while b.first().is_some_and(ws) {
        b = &b[1..];
    }
    while b.last().is_some_and(ws) {
        b = &b[..b.len() - 1];
    }
    b
}

/// pacman -Sp's list of what installing names would install: no remote-access tool or server among them.
pub fn check_transaction(names: &[B], printed: &[u8]) -> Result<(), Fail> {
    for line in printed.split(|&c| c == b'\n') {
        let n = strip_ascii(line);
        if !n.is_empty() && never_add(n) {
            let all = join(names, b", ");
            return Err(refused(&[
                n,
                b", which ",
                &all,
                b" would install: remote-access tools and servers need the owner (labyrinth-agent-window)",
            ]));
        }
    }
    Ok(())
}

pub fn service_argv(action: &[u8], units: &[B]) -> Result<Vec<B>, Fail> {
    if !SERVICE_ACTIONS.contains(&action) {
        return Err(refused(&[action, b": not a service action the agent may take"]));
    }
    if action == b"daemon-reload" {
        if !units.is_empty() {
            return Err(refused(&[b"daemon-reload takes no unit"]));
        }
        return Ok(argv(&[b"systemctl", b"daemon-reload"]));
    }
    if units.is_empty() {
        return Err(refused(&[b"name a unit"]));
    }
    for u in units {
        if !unit(u) {
            return Err(refused(&[u, b": not one of the ecosystem's own units"]));
        }
        if u.starts_with(b"labyrinth-agent") {
            return Err(refused(&[u, b": the agent window's units are the owner's"]));
        }
        if u.starts_with(b"heimdall") && STOPS.contains(&action) {
            return Err(refused(&[u, b": Heimdall watches the agent; only the owner stops or disables it"]));
        }
    }
    let base: &[&[u8]] = match action {
        b"enable-now" => &[b"enable", b"--now"],
        b"disable-now" => &[b"disable", b"--now"],
        b"user-enable" => &[b"--global", b"enable"],
        b"user-disable" => &[b"--global", b"disable"],
        _ => &[action],
    };
    let mut out = argv(&[b"systemctl"]);
    out.extend(argv(base));
    out.extend(units.iter().cloned());
    Ok(out)
}

/// journalctl with read-only options only (never --vacuum, --rotate, --flush, --setup-keys, --directory and the like).
pub fn logs_argv(args: &[B]) -> Result<Vec<B>, Fail> {
    let mut out = argv(&[b"journalctl", b"--no-pager"]);
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let (flag, sep) = match a.iter().position(|&c| c == b'=') {
            Some(k) => (&a[..k], true),
            None => (&a[..], false),
        };
        let optional: Option<fn(&[u8]) -> bool> = match a.as_slice() {
            b"-n" | b"--lines" => Some(lines),
            b"-b" | b"--boot" => Some(boot_id),
            _ => None,
        };
        if LOG_BARE.contains(&a.as_slice()) || log_match(a) || (sep && (LOG_VALUE.contains(&flag) || flag == b"-n" || flag == b"--lines")) {
            out.push(a.clone());
        } else if LOG_VALUE.contains(&a.as_slice()) {
            if i + 1 >= args.len() {
                return Err(refused(&[a, b" needs a value"]));
            }
            out.push(a.clone());
            out.push(args[i + 1].clone());
            i += 1;
        } else if let Some(takes) = optional {
            // journalctl's -n and -b take the next word only when it reads as their value (`-n --rotate` rotates): so here too
            out.push(a.clone());
            if i + 1 < args.len() && takes(&args[i + 1]) {
                out.push(args[i + 1].clone());
                i += 1;
            }
        } else {
            return Err(refused(&[a, b": not a read-only journal option"]));
        }
        i += 1;
    }
    Ok(out)
}

pub fn limbo_argv(verb: &[u8], manifest: &[u8]) -> Result<Vec<B>, Fail> {
    if ![b"up".as_slice(), b"down", b"doctor", b"status"].contains(&verb) {
        return Err(refused(&[b"limbo ", verb, b": not allowed"]));
    }
    let ok = !manifest.is_empty()
        && !manifest.split(|&c| c == b'/').any(|c| c == b"..")
        && LIMBO_DIRS.iter().any(|d| manifest.starts_with(d))
        && manifest.ends_with(b".json");
    let dirs = join(&LIMBO_DIRS.map(|d| d.to_vec()), b" or ");
    if verb == b"up" || verb == b"down" {
        if !ok {
            return Err(refused(&[b"limbo up/down needs a manifest under ", &dirs]));
        }
        return Ok(vec![b"limbo".to_vec(), verb.to_vec(), manifest.to_vec()]);
    }
    if !manifest.is_empty() && !ok {
        return Err(refused(&[b"limbo ", verb, b": a manifest only from under ", &dirs]));
    }
    let mut out = vec![b"limbo".to_vec(), verb.to_vec()];
    if !manifest.is_empty() {
        out.push(manifest.to_vec());
    }
    Ok(out)
}

pub fn deploy_mode(dest: &[u8]) -> Result<u32, Fail> {
    if dest.split(|&c| c == b'/').skip(1).any(|p| p.is_empty() || p == b"." || p == b"..") {
        return Err(refused(&[dest, b": not a clean path"]));
    }
    if DEPLOY_GUARDED.contains(&dest) {
        return Err(refused(&[dest, b": Settings changes it only with the owner's password (polkit), so the agent does not"]));
    }
    for never in DEPLOY_NEVER {
        if dest == never || (dest.starts_with(never) && dest.get(never.len()) == Some(&b'/')) {
            return Err(refused(&[dest, b": root uses what is there (the owner's, through labyrinth-agent-window)"]));
        }
    }
    deploy_pattern(dest).ok_or_else(|| refused(&[dest, b": not a place the agent may write (user-side desktop files only)"]))
}

/// Everything the script decided for one command line (after the help and the check that it runs as root).
pub fn decide(cmd: &[u8], rest: &[B], w: &mut dyn World) -> Result<Plan, Fail> {
    let first = rest.first().map(Vec::as_slice);
    match cmd {
        b"pkg-install" => {
            let (names, copies) = check_pkg_files(rest, w)?;
            let note = cat(&[b"pkg-install: ", &join(rest, b" "), b" copied to ", &w.staged()]);
            let reason = cat(&[b"agent installs ", &join(&names, b", ")]);
            let mut maint = argv(&[b"heimdalld", b"maintenance", b"on", b"--for", b"15m", b"--reason"]);
            maint.push(reason);
            let mut install = argv(&[b"pacman", b"-U", b"--noconfirm"]);
            install.extend(copies);
            Ok(Plan::Install { note, runs: vec![maint, install] })
        }
        b"pkg-add" => {
            check_pkg_names(rest)?;
            for n in rest {
                if !w.in_repos(n)? {
                    return Err(refused(&[n, b": not in the official repositories"]));
                }
            }
            match w.transaction(rest)? {
                None => return Err(refused(&[b"pacman cannot say what ", &join(rest, b", "), b" would install"])),
                Some(printed) => check_transaction(rest, &printed)?,
            }
            let mut install = argv(&[b"pacman", b"-S", b"--needed", b"--noconfirm"]);
            install.extend(rest.iter().cloned());
            Ok(Plan::Runs(vec![install]))
        }
        b"service" if !rest.is_empty() => Ok(Plan::Runs(vec![service_argv(&rest[0], &rest[1..])?])),
        b"logs" => Ok(Plan::Runs(vec![logs_argv(rest)?])),
        b"maintenance" if matches!(first, Some(b"on" | b"off" | b"status")) => {
            let extra = &rest[1..];
            let flag = |x: &B| x == b"--for" || x == b"--reason";
            if !extra.is_empty() && !((extra.len() == 2 || extra.len() == 4) && flag(&extra[0]) && (extra.len() == 2 || flag(&extra[2]))) {
                return Err(refused(&[b"maintenance [--for D] [--reason TEXT]"]));
            }
            let mut a = argv(&[b"heimdalld", b"maintenance"]);
            a.extend(rest.iter().cloned());
            Ok(Plan::Runs(vec![a]))
        }
        b"limbo" if !rest.is_empty() => {
            let a = limbo_argv(&rest[0], rest.get(1).map(Vec::as_slice).unwrap_or(b""))?;
            if a.len() == 3 {
                if let Some(why) = w.root_only(&a[2]) {
                    let tail = b" (limbo starts what a manifest says, as root: only root may write it)";
                    return Err(refused(&[b"limbo ", &rest[0], b": ", &why, tail]));
                }
            }
            Ok(Plan::Runs(vec![a]))
        }
        b"rollback" if !rest.is_empty() => {
            if matches!(first, Some(b"list" | b"status")) && rest.len() == 1 {
                return Ok(Plan::Runs(vec![vec![ROLLBACK.to_vec(), rest[0].clone()]]));
            }
            if first == Some(b"dry-run".as_slice()) && rest.len() == 2 && digits(&rest[1]) {
                return Ok(Plan::Runs(vec![vec![ROLLBACK.to_vec(), rest[1].clone(), b"--dry-run".to_vec()]]));
            }
            Err(refused(&[b"rollback list|status|dry-run N (a real rollback is the owner's, through labyrinth-agent-window)"]))
        }
        b"firewall-status" => Ok(Plan::Runs(vec![argv(&[b"ufw", b"status", b"verbose"])])),
        b"deploy" if rest.len() == 2 => {
            let (src, dest) = (&rest[0], &rest[1]);
            let mode = deploy_mode(dest)?;
            if mode == 0o755 {
                if let Some(wh) = w.root_runs(basename(dest)) {
                    return Err(refused(&[dest, b": root runs it (", &wh, b")"]));
                }
            }
            Ok(Plan::Deploy { src: src.clone(), dest: dest.clone(), mode })
        }
        _ => Err(refused(&[b"unknown command (see --help)"])),
    }
}

// ---------------------------------------------------------------- the system

/// What the programs it starts see of the environment: a fixed PATH and HOME, sudo's own variables, a terminal type that names
/// no file. Nothing else the caller set (PATH, LD_*, SYSTEMD_*, LIMBO_*, PAGER ...), which could steer a program run as root.
pub fn child_env(vars: impl IntoIterator<Item = (OsString, OsString)>) -> Vec<(OsString, OsString)> {
    let mut seen: Vec<(OsString, OsString)> = vec![];
    for (k, v) in vars {
        if !seen.iter().any(|(s, _)| *s == k) {
            seen.push((k, v));
        }
    }
    let get = |k: &str| seen.iter().find(|(s, _)| s == k).map(|(_, v)| v.clone());
    let mut env = vec![(OsString::from("PATH"), OsString::from("/usr/bin")), (OsString::from("HOME"), OsString::from("/root"))];
    for k in KEEP_ENV {
        if let Some(v) = get(k) {
            env.push((k.into(), v));
        }
    }
    if let Some(t) = get("TERM").filter(|t| term_ok(t.as_bytes())) {
        env.push(("TERM".into(), t));
    }
    env
}

/// A program as the script started it: a bare name from /usr/bin (the PATH it gave), with the clean environment.
fn command(a: &[B], env: &[(OsString, OsString)]) -> Command {
    let name = &a[0];
    let path = if name.contains(&b'/') { name.clone() } else { cat(&[b"/usr/bin/", name]) };
    let mut c = Command::new(OsStr::from_bytes(&path));
    c.arg0(OsStr::from_bytes(name)).args(a[1..].iter().map(|x| OsStr::from_bytes(x))).env_clear().envs(env.iter().map(|(k, v)| (k, v)));
    c
}

fn spawn_error(a: &[B], e: std::io::Error) -> Crash {
    py::os_err(e, &show(&a[0]))
}

/// The journal line as given to logger: cut at LOG_MAX bytes (with how much was cut).
pub fn log_bytes(msg: &[u8]) -> B {
    if msg.len() <= LOG_MAX {
        return msg.to_vec();
    }
    cat(&[&msg[..LOG_MAX], format!(" [{} bytes more]", msg.len() - LOG_MAX).as_bytes()])
}

fn log(msg: &[u8], env: &[(OsString, OsString)]) -> Result<(), Crash> {
    let a = vec![
        b"logger".to_vec(),
        b"-t".to_vec(),
        b"labyrinth-agent".to_vec(),
        b"-p".to_vec(),
        b"auth.notice".to_vec(),
        b"--".to_vec(),
        log_bytes(msg),
    ];
    command(&a, env).status().map_err(|e| spawn_error(&a, e))?;
    Ok(())
}

/// A finished program's exit code as the script exited with it (killed by signal N: 256 - N).
fn code(s: std::process::ExitStatus) -> i32 {
    match (s.code(), s.signal()) {
        (Some(c), _) => c,
        (None, Some(sig)) => (256 - sig) & 0xff,
        _ => 1,
    }
}

fn run(a: &[B], env: &[(OsString, OsString)]) -> Result<i32, Crash> {
    log(&cat(&[b"run: ", &join(a, b" ")]), env)?;
    let mut child = command(a, env).spawn().map_err(|e| spawn_error(a, e))?;
    // Ctrl+C reaches the program from the terminal; this waits for it (and does not kill pacman in a transaction)
    let old = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
    let status = child.wait();
    unsafe { libc::signal(libc::SIGINT, old) };
    Ok(code(status.map_err(Crash::from)?))
}

/// pacman's answers, captured as subprocess.run(capture_output=True) did (standard input left to it).
fn captured(a: &[B], env: &[(OsString, OsString)]) -> Result<std::process::Output, Crash> {
    command(a, env).stdin(Stdio::inherit()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().map_err(|e| spawn_error(a, e))
}

/// ^Name\s*:\s*(\S+) searched line by line (re.M), \s and \S as Python has them.
pub fn name_line(text: &str) -> &str {
    let space = |c: char| pycompat::isspace(c);
    let starts = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1));
    for s in starts {
        let Some(r) = text[s..].strip_prefix("Name") else { continue };
        let Some(r) = r.trim_start_matches(space).strip_prefix(':') else { continue };
        let r = r.trim_start_matches(space);
        let end = r.find(space).unwrap_or(r.len());
        if end > 0 {
            return &r[..end];
        }
    }
    ""
}

/// `name` stands in data whole (not as part of a longer name).
pub fn names(data: &[u8], name: &[u8]) -> bool {
    let inside = |c: u8| c.is_ascii_alphanumeric() || b"-_.@+".contains(&c);
    if name.is_empty() || name.len() > data.len() {
        return false;
    }
    (0..=data.len() - name.len()).any(|i| {
        let j = i + name.len();
        &data[i..j] == name && (i == 0 || !inside(data[i - 1])) && (j == data.len() || !inside(data[j]))
    })
}

fn listdir(d: &[u8]) -> Option<Vec<B>> {
    let mut out = vec![];
    for e in std::fs::read_dir(OsStr::from_bytes(d)).ok()? {
        out.push(e.ok()?.file_name().into_vec());
    }
    out.sort();
    Some(out)
}

/// The files of folder d and of its subfolders (one level), in name order.
fn files_in(d: &[u8]) -> Vec<B> {
    let mut out = vec![];
    for e in listdir(d).unwrap_or_default() {
        let p = cat(&[d, b"/", &e]);
        let Ok(m) = std::fs::symlink_metadata(OsStr::from_bytes(&p)) else { continue };
        if m.is_dir() {
            for s in listdir(&p).unwrap_or_default() {
                out.push(cat(&[&p, b"/", &s]));
            }
        } else {
            out.push(p);
        }
    }
    out
}

/// The first file of `places` that names the program, else None.
pub fn root_runs_in(places: &[&str], name: &[u8]) -> Option<B> {
    for d in places {
        for p in files_in(d.as_bytes()) {
            let opened = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(OsStr::from_bytes(&p));
            let Ok(mut f) = opened else { continue };
            if !f.metadata().is_ok_and(|m| m.file_type().is_file()) {
                continue;
            }
            let mut data = vec![];
            if f.read_to_end(&mut data).is_ok() && names(&data, name) {
                return Some(p);
            }
        }
    }
    None
}

/// None when path and every folder above it, to /, are owned by root, writable by root only and no link; else why not.
pub fn root_only_path(path: &[u8]) -> Option<B> {
    let parts: Vec<&[u8]> = path.split(|&c| c == b'/').skip(1).collect();
    let mut chain = vec![b"/".to_vec()];
    for k in 1..=parts.len() {
        chain.push(cat(&[b"/", &parts[..k].join(&b'/')]));
    }
    for (i, p) in chain.iter().enumerate() {
        let m = match std::fs::symlink_metadata(OsStr::from_bytes(p)) {
            Ok(m) => m,
            Err(e) => return Some(cat(&[p, b": ", py::strerror(e.raw_os_error().unwrap_or(0)).as_bytes()])),
        };
        let last = i == chain.len() - 1;
        if m.file_type().is_symlink() {
            return Some(cat(&[p, b" is a link"]));
        }
        if last && !m.file_type().is_file() {
            return Some(cat(&[p, b" is not a regular file"]));
        }
        if !last && !m.is_dir() {
            return Some(cat(&[p, b" is not a folder"]));
        }
        if m.uid() != 0 || m.mode() & 0o022 != 0 {
            return Some(cat(&[p, format!(" is not root's alone (owner {}, mode 0o{:o})", m.uid(), m.mode() & 0o7777).as_bytes()]));
        }
    }
    None
}

/// A fresh folder for pkg-install's copies, in STAGE (made root's own, 0700, if it is not there), named as tempfile.mkdtemp names.
fn stage_dir() -> Result<B, Fail> {
    use std::os::unix::fs::DirBuilderExt;
    let m = match std::fs::symlink_metadata(STAGE) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new().mode(0o700).create(STAGE).map_err(|e| py::os_err(e, STAGE))?;
            std::fs::symlink_metadata(STAGE).map_err(|e| py::os_err(e, STAGE))?
        }
        Err(e) => return Err(py::os_err(e, STAGE).into()),
    };
    if !m.is_dir() || m.uid() != 0 || m.mode() & 0o077 != 0 {
        return Err(refused(&[STAGE.as_bytes(), b" is not a folder only root may enter"]));
    }
    const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    for _ in 0..100 {
        let mut r = [0u8; 8];
        if unsafe { libc::getrandom(r.as_mut_ptr().cast(), r.len(), 0) } != 8 {
            return Err(os_crash("getrandom").into());
        }
        let name: String = r.iter().map(|b| LETTERS[*b as usize % LETTERS.len()] as char).collect();
        let dir = format!("{STAGE}/install-{name}");
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => return Ok(dir.into_bytes()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(py::os_err(e, &dir).into()),
        }
    }
    Err(Crash("No usable temporary directory name found".into()).into())
}

/// The system itself, asked as the script asked it.
pub struct Real<'a> {
    pub env: &'a [(OsString, OsString)],
    /// pkg-install's stage folder and the caller the copies are read as (found at the first copy)
    pub stage: Option<B>,
    pub who: Option<Caller>,
}

impl Real<'_> {
    /// pkg-install's copies taken away (whatever happened).
    pub fn discard(&mut self) {
        if let Some(d) = self.stage.take() {
            let _ = std::fs::remove_dir_all(OsStr::from_bytes(&d));
        }
    }
}

impl World for Real<'_> {
    fn copy_in(&mut self, path: &[u8], i: usize) -> Result<B, Fail> {
        if self.stage.is_none() {
            self.who = caller(std::env::var_os("SUDO_UID"), std::env::var_os("SUDO_GID"))?;
            self.stage = Some(stage_dir()?);
        }
        let data = read_as(path, self.who.as_ref())?;
        let sub = cat(&[self.stage.as_deref().unwrap_or_default(), b"/", i.to_string().as_bytes()]);
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(OsStr::from_bytes(&sub)).map_err(|e| py::os_err(e, &show(&sub)))?;
        let c = cat(&[&sub, b"/", basename(path)]);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(OsStr::from_bytes(&c))
            .map_err(|e| py::os_err(e, &show(&c)))?;
        f.write_all(&data).map_err(|e| py::os_err(e, &show(&c)))?;
        Ok(c)
    }

    fn staged(&self) -> B {
        self.stage.clone().unwrap_or_default()
    }

    fn root_only(&mut self, path: &[u8]) -> Option<B> {
        root_only_path(path)
    }

    fn pkg_name(&mut self, path: &[u8]) -> Result<B, Crash> {
        let o = captured(&[b"pacman".to_vec(), b"-Qip".to_vec(), path.to_vec()], self.env)?;
        // text=True: both decoded strictly, universal newlines
        let out = py::utf8(o.stdout).map_err(Crash)?;
        py::utf8(o.stderr).map_err(Crash)?;
        Ok(name_line(&py::universal(&out)).as_bytes().to_vec())
    }

    fn in_repos(&mut self, name: &[u8]) -> Result<bool, Crash> {
        Ok(captured(&[b"pacman".to_vec(), b"-Si".to_vec(), name.to_vec()], self.env)?.status.code() == Some(0))
    }

    fn transaction(&mut self, names: &[B]) -> Result<Option<B>, Crash> {
        let mut a = argv(&[b"pacman", b"-S", b"--print", b"--print-format", b"%n", b"--needed", b"--noconfirm"]);
        a.extend(names.iter().cloned());
        let o = captured(&a, self.env)?;
        Ok((o.status.code() == Some(0)).then_some(o.stdout))
    }

    fn root_runs(&mut self, name: &[u8]) -> Option<B> {
        root_runs_in(&ROOT_RUNS, name)
    }
}

// ---------------------------------------------------------------- deploy

/// The account sudo ran this for.
#[derive(Debug, PartialEq)]
pub struct Caller {
    pub uid: u32,
    pub gid: u32,
    pub groups: Vec<libc::gid_t>,
    pub name: B,
}

fn id_ok(v: &Option<OsString>) -> Option<u64> {
    let b = v.as_ref()?.as_bytes();
    ((1..=10).contains(&b.len()) && b.iter().all(u8::is_ascii_digit)).then(|| b.iter().fold(0u64, |n, &c| n * 10 + (c - b'0') as u64))
}

/// SUDO_UID and SUDO_GID: the caller, None when that is root (or there was no sudo).
pub fn caller(uid: Option<OsString>, gid: Option<OsString>) -> Result<Option<Caller>, Fail> {
    if uid.is_none() && gid.is_none() {
        return Ok(None);
    }
    let (Some(u), Some(g)) = (id_ok(&uid), id_ok(&gid)) else { return Err(refused(&[b"SUDO_UID and SUDO_GID do not name an account"])) };
    if u >= 0xFFFF_FFFF || g >= 0xFFFF_FFFF {
        return Err(refused(&[b"SUDO_UID and SUDO_GID do not name an account"]));
    }
    let (uid, gid) = (u as u32, g as u32);
    if uid == 0 {
        return Ok(None);
    }
    Ok(Some(match account_name(uid) {
        Some(name) => Caller { uid, gid, groups: grouplist(&name, gid), name },
        None => Caller { uid, gid, groups: vec![gid], name: uid.to_string().into_bytes() },
    }))
}

fn account_name(uid: u32) -> Option<B> {
    let mut buf = vec![0u8; 4096];
    loop {
        let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
        let mut res: *mut libc::passwd = std::ptr::null_mut();
        let rc = unsafe { libc::getpwuid_r(uid, &mut pw, buf.as_mut_ptr().cast(), buf.len(), &mut res) };
        if rc == libc::ERANGE && buf.len() < 1 << 20 {
            buf.resize(buf.len() * 2, 0);
            continue;
        }
        if rc != 0 || res.is_null() {
            return None;
        }
        return Some(unsafe { std::ffi::CStr::from_ptr(pw.pw_name) }.to_bytes().to_vec());
    }
}

fn grouplist(name: &[u8], gid: u32) -> Vec<libc::gid_t> {
    let Ok(cname) = CString::new(name) else { return vec![gid] };
    let mut n: libc::c_int = 32;
    loop {
        let mut groups = vec![0 as libc::gid_t; n as usize];
        let mut got = n;
        if unsafe { libc::getgrouplist(cname.as_ptr(), gid, groups.as_mut_ptr(), &mut got) } >= 0 {
            groups.truncate(got as usize);
            return groups;
        }
        n = got.max(n * 2);
    }
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn os_crash(what: &str) -> Crash {
    py::os_err(std::io::Error::last_os_error(), what)
}

/// The source file's bytes, opened by a child process that is the caller, never through a link at its end: what the caller may
/// not read (/etc/shadow, keys, another account's files) is refused, so deploy never makes it readable to all.
pub fn read_as(src: &[u8], who: Option<&Caller>) -> Result<B, Fail> {
    let path = CString::new(src).map_err(|_| Crash("embedded null byte".into()))?;
    let mut fds = [0 as libc::c_int; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(os_crash("pipe").into());
    }
    let (r, w) = (fds[0], fds[1]);
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(os_crash("fork").into());
    }
    if pid == 0 {
        // the child: only system calls from here on
        unsafe {
            libc::close(r);
            libc::_exit(read_child(&path, who, w));
        }
    }
    unsafe { libc::close(w) };
    let mut out = vec![];
    let mut f = unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(r) };
    let read = f.read_to_end(&mut out);
    drop(f);
    let mut status = 0;
    while unsafe { libc::waitpid(pid, &mut status, 0) } < 0 {
        if errno() != libc::EINTR {
            return Err(os_crash("waitpid").into());
        }
    }
    if read.is_err() || status != 0 || out.is_empty() {
        return Err(Crash(format!("could not read {}", show(src))).into());
    }
    match out[0] {
        b'O' => Ok(out.split_off(1)),
        b'L' | b'N' => Err(refused(&[src, b": not a regular file"])),
        _ => {
            let n: i32 = std::str::from_utf8(&out[1..]).ok().and_then(|s| s.parse().ok()).unwrap_or(0);
            let who_name: &[u8] = who.map(|c| c.name.as_slice()).unwrap_or(b"root");
            Err(refused(&[src, b": ", who_name, b" cannot read it (", py::strerror(n).as_bytes(), b")"]))
        }
    }
}

/// In the forked child: become the caller, open, send "O" and the bytes, or "L" (a link), "N" (not a regular file), "E<errno>".
unsafe fn read_child(path: &CString, who: Option<&Caller>, w: libc::c_int) -> libc::c_int {
    unsafe fn put(w: libc::c_int, mut b: &[u8]) -> bool {
        while !b.is_empty() {
            let n = libc::write(w, b.as_ptr().cast(), b.len());
            if n < 0 {
                if *libc::__errno_location() == libc::EINTR {
                    continue;
                }
                return false;
            }
            b = &b[n as usize..];
        }
        true
    }
    if let Some(c) = who {
        if libc::setgroups(c.groups.len(), c.groups.as_ptr()) != 0 || libc::setgid(c.gid) != 0 || libc::setuid(c.uid) != 0 {
            return 1;
        }
    }
    let fd = libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    if fd < 0 {
        let e = *libc::__errno_location();
        if e == libc::ELOOP {
            return if put(w, b"L") { 0 } else { 1 };
        }
        let mut buf = [0u8; 16];
        buf[0] = b'E';
        let mut digits = [0u8; 12];
        let (mut k, mut v) = (0, e.unsigned_abs());
        loop {
            digits[k] = b'0' + (v % 10) as u8;
            k += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        for i in 0..k {
            buf[1 + i] = digits[k - 1 - i];
        }
        return if put(w, &buf[..1 + k]) { 0 } else { 1 };
    }
    let mut st: libc::stat = std::mem::zeroed();
    if libc::fstat(fd, &mut st) != 0 {
        return 1;
    }
    if st.st_mode & libc::S_IFMT != libc::S_IFREG {
        return if put(w, b"N") { 0 } else { 1 };
    }
    if !put(w, b"O") {
        return 1;
    }
    let mut buf = [0u8; 65536];
    loop {
        let n = libc::read(fd, buf.as_mut_ptr().cast(), buf.len());
        if n < 0 {
            if *libc::__errno_location() == libc::EINTR {
                continue;
            }
            return 1;
        }
        if n == 0 {
            return 0;
        }
        if !put(w, &buf[..n as usize]) {
            return 1;
        }
    }
}

struct Fd(libc::c_int);

impl Drop for Fd {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

fn cstr(b: &[u8]) -> Result<CString, Crash> {
    CString::new(b).map_err(|_| Crash("embedded null byte".into()))
}

/// The folder `path`, its missing folders made (0755), never through a link anywhere on the way.
fn open_dirs(path: &[u8]) -> Result<Fd, Fail> {
    let mut fd = Fd(unsafe { libc::open(c"/".as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) });
    if fd.0 < 0 {
        return Err(os_crash("/").into());
    }
    let mut here = B::new();
    for name in path.split(|&c| c == b'/').skip(1) {
        here.push(b'/');
        here.extend_from_slice(name);
        let c = cstr(name)?;
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatat(fd.0, c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } != 0 {
            if errno() != libc::ENOENT {
                return Err(os_crash(&show(&here)).into());
            }
            if unsafe { libc::mkdirat(fd.0, c.as_ptr(), 0o755) } != 0
                || unsafe { libc::fstatat(fd.0, c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } != 0
            {
                return Err(os_crash(&show(&here)).into());
            }
        }
        if st.st_mode & libc::S_IFMT == libc::S_IFLNK {
            return Err(refused(&[&here, b" is a link"]));
        }
        let next = unsafe { libc::openat(fd.0, c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
        if next < 0 {
            return Err(os_crash(&show(&here)).into());
        }
        fd = Fd(next);
    }
    Ok(fd)
}

/// name in the folder dir: written beside it as NAME.agent-tmp, owned by root with mode, then moved over it.
fn put(dir: &Fd, name: &[u8], data: &[u8], mode: u32) -> Result<(), Crash> {
    let tmp = cstr(&cat(&[name, b".agent-tmp"]))?;
    let target = cstr(name)?;
    // one left by a deploy that was killed
    if unsafe { libc::unlinkat(dir.0, tmp.as_ptr(), 0) } != 0 && errno() != libc::ENOENT {
        return Err(os_crash("agent-tmp"));
    }
    let fd = unsafe {
        libc::openat(dir.0, tmp.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC, mode)
    };
    if fd < 0 {
        return Err(os_crash("agent-tmp"));
    }
    let mut f = unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(fd) };
    let done = (|| -> Result<(), Crash> {
        f.write_all(data)?;
        if unsafe { libc::fchmod(fd, mode) } != 0 || unsafe { libc::fchown(fd, 0, 0) } != 0 {
            return Err(os_crash("agent-tmp"));
        }
        f.flush()?;
        drop(f);
        if unsafe { libc::renameat(dir.0, tmp.as_ptr(), dir.0, target.as_ptr()) } != 0 {
            return Err(os_crash("agent-tmp"));
        }
        Ok(())
    })();
    if done.is_err() {
        unsafe { libc::unlinkat(dir.0, tmp.as_ptr(), 0) };
    }
    done
}

fn dirname(p: &[u8]) -> &[u8] {
    match p.iter().rposition(|&c| c == b'/') {
        Some(0) => b"/",
        Some(i) => &p[..i],
        None => b"",
    }
}

fn deploy(src: &[u8], dest: &[u8], mode: u32, env: &[(OsString, OsString)]) -> Result<i32, Fail> {
    let who = caller(std::env::var_os("SUDO_UID"), std::env::var_os("SUDO_GID"))?;
    let data = read_as(src, who.as_ref())?;
    let dir = open_dirs(dirname(dest))?;
    put(&dir, basename(dest), &data, mode)?;
    drop(dir);
    log(&cat(&[b"deploy: ", src, b" -> ", dest, format!(" (0o{mode:o})").as_bytes()]), env)?;
    Ok(0)
}

// ---------------------------------------------------------------- the program

pub fn main(argv: &[OsString]) -> i32 {
    let args: Vec<B> = argv.iter().map(|a| a.as_bytes().to_vec()).collect();
    if args.len() < 2 || args[1] == b"-h" || args[1] == b"--help" {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(DOC.as_bytes()).and_then(|_| out.write_all(b"\n")).and_then(|_| out.flush());
        return 0;
    }
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("labyrinth-agent: runs as root through sudo (claude-sudo labyrinth-agent ...)");
        return 1;
    }
    // the caller's umask (sudo keeps a stricter one) would make deploy's folders root-only
    unsafe { libc::umask(0o022) };
    let env = child_env(std::env::vars_os());
    let (cmd, rest) = (&args[1], &args[2..]);
    let mut world = Real { env: &env, stage: None, who: None };
    let runs = |list: &[Vec<B>]| -> Result<i32, Fail> {
        let mut rc = 0;
        for a in list {
            rc = run(a, &env)?;
        }
        Ok(rc)
    };
    let outcome = decide(cmd, rest, &mut world).and_then(|plan| match plan {
        Plan::Runs(list) => runs(&list),
        Plan::Install { note, runs: list } => {
            log(&note, &env)?;
            runs(&list)
        }
        Plan::Deploy { src, dest, mode } => deploy(&src, &dest, mode, &env),
    });
    world.discard();
    match outcome {
        Ok(rc) => rc,
        Err(Fail::Refused(why)) => {
            if let Err(c) = log(&cat(&[b"refused: ", cmd, b" ", &join(rest, b" "), b": ", &why]), &env) {
                return py::crashed("labyrinth-agent", &c);
            }
            eprintln!("labyrinth-agent: refused: {}", show(&why));
            2
        }
        Err(Fail::Crash(c)) => py::crashed("labyrinth-agent", &c),
    }
}

pub mod audit;

#[cfg(test)]
mod tests;
