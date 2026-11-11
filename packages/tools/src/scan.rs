//! labyrinth-scan-downloads: looks for malware where files arrive from outside (Downloads, Desktop, drives, mail attachment
//! caches, ~/.config/labyrinth/scan-folders) with Heimdall's scanner (`heimdalld scan-files`, ClamAV), quarantines what is
//! certain (labyrinth-quarantine) and warns about the rest. The Python script of the same name with the same arguments, files,
//! environment, notifications, log lines and exit codes; its help text is src/doc/labyrinth-scan-downloads.txt.
//!
//!   (nothing)   one scan now (labyrinth-scan.timer)
//!   --full      the whole home folder but caches, game libraries, toolchains, build output and disk images (weekly)
//!   --watch     keep running: scan a folder 4 s after new files stop arriving in it (labyrinth-scan-watch.service)
//!   --off/--on  stop / start the timers and the watcher
//!
//! Not 1:1, on purpose:
//!   - the watcher sleeps until the kernel reports a new file, a scan is due, or the 5-minute look for new places (the script
//!     woke every 2 s to check): a scan starts 4 s after the last new file, where the script started it 4 to 6 s after;
//!   - folder and file names that are not UTF-8 are watched as they are (the script stopped with a traceback at the first such
//!     folder, and systemd restarted it, over and over); in the scanner's report such a name has its bad bytes replaced, as for
//!     any text a tool printed that the script could not decode (it stopped there too).

use crate::py::{self, Crash, Value, R};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::time::Duration;

pub const DOC: &str = include_str!("doc/labyrinth-scan-downloads.txt");

/// What the weekly full scan leaves out, under the home folder and anywhere ("[x]name" is a glob for a folder of that name).
pub const FULL_EXCLUDE_HOME: &[&str] = &[".cache", ".local/share/Steam", ".steam", "Games", ".ollama", ".local/share/containers", ".local/share/flatpak",
    ".var/app", ".cargo", ".rustup", ".npm", "go/pkg", ".local/share/labyrinth/quarantine"];
pub const FULL_EXCLUDE_ANYWHERE: &[&str] = &["[n]ode_modules", "[.]git", "[t]arget", "*.iso", "*.qcow2", "*.img", "*.vdi", "*.vmdk"];

/// The script's settings, read from the environment when it started.
pub struct Ctx {
    pub home: String,
    pub conf: String,
    pub state: String,
    pub heimdalld: String,
    pub quarantine: String,
    pub run_media: String,
    pub sync: bool,
}

impl Ctx {
    pub fn from_env() -> Ctx {
        let home = py::home();
        let conf = py::join(&py::env_or("XDG_CONFIG_HOME", &py::join(&home, ".config")), "labyrinth");
        let state = py::join(&py::env_or("XDG_STATE_HOME", &py::join_all(&[&home, ".local", "state"])), "labyrinth");
        let heimdalld = py::env_get("HEIMDALLD", "heimdalld");
        let here = py::here();
        let sibling = py::join(&here, "labyrinth-quarantine");
        let quarantine = match py::env("LABYRINTH_QUARANTINE") {
            Some(q) if !q.is_empty() => q,
            _ if py::exists(&sibling) => sibling,
            _ => "labyrinth-quarantine".into(),
        };
        let user = py::env_or("USER", py::basename(&home));
        let run_media = py::env_or("LABYRINTH_RUN_MEDIA", &format!("/run/media/{user}"));
        Ctx { home, conf, state, heimdalld, quarantine, run_media, sync: py::env_set("LABYRINTH_SCAN_SYNC") }
    }
}

/// The lines of a list file: stripped, not empty, not starting with "#" (before stripping), with ~ expanded. A missing file is
/// no lines; one that is not UTF-8 stopped the script (a Crash).
pub fn lines(path: &str) -> R<Vec<String>> {
    match py::read_text(path) {
        Ok(t) => Ok(py::lines(&t).into_iter().filter(|l| !py::strip(l).is_empty() && !l.starts_with('#')).map(|l| py::expanduser(py::strip(l))).collect()),
        Err(py::ReadError::Os(_)) => Ok(vec![]),
        Err(py::ReadError::Decode(m)) => Err(Crash(format!("UnicodeDecodeError: {m}"))),
    }
}

/// The places files arrive from outside that exist, each once (links resolved).
pub fn inbound(c: &Ctx) -> R<Vec<String>> {
    let mut cands = vec![
        py::join(&c.home, "Downloads"),
        py::join(&c.home, "Desktop"),
        c.run_media.clone(),
        py::join_all(&[&c.home, ".cache", "thunderbird"]),
        py::join_all(&[&c.home, ".cache", "evolution"]),
        py::join_all(&[&c.home, ".cache", "geary"]),
    ];
    cands.extend(lines(&py::join(&c.conf, "scan-folders"))?);
    let mut out: Vec<String> = vec![];
    for cand in cands {
        let r = py::realpath(&cand);
        if py::isdir(&r) && !out.contains(&r) {
            out.push(r);
        }
    }
    Ok(out)
}

/// scan.conf's settings ("key = value" lines); action defaults to quarantine.
pub fn settings(c: &Ctx) -> R<Vec<(String, String)>> {
    let mut out = vec![("action".to_string(), "quarantine".to_string())];
    for l in lines(&py::join(&c.conf, "scan.conf"))? {
        let (k, _, v) = py::partition(&l, "=");
        let (k, v) = (py::strip(k).to_string(), py::strip(v).to_string());
        match out.iter_mut().find(|(x, _)| *x == k) {
            Some(e) => e.1 = v,
            None => out.push((k, v)),
        }
    }
    Ok(out)
}

/// A line in ~/.local/state/labyrinth/scan.log (private), with the local time.
pub fn log(c: &Ctx, text: &str) -> R<()> {
    py::makedirs(&c.state)?;
    let path = py::join(&c.state, "scan.log");
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().append(true).create(true).mode(0o600).open(&path).map_err(|e| py::os_err(e, &path))?;
    f.write_all(format!("{} {text}\n", py::strftime("%Y-%m-%d %H:%M:%S")).as_bytes()).map_err(|e| py::os_err(e, &path))
}

/// The INFECTED lines of `heimdalld scan-files`: (path, signature).
pub fn parse(stdout: &str) -> Vec<(String, String)> {
    py::splitlines(stdout)
        .into_iter()
        .filter_map(|l| l.strip_prefix("INFECTED "))
        .map(|rest| {
            let (path, _, sig) = py::rpartition(rest, ": ");
            (path.to_string(), sig.to_string())
        })
        .collect()
}

/// A definite malware signature (not possibly unwanted, a heuristic guess, a phishing hint or an odd document).
pub fn confident(sig: &str) -> bool {
    let s = sig.to_lowercase();
    !(s.starts_with("pua.")
        || s.starts_with("heuristics.")
        || s.starts_with("win.test.")
        || s.contains("heuristic")
        || s.contains(".phish")
        || s.starts_with("phishing.")
        || s.starts_with("html.phish")
        || s.contains("encrypted")
        || s.contains("exceeds")
        || s.contains("limit"))
}

/// Only a regular file you own, inside one of the watched places, is ever moved.
pub fn quarantinable(path: &str, roots: &[String]) -> bool {
    let st = match std::fs::symlink_metadata(path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if !py::isfile(path) || py::islink(path) || st.uid() != py::getuid() {
        return false;
    }
    let real = py::realpath(path);
    roots.iter().any(|r| real == *r || real.starts_with(&format!("{}/", r.trim_end_matches('/'))))
}

/// What was warned about already (scan-notified.json). A list of text is the normal case; other JSON is kept as the script would
/// have read it, so the same files are skipped and the same writes fail.
pub fn notified(c: &Ctx) -> R<Vec<Value>> {
    let v = match py::load_json(&py::join(&c.state, "scan-notified.json")) {
        Ok(v) => v,
        Err(_) => return Ok(vec![]),
    };
    let items: Vec<Value> = match v {
        Value::Array(a) => a,
        Value::Object(m) => m.keys().map(|k| Value::String(k.clone())).collect(),
        Value::String(s) => s.chars().map(|ch| Value::String(ch.to_string())).collect(),
        Value::Number(_) => return Err(Crash("TypeError: 'int' object is not iterable".into())),
        Value::Bool(_) => return Err(Crash("TypeError: 'bool' object is not iterable".into())),
        Value::Null => return Err(Crash("TypeError: 'NoneType' object is not iterable".into())),
    };
    let mut out: Vec<Value> = vec![];
    for i in items {
        if matches!(i, Value::Array(_) | Value::Object(_)) {
            return Err(Crash("TypeError: unhashable type in scan-notified.json".into()));
        }
        if !out.contains(&i) {
            out.push(i);
        }
    }
    Ok(out)
}

fn sha_allowed(c: &Ctx, path: &str) -> R<bool> {
    let sha = crate::quarantine::sha256_file(path).unwrap_or_default();
    Ok(py::run(&[c.quarantine.as_str(), "allowed", sha.as_str()])?.code == 0)
}

/// A notification from "Malware scan". Waited for (with a 10-minute limit; its answer) or left running on its own.
fn tell(args: &[&str], wait: bool) -> R<String> {
    let mut cmd: Vec<&str> = vec!["notify-send", "-a", "Malware scan"];
    cmd.extend_from_slice(args);
    if wait {
        let mut full = vec!["timeout", "600"];
        full.extend(cmd);
        return Ok(py::strip(&py::run(&full)?.stdout).to_string());
    }
    py::spawn_detached(&cmd, &[])?;
    Ok(String::new())
}

/// The notification of a quarantined file, with Restore / Delete. notify-send waits for the answer, so this runs in a child
/// process of its own (this program again, with --prompt), unless it is that child or the tests ask to wait.
pub fn prompt_quarantined(c: &Ctx, qid: &str, name: &str, sig: &str) -> R<()> {
    if py::env("LABYRINTH_SCAN_PROMPT").as_deref() == Some("child") || c.sync {
        let body = format!("{name} ({sig}) was moved to quarantine. Nothing was deleted.");
        let ans = tell(&["-u", "critical", "-i", "dialog-warning", "-A", "restore=Restore", "-A", "delete=Delete", "Malware quarantined", &body], true)?;
        if ans == "restore" {
            py::run(&[c.quarantine.as_str(), "restore", qid])?;
        } else if ans == "delete" {
            py::run(&[c.quarantine.as_str(), "delete", qid])?;
        }
        return Ok(());
    }
    let me = std::env::current_exe().map(|p| p.into_os_string()).unwrap_or_else(|_| OsString::from("labyrinth-scan-downloads"));
    let argv: Vec<OsString> = vec![me, "--prompt".into(), qid.into(), name.into(), sig.into()];
    py::spawn_detached(&argv, &[("LABYRINTH_SCAN_PROMPT", "child")])
}

/// Quarantine what is certain, warn about the rest; how many files were found.
pub fn handle(c: &Ctx, found: &[(String, String)], roots: &[String]) -> R<usize> {
    let sets = settings(c)?;
    let act = sets.iter().find(|(k, _)| k == "action").map(|(_, v)| v.clone()).unwrap_or_else(|| "quarantine".into());
    let seen = notified(c)?;
    let mut new: Vec<String> = vec![];
    for (path, sig) in found {
        let name = py::basename(path);
        if py::exists(path) && sha_allowed(c, path)? {
            continue; // you said "keep this" for exactly this content
        }
        if act == "quarantine" && confident(sig) && quarantinable(path, roots) {
            let p = py::run(&[c.quarantine.as_str(), "add", path.as_str(), "--signature", sig.as_str()])?;
            if p.code == 0 {
                let qid = py::strip(&p.stdout).to_string();
                log(c, &format!("QUARANTINED {path}: {sig} (id {qid})"))?;
                prompt_quarantined(c, &qid, name, sig)?;
                continue;
            }
            log(c, &format!("could not quarantine {path}: {}", py::strip(&p.stderr)))?;
        }
        if seen.contains(&Value::String(path.clone())) {
            continue;
        }
        log(c, &format!("INFECTED {path}: {sig}"))?;
        let body = format!("{name}\n({sig}) was not moved or deleted. Check it before opening it.");
        tell(&["-u", "critical", "-i", "dialog-warning", "Possible malware found", &body], c.sync)?;
        if !new.contains(path) {
            new.push(path.clone());
        }
    }
    if !new.is_empty() {
        if seen.iter().any(|v| !matches!(v, Value::String(_))) {
            // the script opened the file for writing (emptying it) before sorting failed
            py::write_file(&py::join(&c.state, "scan-notified.json"), "")?;
            return Err(Crash("TypeError: '<' not supported between instances of 'str' and another type in scan-notified.json".into()));
        }
        let mut all: Vec<String> = seen.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        for n in new {
            if !all.contains(&n) {
                all.push(n);
            }
        }
        all.sort();
        let v = Value::Array(all.into_iter().map(Value::String).collect());
        py::write_file(&py::join(&c.state, "scan-notified.json"), &py::pyjson::dumps(&v))?;
    }
    Ok(found.len())
}

/// One scan of these folders (the full one of the home folder with `full`); 1 when something was found.
pub fn scan(c: &Ctx, folders: &[OsString], full: bool) -> R<i32> {
    if folders.is_empty() {
        return Ok(0);
    }
    let state_file = py::join(&c.state, if full { "scan-full-state.json" } else { "scan-state.json" });
    let mut args: Vec<OsString> = vec![c.heimdalld.clone().into(), "scan-files".into(), "--state".into(), state_file.into()];
    let mut excludes = lines(&py::join(&c.conf, "scan-exclude"))?;
    if full {
        let mut all: Vec<String> = FULL_EXCLUDE_HOME.iter().map(|e| py::join(&c.home, e)).collect();
        all.extend(FULL_EXCLUDE_ANYWHERE.iter().map(|e| e.to_string()));
        all.extend(excludes);
        excludes = all;
        args.push("--max-mb".into());
        args.push("100".into()); // past that, a file is a video, a model or a disk image, not something one opens
    }
    for e in excludes {
        args.push("--exclude".into());
        args.push(e.into());
    }
    py::makedirs(&c.state)?;
    args.extend(folders.iter().cloned());
    let limit = if full { 6 * 3600 } else { 1800 };
    let opts = py::Run { inherit_stdin: true, timeout: Some(Duration::from_secs(limit)), timeout_text: Some(limit.to_string()), ..Default::default() };
    let p = match py::run_opts(&args, opts) {
        Ok(p) => p,
        Err(e) => {
            log(c, &format!("scan did not run: {}", e.message()))?;
            return Ok(0);
        }
    };
    if p.code == 2 {
        let err = py::strip(&p.stderr);
        let first = py::splitlines(err).first().copied().unwrap_or("unknown error").to_string();
        log(c, &format!("scan did not run: {first}"))?;
        return Ok(0);
    }
    let n = handle(c, &parse(&p.stdout), &inbound(c)?)?;
    let lines_out = py::splitlines(&p.stdout);
    if let Some(last) = lines_out.iter().rfind(|l| l.starts_with("scanned ")) {
        if !last.starts_with("scanned 0 files") {
            log(c, last)?;
        }
    }
    Ok(if n > 0 { 1 } else { 0 })
}

// ---- watching: scan a folder a few seconds after files stop arriving in it

const IN_CLOSE_WRITE: u32 = 0x8;
const IN_MOVED_TO: u32 = 0x80;
const IN_CREATE: u32 = 0x100;
const IN_ISDIR: u32 = 0x4000_0000;

struct Watcher {
    fd: i32,
    /// watch descriptor -> folder, in the order they were added
    dirs: HashMap<i32, OsString>,
}

impl Watcher {
    /// Watch a folder and its sub-folders, four levels deep, up to 4000 folders.
    fn add(&mut self, path: &OsStr, depth: u32) {
        let mut c = path.as_bytes().to_vec();
        c.push(0);
        let wd = unsafe { libc::inotify_add_watch(self.fd, c.as_ptr() as *const libc::c_char, IN_CLOSE_WRITE | IN_MOVED_TO | IN_CREATE) };
        if wd >= 0 {
            self.dirs.insert(wd, path.to_os_string());
        }
        if depth < 4 && self.dirs.len() < 4000 {
            if let Ok(rd) = std::fs::read_dir(path) {
                for e in rd.flatten() {
                    if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        let mut sub = path.as_bytes().to_vec();
                        if !sub.ends_with(b"/") {
                            sub.push(b'/');
                        }
                        sub.extend_from_slice(e.file_name().as_bytes());
                        self.add(&py::os_from_bytes(sub), depth + 1);
                    }
                }
            }
        }
    }

    fn rewatch(&mut self, c: &Ctx) -> R<Vec<String>> {
        let roots = inbound(c)?;
        for r in &roots {
            let r = OsStr::new(r);
            if !self.dirs.values().any(|d| d == r) {
                self.add(r, 0);
            }
        }
        Ok(roots)
    }
}

/// Keep running: scan a watched folder 4 s after the last new file in it; look for new places (a drive plugged in) every 5
/// minutes.
pub fn watch(c: &Ctx) -> R<i32> {
    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    if fd < 0 {
        log(c, "watch: inotify is not available")?;
        return Ok(1);
    }
    let mut w = Watcher { fd, dirs: HashMap::new() };
    let mut roots = w.rewatch(c)?;
    log(c, &format!("watching {} folder(s) for new files", roots.len()))?;
    // folder -> when its last new file arrived, in the order they first became due
    let mut pending: Vec<(OsString, f64)> = vec![];
    let mut last_roots = py::now();
    let mut buf = vec![0u8; 65536];
    loop {
        // sleep until something arrives, a pending scan is due, or the next look for new places
        let mut due = last_roots + 300.0 + 0.001;
        for (_, t) in &pending {
            due = due.min(t + 4.0);
        }
        let wait_ms = ((due - py::now()) * 1000.0).ceil().clamp(0.0, 600_000.0) as i32;
        let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        let r = unsafe { libc::poll(&mut pfd, 1, wait_ms) };
        let now = py::now();
        py::reap();
        if r > 0 && pfd.revents & libc::POLLIN != 0 {
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            let mut off = 0usize;
            let n = if n > 0 { n as usize } else { 0 };
            while off + 16 <= n {
                let wd = i32::from_ne_bytes(buf[off..off + 4].try_into().unwrap());
                let mask = u32::from_ne_bytes(buf[off + 4..off + 8].try_into().unwrap());
                let ln = u32::from_ne_bytes(buf[off + 12..off + 16].try_into().unwrap()) as usize;
                let raw = &buf[off + 16..(off + 16 + ln).min(n)];
                let name = raw.split(|b| *b == 0).next().unwrap_or(&[]).to_vec();
                off += 16 + ln;
                let base = match w.dirs.get(&wd) {
                    Some(b) => b.clone(),
                    None => continue,
                };
                if mask & IN_ISDIR != 0 {
                    if mask & (IN_CREATE | IN_MOVED_TO) != 0 {
                        let mut sub = base.as_bytes().to_vec();
                        if !sub.ends_with(b"/") && !name.is_empty() {
                            sub.push(b'/');
                        }
                        sub.extend_from_slice(&name);
                        w.add(&py::os_from_bytes(sub), 0);
                    }
                    continue;
                }
                let b = base.as_bytes();
                let top = roots
                    .iter()
                    .find(|x| b == x.as_bytes() || (b.starts_with(x.as_bytes()) && b.get(x.len()) == Some(&b'/')))
                    .map(|x| OsString::from(x.as_str()))
                    .unwrap_or(base);
                match pending.iter_mut().find(|(k, _)| *k == top) {
                    Some(e) => e.1 = now,
                    None => pending.push((top, now)),
                }
            }
        }
        let mut i = 0;
        while i < pending.len() {
            if now - pending[i].1 >= 4.0 {
                // quiet for 4 seconds: the download or unpacking is finished
                let (top, _) = pending.remove(i);
                scan(c, &[top], false)?;
            } else {
                i += 1;
            }
        }
        if now - last_roots > 300.0 {
            // a drive was plugged in, a folder was made
            last_roots = now;
            roots = w.rewatch(c)?;
        }
    }
}

fn units(verb: &str) -> R<i32> {
    py::call(&["systemctl", "--user", verb, "--now", "labyrinth-scan.timer", "labyrinth-scan-watch.service", "labyrinth-scan-full.timer"])
}

fn run(argv: &[OsString]) -> R<i32> {
    let has = |f: &str| argv.iter().any(|a| a == f);
    if has("--off") {
        return units("disable");
    }
    if has("--on") {
        return units("enable");
    }
    let c = Ctx::from_env();
    if has("--prompt") && argv.len() >= 5 {
        let i = argv.iter().position(|a| a == "--prompt").unwrap();
        let get = |k: usize| argv.get(i + k).map(|a| a.to_string_lossy().into_owned()).ok_or_else(|| Crash("IndexError: list index out of range".into()));
        prompt_quarantined(&c, &get(1)?, &get(2)?, &get(3)?)?;
        return Ok(0);
    }
    if has("--watch") {
        return watch(&c);
    }
    if has("--full") {
        return scan(&c, &[OsString::from(c.home.clone())], true);
    }
    let folders: Vec<OsString> = inbound(&c)?.into_iter().map(OsString::from).collect();
    scan(&c, &folders, false)
}

/// The program: argv without the program's own name.
pub fn main(argv: &[OsString]) -> i32 {
    match run(argv) {
        Ok(rc) => rc,
        Err(e) => py::crashed("labyrinth-scan-downloads", &e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_lines() {
        let out = "INFECTED /home/u/Downloads/a: b.exe: Win.Trojan.X\nscanned 2 files\nINFECTED nothing\n";
        assert_eq!(parse(out), vec![("/home/u/Downloads/a: b.exe".into(), "Win.Trojan.X".into()), ("".into(), "nothing".into())]);
    }

    #[test]
    fn what_counts_as_certain() {
        for sig in ["Win.Trojan.Agent-123", "Unix.Malware.Mirai-1"] {
            assert!(confident(sig), "{sig}");
        }
        for sig in ["PUA.Win.Packer.Upx", "Heuristics.Encrypted.PDF", "Win.Test.EICAR_HDB-1", "Html.Phishing.Bank-9", "Doc.Exceeds.Max",
            "Pdf.Something.Heuristic", "Archive.LimitExceeded"] {
            assert!(!confident(sig), "{sig}");
        }
    }
}
