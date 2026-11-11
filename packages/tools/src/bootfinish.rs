//! labyrinth-install-bootfinish: the installer's last step, run inside the installed system after GRUB is in place. The bash
//! script of the same name with the same steps, output, calls and exit code: the boot menu must be usable (the one fatal check),
//! then services, the firewall's defaults, zram, the boot menu's settings, hibernation (a swap file on its own subvolume),
//! drivers for this hardware, snapshots, one new grub.cfg and the boot menu theme, each of which may fail with a warning only.
//!
//! The programs (systemctl, pacman, findmnt, btrfs, snapper, grub-mkconfig, labyrinth-hardware-setup, the theme) are started
//! with their arguments; the edits of /etc/default/grub, ufw's files and /etc/fstab (the script's sed -i, grep and >>), the
//! swap size (awk on /proc/meminfo, df) and the file writes are done here.
//!
//! Fixed in both (script and port):
//!   - a setting added to the end of /etc/default/grub or /etc/fstab was glued to the last line when the file did not end with
//!     a newline (`echo X >> file`); it starts a line of its own now;
//!   - the file system's UUID went into the sed expression that edits the kernel command line unchecked (a `|` or `\` in it would
//!     have broken the edit); it must be a UUID now, else no hibernation;
//!   - "configured zram swap" was said even when its file could not be written.
//!
//! Not 1:1:
//!   - [[:space:]] before "menuentry" is the ASCII blanks (grep also took other Unicode spaces);
//!   - a line of ufw's or GRUB's settings with bytes that are not UTF-8 is replaced whole (sed's `.*` stopped at the first such
//!     byte and kept the rest);
//!   - an edit that fails is a WARNING line on standard output (sed wrote its own message on standard error);
//!   - LABYRINTH_SWAP_GIB (for tests) is read as a decimal number also with leading zeros (bash read 010 as octal 8 in its
//!     comparisons); the size given to btrfs is the text as written, as before;
//!   - when the new grub.cfg cannot be put in place, that is a WARNING (the script said "regenerated grub.cfg" after mv failed);
//!   - when grub-mkconfig wrote no file, grep's "No such file or directory" is not printed (the warning that follows is).

use crate::sh::{self, Io};

/// The boot menu theme's installer (labyrinth-grub-theme).
pub const GRUB_THEME: &str = "/usr/bin/labyrinth-grub-theme";
const CFG: &str = "/boot/grub/grub.cfg";
const DEFAULT_GRUB: &str = "/etc/default/grub";

fn say(m: &str) {
    println!("labyrinth-install-bootfinish: {m}");
}

fn warn(m: &str) {
    println!("labyrinth-install-bootfinish: WARNING: {m}");
}

/// ^[[:space:]]*menuentry
pub fn menuentry(line: &[u8]) -> bool {
    sh::lstrip(line).starts_with(b"menuentry ")
}

/// grep 'usbcore.autosuspend': the `.` is any one character.
pub fn usbcore_any(line: &[u8]) -> bool {
    let (a, b) = (b"usbcore", b"autosuspend");
    (0..line.len()).any(|i| {
        if !line[i..].starts_with(a) {
            return false;
        }
        let j = i + a.len();
        match char_len(&line[j..]) {
            Some(n) => line[j + n..].starts_with(b),
            None => false,
        }
    })
}

/// The length of the UTF-8 character at the start of `b` (None: not a whole, valid one).
fn char_len(b: &[u8]) -> Option<usize> {
    let n = match *b.first()? {
        c if c < 0x80 => 1,
        c if c & 0xE0 == 0xC0 => 2,
        c if c & 0xF0 == 0xE0 => 3,
        c if c & 0xF8 == 0xF0 => 4,
        _ => return None,
    };
    (b.len() >= n && std::str::from_utf8(&b[..n]).is_ok()).then_some(n)
}

/// `s|^(GRUB_CMDLINE_LINUX_DEFAULT=['"])([^'"]*)(['"])|\1\2INSERT\3|`: INSERT before the first quote after the opening one.
pub fn cmdline_insert(line: &[u8], insert: &str) -> Option<Vec<u8>> {
    let p = b"GRUB_CMDLINE_LINUX_DEFAULT=".len();
    if !line.starts_with(b"GRUB_CMDLINE_LINUX_DEFAULT=") || !matches!(line.get(p), Some(b'\'' | b'"')) {
        return None;
    }
    let k = p + 1 + line[p + 1..].iter().position(|&c| c == b'\'' || c == b'"')?;
    std::str::from_utf8(&line[p + 1..k]).ok()?;
    let mut out = line[..k].to_vec();
    out.extend_from_slice(insert.as_bytes());
    out.extend_from_slice(&line[k..]);
    Some(out)
}

/// `s|^(GRUB_CMDLINE_LINUX_DEFAULT=['"])|\1usbcore.autosuspend=-1 |`
pub fn cmdline_prepend(line: &[u8], insert: &str) -> Option<Vec<u8>> {
    let p = b"GRUB_CMDLINE_LINUX_DEFAULT=".len();
    if !line.starts_with(b"GRUB_CMDLINE_LINUX_DEFAULT=") || !matches!(line.get(p), Some(b'\'' | b'"')) {
        return None;
    }
    let mut out = line[..p + 1].to_vec();
    out.extend_from_slice(insert.as_bytes());
    out.extend_from_slice(&line[p + 1..]);
    Some(out)
}

/// `sed -i` that replaces whole lines starting with one of `prefixes` by `new`; a failure is a warning.
fn set_lines(path: &str, rules: &[(&[&str], &str)]) {
    let r = sh::sed_i(path, |l| rules.iter().find(|(ps, _)| ps.iter().any(|p| sh::starts(l, p))).map(|(_, new)| new.as_bytes().to_vec()));
    if let Err(e) = r {
        warn(&format!("could not edit {path} ({e})"));
    }
}

fn add_line(path: &str, line: &str) {
    if let Err(e) = sh::append_line(path, line) {
        warn(&format!("could not add {line} to {path} ({e})"));
    }
}

fn enable_global(unit_file: &str, units: &[&str], ok: &str, bad: &str) {
    if sh::exists(unit_file) {
        let mut argv = vec!["systemctl", "--global", "enable"];
        argv.extend_from_slice(units);
        if sh::quiet(&argv) {
            say(ok);
        } else {
            warn(bad);
        }
    }
}

/// The number of whole GiB of memory, rounded up (the script's awk on /proc/meminfo; each MemTotal line printed in turn).
pub fn meminfo_gib(text: &str) -> String {
    let mut out = String::new();
    for line in text.split('\n') {
        if !line.starts_with("MemTotal:") {
            continue;
        }
        let kb = awk_number(line.split([' ', '\t']).filter(|f| !f.is_empty()).nth(1).unwrap_or(""));
        let v = ((kb + 1048575.0) / 1048576.0).trunc();
        out.push_str(&format!("{}", v as i64));
    }
    out
}

/// What awk makes of a field used as a number: its leading decimal number, else 0.
fn awk_number(s: &str) -> f64 {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    s[..i].parse().unwrap_or(0.0)
}

/// `df -BG --output=avail /`: the free space for ordinary use, in GiB rounded up; None when it cannot be read.
fn free_gib(path: &str) -> Option<u128> {
    let c = std::ffi::CString::new(path).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let bytes = s.f_bavail as u128 * s.f_frsize as u128;
    Some(bytes.div_ceil(1 << 30))
}

/// A file system UUID as findmnt prints it.
pub fn uuid_ok(u: &str) -> bool {
    !u.is_empty() && u.len() <= 64 && u.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-')
}

fn root_fstype() -> String {
    sh::output(&["findmnt", "-no", "FSTYPE", "/"])
}

/// Hibernate: a swap file on btrfs the size of RAM, plus the resume plumbing. Any step failing removes what was made, and the
/// power menu simply does not offer Hibernate. Err: not set up (said why).
fn hibernate() -> Result<(), ()> {
    if root_fstype() != "btrfs" {
        warn("root is not btrfs; no hibernate");
        return Err(());
    }
    let gib = match std::env::var("LABYRINTH_SWAP_GIB") {
        Ok(g) if !g.is_empty() => g,
        _ => meminfo_gib(&String::from_utf8_lossy(&std::fs::read("/proc/meminfo").unwrap_or_default())),
    };
    let n: u128 = match (sh::digits(&gib), gib.parse::<u128>()) {
        (true, Ok(n)) if n >= 1 => n,
        _ => {
            warn("could not work out the swap size");
            return Err(());
        }
    };
    // leave room: no swap file bigger than a third of the free space
    if let Some(free) = free_gib("/") {
        if n.saturating_mul(3) > free {
            warn(&format!("only {free} GiB free; a {gib} GiB swap file would not fit comfortably, skipping hibernate"));
            return Err(());
        }
    }
    if !sh::quiet(&["btrfs", "subvolume", "create", "/swap"]) {
        warn("could not create the /swap subvolume");
        return Err(());
    }
    if !sh::quiet(&["btrfs", "filesystem", "mkswapfile", "--size", &format!("{gib}g"), "/swap/swapfile"]) {
        warn("could not create the swap file");
        sh::quiet(&["btrfs", "subvolume", "delete", "/swap"]);
        return Err(());
    }
    let uuid = sh::output(&["findmnt", "-no", "UUID", "/"]);
    let offset = sh::output(&["btrfs", "inspect-internal", "map-swapfile", "-r", "/swap/swapfile"]);
    if !uuid_ok(&uuid) || !sh::digits(&offset) {
        warn("could not read the swap file's resume offset");
        let _ = sh::rm_f("/swap/swapfile");
        sh::quiet(&["btrfs", "subvolume", "delete", "/swap"]);
        return Err(());
    }
    // fstab: a lower priority than zram, so ordinary swapping stays in memory
    if !sh::grep("/etc/fstab", |l| sh::starts(l, "/swap/swapfile")) {
        add_line("/etc/fstab", "/swap/swapfile none swap defaults,pri=1 0 0");
    }
    if sh::is_file(DEFAULT_GRUB) {
        let insert = format!(" resume=UUID={uuid} resume_offset={offset}");
        if let Err(e) = sh::sed_i(DEFAULT_GRUB, |l| cmdline_insert(l, &insert)) {
            warn(&format!("could not edit {DEFAULT_GRUB} ({e})"));
        }
    }
    // the initramfs has the resume hook already (labyrinth-install-cleanup's drop-in): no rebuild
    say(&format!("hibernate ready: {gib} GiB swap file, resume offset {offset}"));
    Ok(())
}

pub fn main() -> i32 {
    sh::set_prog("labyrinth-install-bootfinish");
    // 1. grub.cfg must exist and boot at least one kernel
    if !sh::non_empty(CFG) || !sh::grep(CFG, menuentry) {
        say(&format!("error: {CFG} is missing or has no menu entries; the system would not boot"));
        return 1;
    }
    if !sh::grep(CFG, |l| sh::contains(l, "initrd")) {
        say(&format!("error: {CFG} lists no initramfs"));
        return 1;
    }
    let entries = std::fs::read(CFG).map(|b| sh::lines(&b).into_iter().filter(|l| menuentry(l)).count()).unwrap_or(0);
    say(&format!("grub.cfg looks usable ({entries} entries)"));

    // 2a. services (`systemctl enable` works in a chroot: it only makes links)
    for unit in [
        "NetworkManager.service",
        "fstrim.timer",
        "ratbagd.service",
        "labyrinth-appnet.service",
        "labyrinth-appnet-ask.service",
        "yggdrasil-powerlimit.service",
    ] {
        // gaming mice (libratbag) are on the full image only; D-Bus starts ratbagd when Piper asks
        if unit == "ratbagd.service" && !sh::quiet(&["pacman", "-Qq", "libratbag"]) {
            continue;
        }
        if sh::quiet(&["systemctl", "enable", unit]) {
            say(&format!("enabled {unit}"));
        } else {
            warn(&format!("could not enable {unit}"));
        }
    }

    // the firewall: incoming blocked, outgoing allowed, through ufw's files (`ufw enable` would change the live system's)
    if sh::have("ufw") {
        set_lines(
            "/etc/default/ufw",
            &[
                (&["DEFAULT_INPUT_POLICY="], "DEFAULT_INPUT_POLICY=\"DROP\""),
                (&["DEFAULT_OUTPUT_POLICY="], "DEFAULT_OUTPUT_POLICY=\"ACCEPT\""),
            ],
        );
        set_lines("/etc/ufw/ufw.conf", &[(&["ENABLED="], "ENABLED=yes")]);
        if sh::quiet(&["pacman", "-Qq", "kdeconnect"])
            && sh::is_file("/etc/ufw/applications.d/kdeconnect")
            && !sh::grep("/etc/ufw/user.rules", |l| sh::contains(l, "KDE Connect"))
        {
            warn("KDE Connect is installed; allow it with: sudo ufw allow 'KDE Connect'");
        }
        if sh::quiet(&["systemctl", "enable", "ufw.service"]) {
            say("firewall on (incoming blocked, outgoing allowed)");
        } else {
            warn("could not enable the firewall");
        }
    }

    enable_global(
        "/usr/lib/systemd/user/openrazer-daemon.service",
        &["openrazer-daemon.service"],
        "enabled the OpenRazer daemon",
        "could not enable the OpenRazer daemon",
    );
    enable_global(
        "/usr/lib/systemd/user/yggdrasil-weather-record.timer",
        &["yggdrasil-weather-record.timer"],
        "enabled weather recording timer",
        "could not enable the weather recording timer",
    );
    enable_global(
        "/usr/lib/systemd/user/labyrinth-appnet-prepare.service",
        &["labyrinth-appnet-prepare.service"],
        "enabled per-app network rules",
        "could not enable the per-app network rules",
    );
    enable_global(
        "/usr/lib/systemd/user/labyrinth-appnet-ask-agent.service",
        &["labyrinth-appnet-ask-agent.service"],
        "enabled the Ask-before-connecting notifications",
        "could not enable the Ask notifications",
    );
    enable_global(
        "/usr/lib/systemd/user/labyrinth-appusage.service",
        &["labyrinth-appusage.service"],
        "enabled app usage counting",
        "could not enable app usage counting",
    );

    // Heimdall's own defaults (its package knows its units)
    if sh::have("heimdall-enable-defaults") {
        if sh::status(&["heimdall-enable-defaults"]) == 0 {
            say("switched on Heimdall's defaults (DNS block list, threat lists, Wi-Fi checks)");
        } else {
            warn("could not switch on Heimdall's defaults");
        }
    }

    // the malware scan of new downloads, and ClamAV's signatures
    enable_global(
        "/usr/lib/systemd/user/labyrinth-scan.timer",
        &["labyrinth-scan.timer", "labyrinth-scan-watch.service", "labyrinth-scan-full.timer"],
        "enabled the malware scan (watcher, timer and the weekly full scan)",
        "could not enable the downloads malware scan",
    );
    if sh::exists("/usr/bin/yggdrasil-gametime") && sh::exists("/usr/lib/systemd/system/clamav-freshclam-once.timer") {
        sh::quiet(&["systemctl", "disable", "clamav-freshclam.service"]);
        if sh::quiet(&["systemctl", "enable", "clamav-freshclam-once.timer"]) {
            say("enabled ClamAV signature updates (every 2 hours, after games)");
        } else {
            warn("could not enable ClamAV signature updates");
        }
    } else if sh::exists("/usr/lib/systemd/system/clamav-freshclam.service") {
        if sh::quiet(&["systemctl", "enable", "clamav-freshclam.service"]) {
            say("enabled ClamAV signature updates");
        } else {
            warn("could not enable ClamAV signature updates");
        }
    }

    // 2b. zram swap for everyday memory pressure (hibernation uses the swap file below)
    if let Err(e) = sh::install_dir("/etc/systemd", 0o755, None) {
        eprintln!("labyrinth-install-bootfinish: could not make /etc/systemd: {e}");
    }
    match sh::write("/etc/systemd/zram-generator.conf", b"[zram0]\nzram-size = min(ram / 2, 16384)\ncompression-algorithm = zstd\n") {
        Ok(()) => say("configured zram swap"),
        Err(_) => warn("could not configure zram swap"),
    }

    // the boot menu: two seconds, the default kernel on top, other systems found by os-prober, USB power saving off for
    // wireless keyboards and mice (they dropped or repeated keys)
    if sh::is_file(DEFAULT_GRUB) {
        set_lines(DEFAULT_GRUB, &[(&["GRUB_TIMEOUT="], "GRUB_TIMEOUT=2")]);
        if sh::grep(DEFAULT_GRUB, |l| sh::starts(l, "GRUB_DISABLE_OS_PROBER=") || sh::starts(l, "#GRUB_DISABLE_OS_PROBER=")) {
            set_lines(DEFAULT_GRUB, &[(&["GRUB_DISABLE_OS_PROBER=", "#GRUB_DISABLE_OS_PROBER="], "GRUB_DISABLE_OS_PROBER=false")]);
        } else {
            add_line(DEFAULT_GRUB, "GRUB_DISABLE_OS_PROBER=false");
        }
        if !sh::grep(DEFAULT_GRUB, |l| sh::starts(l, "GRUB_TOP_LEVEL=")) {
            add_line(DEFAULT_GRUB, "GRUB_TOP_LEVEL=\"/boot/vmlinuz-linux-cachyos\"");
        }
        if !sh::grep(DEFAULT_GRUB, usbcore_any) {
            if let Err(e) = sh::sed_i(DEFAULT_GRUB, |l| cmdline_prepend(l, "usbcore.autosuspend=-1 ")) {
                warn(&format!("could not edit {DEFAULT_GRUB} ({e})"));
            }
        }
    }

    if hibernate().is_err() {
        warn("hibernate not set up");
    }

    // drivers and device tools for this computer: what the hardware does not need is removed (offline)
    if sh::have("labyrinth-hardware-setup") && sh::status(&["labyrinth-hardware-setup"]) != 0 {
        warn("hardware setup did not finish (see /var/log/labyrinth-hardware.log)");
    }

    // 2c. snapshots: snapper on the root subvolume, grub-btrfs for the boot menu
    if root_fstype() == "btrfs" && sh::have("snapper") {
        if sh::run(&["snapper", "--no-dbus", "-c", "root", "create-config", "/"], Io::Inherit, Io::Null) == 0 {
            say("created snapper config for /");
            // the system as installed, kept for good (important=yes is never cleaned up)
            let argv = [
                "snapper",
                "--no-dbus",
                "-c",
                "root",
                "create",
                "--description",
                "Fresh Labyrinth OS install",
                "--userdata",
                "important=yes",
            ];
            if sh::run(&argv, Io::Inherit, Io::Null) == 0 {
                say("took the fresh-install snapshot");
            } else {
                warn("could not take the fresh-install snapshot");
            }
            if !sh::quiet(&["systemctl", "enable", "snapper-timeline.timer", "snapper-cleanup.timer"]) {
                warn("could not enable snapper timers");
            }
            if sh::have("grub-btrfsd") && !sh::quiet(&["systemctl", "enable", "grub-btrfsd.service"]) {
                warn("could not enable grub-btrfsd");
            }
        } else {
            warn("snapper create-config failed; no automatic snapshots (system is otherwise fine)");
        }
    } else {
        warn("root is not btrfs or snapper is missing; skipping snapshots");
    }

    // 2d. one grub.cfg for all of the above; the installer's is kept if this fails
    if sh::is_file(DEFAULT_GRUB) {
        let new = "/boot/grub/grub.cfg.new";
        if sh::quiet(&["grub-mkconfig", "-o", new]) && sh::grep(new, menuentry) {
            match std::fs::rename(new, CFG) {
                Ok(()) => say("regenerated grub.cfg"),
                Err(e) => warn(&format!("could not put the new grub.cfg in place ({e})")),
            }
        } else {
            let _ = sh::rm_f(new);
            warn("could not regenerate grub.cfg; keeping the installer's (no hibernate until it is regenerated)");
        }
    }

    // 2e. the boot menu theme: it puts the old menu back itself when the result is unusable
    if sh::executable(GRUB_THEME) && sh::have("rsvg-convert") {
        if sh::status(&[GRUB_THEME]) == 0 {
            say("boot menu theme installed");
        } else {
            warn("could not install the boot menu theme (the plain menu stays)");
        }
    } else {
        warn("labyrinth-grub-theme or rsvg-convert is missing; plain boot menu");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grub_lines() {
        assert!(menuentry(b"  menuentry 'x'"));
        assert!(!menuentry(b"submenu 'x'"));
        assert!(
            usbcore_any(b"x usbcore.autosuspend=-1")
                && usbcore_any(b"usbcore_autosuspend")
                && usbcore_any("usbcoreéautosuspend".as_bytes())
        );
        assert!(!usbcore_any(b"usbcoreautosuspend") && !usbcore_any(b"usbcore\xffautosuspend"));
        assert_eq!(cmdline_insert(b"GRUB_CMDLINE_LINUX_DEFAULT=\"quiet\"", " r=1").unwrap(), b"GRUB_CMDLINE_LINUX_DEFAULT=\"quiet r=1\"");
        assert_eq!(cmdline_insert(b"GRUB_CMDLINE_LINUX_DEFAULT='a\"b'", " r").unwrap(), b"GRUB_CMDLINE_LINUX_DEFAULT='a r\"b'");
        assert_eq!(cmdline_insert(b"GRUB_CMDLINE_LINUX_DEFAULT=\"open", " r"), None);
        assert_eq!(cmdline_insert(b"GRUB_CMDLINE_LINUX_DEFAULT=quiet", " r"), None);
        assert_eq!(cmdline_prepend(b"GRUB_CMDLINE_LINUX_DEFAULT='q'", "u ").unwrap(), b"GRUB_CMDLINE_LINUX_DEFAULT='u q'");
    }

    #[test]
    fn memory_and_uuid() {
        assert_eq!(meminfo_gib("MemTotal:       65536000 kB\nMemFree: 1 kB\n"), "63");
        assert_eq!(meminfo_gib("MemTotal: 1048576 kB\n"), "1");
        assert_eq!(meminfo_gib("MemTotal: 1048577 kB\n"), "2");
        assert_eq!(meminfo_gib("nothing\n"), "");
        assert!(uuid_ok("0b5e8f9c-1d2e-4f3a-9b8c-7d6e5f4a3b2c"));
        assert!(!uuid_ok("") && !uuid_ok("a|b") && !uuid_ok("a\nb"));
    }
}
