//! Switches the PAM module on or off in /etc/pam.d/system-auth (which login, sudo, su, polkit and the display manager all include).
//! The line is written with a leading "-", so if the module file is ever missing PAM skips it silently and nobody is locked out.
//! It goes right after the first line of the auth block, before pam_systemd_home / pam_unix: those lines use "success=N" skips, and
//! a line inserted between them would shift the skip. A copy of the original is kept next to the file.

const LINE: &str = "-auth      [success=done default=ignore]  pam_labyrinth_distress.so  # labyrinth distress password";
const MARK: &str = "pam_labyrinth_distress.so";

fn path() -> std::path::PathBuf {
    crate::config::env("LABYRINTH_DISTRESS_PAMFILE").map(Into::into).unwrap_or_else(|| "/etc/pam.d/system-auth".into())
}

/// The file with the line added (once), or removed.
pub fn edit(text: &str, enable: bool) -> Result<String, String> {
    let mut lines: Vec<&str> = text.lines().collect();
    lines.retain(|l| !l.contains(MARK));
    if enable {
        // before the first auth line that is not faillock's preauth and not a comment
        let at = lines
            .iter()
            .position(|l| {
                let t = l.trim_start_matches('-').trim_start();
                t.starts_with("auth") && !l.contains("pam_faillock.so") && !t.starts_with('#')
            })
            .ok_or("this PAM file has no auth lines I understand")?;
        // refuse a file where the skip counts would be unsafe to reason about
        if !lines.iter().any(|l| l.contains("pam_unix.so") && l.trim_start_matches('-').trim_start().starts_with("auth")) {
            return Err("this PAM file has no pam_unix auth line; it was not changed".into());
        }
        lines.insert(at, LINE);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

pub fn run(enable: bool) -> serde_json::Value {
    if unsafe { libc::geteuid() } != 0 && !cfg!(feature = "test-paths") {
        return serde_json::json!({"ok": false, "error": "not allowed"});
    }
    let p = path();
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) => return serde_json::json!({"ok": false, "error": e.to_string()}),
    };
    if enable && !std::path::Path::new("/usr/lib/security/pam_labyrinth_distress.so").exists() && !cfg!(feature = "test-paths") {
        return serde_json::json!({"ok": false, "error": "the PAM module is not installed"});
    }
    let new = match edit(&text, enable) {
        Ok(n) => n,
        Err(e) => return serde_json::json!({"ok": false, "error": e}),
    };
    if new == text {
        return serde_json::json!({"ok": true, "changed": false});
    }
    let bak = p.with_extension("labyrinth-original");
    if !bak.exists() {
        let _ = std::fs::copy(&p, &bak);
    }
    let tmp = p.with_extension("labyrinth-new");
    let meta = std::fs::metadata(&p).ok();
    let r = std::fs::write(&tmp, &new).and_then(|_| {
        if let Some(m) = meta {
            std::fs::set_permissions(&tmp, m.permissions())?;
        }
        std::fs::rename(&tmp, &p)
    });
    match r {
        Ok(()) => serde_json::json!({"ok": true, "changed": true}),
        Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSTEM_AUTH: &str = "#%PAM-1.0\n\nauth       required                    pam_faillock.so      preauth\n# Optionally use requisite above\n-auth      [success=2 default=ignore]  pam_systemd_home.so\nauth       [success=1 default=bad]     pam_unix.so          try_first_pass nullok\nauth       [default=die]               pam_faillock.so      authfail\nauth       optional                    pam_permit.so\n\naccount    required                    pam_unix.so\n";

    #[test]
    fn inserted_before_the_skips() {
        let n = edit(SYSTEM_AUTH, true).unwrap();
        let l: Vec<&str> = n.lines().collect();
        let me = l.iter().position(|x| x.contains(MARK)).unwrap();
        let home = l.iter().position(|x| x.contains("pam_systemd_home.so") && x.starts_with("-auth")).unwrap();
        let unix = l.iter().position(|x| x.contains("auth") && x.contains("pam_unix.so")).unwrap();
        assert!(me < home && home + 1 == unix, "the lines the skip counts jump over stay together");
        assert!(l[me].starts_with("-auth"));
    }

    #[test]
    fn twice_is_once_and_off_restores() {
        let on = edit(SYSTEM_AUTH, true).unwrap();
        assert_eq!(edit(&on, true).unwrap(), on);
        assert_eq!(edit(&on, false).unwrap(), SYSTEM_AUTH);
    }

    #[test]
    fn odd_files_are_left_alone() {
        assert!(edit("auth required pam_deny.so\n", true).is_err());
    }
}
