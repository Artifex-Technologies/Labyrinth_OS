//! labyrinth-first-components: at the first login after installing, installs the optional parts chosen in the installer that the
//! medium did not carry (Sarina, Heimdall ...), with labyrinth-apps through Agora's password box, and says so in notifications.
//! The installer leaves their ids in /var/lib/labyrinth/first-login-components.json (LABYRINTH_FIRST_COMPONENTS), owned by the new
//! user; the session's autostart runs this. The Python script of the same name with the same waits (15 s, then up to 120 s for
//! the internet, asking every 10 s), notifications, log (~/.local/state/labyrinth/first-components.log) and exit codes; its text
//! is src/doc/labyrinth-first-components.txt. It takes no arguments (the script ignored any).

use crate::py::{self, Crash, Value, R};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const DOC: &str = include_str!("doc/labyrinth-first-components.txt");
pub const AGORA_SUDO: &str = "/usr/lib/agora/sudo";

pub fn list_path() -> String {
    py::env_get("LABYRINTH_FIRST_COMPONENTS", "/var/lib/labyrinth/first-login-components.json")
}

fn notify(title: &str, body: &str) -> R<()> {
    // no -A: notify-send would wait for an answer
    py::call(&["notify-send", "-a", "Labyrinth", "-i", "system-software-install", title, body])?;
    Ok(())
}

/// True once NetworkManager says the internet is reachable (it may still be connecting just after login).
fn online(wait: f64) -> R<bool> {
    let end = py::now() + wait;
    loop {
        let r = py::run(&["nmcli", "-t", "networking", "connectivity", "check"])?;
        if py::strip(&r.stdout) == "full" {
            return Ok(true);
        }
        if py::now() >= end {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

/// str.isalnum(): at least one character, each a letter or a digit of any script.
fn isalnum(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c != '_' && crate::pycompat::isword(c))
}

/// The component ids left for this user, or none (no file, or it is someone else's). `user` None: $USER.
pub fn todo(path: &str, user: Option<&str>) -> R<Vec<String>> {
    let data = match py::load_json(path) {
        Ok(d) => d,
        Err(_) => return Ok(vec![]),
    };
    let data = match data {
        Value::Object(m) => m,
        other => return Err(Crash(format!("AttributeError: '{}' object has no attribute 'get'", type_name(&other)))),
    };
    let want = match user {
        Some(u) if !u.is_empty() => Some(u.to_string()),
        _ => py::env("USER"),
    };
    let same = match (data.get("user").filter(|v| !v.is_null()), &want) {
        (None, None) => true,
        (Some(Value::String(s)), Some(w)) => s == w,
        _ => false,
    };
    if !same {
        return Ok(vec![]);
    }
    let items: Vec<Value> = match data.get("components") {
        None => vec![],
        Some(Value::Array(a)) => a.clone(),
        Some(Value::String(s)) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        Some(Value::Object(m)) => m.keys().map(|k| Value::String(k.clone())).collect(),
        Some(other) => return Err(Crash(format!("TypeError: '{}' object is not iterable", type_name(other)))),
    };
    Ok(items
        .into_iter()
        .filter_map(|c| match c {
            Value::String(s) if isalnum(&s.replace('-', "")) => Some(s),
            _ => None,
        })
        .collect())
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn run() -> R<i32> {
    let list = list_path();
    let ids = todo(&list, None)?;
    if ids.is_empty() {
        return Ok(0);
    }
    std::thread::sleep(Duration::from_secs(15)); // let the desktop settle first
    if !online(120.0)? {
        notify(
            "Optional parts not installed yet",
            "No internet connection. They will be installed at the next login, or get them in Agora.",
        )?;
        return Ok(0);
    }
    let names = ids.join(", ");
    notify("Installing what you chose", &format!("Getting {names}. Your password is asked once; this can take a while."))?;
    let log_dir = py::expanduser("~/.local/state/labyrinth");
    py::makedirs(&log_dir)?;
    let log_path = py::join(&log_dir, "first-components.log");
    let log = std::fs::OpenOptions::new().append(true).create(true).open(&log_path).map_err(|e| py::os_err(e, &log_path))?;
    let err = log.try_clone().map_err(Crash::from)?;
    let mut c = Command::new("labyrinth-apps");
    c.arg("install").args(&ids).stdin(Stdio::null()).stdout(log).stderr(err);
    if py::exists(AGORA_SUDO) {
        c.env("AGORA_SUDO", AGORA_SUDO);
    }
    let st = c.status().map_err(|e| py::os_err(e, "labyrinth-apps"))?;
    if st.code() == Some(0) {
        let _ = std::fs::remove_file(&list);
        notify("Installed", &format!("{names} are ready."))?;
    } else {
        notify(
            &format!("Could not install {names}"),
            "It will be tried again at the next login; Agora can install them too. Details: ~/.local/state/labyrinth/first-components.log",
        )?;
    }
    Ok(0)
}

pub fn main(_argv: &[String]) -> i32 {
    match run() {
        Ok(rc) => rc,
        Err(c) => py::crashed("labyrinth-first-components", &c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// tests/test_install_system.py's test_the_first_login_list_is_only_the_new_users, carried over.
    #[test]
    fn the_first_login_list_is_only_the_new_users() {
        let d = TempDir::new();
        let f = d.p("list.json");
        std::fs::write(&f, r#"{"user": "ada", "components": ["sarina", "heimdall", "bad; rm -rf /"]}"#).unwrap();
        assert_eq!(todo(&f, Some("ada")).unwrap(), vec!["sarina", "heimdall"], "ids only");
        assert!(todo(&f, Some("bob")).unwrap().is_empty());
        assert!(todo(&d.p("none.json"), Some("ada")).unwrap().is_empty());
    }

    #[test]
    fn ids_are_letters_digits_and_dashes() {
        let d = TempDir::new();
        let f = d.p("list.json");
        std::fs::write(&f, r#"{"user": "ada", "components": ["a-b", "-", "", "x_y", "é1", 5, null, "a b"]}"#).unwrap();
        assert_eq!(todo(&f, Some("ada")).unwrap(), vec!["a-b", "é1"]);
        std::fs::write(&f, r#"[1]"#).unwrap();
        assert!(todo(&f, Some("ada")).is_err(), "the script stopped on a list");
    }

    /// tests/test_install_system.py's test_the_first_login_list_is_where_labyrinth_first_components_reads_it.
    #[test]
    fn the_first_login_list_is_where_the_installer_writes_it() {
        let conf = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profile/airootfs/usr/share/yggdrasil/integrations/labyrinth-installer.conf"
        ))
        .unwrap();
        let line = conf.lines().find(|l| l.trim_start().starts_with("first_login")).expect("first_login in labyrinth-installer.conf");
        let value = line.split_once('=').unwrap().1.split('#').next().unwrap().trim();
        let _env = crate::testutil::env_lock();
        std::env::remove_var("LABYRINTH_FIRST_COMPONENTS");
        assert_eq!(list_path(), value);
    }
}
