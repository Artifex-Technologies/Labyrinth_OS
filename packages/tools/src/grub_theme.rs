//! labyrinth-grub-theme [--preview FILE]: installs the boot menu theme (grub-evodevo, MIT, vendored in
//! /usr/share/labyrinth/grub-evodevo) with Labyrinth's settings, drawn for the screen's own resolution, and puts the old menu
//! back when the result has no menu entries. The sh script of the same name with the same arguments, output, files, calls and
//! exit codes.
//!
//! The theme's own install.sh (vendored, third-party) is still run with sh, and cp copies the theme and the preview; the
//! settings in install.sh (the script's sed -i), the screen's resolution (/sys/class/drm), the changes to /etc/default/grub,
//! the icon names read from grub.cfg (grep -o) and the old menu kept for putting back are done here.
//!
//! Fixed in both (script and port):
//!   - `--preview FILE` with a relative FILE wrote the picture into the work folder (the script had changed into it), which was
//!     then removed: the picture was lost. FILE is taken from where the command was given now;
//!   - a GRUB_GFXMODE or GRUB_GFXPAYLOAD_LINUX line added to /etc/default/grub was glued to a last line without a newline;
//!   - an icon was copied under the name of any class grub.cfg gave its entry; a class that is not a plain name ([A-Za-z0-9_-])
//!     is skipped now (a `/` in it would have written outside the icons' folder).
//!
//! Not 1:1: a monitor mode that is not WIDTHxHEIGHT (digits) is skipped (the script took e.g. 1920x1080x5 and stopped on its own
//! arithmetic); when /etc/default/grub or grub.cfg cannot be read before the install, this program says so in its own words
//! (cp did); FILE is checked before anything else (the script made its work folder first).

use crate::sh::{self, Io};
use std::fs;

const SRC: &str = "/usr/share/labyrinth/grub-evodevo";
const ICONS: &str = "/usr/share/grub/themes/evodevo/icons";
const DEFAULT_GRUB: &str = "/etc/default/grub";
const CFG: &str = "/boot/grub/grub.cfg";

struct Work(String);

impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The first line of a connected monitor's modes, when it is WIDTHxHEIGHT (after the script's ${m%%[!0-9x]*}).
pub fn mode_of(first_line: &str) -> Option<(u64, u64)> {
    let m: String = first_line.chars().take_while(|c| c.is_ascii_digit() || *c == 'x').collect();
    let (w, h) = m.split_once('x')?;
    if w.is_empty() || h.is_empty() || !sh::digits(w) || !sh::digits(h) {
        return None;
    }
    Some((w.parse().ok()?, h.parse().ok()?))
}

/// The screen's own resolution: the first mode of the first connected monitor (in the order of their names), else 1920x1080.
fn native() -> (u64, u64) {
    for f in sh::glob("/sys/class/drm", |n| n.starts_with("card") && n[4..].contains('-')) {
        if sh::cat(&format!("{f}/status")).as_deref() != Some("connected") {
            continue;
        }
        if let Some(m) = sh::head1(&format!("{f}/modes")).and_then(|l| mode_of(&l)) {
            return m;
        }
    }
    (1920, 1080)
}

/// install.sh with each NAME's first NAME=... line set to NAME='VALUE' (the script's set_var, one sed -i each).
pub fn set_vars(text: &[u8], vars: &[(&str, String)]) -> Vec<u8> {
    let mut lines: Vec<Vec<u8>> = text.split_inclusive(|&c| c == b'\n').map(|l| l.to_vec()).collect();
    for (name, value) in vars {
        let key = format!("{name}=");
        if let Some(l) = lines.iter_mut().find(|l| l.starts_with(key.as_bytes())) {
            let nl = l.ends_with(b"\n");
            *l = format!("{name}='{value}'{}", if nl { "\n" } else { "" }).into_bytes();
        }
    }
    lines.concat()
}

/// What `grep -o -E "^[[:space:]]*menuentry '[^']*'( --class [^ ]+)+"` takes from a line of grub.cfg, as `read -r` gives it
/// (the blanks at its start gone).
pub fn entry_line(line: &str) -> Option<&str> {
    let t = line.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let rest = t.strip_prefix("menuentry '")?;
    let close = rest.find('\'')?;
    let mut end = "menuentry '".len() + close + 1;
    let mut n = 0;
    while let Some(after) = t[end..].strip_prefix(" --class ") {
        let len = after.find(' ').unwrap_or(after.len());
        if len == 0 {
            break;
        }
        end += " --class ".len() + len;
        n += 1;
    }
    (n > 0).then(|| t[..end].trim_end_matches([' ', '\t']))
}

/// The theme's class (evodevo_entry_N) and the entry's own first class in such a line.
pub fn classes(line: &str) -> (Option<String>, Option<String>) {
    let entry = line.find("evodevo_entry_").and_then(|i| {
        let digits: String = line[i + 14..].chars().take_while(|c| c.is_ascii_digit()).collect();
        (!digits.is_empty()).then(|| format!("evodevo_entry_{digits}"))
    });
    let mut own = None;
    let mut rest = line;
    while let Some(i) = rest.find("--class ") {
        let after = &rest[i + 8..];
        let len = after.find(' ').unwrap_or(after.len());
        let c = &after[..len];
        rest = &after[len..];
        if len > 0 && !c.starts_with("evodevo_entry_") {
            own = Some(c.to_string());
            break;
        }
    }
    (entry, own)
}

/// A class that may name a file: [A-Za-z0-9_-]+
pub fn plain(c: &str) -> bool {
    !c.is_empty() && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-grub-theme");
    let mut preview = None;
    if args.get(1).map(|a| a == "--preview").unwrap_or(false) {
        match args.get(2) {
            Some(f) if !f.is_empty() => {
                // where the command was given, not the work folder
                preview = Some(if f.starts_with('/') { f.clone() } else { format!("{}/{f}", crate::py::getcwd()) });
            }
            _ => {
                eprintln!("labyrinth-grub-theme: usage: labyrinth-grub-theme --preview FILE");
                return 1;
            }
        }
    }
    let tmpdir = std::env::var("TMPDIR").ok().filter(|t| !t.is_empty()).unwrap_or_else(|| "/tmp".into());
    let work = match sh::mkdtemp(&format!("{tmpdir}/tmp.XXXXXXXXXX")) {
        Ok(w) => Work(w),
        Err(e) => {
            eprintln!("labyrinth-grub-theme: cannot make a work folder: {e}");
            return 1;
        }
    };
    let w = work.0.clone();
    let code = sh::status(&["cp", "-r", &format!("{SRC}/."), &format!("{w}/")]);
    if code != 0 {
        return code;
    }
    let (sw, shh) = native();
    let install = format!("{w}/install.sh");
    let vars: Vec<(&str, String)> = vec![
        ("ScreenWidth", sw.to_string()),
        ("ScreenHeight", shh.to_string()),
        ("FontSize", ((22 * shh + 540) / 1080).to_string()),
        ("Wallpaper", "#000000".into()),
        ("Title", "Labyrinth OS".into()),
        // H S V (degrees, percent, percent): the Labyrinth purple (hue 259) on black
        ("MenuBg", "259 30 9".into()),
        ("HeaderBg", "259 35 12".into()),
        ("FocusBg", "259 61 55".into()),
        ("TextFg", "0 0 100".into()),
        ("IconFg", "0 0 88".into()),
        ("BarBg", "0 0 100".into()),
        ("BarFg", "259 61 84".into()),
        ("MenuOpacity", "0.88".into()),
        ("HeaderOpacity", "0.95".into()),
        ("FocusOpacity", "0.90".into()),
    ];
    let set = fs::read(&install).and_then(|t| {
        let new = set_vars(&t, &vars);
        if new != t {
            let m = fs::metadata(&install)?;
            use std::os::unix::fs::MetadataExt;
            sh::replace_file(&install, &new, m.mode() & 0o7777, Some((m.uid(), m.gid())))?;
        }
        Ok(())
    });
    if let Err(e) = set {
        eprintln!("sed: can't read install.sh: {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
        return 2;
    }

    let run_install = |extra: &[&str], log: Option<&str>| -> i32 {
        let mut c = std::process::Command::new("sh");
        c.arg("./install.sh").args(extra).current_dir(&w);
        let _ = std::io::Write::flush(&mut std::io::stdout());
        match log {
            Some(l) => match fs::File::create(l).and_then(|f| Ok((f.try_clone()?, f))) {
                Ok((o, e)) => c.stdout(o).stderr(e),
                Err(_) => c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()),
            },
            None => c.stdout(std::process::Stdio::null()),
        };
        c.status().map(|s| s.code().unwrap_or(1)).unwrap_or(127)
    };

    if let Some(p) = preview {
        let code = run_install(&["-preview"], None);
        if code != 0 {
            return code;
        }
        std::env::set_current_dir(&w).ok();
        if sh::run(&["cp", "preview.png", &p], Io::Inherit, Io::Null) != 0 {
            let mut pngs: Vec<String> =
                sh::glob(".", |n| n.ends_with(".png")).into_iter().map(|x| x.trim_start_matches("./").to_string()).collect();
            if pngs.is_empty() {
                pngs.push("*.png".into());
            }
            let mut argv: Vec<String> = vec!["cp".into()];
            argv.extend(pngs.into_iter().map(|n| format!("./{n}")));
            argv.push(p);
            let code = sh::status(&argv);
            if code != 0 {
                return code;
            }
        }
        return 0;
    }

    if sh::euid() != 0 {
        eprintln!("labyrinth-grub-theme: run as root");
        return 1;
    }
    let (old_default, old_cfg) = match (fs::read(DEFAULT_GRUB), fs::read(CFG)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("labyrinth-grub-theme: cannot keep the old menu: {e}");
            return 1;
        }
    };
    let native = format!("{sw}x{shh}");
    // GRUB must draw at the size the theme is made for (and picks its own if the firmware cannot show that one)
    if sh::grep(DEFAULT_GRUB, |l| sh::starts(l, "GRUB_GFXMODE=") || sh::starts(l, "#GRUB_GFXMODE=")) {
        let line = format!("GRUB_GFXMODE={native},auto");
        let _ = sh::sed_i(DEFAULT_GRUB, |l| {
            (sh::starts(l, "GRUB_GFXMODE=") || sh::starts(l, "#GRUB_GFXMODE=")).then(|| line.clone().into_bytes())
        });
    } else {
        let _ = sh::append_line(DEFAULT_GRUB, &format!("GRUB_GFXMODE={native},auto"));
    }
    if !sh::grep(DEFAULT_GRUB, |l| sh::starts(l, "GRUB_GFXPAYLOAD_LINUX=")) {
        let _ = sh::append_line(DEFAULT_GRUB, "GRUB_GFXPAYLOAD_LINUX=keep");
    }

    let log = format!("{w}/install.log");
    let menu = |l: &[u8]| sh::lstrip(l).starts_with(b"menuentry ");
    if run_install(&[], Some(&log)) == 0 && sh::grep(CFG, menu) && sh::grep(CFG, |l| sh::contains(l, "initrd")) {
        // the theme gives each entry its icon by a class it writes into grub.cfg (evodevo_entry_N), which every later
        // grub-mkconfig loses; GRUB also picks an icon by the entry's own first class, which grub-mkconfig always writes:
        // each icon is kept under that name too
        let text = String::from_utf8_lossy(&fs::read(CFG).unwrap_or_default()).into_owned();
        for line in text.lines().filter_map(entry_line) {
            if let (Some(entry), Some(own)) = classes(line) {
                let (from, to) = (format!("{ICONS}/{entry}.png"), format!("{ICONS}/{own}.png"));
                if plain(&own) && sh::is_file(&from) && !sh::exists(&to) {
                    let code = sh::status(&["cp", &from, &to]);
                    if code != 0 {
                        return code;
                    }
                }
            }
        }
        println!("labyrinth-grub-theme: installed");
        0
    } else {
        eprintln!("labyrinth-grub-theme: failed; putting the old menu back");
        let log_text = fs::read(&log).unwrap_or_default();
        use std::io::Write;
        let _ = std::io::stderr().write_all(sh::tail(&log_text, 5));
        let _ = sh::write(DEFAULT_GRUB, &old_default);
        let _ = sh::write(CFG, &old_cfg);
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes() {
        assert_eq!(mode_of("1920x1080"), Some((1920, 1080)));
        assert_eq!(mode_of("3440x1440i"), Some((3440, 1440)));
        assert_eq!(mode_of("x1080"), None);
        assert_eq!(mode_of("1920x"), None);
        assert_eq!(mode_of("1920x1080x5"), None);
        assert_eq!(mode_of(""), None);
    }

    #[test]
    fn variables() {
        let t = b"A=1\nScreenWidth=1\nScreenWidth=2\nB=ScreenWidth=3".to_vec();
        assert_eq!(set_vars(&t, &[("ScreenWidth", "9".into()), ("B", "x".into())]), b"A=1\nScreenWidth='9'\nScreenWidth=2\nB='x'".to_vec());
    }

    #[test]
    fn entries() {
        let l = "\tmenuentry 'Labyrinth OS' --class labyrinth --class gnu-linux --class evodevo_entry_1 $menuentry_id_option 'x' {";
        let m = entry_line(l).unwrap();
        assert_eq!(m, "menuentry 'Labyrinth OS' --class labyrinth --class gnu-linux --class evodevo_entry_1");
        assert_eq!(classes(m), (Some("evodevo_entry_1".into()), Some("labyrinth".into())));
        assert_eq!(entry_line("menuentry 'x' {"), None);
        assert_eq!(
            classes("menuentry 'a' --class evodevo_entry_12 --class windows"),
            (Some("evodevo_entry_12".into()), Some("windows".into()))
        );
        assert!(plain("uefi_cd_dvd") && !plain("../x") && !plain(""));
    }
}
