//! pam_labyrinth_distress.so: the distress password at sudo, login, su and every other password prompt.
//!
//! In the auth stack, before pam_unix:   -auth [success=done default=ignore] pam_labyrinth_distress.so
//! It does nothing at all (not even a prompt) unless /etc/labyrinth/distress.json exists. When it does, it takes the typed password
//! (asking for it the way PAM does, so pam_unix then reuses it with try_first_pass) and hands it to /usr/bin/labyrinth-distress,
//! which says whether it is the distress password and, if so, does what was set up. Only a "silent login" answers success; every
//! other answer is PAM_IGNORE, so the stack carries on and the password is judged as usual. It can never fail a login by itself,
//! and any trouble (no helper, a crash, not root) is PAM_IGNORE too.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

const PAM_SUCCESS: c_int = 0;
const PAM_IGNORE: c_int = 25;
const PAM_USER: c_int = 2;
const PAM_AUTHTOK: c_int = 6;

#[link(name = "pam")]
extern "C" {
    fn pam_get_user(pamh: *mut c_void, user: *mut *const c_char, prompt: *const c_char) -> c_int;
    fn pam_get_authtok(pamh: *mut c_void, item: c_int, authtok: *mut *const c_char, prompt: *const c_char) -> c_int;
}

#[cfg(not(feature = "test-paths"))]
const HELPER: &str = "/usr/bin/labyrinth-distress";
#[cfg(not(feature = "test-paths"))]
const SETTINGS: &str = "/etc/labyrinth/distress.json";

/// Test build only: where the helper and its settings are, and root is not required. NEVER in a release build.
#[cfg(feature = "test-paths")]
fn test_env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

pub fn json_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// True when the helper answers that this password fired the distress action and the session should be let in.
pub fn ask(user: &str, password: &str) -> bool {
    #[cfg(not(feature = "test-paths"))]
    let mut cmd = Command::new(HELPER);
    #[cfg(feature = "test-paths")]
    let mut cmd = Command::new(test_env("LABYRINTH_TEST_HELPER", "/usr/bin/labyrinth-distress"));
    cmd.arg("check-for").env_clear();
    #[cfg(feature = "test-paths")]
    for (k, v) in std::env::vars().filter(|(k, _)| k.starts_with("LABYRINTH_")) {
        cmd.env(k, v);
    }
    let Ok(mut child) = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else { return false };
    if let Some(mut i) = child.stdin.take() {
        let _ = i.write_all(format!("{{\"user\":{},\"password\":{}}}", json_string(user), json_string(password)).as_bytes());
    }
    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    let _ = child.wait();
    out.contains("\"result\":\"distress\"") && out.contains("\"unlock\":true")
}

fn authenticate(pamh: *mut c_void) -> c_int {
    // Only root may judge passwords (sudo, login, su and polkit's helper are all setuid or root), and only when it is set up.
    #[cfg(not(feature = "test-paths"))]
    let (is_root, settings) = (unsafe { libc::geteuid() } == 0, SETTINGS.to_string());
    #[cfg(feature = "test-paths")]
    let (is_root, settings) = (true, format!("{}/distress.json", test_env("LABYRINTH_DISTRESS_DIR", "/etc/labyrinth")));
    if !is_root || !std::path::Path::new(&settings).exists() {
        return PAM_IGNORE;
    }
    unsafe {
        let mut user: *const c_char = std::ptr::null();
        if pam_get_user(pamh, &mut user, std::ptr::null()) != PAM_SUCCESS || user.is_null() {
            return PAM_IGNORE;
        }
        let mut tok: *const c_char = std::ptr::null();
        if pam_get_authtok(pamh, PAM_AUTHTOK, &mut tok, std::ptr::null()) != PAM_SUCCESS || tok.is_null() {
            return PAM_IGNORE;
        }
        let (u, p) = (CStr::from_ptr(user).to_string_lossy().into_owned(), CStr::from_ptr(tok).to_string_lossy().into_owned());
        if ask(&u, &p) {
            PAM_SUCCESS
        } else {
            PAM_IGNORE
        }
    }
}

#[no_mangle]
pub extern "C" fn pam_sm_authenticate(pamh: *mut c_void, _flags: c_int, _argc: c_int, _argv: *const *const c_char) -> c_int {
    // a panic must never cross into the C caller
    std::panic::catch_unwind(|| authenticate(pamh)).unwrap_or(PAM_IGNORE)
}

#[no_mangle]
pub extern "C" fn pam_sm_setcred(_pamh: *mut c_void, _flags: c_int, _argc: c_int, _argv: *const *const c_char) -> c_int {
    PAM_IGNORE
}

#[allow(dead_code)]
const _: c_int = PAM_USER;

#[cfg(test)]
mod tests {
    #[test]
    fn json_escapes() {
        assert_eq!(super::json_string("a\"b\\c\n"), "\"a\\\"b\\\\c\\u000a\"");
    }
}
