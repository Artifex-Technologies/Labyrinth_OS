//! labyrinth-agent-setup: gives the engineering agent (Claude) its own account and the sudo rules that let it run the gate
//! (labyrinth-agent) and nothing else as root; --remove takes the rules away. The bash script of the same name with the same
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

use crate::pycompat::json::{self, Value};
use crate::sh::{self, Io};
use std::fs;
use std::os::unix::fs::MetadataExt;

pub const DOC: &str = include_str!("doc/labyrinth-agent-setup.txt");
const SUDOERS: &str = "/etc/sudoers.d/50-labyrinth-agent";
const CLAUDE_SUDO: &str = "/usr/local/bin/claude-sudo";
/// The gate sudo lets the agent run, and the owner's window (the script checks both are there).
pub const GATE: &str = "/usr/local/sbin/labyrinth-agent";
pub const WINDOW: &str = "/usr/bin/labyrinth-agent-window";
const HEIMDALL: &str = "/etc/heimdall/config.json";

fn err(msg: &str, code: i32) -> i32 {
    eprintln!("{msg}");
    code
}

/// ^[a-z][a-z0-9-]{1,30}$
pub fn agent_name_ok(a: &str) -> bool {
    let b = a.as_bytes();
    (2..=31).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b[1..].iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

pub fn rules(agent: &str, owner: &str, owner_login: bool) -> String {
    let owner_rule = if owner_login {
        format!("Defaults:{owner} timestamp_type=global, timestamp_timeout=720\n{owner} ALL=({agent}) ALL")
    } else {
        format!("{owner} ALL=({agent}) NOPASSWD: ALL")
    };
    format!(
        "# Written by labyrinth-agent-setup. See that script for what this means.\n\
         Defaults:{agent} !requiretty, log_input, log_output, iolog_dir=/var/log/sudo-io/{agent}\n\
         {agent} ALL=(root) NOPASSWD: {GATE}, {GATE} *\n\
         {owner_rule}\n"
    )
}

pub fn claude_sudo(agent: &str) -> String {
    format!("#!/bin/sh\n# claude-sudo CMD...: run CMD as root through the audited agent account ({agent}).\nexec sudo -n -u {agent} sudo -n \"$@\"\n")
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
        for f in [SUDOERS, CLAUDE_SUDO] {
            if let Err(e) = sh::rm_f(f) {
                return err(&format!("rm: cannot remove '{f}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0))), 1);
            }
        }
        println!("removed the sudo rules; the account {agent} and its home are left (userdel -r {agent} deletes them)");
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
    // sudo's recordings of the agent; the Limbo manifests the agent may start, root's alone (labyrinth-agent refuses a manifest
    // when it or a folder above it is not root's or may be written by others)
    for (dir, mode) in [("/var/log/sudo-io", 0o750), ("/var/lib/limbo/build-agent", 0o755)] {
        if let Err(e) = sh::install_dir(dir, mode, Some((0, 0))) {
            return err(&format!("install: cannot make {dir}: {e}"), 1);
        }
    }
    for (p, from) in [(GATE, "the Labyrinth profile"), (WINDOW, "the labyrinth-tools package")] {
        if !sh::executable(p) {
            return err(&format!("{p} is missing (it comes with {from})"), 1);
        }
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
    println!("done: account {agent} (locked password), rules in {SUDOERS}, helper claude-sudo, I/O logs in /var/log/sudo-io/{agent}");
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
        assert!(agent_name_ok("claude-agent") && agent_name_ok("ab"));
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
        assert!(r.contains("claude-agent ALL=(root) NOPASSWD: /usr/local/sbin/labyrinth-agent, /usr/local/sbin/labyrinth-agent *\n"));
        assert!(r.ends_with("owner ALL=(claude-agent) NOPASSWD: ALL\n"));
        let r = rules("claude-agent", "owner", true);
        assert!(r.ends_with("Defaults:owner timestamp_type=global, timestamp_timeout=720\nowner ALL=(claude-agent) ALL\n"));
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
