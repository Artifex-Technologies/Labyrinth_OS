//! An allow-list of its own, for the tests and the fuzz target, written apart from the checks in agent.rs (it calls none of
//! them): whatever `decide` returns for any command line, each program it would start as root must be one listed here, with
//! arguments as these programs themselves would read them (journalctl's and heimdalld's option parsing are modelled: an option
//! that takes its value only when the next word reads as one, Go's flags), and a deploy must go where this allows. `Mock` is a
//! made-up system whose answers the case chooses.

use super::{decide, Plan, World, B};
use crate::py::Crash;

/// A made-up system.
#[derive(Default)]
pub struct Mock {
    /// the regular files, each with the package name pacman -Qip finds in it ("" for none)
    pub files: Vec<(B, B)>,
    /// the names in the repositories, each with what installing it would also install
    pub repo: Vec<(B, Vec<B>)>,
    /// pacman -Sp fails
    pub sp_fails: bool,
    /// the programs root is told to run
    pub root_run: Vec<B>,
}

impl World for Mock {
    fn regular(&mut self, path: &[u8]) -> Result<bool, Crash> {
        Ok(self.files.iter().any(|(p, _)| p == path))
    }

    fn pkg_name(&mut self, path: &[u8]) -> Result<B, Crash> {
        Ok(self.files.iter().find(|(p, _)| p == path).map(|(_, n)| n.clone()).unwrap_or_default())
    }

    fn in_repos(&mut self, name: &[u8]) -> Result<bool, Crash> {
        Ok(self.repo.iter().any(|(n, _)| n == name))
    }

    fn transaction(&mut self, names: &[B]) -> Result<Option<B>, Crash> {
        if self.sp_fails {
            return Ok(None);
        }
        let mut out = B::new();
        for n in names {
            out.extend_from_slice(n);
            out.push(b'\n');
            for (_, deps) in self.repo.iter().filter(|(r, _)| r == n) {
                for d in deps {
                    out.extend_from_slice(d);
                    out.push(b'\n');
                }
            }
        }
        Ok(Some(out))
    }

    fn root_runs(&mut self, name: &[u8]) -> Option<B> {
        self.root_run.iter().any(|r| r == name).then(|| b"/usr/lib/systemd/system/made-up.service".to_vec())
    }
}

/// The made-up system the fuzz target and the random cases use: ecosystem packages and others, repository names with and
/// without a remote-access tool among what they bring, two programs root runs.
pub fn standard_mock() -> Mock {
    let v = |s: &str| s.as_bytes().to_vec();
    Mock {
        files: vec![
            (v("/tmp/yggdrasil-a-1-1-x86_64.pkg.tar.zst"), v("yggdrasil-a")),
            (v("/tmp/heimdall-git-1-1-x86_64.pkg.tar.xz"), v("heimdall-git")),
            (v("/tmp/openssh-1-1-x86_64.pkg.tar.zst"), v("openssh")),
            (v("/tmp/look-1-1-x86_64.pkg.tar.zst"), v("yggdrasil")),
            (v("/tmp/sudo-1-1-x86_64.pkg.tar.zst"), v("sudo")),
            (v("/tmp/none-1-1-x86_64.pkg.tar.zst"), v("")),
        ],
        repo: vec![
            (v("gnu-efi"), vec![]),
            (v("htop"), vec![v("ncurses")]),
            (v("sshfs"), vec![v("fuse3"), v("openssh")]),
            (v("remmina"), vec![v("freerdp"), v("gnome-remote-desktop")]),
            (v("tailscale"), vec![]),
            (v("python-lsp"), vec![v("python-lsp-server")]),
        ],
        sp_fails: false,
        root_run: vec![v("yggdrasil-spell"), v("yggdrasil-gametime")],
    }
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

const ECOSYSTEM: [&str; 7] = ["yggdrasil", "labyrinth", "heimdall", "sarina", "limbo", "liki", "agora"];
const REMOTE: &str = "tailscale headscale ngrok frp frpc frps chisel rathole bore bore-cli meshagent tor openssh dropbear tinyssh \
                      tigervnc x11vnc wayvnc xrdp gnome-remote-desktop krfb krdp ttyd cockpit sunshine samba ksmbd-tools nfs-utils vsftpd";
const REMOTE_FAMILIES: &str = "zerotier netbird cloudflared anydesk teamviewer rustdesk";

/// A package the agent may never install alone: a remote-access or tunnel tool, or a server.
pub fn remote(n: &[u8]) -> bool {
    let t = s(n);
    REMOTE.split_whitespace().any(|r| t == r) || REMOTE_FAMILIES.split_whitespace().any(|f| t.starts_with(f)) || t.ends_with("-server")
}

fn eco_package(n: &[u8]) -> bool {
    let t = s(n);
    !n.is_empty()
        && n.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"-._+".contains(c))
        && ECOSYSTEM.iter().any(|e| t.starts_with(e))
}

fn clean_components(p: &[u8]) -> bool {
    p.first() == Some(&b'/') && p[1..].split(|&c| c == b'/').all(|c| !c.is_empty() && c != b"." && c != b"..")
}

fn package_file(p: &[u8]) -> bool {
    p.first() == Some(&b'/')
        && !p.split(|&c| c == b'/').any(|c| c == b"..")
        && (p.ends_with(b".pkg.tar.zst") || p.ends_with(b".pkg.tar.xz"))
}

/// journalctl as systemd 262 reads its arguments: the options the agent may use (anything else in an option's place, in full or
/// abbreviated, is a violation), which take a value always, which only when attached or when the next word reads as one.
fn journalctl(args: &[B]) -> Result<(), String> {
    const LONG_NONE: [&str; 9] = ["no-pager", "dmesg", "quiet", "follow", "reverse", "catalog", "no-hostname", "all", "case-sensitive"];
    const LONG_REQ: [&str; 7] = ["unit", "output", "since", "until", "priority", "identifier", "grep"];
    let lines = |w: &[u8]| {
        w == b"all" || {
            let d = w.strip_prefix(b"+").unwrap_or(w);
            !d.is_empty() && d.iter().all(u8::is_ascii_digit)
        }
    };
    let boot = |w: &[u8]| {
        let d = w.strip_prefix(b"-").unwrap_or(w);
        (!d.is_empty() && d.iter().all(u8::is_ascii_digit)) || (w.len() == 32 && w.iter().all(u8::is_ascii_hexdigit))
    };
    let fields = ["_COMM", "_PID", "_UID", "_TRANSPORT", "_SYSTEMD_UNIT", "_SYSTEMD_USER_UNIT", "SYSLOG_IDENTIFIER", "PRIORITY"];
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if a == b"--" {
            return Err("journalctl: -- (what follows would be read as matches)".into());
        }
        if let Some(long) = a.strip_prefix(b"--") {
            let (name, attached) = match long.iter().position(|&c| c == b'=') {
                Some(k) => (s(&long[..k]), true),
                None => (s(long), false),
            };
            if LONG_REQ.contains(&name.as_str()) {
                if !attached {
                    i += 1; // its value, whatever it is
                }
            } else if name == "lines" || name == "boot" {
                if !attached && i < args.len() && (if name == "lines" { lines(&args[i]) } else { boot(&args[i]) }) {
                    i += 1;
                }
            } else if !LONG_NONE.contains(&name.as_str()) || (attached && name != "case-sensitive") {
                return Err(format!("journalctl option {}", s(a)));
            }
        } else if a.len() > 1 && a[0] == b'-' {
            let cluster = &a[1..];
            for (k, &c) in cluster.iter().enumerate() {
                let rest = &cluster[k + 1..];
                match c {
                    b'k' | b'q' | b'f' | b'r' | b'x' | b'a' => {}
                    b'u' | b'o' | b'p' | b't' | b'g' => {
                        if rest.is_empty() {
                            i += 1;
                        }
                        break;
                    }
                    b'n' | b'b' => {
                        if rest.is_empty() && i < args.len() && (if c == b'n' { lines(&args[i]) } else { boot(&args[i]) }) {
                            i += 1;
                        }
                        break;
                    }
                    _ => return Err(format!("journalctl option -{} in {}", c as char, s(a))),
                }
            }
        } else {
            // a match: FIELD=VALUE of the fields the agent may read by
            let t = s(a);
            let ok = t
                .split_once('=')
                .is_some_and(|(f, v)| fields.contains(&f) && v.bytes().all(|c| c.is_ascii_alphanumeric() || b"@._:/-".contains(&c)));
            if !ok {
                return Err(format!("journalctl match {}", s(a)));
            }
        }
    }
    Ok(())
}

/// heimdalld maintenance as Go's flag package reads it: an action, then --for and --reason (each taking the next word).
fn heimdalld(a: &[B], pkg_install: bool) -> Result<(), String> {
    if a.get(1).map(Vec::as_slice) != Some(b"maintenance") || !matches!(a.get(2).map(Vec::as_slice), Some(b"on" | b"off" | b"status")) {
        return Err(format!("heimdalld {:?}", a.iter().map(|x| s(x)).collect::<Vec<_>>()));
    }
    let mut i = 3;
    while i < a.len() {
        let w = &a[i];
        if !(w.len() >= 2 && w[0] == b'-') {
            return Err(format!("heimdalld: a word after the flags: {}", s(w)));
        }
        let name = w.strip_prefix(b"--").or_else(|| w.strip_prefix(b"-")).unwrap_or(w);
        let (name, attached) = match name.iter().position(|&c| c == b'=') {
            Some(k) => (&name[..k], true),
            None => (name, false),
        };
        if name != b"for" && name != b"reason" {
            return Err(format!("heimdalld flag {}", s(w)));
        }
        i += if attached { 1 } else { 2 };
    }
    if pkg_install && a.len() != 7 {
        return Err("heimdalld: the install's window".into());
    }
    Ok(())
}

fn systemctl(a: &[B]) -> Result<(), String> {
    let words: Vec<&[u8]> = a[1..].iter().map(Vec::as_slice).collect();
    if words == [b"daemon-reload".as_slice()] {
        return Ok(());
    }
    let (verb, units): (&[&[u8]], &[&[u8]]) = match words.as_slice() {
        [b"enable", b"--now", u @ ..] => (&[b"enable-now"], u),
        [b"disable", b"--now", u @ ..] => (&[b"disable-now"], u),
        [b"--global", b"enable", u @ ..] => (&[b"user-enable"], u),
        [b"--global", b"disable", u @ ..] => (&[b"user-disable"], u),
        [v, u @ ..]
            if [b"start".as_slice(), b"stop", b"restart", b"reload", b"status", b"is-active", b"enable", b"disable"].contains(v) =>
        {
            (std::slice::from_ref(v), u)
        }
        _ => return Err(format!("systemctl {:?}", words.iter().map(|x| s(x)).collect::<Vec<_>>())),
    };
    if units.is_empty() {
        return Err("systemctl without a unit".into());
    }
    let heads = ["yggdrasil-", "labyrinth-", "heimdall", "limbo", "sarina", "ollama", "systemd-sysctl", "greetd-test"];
    for u in units {
        let t = s(u);
        if !u.iter().all(|c| c.is_ascii_alphanumeric() || b"@._:-".contains(c)) || !heads.iter().any(|h| t.starts_with(h)) {
            return Err(format!("systemctl unit {t}"));
        }
        if t.starts_with("labyrinth-agent") {
            return Err(format!("systemctl on the agent window's {t}"));
        }
        if t.starts_with("heimdall") && matches!(verb[0], b"stop" | b"disable" | b"disable-now" | b"user-disable") {
            return Err(format!("systemctl {} {t}", s(verb[0])));
        }
    }
    Ok(())
}

fn limbo(a: &[B]) -> Result<(), String> {
    let manifest_ok = |m: &[u8]| {
        (m.starts_with(b"/var/lib/limbo/build-agent/") || m.starts_with(b"/etc/limbo/validation/"))
            && m.ends_with(b".json")
            && !m.split(|&c| c == b'/').any(|c| c == b"..")
    };
    match a.iter().map(Vec::as_slice).collect::<Vec<_>>().as_slice() {
        [_, b"up" | b"down", m] if manifest_ok(m) => Ok(()),
        [_, b"doctor" | b"status"] => Ok(()),
        [_, b"doctor" | b"status", m] if manifest_ok(m) => Ok(()),
        _ => Err(format!("limbo {:?}", a.iter().map(|x| s(x)).collect::<Vec<_>>())),
    }
}

fn pacman(a: &[B], m: &mut Mock) -> Result<(), String> {
    let words: Vec<&[u8]> = a[1..].iter().map(Vec::as_slice).collect();
    match words.as_slice() {
        [b"-U", b"--noconfirm", files @ ..] if !files.is_empty() => {
            for f in files {
                let name = m.pkg_name(f).unwrap_or_default();
                if !package_file(f) || !m.regular(f).unwrap_or(false) || !eco_package(&name) {
                    return Err(format!("pacman -U {} ({})", s(f), s(&name)));
                }
            }
            Ok(())
        }
        [b"-S", b"--needed", b"--noconfirm", names @ ..] if !names.is_empty() => {
            for n in names {
                let target = (1..=100).contains(&n.len())
                    && n[0] != b'-'
                    && n.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"@._+-".contains(c));
                if !target || remote(n) || !m.in_repos(n).unwrap_or(false) {
                    return Err(format!("pacman -S {}", s(n)));
                }
                for (_, deps) in m.repo.iter().filter(|(r, _)| r == n) {
                    if let Some(d) = deps.iter().find(|d| remote(d)) {
                        return Err(format!("pacman -S {} brings {}", s(n), s(d)));
                    }
                }
            }
            Ok(())
        }
        _ => Err(format!("pacman {:?}", words.iter().map(|x| s(x)).collect::<Vec<_>>())),
    }
}

fn deploy(dest: &[u8], mode: u32, m: &mut Mock) -> Result<(), String> {
    let t = s(dest);
    if !clean_components(dest) || std::str::from_utf8(dest).is_err() {
        return Err(format!("deploy to {t}"));
    }
    for never in ["/usr/share/yggdrasil/gpu-unlock", "/usr/share/labyrinth/pkgbuilds", "/usr/share/labyrinth/grub-evodevo"] {
        if t == never || t.starts_with(&format!("{never}/")) {
            return Err(format!("deploy to {t}"));
        }
    }
    let file = t.rsplit('/').next().unwrap_or("");
    let plain =
        |x: &str, slash: bool| !x.is_empty() && x.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c) || (slash && c == b'/'));
    let ok = if let Some(p) = t.strip_prefix("/usr/bin/") {
        let program = p == "minos"
            || [p.strip_prefix("yggdrasil-"), p.strip_prefix("ygg-")]
                .into_iter()
                .flatten()
                .any(|r| !r.is_empty() && r.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'));
        program && mode == 0o755 && m.root_runs(file.as_bytes()).is_none()
    } else {
        mode == 0o644
            && (["/usr/share/yggdrasil/", "/usr/share/labyrinth/"].iter().any(|d| t.strip_prefix(d).is_some_and(|r| plain(r, true)))
                || t.strip_prefix("/usr/share/icons/")
                    .is_some_and(|r| [".svg", ".png"].iter().any(|x| r.strip_suffix(x).is_some_and(|b| plain(b, true))))
                || ["/usr/share/applications/", "/usr/share/wayland-sessions/"]
                    .iter()
                    .any(|d| t.strip_prefix(d).and_then(|r| r.strip_suffix(".desktop")).is_some_and(|b| plain(b, false)))
                || t.strip_prefix("/etc/yggdrasil/")
                    .is_some_and(|r| [".json", ".conf"].iter().any(|x| r.strip_suffix(x).is_some_and(|b| plain(b, false)))))
    };
    if ok {
        Ok(())
    } else {
        Err(format!("deploy to {t} ({mode:o})"))
    }
}

/// What a plan would do as root, against this module's allow-list.
pub fn audit(plan: &Plan, m: &mut Mock) -> Result<(), String> {
    match plan {
        Plan::Deploy { dest, mode, .. } => deploy(dest, *mode, m),
        Plan::Runs(list) => {
            if list.is_empty() {
                return Err("nothing to run".into());
            }
            let pkg_install = list.len() == 2;
            if pkg_install && (list[0].first().map(Vec::as_slice) != Some(b"heimdalld") || list[1].get(1).map(Vec::as_slice) != Some(b"-U"))
            {
                return Err("two programs that are not an install".into());
            }
            for a in list {
                match a.first().map(Vec::as_slice) {
                    Some(b"journalctl") if a.get(1).map(Vec::as_slice) == Some(b"--no-pager") => journalctl(&a[2..])?,
                    Some(b"systemctl") => systemctl(a)?,
                    Some(b"heimdalld") => heimdalld(a, pkg_install)?,
                    Some(b"limbo") => limbo(a)?,
                    Some(b"pacman") => pacman(a, m)?,
                    Some(b"ufw") if a[1..] == [b"status".to_vec(), b"verbose".to_vec()] => {}
                    Some(b"/usr/local/bin/labyrinth-rollback") => match a[1..].iter().map(Vec::as_slice).collect::<Vec<_>>().as_slice() {
                        [b"list" | b"status"] => {}
                        [n, b"--dry-run"] if !n.is_empty() && n.iter().all(u8::is_ascii_digit) => {}
                        _ => return Err(format!("labyrinth-rollback {:?}", a.iter().map(|x| s(x)).collect::<Vec<_>>())),
                    },
                    _ => return Err(format!("program {:?}", a.iter().map(|x| s(x)).collect::<Vec<_>>())),
                }
            }
            Ok(())
        }
    }
}

/// One command line (the bytes split at each 0) through `decide` on the standard made-up system: a panic if anything it would
/// do as root is outside the allow-list. The fuzz target and the random cases call this.
pub fn check_one(words: &[B]) {
    let Some((cmd, rest)) = words.split_first() else { return };
    let mut m = standard_mock();
    if let Ok(plan) = decide(cmd, rest, &mut m) {
        if let Err(why) = audit(&plan, &mut m) {
            panic!("outside the allow-list: {why}\n  command line: {:?}", words.iter().map(|w| s(w)).collect::<Vec<_>>());
        }
    }
}

/// The fuzz target's input: words separated by 0 bytes.
pub fn fuzz_one(data: &[u8]) {
    let words: Vec<B> = data.split(|&c| c == 0).map(<[u8]>::to_vec).collect();
    check_one(&words);
}
