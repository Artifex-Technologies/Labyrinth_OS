//! labyrinth-bluetooth-privacy [FILE [OS-RELEASE]]: Bluetooth address privacy on by default (R19 of
//! my reference plan, in my private plans: docs/plans/PLAN_REFERENCE_2026-11.md). Without `Privacy` in
//! /etc/bluetooth/main.conf, Bluetooth Low Energy announces and connects
//! with this computer's fixed address, which anything nearby can record to follow it from place to place; with
//! `Privacy = device` the address changes every 15 minutes and only paired devices recognise it. Heimdall's hardening check
//! `bluetooth-privacy-off` warns until the file says so. BlueZ reads no drop-in folder, so the line goes into main.conf itself.
//!
//! Run as root by the labyrinth-tools scriptlet (post_install, post_upgrade) and by its pacman hook after bluez is installed or
//! updated (the hook covers the image, where bluez may come after labyrinth-tools). Only on Labyrinth (an ID=labyrinth line in
//! /etc/os-release). It adds `Privacy = device` (with a comment line above it) right after the first `[General]` line, or a
//! `[General]` group with it at the end when there is none, and only when no uncommented `Privacy` line is there: one the owner
//! wrote, `Privacy = off` included, is theirs. Everything else stays byte for byte. It leaves the file alone when it is missing
//! (bluez not installed), is not a plain file (a link is never followed), is not UTF-8, or has a line GLib cannot read (BlueZ
//! then ignores the whole file; Heimdall's check names the line). The new file is written whole beside the old one with its mode
//! and owner, and renamed over it. bluetoothd reads the file when it starts, so it applies at the next boot (or restart of
//! bluetooth.service); this program restarts nothing. It always exits 0, as a scriptlet must. FILE and OS-RELEASE are for tests.
//!
//! The file is read as GLib reads key files for BlueZ (Heimdall `internal/hardening/bluetooth.go`, keyFile, checked against GLib
//! itself): a line is left-trimmed and a "\r" before "\n" dropped; empty and "#" lines are comments; "[name]" with only spaces
//! after it is a group; "key = value" has the spaces around "=" removed (the value keeps the rest, a trailing "# comment"
//! included); anything else, or a key before the first group, makes GLib refuse the whole file. The last value of a key wins.

use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

const CONF: &str = "/etc/bluetooth/main.conf";
const OS_RELEASE: &str = "/etc/os-release";
const OURS: &str = "# Labyrinth OS (labyrinth-bluetooth-privacy): Bluetooth uses a private address that changes. For the fixed\n\
                    # address write off in place of device; a deleted line comes back with the next update.\n\
                    Privacy = device\n";

/// g_ascii_isspace.
const GLIB_SPACE: &[char] = &[' ', '\t', '\n', '\x0b', '\x0c', '\r'];

/// ARGS as the program got them (its own name first).
pub fn main(args: &[String]) -> i32 {
    let args = args.get(1..).unwrap_or_default();
    let file = args.first().map(String::as_str).unwrap_or(CONF);
    let os_release = args.get(1).map(String::as_str).unwrap_or(OS_RELEASE);
    let on_labyrinth = std::fs::read(os_release).is_ok_and(|t| crate::os_release::is_id(&String::from_utf8_lossy(&t), "labyrinth"));
    if !on_labyrinth {
        return 0;
    }
    if let Err(why) = apply(file) {
        eprintln!("labyrinth-bluetooth-privacy: {file}: {why}; left as it is");
    }
    0
}

/// Adds the line to FILE when it should (see the top of this file). Ok also when nothing needed doing; Err says why a file
/// that is there was left alone.
fn apply(file: &str) -> Result<(), String> {
    let path = std::path::Path::new(file);
    let parent = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    if !std::fs::symlink_metadata(parent).is_ok_and(|m| m.is_dir()) {
        return Err("its folder is not a plain folder".into());
    }
    // O_NOFOLLOW: a link planted at the file's place is refused, not followed; the checks below are on what was opened
    let mut f = match std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Err("it is a link".into()),
        Err(e) => return Err(e.to_string()),
    };
    let meta = f.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("it is not a plain file".into());
    }
    let mut raw = Vec::new();
    f.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let text = String::from_utf8(raw).map_err(|_| "it is not UTF-8".to_string())?;
    let Some(new) = edit(&text)? else { return Ok(()) };
    crate::sh::replace_file(file, new.as_bytes(), meta.permissions().mode() & 0o7777, Some((meta.uid(), meta.gid())))
        .map_err(|e| e.to_string())
}

/// What GLib makes of one line of a key file.
#[derive(Debug, PartialEq)]
enum Line<'a> {
    Comment,
    Group(&'a str),
    Key(&'a str),
    Bad,
}

fn classify(line: &str) -> Line<'_> {
    let line = line.strip_suffix('\r').unwrap_or(line).trim_start_matches(GLIB_SPACE);
    if line.is_empty() || line.starts_with('#') {
        return Line::Comment;
    }
    if let Some(rest) = line.strip_prefix('[') {
        return match rest.find(']') {
            Some(end) if rest[end + 1..].trim_start_matches(GLIB_SPACE).is_empty() => Line::Group(&rest[..end]),
            _ => Line::Bad,
        };
    }
    match line.find('=') {
        Some(eq) if eq > 0 => Line::Key(line[..eq].trim_end_matches(GLIB_SPACE)),
        _ => Line::Bad,
    }
}

/// The file with `Privacy = device` added, None when it stays as it is (a Privacy line is there), Err when GLib would refuse
/// it (BlueZ then reads none of it, and nothing added here would count).
pub fn edit(text: &str) -> Result<Option<String>, String> {
    let mut started = false;
    let mut general: Option<usize> = None; // the byte offset just after the first [General] line
    let mut offset = 0;
    for (i, piece) in text.split_inclusive('\n').enumerate() {
        let line = piece.strip_suffix('\n').unwrap_or(piece);
        match classify(line) {
            Line::Comment => {}
            Line::Group(name) => {
                started = true;
                if name == "General" && general.is_none() {
                    general = Some(offset + piece.len());
                }
            }
            Line::Key(_) if !started => {
                return Err(format!("line {} is a setting before any [group], so BlueZ reads none of the file", i + 1))
            }
            Line::Key("Privacy") => return Ok(None),
            Line::Key(_) => {}
            Line::Bad => return Err(format!("line {} is one BlueZ cannot read, so it reads none of the file", i + 1)),
        }
        offset += piece.len();
    }
    let mut out = String::with_capacity(text.len() + OURS.len() + 16);
    match general {
        Some(at) => {
            out.push_str(&text[..at]);
            if !text[..at].ends_with('\n') {
                out.push('\n'); // [General] was the last line, without a newline
            }
            out.push_str(OURS);
            out.push_str(&text[at..]);
        }
        None => {
            out.push_str(text);
            if !text.is_empty() && !text.ends_with('\n') {
                out.push('\n');
            }
            if !text.is_empty() {
                out.push('\n');
            }
            out.push_str("[General]\n");
            out.push_str(OURS);
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The value BlueZ gets for [General] Privacy, read as GLib reads it (the last one wins), None when unset; Err: GLib refuses
    /// the file.
    fn privacy(text: &str) -> Result<Option<String>, ()> {
        let (mut group, mut val) = (None, None);
        for line in text.split('\n') {
            match classify(line) {
                Line::Comment => {}
                Line::Group(g) => group = Some(g),
                Line::Key(_) if group.is_none() => return Err(()),
                Line::Key(k) => {
                    if group == Some("General") && k == "Privacy" {
                        let l = line.strip_suffix('\r').unwrap_or(line).trim_start_matches(GLIB_SPACE);
                        val = Some(l[l.find('=').unwrap() + 1..].trim_start_matches(GLIB_SPACE).to_string());
                    }
                }
                Line::Bad => return Err(()),
            }
        }
        Ok(val)
    }

    // the start of bluez 5's own main.conf: every option commented out
    const UPSTREAM: &str = "[General]\n\n# Default adapter name\n# Defaults to 'BlueZ X.YZ'\n#Name = BlueZ\n\n\
                            # Default privacy setting.\n#Privacy = off\n\n[BR]\n#PageScanType=\n\n[LE]\n#MinAdvertisementInterval=\n";

    #[test]
    fn added_under_general_and_nothing_else_changes() {
        let out = edit(UPSTREAM).unwrap().unwrap();
        assert_eq!(out, format!("[General]\n{OURS}\n# Default adapter name\n# Defaults to 'BlueZ X.YZ'\n#Name = BlueZ\n\n# Default privacy setting.\n#Privacy = off\n\n[BR]\n#PageScanType=\n\n[LE]\n#MinAdvertisementInterval=\n"));
        assert_eq!(out.replacen(OURS, "", 1), UPSTREAM, "byte for byte apart from the added lines");
        assert_eq!(privacy(&out), Ok(Some("device".into())));
        assert_eq!(edit(&out), Ok(None), "the second run changes nothing");
    }

    #[test]
    fn a_privacy_line_of_the_owner_stays() {
        for t in [
            "[General]\nPrivacy = off\n",
            "[General]\n  Privacy=network\n",
            "[General]\nPrivacy = device # mine\n", // the comment is part of the value: BlueZ takes it as unknown, but it is the owner's
            "[General]\nPrivacy\t= \n",
            "[General]\nName = x\n[LE]\nPrivacy = off\n", // in the wrong group, still the owner's line
            "[General]\r\nPrivacy = off\r\n",
        ] {
            assert_eq!(edit(t), Ok(None), "{t:?}");
        }
    }

    #[test]
    fn commented_lines_do_not_count() {
        for t in ["[General]\n#Privacy = device\n", "[General]\n   # Privacy = off\n", "[General]\nPrivacyX = 1\nNamePrivacy = 2\n"] {
            let out = edit(t).unwrap().unwrap();
            assert_eq!(privacy(&out), Ok(Some("device".into())), "{t:?}");
        }
    }

    #[test]
    fn the_last_value_wins_so_the_first_general_is_enough() {
        // a group given twice is one group in GLib; the line goes after the first header, and nothing later sets Privacy
        let out = edit("[General]\nName = a\n[LE]\n[General]\nName = b\n").unwrap().unwrap();
        assert_eq!(out, format!("[General]\n{OURS}Name = a\n[LE]\n[General]\nName = b\n"));
        assert_eq!(privacy(&out), Ok(Some("device".into())));
    }

    #[test]
    fn without_a_general_group_one_is_added_at_the_end() {
        assert_eq!(edit("[LE]\nFoo = 1").unwrap().unwrap(), format!("[LE]\nFoo = 1\n\n[General]\n{OURS}"));
        assert_eq!(edit("").unwrap().unwrap(), format!("[General]\n{OURS}"));
        assert_eq!(edit("# only comments\n").unwrap().unwrap(), format!("# only comments\n\n[General]\n{OURS}"));
        // a [General] that is the last line, without a newline
        assert_eq!(edit("[General]").unwrap().unwrap(), format!("[General]\n{OURS}"));
        assert_eq!(edit("[General]  \t").unwrap().unwrap(), format!("[General]  \t\n{OURS}"));
        for t in ["[LE]\nFoo = 1", "", "[General]", "[General]\r\nName = x\r\n"] {
            assert_eq!(privacy(&edit(t).unwrap().unwrap()), Ok(Some("device".into())), "{t:?}");
        }
    }

    #[test]
    fn a_file_glib_refuses_is_left_alone() {
        for t in [
            "[General]\nnot a setting\n",
            "Privacy = device\n[General]\n", // a key before any group
            "[General] x\n",
            "[General\n",
            "[General]\n= value\n",
            "[General]\r\nName = x\nbad\r\n",
        ] {
            assert!(edit(t).is_err(), "{t:?}");
        }
        assert!(edit("[General]\n\nbad\n").unwrap_err().starts_with("line 3 "));
    }

    #[test]
    fn lines_read_as_glib_reads_them() {
        assert_eq!(classify("  [General]  "), Line::Group("General"));
        assert_eq!(classify("[Gen eral]"), Line::Group("Gen eral"));
        assert_eq!(classify("[General]\r"), Line::Group("General"));
        assert_eq!(classify("\t# x"), Line::Comment);
        assert_eq!(classify("\r"), Line::Comment);
        assert_eq!(classify("Privacy  =device"), Line::Key("Privacy"));
        assert_eq!(classify("[a]b]"), Line::Bad);
        assert_eq!(classify("=x"), Line::Bad);
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("bt-privacy-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn run(d: &std::path::Path, os: &str) -> i32 {
        let osr = d.join("os-release");
        std::fs::write(&osr, os).unwrap();
        main(&["labyrinth-bluetooth-privacy".into(), d.join("main.conf").to_string_lossy().into(), osr.to_string_lossy().into()])
    }

    #[test]
    fn written_whole_with_its_mode_and_only_on_labyrinth() {
        let d = dir("write");
        let f = d.join("main.conf");
        std::fs::write(&f, UPSTREAM).unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o640)).unwrap();
        assert_eq!(run(&d, "NAME=\"LykanOS\"\nID=lykanos\n"), 0);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), UPSTREAM, "another system keeps its own");
        assert_eq!(run(&d, "NAME=\"Labyrinth OS\"\nID=labyrinth\n"), 0);
        assert!(std::fs::read_to_string(&f).unwrap().contains("[General]\n# Labyrinth OS"));
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o7777, 0o640);
        let mut names: Vec<_> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["main.conf", "os-release"], "no temporary file left");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_link_is_never_followed_and_a_missing_file_not_made() {
        let d = dir("link");
        let target = d.join("elsewhere");
        std::fs::write(&target, "[General]\n").unwrap();
        std::os::unix::fs::symlink(&target, d.join("main.conf")).unwrap();
        assert_eq!(run(&d, "ID=labyrinth\n"), 0);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "[General]\n");
        assert!(std::fs::symlink_metadata(d.join("main.conf")).unwrap().file_type().is_symlink());
        std::fs::remove_file(d.join("main.conf")).unwrap();
        assert_eq!(run(&d, "ID=labyrinth\n"), 0);
        assert!(!d.join("main.conf").exists(), "no bluez, no file");
        // a folder whose path goes through a link is not written either
        let real = d.join("real");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(real.join("main.conf"), "[General]\n").unwrap();
        std::os::unix::fs::symlink(&real, d.join("linked")).unwrap();
        let osr = d.join("os-release");
        let conf = d.join("linked/main.conf");
        main(&["x".into(), conf.to_string_lossy().into(), osr.to_string_lossy().into()]);
        assert_eq!(std::fs::read_to_string(real.join("main.conf")).unwrap(), "[General]\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_bluez_cannot_read_or_not_utf8_stays_byte_for_byte() {
        let d = dir("bad");
        let f = d.join("main.conf");
        for content in [b"[General]\nbad line\n".to_vec(), b"[General]\n#\xff\n".to_vec()] {
            std::fs::write(&f, &content).unwrap();
            assert_eq!(run(&d, "ID=\"labyrinth\"\n"), 0);
            assert_eq!(std::fs::read(&f).unwrap(), content);
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
