//! labyrinth-distress: the root helper behind Settings > Security > Distress password and the lock screen.
//! Every request is one JSON object on stdin and one JSON object on stdout. It runs through pkexec (action org.labyrinth.distress),
//! so the account it acts for is the one pkexec reports (PKEXEC_UID), never a name taken from the request.
//!
//!   open     {password}                          the settings and the event log, after the account password is checked
//!   save     {password, second_password?, config, new_password?}
//!   check    {password}                          lock screen / prompts: does this password fire the distress action?
//!   plan     {config}                            what section 2 would do on this computer (no secrets)
//!   run-extra                                    internal: carries out section 2 a moment after `check` answered

mod actions;
mod config;
mod hash;
mod pam;
mod pamconf;
mod trigger;

use config::{Config, Extra};
use serde_json::{json, Value};
use std::io::Read;

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn account() -> Result<String, String> {
    let uid: u32 = match std::env::var("PKEXEC_UID") {
        Ok(u) => u.parse().map_err(|_| "bad PKEXEC_UID")?,
        // run directly (tests, or as root in a terminal): the real user id of the caller
        Err(_) => unsafe { libc::getuid() },
    };
    for l in std::fs::read_to_string("/etc/passwd").unwrap_or_default().lines() {
        let p: Vec<&str> = l.split(':').collect();
        if p.len() > 2 && p[2].parse::<u32>() == Ok(uid) {
            return Ok(p[0].to_string());
        }
    }
    Err("this account is not known".into())
}

fn verify_account(user: &str, password: &str) -> bool {
    let service = config::env("LABYRINTH_DISTRESS_PAM").map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "login".into());
    pam::Pam { service }.check(user, password).unwrap_or(false)
}

fn log_event(kind: &str, detail: &str) {
    let d = config::state_dir();
    if std::fs::create_dir_all(&d).is_ok() {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).mode(0o600).open(d.join("events.log")) {
            let _ = writeln!(f, "{}", json!({"time": now(), "kind": kind, "detail": detail}));
        }
    }
    // The system journal gets a plain line too, for Heimdall and for the administrator.
    let _ = std::process::Command::new("logger").args(["-t", "labyrinth-distress", "-p", "auth.warning", &format!("{kind}: {detail}")]).status();
}

fn events() -> Vec<Value> {
    let t = std::fs::read_to_string(config::state_dir().join("events.log")).unwrap_or_default();
    let mut v: Vec<Value> = t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    v.reverse();
    v.truncate(100);
    v
}

fn fail(msg: &str) -> Value {
    json!({"ok": false, "error": msg})
}

/// The settings as the page may keep them: whether a password is set, never the password or its hash.
fn open(req: &Value) -> Value {
    let user = match account() {
        Ok(u) => u,
        Err(e) => return fail(&e),
    };
    if !verify_account(&user, req["password"].as_str().unwrap_or("")) {
        return fail("That is not your password");
    }
    json!({"ok": true, "config": config::load().public(), "events": events()})
}

/// Saving rules, apart from the account password being right (checked by the caller): returns the config to store.
fn merge_save(old: &Config, new: &Config, new_password: Option<&str>, account_password_equals: impl Fn(&str) -> bool) -> Result<Config, String> {
    let mut c = new.clone();
    c.hash = old.hash.clone();
    if let Some(p) = new_password.filter(|p| !p.is_empty()) {
        if p.chars().count() < 6 {
            return Err("The distress password must be at least 6 characters".into());
        }
        if account_password_equals(p) {
            return Err("The distress password must be different from your real password".into());
        }
        c.hash = hash::hash(p)?;
    }
    if c.enabled && c.hash.is_empty() {
        return Err("Set the distress password first".into());
    }
    let problems = c.problems();
    if c.enabled && !problems.is_empty() {
        return Err(problems.join(". "));
    }
    Ok(c)
}

fn save(req: &Value) -> Value {
    let user = match account() {
        Ok(u) => u,
        Err(e) => return fail(&e),
    };
    let pw = req["password"].as_str().unwrap_or("");
    if !verify_account(&user, pw) {
        return fail("That is not your password");
    }
    let new: Config = match serde_json::from_value(req["config"].clone()) {
        Ok(c) => c,
        Err(_) => return fail("The settings could not be read"),
    };
    // Section 2 can destroy data: the account password is asked for a second time, separately.
    if new.enabled && new.extra != Extra::None && !verify_account(&user, req["second_password"].as_str().unwrap_or("")) {
        return fail("Section 2 needs your password once more, and it was not right");
    }
    let old = config::load();
    let cfg = match merge_save(&old, &new, req["new_password"].as_str(), |p| verify_account(&user, p)) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    if let Err(e) = config::save(&cfg) {
        return fail(&e);
    }
    trigger::store(&[]);
    // The PAM module line is in the system's auth stack only while the password is on and meant for prompts.
    let pam = pamconf::run(cfg.enabled && cfg.in_prompts);
    if !pam["ok"].as_bool().unwrap_or(false) && cfg.in_prompts {
        let why = pam["error"].as_str().unwrap_or("unknown").to_string();
        log_event("pam", &format!("could not change the PAM setup: {why}"));
        return json!({"ok": false, "error": format!("Saved, but terminals and other prompts could not be switched on: {why}")});
    }
    log_event("saved", &format!("distress password {} (extra: {:?})", if cfg.enabled { "on" } else { "off" }, cfg.extra));
    json!({"ok": true, "config": cfg.public()})
}

/// Does the typed password fire the distress action? Answers {"result":"no"} for anything else, including a distress password typed
/// too few times so far: that looks exactly like a wrong password.
fn check(req: &Value) -> Value {
    match account() {
        Ok(user) => check_user(req, &user, false),
        Err(e) => fail(&e),
    }
}

/// From the PAM module (root, environment cleared): the account is named in the request. Only for a direct root caller, never
/// through pkexec, so nobody can ask about another account. Only when the password is also meant to work in prompts.
fn check_for(req: &Value) -> Value {
    if std::env::var_os("PKEXEC_UID").is_some() || (unsafe { libc::geteuid() } != 0 && !cfg!(feature = "test-paths")) {
        return fail("not allowed");
    }
    match req["user"].as_str().filter(|u| !u.is_empty()) {
        Some(u) => check_user(req, u, true),
        None => fail("no user"),
    }
}

fn check_user(req: &Value, user: &str, prompts_only: bool) -> Value {
    let cfg = config::load();
    if prompts_only && !cfg.in_prompts {
        return json!({"ok": true, "result": "no"});
    }
    let pw = req["password"].as_str().unwrap_or("");
    if !cfg.active(now()) || !hash::verify(pw, &cfg.hash) {
        return json!({"ok": true, "result": "no"});
    }
    let mut hits = trigger::load();
    let fired = trigger::register(&mut hits, now(), cfg.count, cfg.window_secs);
    trigger::store(&hits);
    if !fired {
        return json!({"ok": true, "result": "no"});
    }
    let dry = config::env("LABYRINTH_DISTRESS_DRY").is_some();
    log_event("fired", &format!("the distress password was used for {user}{}", if prompts_only { " at a password prompt" } else { "" }));
    let ctx = actions::Ctx::detect(user);
    if cfg.decoy {
        // Locking the real account only makes sense once something can open the decoy session; until then it would just lock you out.
        if std::path::Path::new("/usr/lib/labyrinth/decoy-session").exists() {
            for l in actions::execute(&actions::decoy_steps(&ctx), dry) {
                log_event("decoy", &l);
            }
        } else {
            log_event("decoy", "the decoy session is not available on this system yet; nothing was changed");
        }
    }
    if cfg.extra != Extra::None {
        // Section 2 runs a moment later and detached, so the lock screen gets its answer first.
        use std::os::unix::process::CommandExt;
        if let Ok(exe) = std::env::current_exe() {
            let mut cmd = std::process::Command::new(exe);
            cmd.arg("run-extra").arg(user).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
            let _ = cmd.spawn();
        }
    }
    json!({"ok": true, "result": "distress", "unlock": cfg.silent_login, "decoy": cfg.decoy})
}

fn run_extra(user: &str) {
    std::thread::sleep(std::time::Duration::from_secs(2));
    let cfg = config::load();
    let dry = config::env("LABYRINTH_DISTRESS_DRY").is_some();
    for l in actions::execute(&actions::extra_steps(&cfg, &actions::Ctx::detect(user)), dry) {
        log_event("action", &l);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("");
    if cmd == "run-extra" {
        run_extra(args.get(2).map(String::as_str).unwrap_or(""));
        return;
    }
    let mut body = String::new();
    let _ = std::io::stdin().read_to_string(&mut body);
    let req: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let out = match cmd {
        "open" => open(&req),
        "save" => save(&req),
        "check" => check(&req),
        "check-for" => check_for(&req),
        "pam-enable" | "pam-disable" => pamconf::run(cmd == "pam-enable"),
        "plan" => {
            let cfg: Config = serde_json::from_value(req["config"].clone()).unwrap_or_default();
            let ctx = actions::Ctx::detect(&account().unwrap_or_default());
            json!({"ok": true, "steps": actions::describe(&cfg, &ctx)})
        }
        _ => fail("unknown command"),
    };
    println!("{out}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config { enabled: true, ..Config::default() }
    }

    #[test]
    fn needs_a_password_when_enabled() {
        assert!(merge_save(&Config::default(), &cfg(), None, |_| false).is_err());
    }

    #[test]
    fn refuses_the_real_password() {
        assert!(merge_save(&Config::default(), &cfg(), Some("hunter22"), |p| p == "hunter22").is_err());
    }

    #[test]
    fn refuses_short() {
        assert!(merge_save(&Config::default(), &cfg(), Some("abc"), |_| false).is_err());
    }

    #[test]
    fn keeps_the_old_hash_without_a_new_password() {
        let old = Config { hash: "$y$old".into(), ..Config::default() };
        let c = merge_save(&old, &cfg(), None, |_| false).unwrap();
        assert_eq!(c.hash, "$y$old");
    }

    #[test]
    fn stores_a_hash_not_the_password() {
        let c = merge_save(&Config::default(), &cfg(), Some("sunrise2"), |_| false).unwrap();
        assert!(c.hash.starts_with("$y$") && !c.hash.contains("sunrise2"));
        assert!(hash::verify("sunrise2", &c.hash));
    }
}
