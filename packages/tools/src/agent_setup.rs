//! labyrinth-agent-setup: gives the engineering agent (Claude) its own account and the sudo rules that let it run the gate
//! (labyrinth-agent) and nothing else as root, each session recorded (labyrinth-agent-rec) and every program in it in the kernel
//! audit log; --remove takes the rules away. The bash script of the same name with the same
//! options, output, files (the sudoers file, claude-sudo, Heimdall's maintenance_actors), calls and exit codes; its help is
//! src/doc/labyrinth-agent-setup.txt (what the rules mean is there).
//!
//! useradd, passwd, visudo and systemctl are still those programs. The sudoers file is written whole in /etc/sudoers.d under a
//! name sudo skips (a dot in it), checked by visudo there, given mode 0440 and root as owner, and renamed into place: sudo
//! never sees half a file or none (the script checked a copy in /tmp and copied it in with install). Heimdall's config.json is
//! read and written here (the script ran Python for it), whole and renamed into place, keeping its mode and owner.
//!
//! Fixed in both (script and port): the owner's name went into sudoers with no check but that the account exists (a numeric name
//! is a uid to `id`); it must be an account name now (^[A-Za-z_][A-Za-z0-9._-]{0,31}$). And the agent's account could be any
//! existing one: `--agent root` locked root's password and gave the owner root without one; an account below uid 1000, or the
//! owner's own, is refused now.
//!
//! Not 1:1:
//!   - `--owner` or `--agent` without a value is a usage error (exit code 2); bash stopped on the unbound $2 (exit code 1);
//!   - Heimdall's config.json holding NaN or Infinity, or a number beyond 64 bits, is read as JSON's standard reads it (Python
//!     read NaN and Infinity, and kept big numbers exact); a file that is not JSON, not an object, or whose maintenance_actors is
//!     not a list stops it with exit code 1 and one line, as Python's traceback did;
//!   - it no longer needs python3 for that step (the script skipped it without python3).
//!
//! Since 2026-11-19 (sudo-rs records no session): the agent's one root rule is the recorder, /usr/bin/labyrinth-agent-rec, and
//! the agent's real rules (the gate here, labyrinth-agent-window's list) are written for a second account, `{agent}-rules`
//! (useradd -r, no home, nologin, password locked), that no rule lets anyone become: the recorder asks sudo about it. The
//! recorder sets each session's login uid to the agent's, and /etc/audit/rules.d/50-labyrinth-agent.rules logs every execve
//! with that login uid (auditd enabled, the rules loaded with augenrules). Running it again on a system that has the agent's
//! account moves that system to this design.

use crate::agent_rec::{rules_user, REC};
use crate::pycompat::json::{self, Value};
use crate::sh::{self, Io};
use std::fs;
use std::os::unix::fs::MetadataExt;

pub const DOC: &str = include_str!("doc/labyrinth-agent-setup.txt");
const SUDOERS: &str = "/etc/sudoers.d/50-labyrinth-agent";
const CLAUDE_SUDO: &str = "/usr/local/bin/claude-sudo";
pub const AUDIT_RULES: &str = "/etc/audit/rules.d/50-labyrinth-agent.rules";
/// The key of the audit rule (ausearch -k labyrinth-agent).
pub const AUDIT_KEY: &str = "labyrinth-agent";
const NOLOGIN: &str = "/usr/bin/nologin";
/// The gate sudo lets the agent run, and the owner's window (the script checks both are there); both come with labyrinth-tools.
pub const GATE: &str = "/usr/bin/labyrinth-agent";
pub const WINDOW: &str = "/usr/bin/labyrinth-agent-window";
/// The Python gate of before: sudo looks in /usr/local/sbin first (secure_path), so it would be the one found, and the rule does
/// not allow it; it is removed.
const OLD_GATE: &str = "/usr/local/sbin/labyrinth-agent";
const HEIMDALL: &str = "/etc/heimdall/config.json";

fn err(msg: &str, code: i32) -> i32 {
    eprintln!("{msg}");
    code
}

/// ^[a-z][a-z0-9-]{1,25}$: at most 26, so that its rules account (`{agent}-rules`) is a valid name too (32 at most).
pub fn agent_name_ok(a: &str) -> bool {
    let b = a.as_bytes();
    (2..=26).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b[1..].iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

pub fn rules(agent: &str, owner: &str, owner_login: bool) -> String {
    let owner_rule = if owner_login {
        format!("Defaults:{owner} timestamp_timeout=720\n{owner} ALL=({agent}) ALL")
    } else {
        format!("{owner} ALL=({agent}) NOPASSWD: ALL")
    };
    let rules = rules_user(agent);
    format!(
        "# Written by labyrinth-agent-setup. See that script for what this means.\n\
         # The agent runs one program as root, the recorder; what it may run through it is {rules}'s (nobody can become that\n\
         # account: the recorder asks sudo about it).\n\
         {agent} ALL=(root) NOPASSWD: {REC}, {REC} *\n\
         {rules} ALL=(root) NOPASSWD: {GATE}, {GATE} *\n\
         {owner_rule}\n"
    )
}

pub fn claude_sudo(agent: &str) -> String {
    format!(
        "#!/bin/sh\n# claude-sudo CMD...: run CMD as root through the audited agent account ({agent}), recorded by labyrinth-agent-rec.\n\
         exec sudo -n -u {agent} sudo -n {REC} -- \"$@\"\n"
    )
}

/// The kernel audit rules: every program started (execve, execveat; 64- and 32-bit calls) with the agent's login uid, which
/// labyrinth-agent-rec gives each of its sessions.
pub fn audit_rules(agent: &str, uid: u32) -> String {
    let mut s = format!(
        "## Written by labyrinth-agent-setup: every program started in {agent}'s root sessions. labyrinth-agent-rec sets the\n\
         ## login uid of each session to {agent}'s ({uid}), so these match the session and all it starts.\n"
    );
    for arch in ["b64", "b32"] {
        s.push_str(&format!("-a always,exit -F arch={arch} -S execve -S execveat -F auid={uid} -k {AUDIT_KEY}\n"));
    }
    s
}

/// The account's login shell (None: no such account).
fn login_shell(name: &str) -> Option<String> {
    let c = std::ffi::CString::new(name).ok()?;
    let pw = unsafe { libc::getpwnam(c.as_ptr()) };
    if pw.is_null() || unsafe { (*pw).pw_shell }.is_null() {
        return None;
    }
    Some(unsafe { std::ffi::CStr::from_ptr((*pw).pw_shell) }.to_string_lossy().into_owned())
}

/// The rules account: made when missing (a system account, no home, no shell, password locked); one that is there must be a
/// system account nobody logs in to. Err: the exit code (said why).
fn rules_account(name: &str) -> Result<(), i32> {
    match sh::user_id(name) {
        None => {
            let c = sh::status(&["useradd", "-r", "-M", "-s", NOLOGIN, "-c", "The engineering agent's sudo rules (nobody logs in)", name]);
            if c != 0 {
                return Err(c);
            }
        }
        Some((uid, _)) if uid == 0 || uid >= 1000 => {
            return Err(err(&format!("{name} exists and is not a system account; it would hold the agent's root rules"), 2));
        }
        Some(_) => {
            let shell = login_shell(name).unwrap_or_default();
            if !["/usr/bin/nologin", "/sbin/nologin", "/usr/sbin/nologin", "/bin/false", "/usr/bin/false"].contains(&shell.as_str()) {
                return Err(err(
                    &format!("{name} has the login shell {shell}; it holds the agent's root rules, so nobody may log in to it"),
                    2,
                ));
            }
        }
    }
    match sh::run(&["passwd", "-l", name], Io::Null, Io::Inherit) {
        0 => Ok(()),
        c => Err(c),
    }
}

/// augenrules --load, when the audit package is there: the rules in /etc/audit/rules.d into the kernel. Err: why not.
fn load_audit() -> Result<(), String> {
    if !sh::executable("/usr/bin/augenrules") {
        return Err("augenrules is missing (the audit package)".into());
    }
    match sh::run(&["/usr/bin/augenrules", "--load"], Io::Null, Io::Inherit) {
        0 => Ok(()),
        c => Err(format!("augenrules --load failed (exit code {c})")),
    }
}

/// The audit rule written, auditd on (now and at boot, with the unit that loads the rules at boot where audit has one), the
/// rules loaded. Err: what failed (the rest was done).
fn audit_on(agent: &str, uid: u32) -> Result<(), String> {
    let dir = crate::py::dirname(AUDIT_RULES);
    sh::mkdir_p_mode(&dir, 0o750).map_err(|e| format!("cannot make {dir}: {e}"))?;
    sh::replace_file(AUDIT_RULES, audit_rules(agent, uid).as_bytes(), 0o640, Some((0, 0)))
        .map_err(|e| format!("cannot write {AUDIT_RULES}: {e}"))?;
    let mut units = vec!["enable", "--now", "auditd.service"];
    if sh::exists("/usr/lib/systemd/system/audit-rules.service") {
        units.push("audit-rules.service");
    }
    let mut argv = vec!["systemctl"];
    argv.extend(units);
    let c = sh::run(&argv, Io::Null, Io::Inherit);
    let loaded = load_audit();
    if c != 0 {
        return Err(format!("systemctl enable --now auditd failed (exit code {c})"));
    }
    loaded
}

/// Heimdall's maintenance_actors with the owner and the agent in it (Python's json.load, list.append and json.dump(indent=2)).
pub fn add_actors(text: &str, names: &[&str]) -> Result<String, String> {
    let mut c = json::loads(text)?;
    let obj = c.as_object_mut().ok_or("the file is not a JSON object")?;
    let a = obj.entry("maintenance_actors").or_insert_with(|| Value::Array(vec![]));
    let list = a.as_array_mut().ok_or("maintenance_actors is not a list")?;
    for u in names {
        if !list.iter().any(|v| v.as_str() == Some(u)) {
            list.push(Value::String(u.to_string()));
        }
    }
    Ok(json::dumps_indent(&c, 2))
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-agent-setup");
    let mut owner = std::env::var("SUDO_USER").unwrap_or_default();
    let mut agent = "claude-agent".to_string();
    let (mut remove, mut owner_login) = (false, false);
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            o @ ("--owner" | "--agent") => {
                let Some(v) = args.get(i + 1) else {
                    return err(&format!("{o} needs a value"), 2);
                };
                if o == "--owner" {
                    owner = v.clone();
                } else {
                    agent = v.clone();
                }
                i += 1;
            }
            "--remove" => remove = true,
            "--owner-login" => owner_login = true,
            "-h" | "--help" => {
                print!("{DOC}");
                return 0;
            }
            o => return err(&format!("unknown option {o}"), 2),
        }
        i += 1;
    }
    if sh::euid() != 0 {
        return err("run as root: sudo labyrinth-agent-setup", 1);
    }
    if !agent_name_ok(&agent) {
        return err("bad account name", 2);
    }
    if remove {
        let had_audit = sh::exists(AUDIT_RULES);
        for f in [SUDOERS, CLAUDE_SUDO, AUDIT_RULES] {
            if let Err(e) = sh::rm_f(f) {
                return err(&format!("rm: cannot remove '{f}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0))), 1);
            }
        }
        let rules = rules_user(&agent);
        println!(
            "removed the sudo rules and the audit rule; the accounts {agent} and {rules} and {agent}'s home are left (userdel -r {agent}; userdel {rules})"
        );
        if had_audit {
            if let Err(e) = load_audit() {
                return err(&format!("the audit rule is removed but still loaded: {e}"), 1);
            }
        }
        return 0;
    }
    if owner.is_empty() {
        return err("say who the owner is: --owner USER", 2);
    }
    if !sh::account_name(&owner) {
        return err("bad owner name", 2);
    }
    if sh::user_id(&owner).is_none() {
        return err(&format!("no such user: {owner}"), 2);
    }
    if owner == agent {
        return err("the owner and the agent must be two accounts", 2);
    }
    match sh::user_id(&agent) {
        Some((uid, _)) if uid < 1000 => return err(&format!("{agent} is a system account, not one for the agent"), 2),
        Some(_) => {}
        None => {
            let code = sh::status(&["useradd", "-m", "-s", "/bin/bash", "-c", "Claude, the engineering agent (audited account)", &agent]);
            if code != 0 {
                return code;
            }
        }
    }
    let code = sh::run(&["passwd", "-l", &agent], Io::Null, Io::Inherit);
    if code != 0 {
        return code;
    }
    if let Err(code) = rules_account(&rules_user(&agent)) {
        return code;
    }
    let Some((agent_uid, _)) = sh::user_id(&agent) else { return err(&format!("no such user: {agent}"), 1) };
    // the Limbo manifests the agent may start, root's alone (labyrinth-agent refuses a manifest when it or a folder above it is
    // not root's or may be written by others)
    let dir = "/var/lib/limbo/build-agent";
    if let Err(e) = sh::install_dir(dir, 0o755, Some((0, 0))) {
        return err(&format!("install: cannot make {dir}: {e}"), 1);
    }
    for p in [GATE, WINDOW, REC] {
        if !sh::executable(p) {
            return err(&format!("{p} is missing (it comes with the labyrinth-tools package)"), 1);
        }
    }
    if sh::exists(OLD_GATE) {
        if let Err(e) = sh::rm_f(OLD_GATE) {
            return err(&format!("rm: cannot remove '{OLD_GATE}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0))), 1);
        }
        println!("removed the old Python gate {OLD_GATE} (labyrinth-tools installs {GATE})");
    }
    if let Err(code) = write_sudoers(SUDOERS, &rules(&agent, &owner, owner_login), "the sudoers file did not validate; nothing changed") {
        return code;
    }
    if let Err(e) = sh::replace_file(CLAUDE_SUDO, claude_sudo(&agent).as_bytes(), 0o755, None) {
        return err(&format!("cannot write {CLAUDE_SUDO}: {e}"), 1);
    }
    // Heimdall: who may open a maintenance window
    if sh::is_file(HEIMDALL) {
        let real = fs::canonicalize(HEIMDALL).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| HEIMDALL.into());
        let done = fs::read(&real)
            .map_err(|e| e.to_string())
            .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
            .and_then(|t| add_actors(&t, &[&owner, &agent]))
            .and_then(|out| {
                let m = fs::metadata(&real).map_err(|e| e.to_string())?;
                sh::replace_file(&real, out.as_bytes(), m.mode() & 0o7777, Some((m.uid(), m.gid()))).map_err(|e| e.to_string())
            });
        if let Err(e) = done {
            return err(&format!("labyrinth-agent-setup: {HEIMDALL}: {e}"), 1);
        }
        sh::quiet(&["systemctl", "try-restart", "heimdall.service"]);
    }
    if let Err(e) = audit_on(&agent, agent_uid) {
        return err(&format!("the sudo rules are in place, but the audit rule is not on: {e}"), 1);
    }
    println!(
        "done: account {agent} (locked password), rules in {SUDOERS} (the agent's for {}), helper claude-sudo; every session is \
         recorded in /var/log/labyrinth-agent and every program in it in the audit log (ausearch -k {AUDIT_KEY})",
        rules_user(&agent)
    );
    0
}

/// A sudoers file written whole under a name sudo skips, checked by visudo, made root's and 0440, renamed into place. Err: the
/// exit code (said why).
pub fn write_sudoers(path: &str, text: &str, invalid: &str) -> Result<(), i32> {
    let dir = std::path::Path::new(path).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_else(|| "/".into());
    let (tmp, mut f) =
        sh::mkstemp(&format!("{dir}/.{}.XXXXXX", sh::base(path))).map_err(|e| err(&format!("cannot write in {dir}: {e}"), 1))?;
    let fail = |code| {
        let _ = fs::remove_file(&tmp);
        code
    };
    use std::io::Write;
    if let Err(e) = f.write_all(text.as_bytes()).and_then(|_| f.sync_all()) {
        return Err(fail(err(&format!("cannot write {tmp}: {e}"), 1)));
    }
    if sh::run(&["visudo", "-cf", &tmp], Io::Null, Io::Inherit) != 0 {
        return Err(fail(err(invalid, 1)));
    }
    use std::os::unix::fs::PermissionsExt;
    let placed = std::os::unix::fs::fchown(&f, Some(0), Some(0))
        .and_then(|_| f.set_permissions(fs::Permissions::from_mode(0o440)))
        .and_then(|_| fs::rename(&tmp, path));
    placed.map_err(|e| fail(err(&format!("cannot put {path} in place: {e}"), 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(agent_name_ok("claude-agent") && agent_name_ok("ab") && agent_name_ok(&"a".repeat(26)));
        assert!(sh::account_name(&rules_user(&"a".repeat(26))) && !agent_name_ok(&"a".repeat(27)));
        assert!(
            !agent_name_ok("a")
                && !agent_name_ok("Root")
                && !agent_name_ok("1ab")
                && !agent_name_ok(&"a".repeat(32))
                && !agent_name_ok("a_b")
        );
    }

    #[test]
    fn the_rules() {
        let r = rules("claude-agent", "owner", false);
        // the agent itself: the recorder and nothing else; the gate is its rules account's
        let agent: Vec<&str> = r.lines().filter(|l| l.starts_with("claude-agent ")).collect();
        assert_eq!(agent, ["claude-agent ALL=(root) NOPASSWD: /usr/bin/labyrinth-agent-rec, /usr/bin/labyrinth-agent-rec *"]);
        assert!(r.contains("\nclaude-agent-rules ALL=(root) NOPASSWD: /usr/bin/labyrinth-agent, /usr/bin/labyrinth-agent *\n"));
        // nobody may become the rules account
        assert!(!r.contains("(claude-agent-rules)") && !r.contains("=(ALL"));
        assert!(r.ends_with("owner ALL=(claude-agent) NOPASSWD: ALL\n"));
        let r = rules("claude-agent", "owner", true);
        assert!(r.ends_with("Defaults:owner timestamp_timeout=720\nowner ALL=(claude-agent) ALL\n"));
        // sudo-rs reads every line: it has no I/O logging (log_input, iolog_dir), requiretty or timestamp_type=global
        assert!(!r.contains("log_input") && !r.contains("requiretty") && !r.contains("global"));
    }

    #[test]
    fn helper_and_audit_rules() {
        assert_eq!(
            claude_sudo("claude-agent"),
            "#!/bin/sh\n# claude-sudo CMD...: run CMD as root through the audited agent account (claude-agent), recorded by \
             labyrinth-agent-rec.\nexec sudo -n -u claude-agent sudo -n /usr/bin/labyrinth-agent-rec -- \"$@\"\n"
        );
        let a = audit_rules("claude-agent", 1001);
        let rules: Vec<&str> = a.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(
            rules,
            [
                "-a always,exit -F arch=b64 -S execve -S execveat -F auid=1001 -k labyrinth-agent",
                "-a always,exit -F arch=b32 -S execve -S execveat -F auid=1001 -k labyrinth-agent"
            ]
        );
        assert!(a.lines().all(|l| l.starts_with("## ") || l.starts_with("-a ")));
    }

    #[test]
    fn heimdall_actors() {
        assert_eq!(
            add_actors("{\"a\": 1}", &["o", "c"]).unwrap(),
            "{\n  \"a\": 1,\n  \"maintenance_actors\": [\n    \"o\",\n    \"c\"\n  ]\n}"
        );
        assert_eq!(
            add_actors("{\"maintenance_actors\": [\"c\"]}", &["o", "c"]).unwrap(),
            "{\n  \"maintenance_actors\": [\n    \"c\",\n    \"o\"\n  ]\n}"
        );
        assert!(add_actors("[]", &["o"]).is_err());
        assert!(add_actors("{\"maintenance_actors\": \"o\"}", &["o"]).is_err());
        assert!(add_actors("not json", &["o"]).is_err());
    }
}
