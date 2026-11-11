//! labyrinth-rollback: puts the whole system back to a snapshot in one command, and undoes that. The bash script of the same name
//! (profile/airootfs/usr/local/bin/labyrinth-rollback) with the same commands, options, output, log lines, calls and exit codes;
//! its help is src/doc/labyrinth-rollback.txt. The layout it works on, and why it renames subvolumes by hand rather than through
//! `snapper rollback`, is in the script's header.
//!
//! mount, umount, btrfs, findmnt, lsblk, logger and cp are still those programs (each started with its arguments, never through a
//! shell); the rest is done here: reading snapper's info.xml (the script's sed), the snapshot list's order (ls | grep | sort -n),
//! comparing GRUB's modules (diff -rq), the renames (mv), the log lines (date) and sync.
//!
//! Not 1:1:
//!   - Every rename refuses to replace or enter anything already there (renameat2 with RENAME_NOREPLACE): `mv A B` moved A into B
//!     when B was a folder. The script's `undo` did not check that its @rolled-back-DATE name was free (fixed in both: it says so
//!     and stops), and a check made before the "yes" question could be stale after it.
//!   - `--device` with no value is a usage error (exit code 2); the script stopped silently with exit code 1 (fixed in both).
//!   - A snapshot's date is read in snapper's own form (YYYY-MM-DD HH:MM:SS, UTC), also with one-digit fields, a two-digit year,
//!     a fraction of a second or without the seconds or the time; GNU date took many more forms. Anything else is shown as written
//!     (as the script did when date could not read it).
//!   - At the "yes" question, end of input (Ctrl+D) stops with "stopped; nothing was changed" (the script ended silently, exit 1).
//!   - When /var/log cannot be written the log line is skipped quietly (bash printed its redirection error).
//!
//! Fixed in both: a snapshot without info.xml (an interrupted snapper run leaves one) made `N` and `N --dry-run` stop silently
//! with exit code 2 (sed's failure reached `set -e` through pipefail); its date and description are unknown now, as in `list`.

use crate::sh::{self, Io};
use std::fs;

pub const DOC: &str = include_str!("doc/labyrinth-rollback.txt");
const LOG: &str = "/var/log/labyrinth-rollback.log";

#[derive(Default)]
struct St {
    device: String,
    dry: bool,
    yes: bool,
    cmd: String,
    arg: String,
    top: String,
    mounted: bool,
}

/// Err(code): stop with that exit code (after the clean-up).
type Res<T> = Result<T, i32>;

fn die<T>(msg: &str) -> Res<T> {
    eprintln!("labyrinth-rollback: {msg}");
    Err(1)
}

fn usage<T>(code: i32) -> Res<T> {
    print!("{DOC}");
    Err(code)
}

fn say(msg: &str) {
    println!("{msg}");
}

/// A line in the log (on @log, so it survives the rollback) and the journal.
fn note(msg: &str) {
    let _ = sh::append(LOG, format!("{} {msg}\n", sh::date("%Y-%m-%d %H:%M:%S")).as_bytes());
    sh::run(&["logger", "-t", "labyrinth-rollback", "-p", "user.warning", "--", msg], Io::Inherit, Io::Null);
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-rollback");
    let mut st = St::default();
    let code = match run(&mut st, args) {
        Ok(()) => 0,
        Err(c) => c,
    };
    cleanup(&st);
    code
}

fn cleanup(st: &St) {
    if st.mounted && !sh::quiet(&["umount", &st.top]) {
        sh::quiet(&["umount", "-l", &st.top]);
    }
    if !st.top.is_empty() {
        let _ = fs::remove_dir(&st.top);
    }
}

fn run(st: &mut St, args: &[String]) -> Res<()> {
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--device" => {
                if i + 1 >= args.len() {
                    return usage(2);
                }
                st.device = args[i + 1].clone();
                i += 1;
            }
            _ if a.starts_with("--device=") => st.device = a["--device=".len()..].to_string(),
            "--dry-run" => st.dry = true,
            "--yes" | "-y" => st.yes = true,
            "-h" | "--help" => return usage(0),
            "list" | "status" | "undo" => {
                if !st.cmd.is_empty() {
                    return usage(2);
                }
                st.cmd = a.to_string();
            }
            _ => {
                if !st.cmd.is_empty() || !sh::digits(a) {
                    return usage(2);
                }
                st.cmd = "rollback".into();
                st.arg = a.to_string();
            }
        }
        i += 1;
    }
    if st.cmd.is_empty() {
        return usage(2);
    }
    if sh::euid() != 0 {
        return die("needs root: run it with sudo");
    }
    match st.cmd.as_str() {
        "list" => list(st),
        "status" => status(st),
        "rollback" => rollback(st),
        _ => undo(st),
    }
}

// ---- the partition

/// Does this partition's top level hold @/.snapshots?
fn has_layout(dev: &str) -> bool {
    let m = match sh::mkdtemp("/run/labyrinth-rollback-probe.XXXXXX") {
        Ok(m) => m,
        Err(_) => return false,
    };
    let mut ok = false;
    if sh::run(&["mount", "-o", "ro,subvolid=5", dev, &m], Io::Inherit, Io::Null) == 0 {
        ok = sh::is_dir(&format!("{m}/@/.snapshots"));
        sh::status(&["umount", &m]);
    }
    if let Err(e) = fs::remove_dir(&m) {
        eprintln!("rmdir: failed to remove '{m}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
    }
    ok
}

/// --device, else the btrfs that holds /home (also when booted into a snapshot, where / is an overlay), else /, else the one
/// btrfs partition on this computer that has @/.snapshots (the USB stick case).
fn find_device() -> Res<String> {
    for mp in ["/home", "/"] {
        let src = sh::output(&["findmnt", "-no", "SOURCE", mp]);
        let fs = sh::output(&["findmnt", "-no", "FSTYPE", mp]);
        if fs == "btrfs" && !src.is_empty() {
            return Ok(src.split('[').next().unwrap_or("").to_string());
        }
    }
    let (_, out) = sh::capture(&["lsblk", "-rpno", "NAME,FSTYPE"], Io::Inherit);
    let mut found = vec![];
    // `while read -r name fstype`: whole lines only (read gives up on a last line without its newline)
    let mut rest = &out[..];
    while let Some(n) = rest.iter().position(|&c| c == b'\n') {
        let line = String::from_utf8_lossy(&rest[..n]).into_owned();
        rest = &rest[n + 1..];
        let line = line.trim_matches(|c| c == ' ' || c == '\t');
        let (name, fstype) = match line.find([' ', '\t']) {
            Some(k) => (&line[..k], line[k..].trim_matches(|c| c == ' ' || c == '\t')),
            None => (line, ""),
        };
        if fstype == "btrfs" && has_layout(name) {
            found.push(name.to_string());
        }
    }
    if found.len() != 1 {
        let list = if found.is_empty() { "none".to_string() } else { found.join(" ") };
        return die(&format!("found {} btrfs partitions with snapshots ({list}); say which with --device", found.len()));
    }
    Ok(found.remove(0))
}

/// The partition's top level (subvolid=5) mounted at a new folder in /run, read-only or not.
fn mount_top(st: &mut St, how: &str) -> Res<()> {
    if st.device.is_empty() {
        st.device = find_device()?;
    }
    if !sh::is_block(&st.device) {
        return die(&format!("{} is not a disk partition", st.device));
    }
    st.top = match sh::mkdtemp("/run/labyrinth-rollback.XXXXXX") {
        Ok(t) => t,
        Err(e) => return die(&format!("could not make a folder in /run: {e}")),
    };
    if sh::status(&["mount", "-o", &format!("{how},subvolid=5"), &st.device, &st.top]) != 0 {
        return die(&format!("could not open the top of {}", st.device));
    }
    st.mounted = true;
    if !sh::is_dir(&format!("{}/@", st.top)) {
        return die(&format!("{} has no @ subvolume: not Labyrinth's layout", st.device));
    }
    Ok(())
}

fn is_subvol(p: &str) -> bool {
    sh::quiet(&["btrfs", "subvolume", "show", p])
}

// ---- snapper's info.xml

/// The script's `sed -n "s:.*<TAG>\(.*\)</TAG>.*:\1:p" FILE | head -1`: on the first line that has <TAG> before a </TAG>, what
/// lies between the last such <TAG> and the last </TAG> (sed's longest matches). A line that is not UTF-8 never matched.
pub fn xml(path: &str, tag: &str) -> String {
    let b = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    for line in sh::lines(&b) {
        let line = match std::str::from_utf8(line) {
            Ok(l) => l,
            Err(_) => continue,
        };
        if let Some(j) = line.rfind(&close) {
            if let Some(i) = line[..j].rfind(&open) {
                return line[i + open.len()..j].to_string();
            }
        }
    }
    String::new()
}

/// Snapper's time (UTC) as local "YYYY-MM-DD HH:MM", or None when it is not a date written as snapper writes one.
pub fn local_time(d: &str) -> Option<String> {
    let d = d.trim_matches(|c: char| c == ' ' || c == '\t');
    let (day, time) = match d.find(' ') {
        Some(k) => (&d[..k], d[k..].trim_start_matches(' ')),
        None => (d, ""),
    };
    let num = |s: &str, max_len: usize| -> Option<i64> {
        if s.is_empty() || s.len() > max_len || !s.bytes().all(|c| c.is_ascii_digit()) {
            None
        } else {
            s.parse().ok()
        }
    };
    let ymd: Vec<&str> = day.split('-').collect();
    if ymd.len() != 3 {
        return None;
    }
    let mut year = num(ymd[0], 4)?;
    if ymd[0].len() == 2 {
        year += if year >= 69 { 1900 } else { 2000 };
    }
    let (mon, mday) = (num(ymd[1], 2)?, num(ymd[2], 2)?);
    let (mut h, mut mi, mut s) = (0, 0, 0);
    if !time.is_empty() {
        let hms: Vec<&str> = time.split(':').collect();
        if hms.len() < 2 || hms.len() > 3 {
            return None;
        }
        h = num(hms[0], 2)?;
        mi = num(hms[1], 2)?;
        if hms.len() == 3 {
            let sec = hms[2];
            let whole = match sec.find('.') {
                Some(k) if k > 0 && sec[k + 1..].bytes().all(|c| c.is_ascii_digit()) && sec.len() > k + 1 => &sec[..k],
                Some(_) => return None,
                None => sec,
            };
            s = num(whole, 2)?;
        }
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let dim = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&mon)
        || mday < 1
        || mday > dim[(mon - 1) as usize]
        || h > 23
        || mi > 59
        || s > 59
    {
        return None;
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = (year - 1900) as i32;
    tm.tm_mon = (mon - 1) as i32;
    tm.tm_mday = mday as i32;
    tm.tm_hour = h as i32;
    tm.tm_min = mi as i32;
    tm.tm_sec = s as i32;
    let t = unsafe { libc::timegm(&mut tm) };
    Some(crate::py::strftime_at("%Y-%m-%d %H:%M", t as i64))
}

/// The script's `when`: the snapshot's date in local time, or as written when it cannot be read as one.
fn when(info: &str) -> String {
    let d = xml(info, "date");
    if d.is_empty() {
        return d;
    }
    local_time(&d).unwrap_or(d)
}

// ---- GRUB's modules

/// `diff -rq A B` finds no difference: the same names, folders in the same places, files with the same bytes (links followed);
/// anything that cannot be read, or that is neither a folder nor a file, counts as a difference.
pub fn same_tree(a: &str, b: &str) -> bool {
    fn names(d: &str) -> Option<Vec<std::ffi::OsString>> {
        let mut v: Vec<_> = fs::read_dir(d).ok()?.map(|e| e.map(|e| e.file_name())).collect::<Result<_, _>>().ok()?;
        v.sort();
        Some(v)
    }
    let (na, nb) = match (names(a), names(b)) {
        (Some(x), Some(y)) => (x, y),
        _ => return false,
    };
    if na != nb {
        return false;
    }
    na.iter().all(|n| {
        let (pa, pb) = (format!("{a}/{}", n.to_string_lossy()), format!("{b}/{}", n.to_string_lossy()));
        match (fs::metadata(&pa), fs::metadata(&pb)) {
            (Ok(x), Ok(y)) if x.is_dir() && y.is_dir() => same_tree(&pa, &pb),
            (Ok(x), Ok(y)) if x.is_file() && y.is_file() => {
                x.len() == y.len() && matches!((fs::read(&pa), fs::read(&pb)), (Ok(p), Ok(q)) if p == q)
            }
            _ => false,
        }
    })
}

/// GRUB's modules in `now` (the system now) and `next` (the one that will boot) are not the same.
fn grub_differs(now: &str, next: &str) -> bool {
    let (a, b) = (format!("{now}/boot/grub/x86_64-efi"), format!("{next}/boot/grub/x86_64-efi"));
    sh::is_dir(&a) && sh::is_dir(&format!("{next}/boot/grub")) && !same_tree(&a, &b)
}

fn confirm(st: &St, what: &str) -> Res<()> {
    if st.yes {
        return Ok(());
    }
    if !sh::stdin_is_terminal() {
        return die("not confirmed: add --yes (or run it in a terminal); nothing was changed");
    }
    eprint!("{what} Type yes to go ahead: ");
    let mut a = String::new();
    match std::io::stdin().read_line(&mut a) {
        Ok(n) if n > 0 && a.trim_matches([' ', '\t', '\n']) == "yes" => Ok(()),
        _ => die("stopped; nothing was changed"),
    }
}

fn mv(from: &str, to: &str) -> Res<()> {
    sh::rename_new(from, to).or_else(|e| die(&format!("could not move {from} to {to}: {e}")))
}

/// .snapshots from `from` into `to`. A snapshot of @ holds an empty folder where the nested .snapshots subvolume was.
fn move_snapshots(from: &str, to: &str) -> Res<()> {
    if !sh::is_dir(&format!("{from}/.snapshots")) {
        return Ok(());
    }
    let dst = format!("{to}/.snapshots");
    if sh::exists(&dst) {
        if is_subvol(&dst) {
            return die(&format!("{dst} is already a subvolume; stopping before anything is mixed up"));
        }
        if let Err(e) = fs::remove_dir(&dst) {
            eprintln!("rmdir: failed to remove '{dst}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
            return die(&format!("{dst} is not empty; stopping"));
        }
    }
    mv(&format!("{from}/.snapshots"), &dst)
}

/// GRUB's boot loader on the EFI partition (in no snapshot) loads its modules from @/boot/grub: if grub-install ran after the
/// snapshot, the system that boots must have the current modules, or the boot menu may not start.
fn carry_grub(from: &str, to: &str) -> Res<()> {
    if grub_differs(from, to) {
        let dst = format!("{to}/boot/grub/x86_64-efi");
        if let Err(e) = sh::rm_rf(&dst) {
            return die(&format!("could not remove {dst}: {e}"));
        }
        if sh::status(&["cp", "-a", "--", &format!("{from}/boot/grub/x86_64-efi"), &dst]) != 0 {
            return die(&format!("could not copy GRUB's modules into {dst}"));
        }
        note("carried GRUB's x86_64-efi modules over from the previous system");
    }
    Ok(())
}

// ---- the commands

fn list(st: &mut St) -> Res<()> {
    mount_top(st, "ro")?;
    let dir = format!("{}/@/.snapshots", st.top);
    if !sh::is_dir(&dir) {
        return die(&format!("no snapshots on {}", st.device));
    }
    println!("{} {} {} Description", sh::pad("Number", 6), sh::pad("Date", 17), sh::pad("Kind", 8));
    let mut nums: Vec<String> = match fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| sh::digits(n)).collect(),
        Err(_) => vec![],
    };
    // sort -n: by value (leading zeros do not count), then by the bytes
    nums.sort_by(|a, b| {
        let (x, y) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
        x.len().cmp(&y.len()).then_with(|| x.cmp(y)).then_with(|| a.cmp(b))
    });
    for f in nums {
        if !sh::is_dir(&format!("{dir}/{f}/snapshot")) {
            continue;
        }
        let info = format!("{dir}/{f}/info.xml");
        println!("{} {} {} {}", sh::pad(&f, 6), sh::pad(&when(&info), 17), sh::pad(&xml(&info, "type"), 8), xml(&info, "description"));
    }
    Ok(())
}

fn status(st: &mut St) -> Res<()> {
    mount_top(st, "ro")?;
    say(&format!("Partition: {}", st.device));
    let mark = format!("{}/@/.labyrinth-rollback", st.top);
    if sh::is_file(&mark) {
        say(&format!("The system (@) was made by a rollback: {}", sh::cat(&mark).unwrap_or_default()));
    } else {
        say("The system (@) has not been rolled back.");
    }
    let mut kept: Vec<String> = sh::glob(&st.top, |n| n.starts_with("@broken-"));
    kept.extend(sh::glob(&st.top, |n| n.starts_with("@rolled-back-")));
    let kept: Vec<&str> = kept.iter().filter(|k| sh::exists(k)).map(|k| sh::base(k)).collect();
    if kept.is_empty() {
        say("No earlier systems are kept.");
    } else {
        say(&format!("Earlier systems kept (delete them when sure; see the recovery guide): {}", kept.join(" ")));
    }
    Ok(())
}

fn rollback(st: &mut St) -> Res<()> {
    let n = st.arg.clone();
    mount_top(st, if st.dry { "ro" } else { "rw" })?;
    let top = st.top.clone();
    let info = format!("{top}/@/.snapshots/{n}/info.xml");
    let snap = format!("{top}/@/.snapshots/{n}/snapshot");
    if !is_subvol(&snap) {
        return die(&format!("snapshot {n} does not exist on {} (see: labyrinth-rollback list)", st.device));
    }
    let desc = xml(&info, "description");
    let date = when(&info);
    let kept = format!("@broken-{}", sh::date("%Y-%m-%d-%H%M%S"));
    if sh::exists(&format!("{top}/{kept}")) {
        return die(&format!("{kept} already exists"));
    }
    let grub = grub_differs(&format!("{top}/@"), &snap);
    let or = |s: &str, d: &str| if s.is_empty() { d.to_string() } else { s.to_string() };
    say(&format!(
        "Roll the system on {} back to snapshot {n} ({}, {}):",
        st.device,
        or(&date, "unknown date"),
        or(&desc, "no description")
    ));
    say(&format!("  1. the current system is kept as {kept}"));
    say(&format!("  2. a writable copy of snapshot {n} becomes the system"));
    say("  3. the snapshot collection moves into it");
    if grub {
        say("  4. GRUB was reinstalled after this snapshot: the current GRUB files go into it");
    }
    say("Your home folder, /var/cache and /var/log are not touched. 'labyrinth-rollback undo' reverses this.");
    if st.dry {
        say("Dry run: nothing was changed.");
        return Ok(());
    }
    confirm(st, "It takes effect at the next restart.")?;
    note(&format!("rollback to snapshot {n} ({date}, {desc}) on {}; the previous system is kept as {kept}", st.device));
    mv(&format!("{top}/@"), &format!("{top}/{kept}"))?;
    let copied = sh::run(
        &["btrfs", "subvolume", "snapshot", &format!("{top}/{kept}/.snapshots/{n}/snapshot"), &format!("{top}/@")],
        Io::Null,
        Io::Inherit,
    );
    if copied != 0 {
        mv(&format!("{top}/{kept}"), &format!("{top}/@"))?;
        note(&format!("copying snapshot {n} failed; the previous system was put back"));
        return die(&format!("could not copy snapshot {n}; the system was put back as it was"));
    }
    move_snapshots(&format!("{top}/{kept}"), &format!("{top}/@"))?;
    carry_grub(&format!("{top}/{kept}"), &format!("{top}/@"))?;
    let mark = format!("from snapshot {n} ({date}, {desc}) at {}; previous system: {kept}\n", sh::date("%Y-%m-%d %H:%M:%S"));
    if let Err(e) = sh::write(&format!("{top}/@/.labyrinth-rollback"), mark.as_bytes()) {
        return die(&format!("could not write {top}/@/.labyrinth-rollback: {e}"));
    }
    unsafe { libc::sync() };
    note(&format!("rollback to snapshot {n} done"));
    say(&format!("Done. Restart to start the system as it was in snapshot {n}."));
    say(&format!("When everything works, {kept} can be deleted to free space (see the recovery guide); until then it is the way back."));
    Ok(())
}

fn undo(st: &mut St) -> Res<()> {
    mount_top(st, if st.dry { "ro" } else { "rw" })?;
    let top = st.top.clone();
    let last = match sh::glob(&top, |n| n.starts_with("@broken-")).pop() {
        Some(l) => l,
        None => return die("there is no earlier system to put back (no @broken-*)"),
    };
    let aside = format!("@rolled-back-{}", sh::date("%Y-%m-%d-%H%M%S"));
    say(&format!("Put back {} as the system on {}; the current one is kept as {aside}.", sh::base(&last), st.device));
    if st.dry {
        say("Dry run: nothing was changed.");
        return Ok(());
    }
    if sh::exists(&format!("{top}/{aside}")) {
        return die(&format!("{aside} already exists"));
    }
    confirm(st, "It takes effect at the next restart.")?;
    note(&format!("undo: {} becomes the system again; the current one is kept as {aside}", sh::base(&last)));
    mv(&format!("{top}/@"), &format!("{top}/{aside}"))?;
    mv(&last, &format!("{top}/@"))?;
    move_snapshots(&format!("{top}/{aside}"), &format!("{top}/@"))?;
    carry_grub(&format!("{top}/{aside}"), &format!("{top}/@"))?;
    if let Err(e) = sh::rm_f(&format!("{top}/@/.labyrinth-rollback")) {
        return die(&format!("could not remove {top}/@/.labyrinth-rollback: {e}"));
    }
    unsafe { libc::sync() };
    note("undo done");
    say("Done. Restart to start the system that was set aside.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_takes_what_sed_took() {
        let t = crate::testutil::TempDir::new();
        let p = t.p("info.xml");
        fs::write(&p, "<snapshot>\n  <date>2026-10-26 22:00:01</date>\n  <description>a <b> c</description>\n</snapshot>\n").unwrap();
        assert_eq!(xml(&p, "date"), "2026-10-26 22:00:01");
        assert_eq!(xml(&p, "description"), "a <b> c");
        assert_eq!(xml(&p, "type"), "");
        fs::write(&p, "<d>a</d><d>b</d>\n<d>c</d>\n").unwrap();
        assert_eq!(xml(&p, "d"), "b");
        fs::write(&p, b"<d>\xff</d>\n<d>ok</d>\n").unwrap();
        assert_eq!(xml(&p, "d"), "ok");
        assert_eq!(xml(&t.p("none"), "d"), "");
    }

    #[test]
    fn snapper_dates() {
        let _g = crate::testutil::env_lock();
        std::env::set_var("TZ", "America/Denver");
        assert_eq!(local_time("2026-10-26 22:00:01").as_deref(), Some("2026-10-26 16:00"));
        assert_eq!(local_time("2026-10-26").as_deref(), Some("2026-10-25 18:00"));
        assert_eq!(local_time("2026-1-2 3:04:05").as_deref(), Some("2026-01-01 20:04"));
        assert_eq!(local_time("26-10-26 22:00:01").as_deref(), Some("2026-10-26 16:00"));
        assert_eq!(local_time("2026-10-26 22:00:01.5").as_deref(), Some("2026-10-26 16:00"));
        for bad in ["2026-02-30 10:00:00", "2026-10-26 24:00:00", "2026-10-26 23:59:60", "yesterday", "", "2026-10-26 22"] {
            assert_eq!(local_time(bad), None, "{bad}");
        }
    }

    #[test]
    fn trees() {
        let t = crate::testutil::TempDir::new();
        for (d, body) in [("a", "x"), ("b", "x")] {
            fs::create_dir_all(t.p(&format!("{d}/sub"))).unwrap();
            fs::write(t.p(&format!("{d}/sub/m.mod")), body).unwrap();
        }
        assert!(same_tree(&t.p("a"), &t.p("b")));
        fs::write(t.p("b/sub/m.mod"), "y").unwrap();
        assert!(!same_tree(&t.p("a"), &t.p("b")));
        fs::write(t.p("b/sub/m.mod"), "x").unwrap();
        fs::write(t.p("b/extra"), "").unwrap();
        assert!(!same_tree(&t.p("a"), &t.p("b")));
        assert!(!same_tree(&t.p("a"), &t.p("missing")));
    }
}
