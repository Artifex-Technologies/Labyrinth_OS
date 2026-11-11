//! labyrinth-install-cleanup: runs inside the installed system right after the installer copied the live image onto the disk.
//! It removes what only the live medium needs (its passwordless account, autologin, sudo and polkit rules), seeds /etc/skel,
//! puts the kernels back in /boot and writes a stock initramfs preset for each, and adds the resume and snapshot-overlay hooks.
//! The bash script of the same name with the same steps, output, files, calls and exit codes; LABYRINTH_ROOT (tests only)
//! puts a folder in front of every path, as it did.
//!
//! cp -a (the live user's settings), getent, userdel and chattr are still those programs; the removals, the kernels' copies, the
//! presets (the script's sed), the owner and modes of /etc/skel (chown -R, chmod -R) and the drop-ins are done here.
//!
//! Not 1:1: an error (a file that cannot be removed or written) is said in this program's words on standard error, where the
//! script let rm, install or bash say it; the exit code is 1 in both.

use crate::sh;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const LIVE_ONLY: [&str; 11] = [
    "/etc/mkinitcpio.conf.d/archiso.conf",
    "/etc/greetd/live.toml",
    "/etc/systemd/system/greetd.service.d/live.conf",
    "/etc/systemd/system/getty@tty1.service.d/autologin.conf",
    "/etc/systemd/system/serial-getty@ttyS0.service.d/autologin.conf",
    "/etc/sudoers.d/00-live-nopasswd",
    "/etc/polkit-1/rules.d/49-live-nopasswd.rules",
    "/etc/sysusers.d/live.conf",
    "/etc/tmpfiles.d/live-home.conf",
    "/usr/share/applications/labyrinth-install.desktop",
    "/usr/share/yggdrasil/integrations/labyrinth-installer.conf",
];

const RESUME: &str = include_str!("text/labyrinth-resume.conf");
const OVERLAY: &str = include_str!("text/labyrinth-snapshot-overlay.conf");

fn say(m: &str) {
    println!("labyrinth-install-cleanup: {m}");
}

type Res = Result<(), i32>;

/// Stop as `set -e` did, with the step's error said.
fn fail(what: &str, e: io::Error) -> i32 {
    eprintln!("labyrinth-install-cleanup: {what}: {e}");
    1
}

/// Every file and folder under `p` (p first; links are not followed).
fn walk(p: &Path, f: &mut dyn FnMut(&Path, &fs::Metadata) -> io::Result<()>) -> io::Result<()> {
    let m = fs::symlink_metadata(p)?;
    f(p, &m)?;
    if m.is_dir() {
        for e in fs::read_dir(p)? {
            walk(&e?.path(), f)?;
        }
    }
    Ok(())
}

/// chmod's u+rwX,go+rX,go-w on one mode.
pub fn readable_by_all(mode: u32, dir: bool) -> u32 {
    let x = dir || mode & 0o111 != 0;
    let mut m = mode | 0o644;
    if x {
        m |= 0o111;
    }
    m & !0o022
}

/// `ln -sf TARGET LINK`: LINK replaced by a link to TARGET; when LINK is a folder (or a link to one), the link goes inside it,
/// named as TARGET's last part, as ln does.
fn ln_sf(target: &str, link: &str) -> io::Result<()> {
    let link = if sh::is_dir(link) { format!("{link}/{}", sh::base(target)) } else { link.to_string() };
    let tmp = format!("{link}.labyrinth-ln");
    let _ = fs::remove_file(&tmp);
    std::os::unix::fs::symlink(target, &tmp)?;
    fs::rename(&tmp, &link).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// `install -Dm644 SRC DST`: a copy with mode 0644, its folders made (0755) when missing; DST replaced, never written through.
fn install_644(src: &str, dst: &str) -> io::Result<()> {
    if let Some(parent) = Path::new(dst).parent() {
        if !sh::is_dir(&parent.to_string_lossy()) {
            sh::install_dir(&parent.to_string_lossy(), 0o755, None)?;
        }
    }
    let mut from = fs::File::open(src)?;
    let (tmp, mut to) = sh::mkstemp(&format!("{dst}.XXXXXX"))?;
    let r = (|| {
        io::copy(&mut from, &mut to)?;
        to.set_permissions(fs::Permissions::from_mode(0o644))?;
        fs::rename(&tmp, dst)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

/// A kernel's package base as it may name files in /boot.
pub fn pkgbase_ok(b: &str) -> bool {
    !b.is_empty() && b.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'+' | b'-'))
}

pub fn main() -> i32 {
    sh::set_prog("labyrinth-install-cleanup");
    match run(&std::env::var("LABYRINTH_ROOT").unwrap_or_default()) {
        Ok(()) => 0,
        Err(c) => c,
    }
}

fn rm(p: &str) -> Res {
    sh::rm_f(p).map_err(|e| fail(&format!("cannot remove {p}"), e))
}

fn run(r: &str) -> Res {
    // 1. new accounts start from the live user's desktop settings (useradd copies /etc/skel)
    if sh::is_dir(&format!("{r}/home/live/.config")) {
        let skel = format!("{r}/etc/skel");
        sh::install_dir(&skel, 0o755, None).map_err(|e| fail(&format!("cannot make {skel}"), e))?;
        let code = sh::status(&["cp", "-a", &format!("{r}/home/live/.config"), &format!("{skel}/")]);
        if code != 0 {
            return Err(code);
        }
        let conf = format!("{skel}/.config");
        // cp -a kept the live user's uid (1000), the first installed account's: the template of every new account would be
        // writable by that one person. It is root's and readable by all (only root can; the tests run as a normal user).
        if sh::euid() == 0 {
            walk(Path::new(&conf), &mut |p, _| std::os::unix::fs::lchown(p, Some(0), Some(0)))
                .map_err(|e| fail(&format!("cannot change the owner of {conf}"), e))?;
        }
        walk(Path::new(&conf), &mut |p, m| {
            if m.file_type().is_symlink() {
                return Ok(());
            }
            let mode = m.permissions().mode() & 0o7777;
            let new = readable_by_all(mode, m.is_dir());
            if new != mode {
                fs::set_permissions(p, fs::Permissions::from_mode(new))?;
            }
            Ok(())
        })
        .map_err(|e| fail(&format!("cannot change the modes in {conf}"), e))?;
        say("seeded /etc/skel from the live user's .config");
    }

    // 2. the live-only files and the account's home
    for f in LIVE_ONLY {
        rm(&format!("{r}{f}"))?;
    }
    let _ = fs::remove_dir(format!("{r}/etc/systemd/system/greetd.service.d"));
    let home = format!("{r}/home/live");
    sh::rm_rf(&home).map_err(|e| fail(&format!("cannot remove {home}"), e))?;
    say("removed live-only configuration");

    // the serial-port login is for testing the live medium; on a desktop it fails and restarts every 10 seconds: off and masked
    rm(&format!("{r}/etc/systemd/system/getty.target.wants/serial-getty@ttyS0.service"))?;
    let sys = format!("{r}/etc/systemd/system");
    match sh::install_dir(&sys, 0o755, None) {
        Ok(()) => {
            if let Err(e) = ln_sf("/dev/null", &format!("{sys}/serial-getty@ttyS0.service")) {
                eprintln!("labyrinth-install-cleanup: cannot mask serial-getty@ttyS0.service: {e}");
            }
        }
        Err(e) => eprintln!("labyrinth-install-cleanup: cannot make {sys}: {e}"),
    }

    // the live account must not exist on the installed system (a package hook may have made it), so the installer's user gets
    // uid 1000
    if r.is_empty() && sh::quiet(&["getent", "passwd", "live"]) {
        if sh::quiet(&["userdel", "-f", "live"]) {
            say("removed the live account");
        } else {
            say("WARNING: could not remove the live account");
        }
    }

    // 3a. GRUB cannot read zstd-compressed btrfs files whose size is not a multiple of 4096 (a kernel loads 512 bytes short):
    // /boot is NOCOW (never compressed), before any kernel or initramfs is written there
    if r.is_empty() && sh::output(&["findmnt", "-no", "FSTYPE", "/"]) == "btrfs" {
        if sh::run(&["chattr", "+C", "/boot"], sh::Io::Inherit, sh::Io::Null) == 0 {
            say("marked /boot NOCOW so GRUB can read the kernels");
        } else {
            say("WARNING: could not mark /boot NOCOW; GRUB may fail to read some kernels");
        }
    }

    // 3. a stock initramfs preset for every installed kernel, from mkinitcpio's own template
    let template = format!("{r}/usr/share/mkinitcpio/hook.preset");
    if !sh::non_empty(&template) {
        say(&format!("error: {template} missing or empty, cannot restore presets"));
        return Err(1);
    }
    let tpl = fs::read(&template).map_err(|e| fail(&format!("cannot read {template}"), e))?;
    let presets = format!("{r}/etc/mkinitcpio.d");
    sh::install_dir(&presets, 0o755, None).map_err(|e| fail(&format!("cannot make {presets}"), e))?;
    // archiso's presets go first, so a kernel no longer installed leaves nothing behind
    // (as rm with all of them: each one it can, then the first error)
    let removed: Vec<Res> = sh::glob(&presets, |n| n.ends_with(".preset")).iter().map(|p| rm(p)).collect();
    removed.into_iter().collect::<Res>()?;
    let mut installed: Vec<String> = vec![];
    let modules = format!("{r}/usr/lib/modules");
    for dir in sh::glob(&modules, |_| true).into_iter().filter(|d| sh::is_dir(d)).map(|d| d + "/") {
        if !sh::readable(&format!("{dir}pkgbase")) || !sh::readable(&format!("{dir}vmlinuz")) {
            continue;
        }
        let base = sh::cat(&format!("{dir}pkgbase")).unwrap_or_default();
        if !pkgbase_ok(&base) {
            say(&format!("error: odd kernel pkgbase '{base}' in {dir}"));
            return Err(1);
        }
        // 4. the kernel back in /boot (mkarchiso emptied it; the package's own copy in /usr/lib/modules survives)
        let dst = format!("{r}/boot/vmlinuz-{base}");
        install_644(&format!("{dir}vmlinuz"), &dst).map_err(|e| fail(&format!("cannot install {dst}"), e))?;
        let preset = replace_all(&tpl, b"%PKGBASE%", base.as_bytes());
        let file = format!("{presets}/{base}.preset");
        sh::write(&file, &preset).map_err(|e| fail(&format!("cannot write {file}"), e))?;
        // an empty or wrong preset builds an initramfs that cannot boot, and nothing would notice until the first boot
        if !sh::lines(&preset).iter().any(|l| l.starts_with(b"PRESETS=")) {
            say(&format!("error: generated {base}.preset has no PRESETS line"));
            return Err(1);
        }
        installed.push(base);
    }
    if installed.is_empty() {
        say("error: no kernel with a pkgbase and vmlinuz found under /usr/lib/modules");
        return Err(1);
    }
    say(&format!("restored kernels and presets: {}", installed.join(" ")));
    // the LTS kernel is the way back when a new kernel or NVIDIA module breaks the boot
    if !installed.iter().any(|k| k == "linux-cachyos-lts") {
        say("WARNING: linux-cachyos-lts is not installed; there is no fallback kernel");
    }

    // 6. the resume hook (does nothing until the kernel command line has resume=) and grub-btrfs' overlay for snapshots, before
    // the installer builds the initramfs; drop-ins, because the installer rewrites HOOKS in mkinitcpio.conf
    let conf_d = format!("{r}/etc/mkinitcpio.conf.d");
    sh::install_dir(&conf_d, 0o755, None).map_err(|e| fail(&format!("cannot make {conf_d}"), e))?;
    let f = format!("{conf_d}/labyrinth-resume.conf");
    sh::write(&f, RESUME.as_bytes()).map_err(|e| fail(&format!("cannot write {f}"), e))?;
    say("added the resume hook drop-in");
    let f = format!("{conf_d}/labyrinth-snapshot-overlay.conf");
    sh::write(&f, OVERLAY.as_bytes()).map_err(|e| fail(&format!("cannot write {f}"), e))?;
    say("added the snapshot overlay hook drop-in");
    Ok(())
}

/// sed's s|WHAT|WITH|g (WITH holds none of sed's special characters: pkgbase_ok).
fn replace_all(text: &[u8], what: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with(what) {
            out.extend_from_slice(with);
            i += what.len();
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_names() {
        assert_eq!(readable_by_all(0o600, false), 0o644);
        assert_eq!(readable_by_all(0o700, false), 0o755);
        assert_eq!(readable_by_all(0o700, true), 0o755);
        assert_eq!(readable_by_all(0o777, false), 0o755);
        assert_eq!(readable_by_all(0o2770, true), 0o2755);
        assert!(pkgbase_ok("linux-cachyos-lts") && pkgbase_ok("linux6.12+x_y"));
        assert!(!pkgbase_ok("") && !pkgbase_ok("a b") && !pkgbase_ok("a/b") && !pkgbase_ok("a|b"));
        assert_eq!(replace_all(b"x%PKGBASE%y%PKGBASE%", b"%PKGBASE%", b"k"), b"xkyk");
    }
}
