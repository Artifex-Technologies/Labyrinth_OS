//! labyrinth-launch ID [FILE-OR-URL ...]: starts an app the Labyrinth way, each in a systemd scope of its own inside a slice
//! named after it (what the per-app network rules match), counting the launch (~/.local/share/labyrinth/app-usage.json) and, when
//! its settings ask (~/.config/labyrinth/apps/ID.json), in a bubblewrap sandbox. The Python script of the same name with the same
//! arguments, files and exit codes; its help text is src/doc/labyrinth-launch.txt. Used by the dock, the start menu and Settings
//! (Yggdrasil's integration file labyrinth-app-rules.conf).

use crate::py::{self, Crash, Map, Value, R};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

pub const DOC: &str = include_str!("doc/labyrinth-launch.txt");

fn home() -> String {
    py::home()
}

pub fn data_home() -> String {
    py::env_or("XDG_DATA_HOME", &py::join_all(&[&home(), ".local", "share"]))
}

pub fn config_home() -> String {
    py::env_or("XDG_CONFIG_HOME", &py::join(&home(), ".config"))
}

/// Where .desktop files are looked for, each once.
pub fn desktop_dirs() -> Vec<String> {
    let mut dirs = vec![py::join(&data_home(), "applications")];
    for d in py::env_or("XDG_DATA_DIRS", "/usr/local/share:/usr/share").split(':') {
        if !d.is_empty() {
            dirs.push(py::join(d, "applications"));
        }
    }
    dirs.push(py::join_all(&[&data_home(), "flatpak", "exports", "share", "applications"]));
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    let mut out: Vec<String> = vec![];
    for d in dirs {
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// The .desktop file of an id (also inside sub-folders, where the name has a dash for a slash).
pub fn find_desktop(app_id: &str) -> Option<String> {
    let app_id = app_id.strip_suffix(".desktop").unwrap_or(app_id);
    for d in desktop_dirs() {
        let p = py::join(&d, &format!("{app_id}.desktop"));
        if py::isfile(&p) {
            return Some(p);
        }
        let p = py::join(&d, &format!("{}.desktop", app_id.replacen('-', "/", 1)));
        if app_id.contains('-') && py::isfile(&p) {
            return Some(p);
        }
    }
    None
}

/// The [Desktop Entry] keys of a .desktop file (first value wins; localised names skipped).
pub fn parse_desktop(path: &str) -> R<Vec<(String, String)>> {
    let text = py::read_text_lossy(path).map_err(|e| py::os_err(e, path))?;
    let mut out: Vec<(String, String)> = vec![];
    let mut inside = false;
    for line in py::lines(&text) {
        let line = py::strip(line);
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
        } else if inside && line.contains('=') && !line.starts_with('#') {
            let (k, _, v) = py::partition(line, "=");
            let k = py::strip(k);
            if !k.contains('[') && !out.iter().any(|(x, _)| x == k) {
                out.push((k.to_string(), py::strip(v).to_string()));
            }
        }
    }
    Ok(out)
}

pub fn get<'a>(entry: &'a [(String, String)], key: &str) -> &'a str {
    entry.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// The Exec line as an argument list, with the Desktop Entry spec's field codes filled in.
pub fn expand_exec(exec_line: &str, files: &[String], name: &str, icon: &str, path: &str) -> Vec<String> {
    let parts = py::shlex_split(exec_line).unwrap_or_else(|_| py::split(exec_line).into_iter().map(String::from).collect());
    let mut out = vec![];
    for p in parts {
        match p.as_str() {
            "%F" | "%U" => out.extend(files.iter().cloned()),
            "%f" | "%u" => out.extend(files.iter().take(1).cloned()),
            "%i" => {
                if !icon.is_empty() {
                    out.push("--icon".into());
                    out.push(icon.into());
                }
            }
            _ => {
                // %c and %k inside an argument; %% is a literal percent; the other codes are dropped
                let p = p.replace("%%", "\0").replace("%c", name).replace("%k", path);
                let mut q = String::with_capacity(p.len());
                let mut it = p.chars().peekable();
                while let Some(c) = it.next() {
                    if c == '%' {
                        if let Some(&n) = it.peek() {
                            if "fFuUdDnNvm".contains(n) {
                                it.next();
                                continue;
                            }
                        }
                    }
                    q.push(c);
                }
                let q = q.replace('\0', "%");
                if !q.is_empty() {
                    out.push(q);
                }
            }
        }
    }
    out
}

/// The slice an app runs in; labyrinth-appnet builds the same name.
pub fn slice_name(app_id: &str) -> String {
    crate::appnet::slice_name(app_id)
}

fn clean(app_id: &str) -> String {
    app_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect()
}

pub fn policy_path(app_id: &str) -> String {
    py::join_all(&[&config_home(), "labyrinth", "apps", &format!("{app_id}.json")])
}

/// The app's settings (an object), or none.
pub fn load_policy(app_id: &str) -> Map<String, Value> {
    match py::load_json(&policy_path(app_id)) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn int_of(v: &Value) -> R<i128> {
    match v {
        Value::Bool(b) => Ok(*b as i128),
        Value::Number(n) => Ok(n
            .as_i64()
            .map(|x| x as i128)
            .or_else(|| n.as_u64().map(|x| x as i128))
            .unwrap_or_else(|| n.as_f64().unwrap_or(0.0).trunc() as i128)),
        Value::String(s) => {
            crate::pycompat::int(s).ok_or_else(|| Crash(format!("ValueError: invalid literal for int() with base 10: {}", py::repr(s))))
        }
        other => Err(Crash(format!(
            "TypeError: int() argument must be a string, a bytes-like object or a real number, not {}",
            py::pyjson::py_repr(other)
        ))),
    }
}

/// Remember that the app was started: how many times, and when last.
pub fn count_launch(app_id: &str, now: Option<i64>) -> R<()> {
    let path = py::join_all(&[&data_home(), "labyrinth", "app-usage.json"]);
    if py::makedirs(&py::dirname(&path)).is_err() {
        return Ok(());
    }
    let mut usage = match py::load_json(&path) {
        Ok(v) => v,
        Err(_) => Value::Object(Map::new()),
    };
    let top = match &mut usage {
        Value::Object(m) => m,
        _ => return Err(Crash("AttributeError: app-usage.json is not an object".into())),
    };
    let mut e = match top.get(app_id) {
        Some(v) if crate::appnet::truthy(v) => match v {
            Value::Object(m) => m.clone(),
            _ => return Err(Crash("AttributeError: an entry of app-usage.json is not an object".into())),
        },
        _ => Map::new(),
    };
    let launches = int_of(e.get("launches").unwrap_or(&Value::from(0)))? + 1;
    e.insert("launches".into(), Value::from(launches as i64));
    let last = now.unwrap_or_else(|| py::now() as i64);
    e.insert("last".into(), Value::from(last));
    top.insert(app_id.to_string(), Value::Object(e));
    let tmp = format!("{path}.tmp");
    if py::write_file(&tmp, &py::pyjson::dumps(&usage)).is_err() || py::replace(&tmp, &path).is_err() {
        return Ok(());
    }
    Ok(())
}

/// The folders of the home folder that belong to the app, by the usual names (its id, name, program, window class).
pub fn app_dirs_of(entry: &[(String, String)], app_id: &str) -> Vec<String> {
    let mut names: Vec<String> = vec![app_id.into(), get(entry, "Name").into(), get(entry, "StartupWMClass").into()];
    if let Ok(parts) = py::shlex_split(get(entry, "Exec")) {
        if let Some(first) = parts.first() {
            names.push(py::basename(first).to_string());
        }
    }
    let mut set: Vec<String> = vec![];
    for n in names {
        if !n.is_empty() && !n.contains('/') && !n.starts_with('.') && !set.contains(&n) {
            set.push(n);
        }
    }
    for n in set.clone() {
        let l = n.to_lowercase();
        if !set.contains(&l) {
            set.push(l);
        }
    }
    set.sort();
    let mut out: Vec<String> = vec![];
    for b in [config_home(), py::join(&home(), ".cache"), data_home()] {
        for n in &set {
            let p = py::join(&b, n);
            if py::isdir(&p) && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    let flat = py::join_all(&[&home(), ".var", "app", app_id]);
    if py::isdir(&flat) {
        out.push(flat);
    }
    out
}

/// The values a policy's "folders" gives when iterated (a list's items, an object's keys, a text's characters).
fn folders_of(v: Option<&Value>) -> R<Vec<String>> {
    let items: Vec<Value> = match v {
        None => vec![],
        Some(Value::Array(a)) => a.clone(),
        Some(Value::Object(m)) => m.keys().map(|k| Value::String(k.clone())).collect(),
        Some(Value::String(s)) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        Some(other) => return Err(Crash(format!("TypeError: '{}' object is not iterable", py::pyjson::py_repr(other)))),
    };
    let mut out = vec![];
    for i in items {
        match i {
            Value::String(s) => out.push(s),
            Value::Bool(_) | Value::Number(_) => {
                // os.path.isdir() of a number looks at that file descriptor; realpath() of it then fails
                let fd = match &i {
                    Value::Bool(b) => *b as i32,
                    Value::Number(n) if n.is_i64() => n.as_i64().unwrap_or(-1) as i32,
                    _ => return Err(Crash("TypeError: stat: path should be string, bytes, os.PathLike or integer, not float".into())),
                };
                let mut st: libc::stat = unsafe { std::mem::zeroed() };
                if unsafe { libc::fstat(fd, &mut st) } == 0 && (st.st_mode & libc::S_IFMT) == libc::S_IFDIR {
                    return Err(Crash("TypeError: expected str, bytes or os.PathLike object, not int".into()));
                }
            }
            other => {
                return Err(Crash(format!(
                    "TypeError: stat: path should be string, bytes, os.PathLike or integer, not {}",
                    py::pyjson::py_repr(&other)
                )))
            }
        }
    }
    Ok(out)
}

/// bubblewrap's arguments for the app's policy: its own view of the home folder, and maybe of other programs.
pub fn bwrap_args(app_id: &str, entry: &[(String, String)], policy: &Map<String, Value>) -> R<Vec<String>> {
    let home = home();
    let run = py::env("XDG_RUNTIME_DIR").unwrap_or_else(|| format!("/run/user/{}", py::getuid()));
    let mut a: Vec<String> = ["bwrap", "--die-with-parent", "--new-session", "--unshare-ipc"].iter().map(|s| s.to_string()).collect();
    let mut add = |xs: &[&str]| a.extend(xs.iter().map(|s| s.to_string()));
    if policy.get("see_others") == Some(&Value::Bool(false)) {
        add(&["--unshare-pid", "--proc", "/proc"]);
    } else {
        add(&["--ro-bind", "/proc", "/proc"]);
    }
    for d in ["/usr", "/etc", "/opt", "/sys"] {
        if py::exists(d) {
            add(&["--ro-bind", d, d]);
        }
    }
    for link in ["/bin", "/sbin", "/lib", "/lib64"] {
        if py::islink(link) {
            let t = std::fs::read_link(link).map_err(|e| py::os_err(e, link))?.to_string_lossy().into_owned();
            add(&["--symlink", &t, link]);
        }
    }
    add(&["--dev", "/dev"]);
    for d in ["/dev/dri", "/dev/snd", "/dev/input", "/dev/shm"] {
        if py::exists(d) {
            add(&["--dev-bind", d, d]);
        }
    }
    let mut devs: Vec<String> = std::fs::read_dir("/dev")
        .map_err(|e| py::os_err(e, "/dev"))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    devs.sort();
    for n in devs {
        if n.starts_with("nvidia") || n == "video0" || n == "video1" {
            let p = format!("/dev/{n}");
            add(&["--dev-bind", &p, &p]);
        }
    }
    add(&["--tmpfs", "/tmp", "--tmpfs", &run]);
    for sock in ["wayland-0", "wayland-1", "pipewire-0", "pulse", "bus"] {
        let p = py::join(&run, sock);
        if py::exists(&p) {
            add(&["--bind", &p, &p]);
        }
    }
    add(&["--setenv", "HOME", &home]);
    let files_mode = policy.get("files").cloned().unwrap_or(Value::String("all".into()));
    if files_mode == Value::String("folders".into()) {
        add(&["--tmpfs", &home]);
        for d in app_dirs_of(entry, app_id) {
            add(&["--bind", &d, &d]);
        }
        for d in folders_of(policy.get("folders"))? {
            if py::isdir(&d) && py::realpath(&d).starts_with(&home) {
                add(&["--bind", &d, &d]);
            }
        }
        // the look, read-only
        let ch = config_home();
        let dh = data_home();
        for ro in [
            py::join_all(&[&ch, "yggdrasil", "appearance.conf"]),
            py::join(&ch, "kdeglobals"),
            py::join(&ch, "gtk-3.0"),
            py::join(&ch, "gtk-4.0"),
            py::join(&dh, "icons"),
            py::join(&dh, "fonts"),
            py::join(&ch, "labyrinth"),
        ] {
            if py::exists(&ro) {
                add(&["--ro-bind", &ro, &ro]);
            }
        }
    } else {
        add(&["--bind", &home, &home]);
    }
    Ok(a)
}

/// The app's network mode from the system's rules file (block, ethernet, wifi or ask), or none.
pub fn network_rule(app_id: &str, state: Option<&str>) -> Option<String> {
    let path = match state {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => py::env_get("LABYRINTH_APPNET_STATE", "/etc/labyrinth/app-network.json"),
    };
    let d = py::load_json(&path).ok()?;
    let rules = match d {
        Value::Object(m) => m.get("rules").cloned().unwrap_or(Value::Object(Map::new())),
        _ => return None,
    };
    let mine = match rules {
        Value::Object(r) => r.get(&py::getuid().to_string()).cloned().unwrap_or(Value::Object(Map::new())),
        _ => return None,
    };
    match mine {
        Value::Object(m) => match m.get(app_id) {
            Some(Value::String(s)) if ["block", "ethernet", "wifi", "ask"].contains(&s.as_str()) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// An app with a network limit starts in a slice that exists and is covered by the loaded rule.
fn ensure_network_rule(app_id: &str) -> R<()> {
    if network_rule(app_id, None).is_some() && py::which("systemctl").is_some() {
        py::run(&["systemctl", "--user", "start", &slice_name(app_id)])?;
        py::run(&["systemctl", "start", "labyrinth-appnet.service"])?;
    }
    Ok(())
}

/// (the arguments that start the app, its working folder), or None when there is no such app.
pub fn build_command(app_id: &str, files: &[String]) -> R<Option<(Vec<String>, Option<String>)>> {
    let path = match find_desktop(app_id) {
        Some(p) => p,
        None => return Ok(None),
    };
    let entry = parse_desktop(&path)?;
    let mut argv = expand_exec(get(&entry, "Exec"), files, get(&entry, "Name"), get(&entry, "Icon"), &path);
    if argv.is_empty() {
        return Ok(None);
    }
    if get(&entry, "Terminal").to_lowercase() == "true" {
        let term = if py::which("ghostty").is_some() { "ghostty" } else { "xterm" };
        argv = [term.to_string(), "-e".to_string()].into_iter().chain(argv).collect();
    }
    let policy = load_policy(app_id);
    if policy.get("sandbox").map(crate::appnet::truthy).unwrap_or(false) && py::which("bwrap").is_some() {
        let mut b = bwrap_args(app_id, &entry, &policy)?;
        b.push("--".into());
        b.extend(argv);
        argv = b;
    }
    let ms = (py::now() * 1000.0) as i64 % 100_000_000;
    let mut wrapped: Vec<String> = ["systemd-run", "--user", "--scope", "--quiet", "--collect"].iter().map(|s| s.to_string()).collect();
    wrapped.push(format!("--slice={}", slice_name(app_id)));
    wrapped.push(format!("--unit=lapp_{}_{ms}", clean(app_id)));
    wrapped.extend(argv);
    let cwd = get(&entry, "Path");
    Ok(Some((wrapped, if cwd.is_empty() { None } else { Some(cwd.to_string()) })))
}

fn run(argv: &[String]) -> R<i32> {
    let app_id = &argv[1];
    let files: Vec<String> = argv[2..].to_vec();
    let (mut cmd, cwd) = match build_command(app_id, &files)? {
        Some(b) => b,
        None => {
            eprintln!("labyrinth-launch: no app called {}", py::repr(app_id));
            return Ok(1);
        }
    };
    if py::which("systemd-run").is_none() {
        cmd = match cmd.iter().position(|c| c == "--") {
            Some(i) => cmd[i + 1..].to_vec(),
            None => cmd
                .into_iter()
                .filter(|c| {
                    !["systemd-run", "--user", "--scope", "--quiet", "--collect", "--slice=", "--unit="].iter().any(|p| c.starts_with(p))
                })
                .collect(),
        };
    }
    count_launch(app_id, None)?;
    ensure_network_rule(app_id)?;
    if cmd.is_empty() {
        return Err(Crash("IndexError: list index out of range".into()));
    }
    let mut c = Command::new(&cmd[0]);
    c.args(&cmd[1..]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(d) = cwd.filter(|d| py::isdir(d)) {
        c.current_dir(d);
    }
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    c.spawn().map_err(|e| Crash(py::oserror(&e, Some(&cmd[0]))))?;
    Ok(0)
}

/// The program: argv[0] is its name.
pub fn main(argv: &[String]) -> i32 {
    if argv.len() < 2 || argv[1] == "-h" || argv[1] == "--help" {
        println!("{DOC}");
        return if argv.len() >= 2 { 0 } else { 2 };
    }
    match run(argv) {
        Ok(rc) => rc,
        Err(e) => py::crashed("labyrinth-launch", &e),
    }
}

#[cfg(test)]
mod tests;
