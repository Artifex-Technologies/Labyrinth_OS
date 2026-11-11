//! labyrinth-apps: installs or removes Labyrinth's optional software (list, info, install ID... / --preset NAME, remove ID...,
//! --dry-run), from /usr/share/labyrinth/components.json: pacman packages, AUR packages through yay, and builds of the PKGBUILDs
//! in /usr/share/labyrinth/pkgbuilds. Agora (the app store) calls it, with AGORA_SUDO for its password box. The Python script
//! of the same name with the same commands, plan, output, environment (LABYRINTH_COMPONENTS, LABYRINTH_PKGBUILDS,
//! LABYRINTH_PACMAN_SYNC, AGORA_SUDO, LABYRINTH_SUDO) and exit codes; its help text is src/doc/labyrinth-apps.txt.
//!
//! Not 1:1: each "==>" line is written as its step starts (the script's output was block-buffered when it was not a terminal, so
//! in Agora or a log the lines came after the steps' own output); for a components.json that is not JSON the parser's own words
//! after "cannot read FILE: " differ (Python's json and serde_json say it differently); component ids that are not text are refused
//! (exit code 1).

use crate::py::{self, Crash, Map, Value, R};

pub const DOC: &str = include_str!("doc/labyrinth-apps.txt");

fn manifest() -> String {
    py::env_get("LABYRINTH_COMPONENTS", "/usr/share/labyrinth/components.json")
}

fn pkgbuilds() -> String {
    py::env_get("LABYRINTH_PKGBUILDS", "/usr/share/labyrinth/pkgbuilds")
}

pub type Comp = Map<String, Value>;

/// The components in the file's order, and the presets.
pub struct Manifest {
    pub comps: Vec<(String, Comp)>,
    pub presets: Vec<(String, Vec<String>)>,
}

impl Manifest {
    pub fn get(&self, id: &str) -> Option<&Comp> {
        self.comps.iter().find(|(i, _)| i == id).map(|(_, c)| c)
    }
}

/// Why components.json could not be read: what the script printed after "cannot read FILE: " (exit code 2), or a Crash.
pub enum LoadError {
    Said(String),
    Crash(Crash),
}

/// The items a value gives when iterated (a list's items, a text's characters, an object's keys).
fn iter_of(v: Option<&Value>) -> R<Vec<Value>> {
    Ok(match v {
        None => vec![],
        Some(Value::Array(a)) => a.clone(),
        Some(Value::String(s)) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        Some(Value::Object(m)) => m.keys().map(|k| Value::String(k.clone())).collect(),
        Some(other) => return Err(Crash(format!("TypeError: '{}' object is not iterable", py::pyjson::py_repr(other)))),
    })
}

fn texts(v: Option<&Value>) -> R<Vec<String>> {
    iter_of(v)?
        .into_iter()
        .map(|x| match x {
            Value::String(s) => Ok(s),
            other => Err(Crash(format!("TypeError: expected str instance, {} found", py::pyjson::py_repr(&other)))),
        })
        .collect()
}

pub fn load(path: &str) -> Result<Manifest, LoadError> {
    let data = match py::read_text(path) {
        Ok(t) => py::pyjson::loads(&t).map_err(LoadError::Said)?,
        Err(e) => return Err(LoadError::Said(e.message(path))),
    };
    let data = match data {
        Value::Object(m) => m,
        _ => return Err(LoadError::Crash(Crash("TypeError: components.json is not an object".into()))),
    };
    let list = match data.get("components") {
        Some(Value::Array(a)) => a.clone(),
        Some(_) => return Err(LoadError::Crash(Crash("TypeError: \"components\" is not a list".into()))),
        None => return Err(LoadError::Said("'components'".into())),
    };
    let mut comps: Vec<(String, Comp)> = vec![];
    for c in list {
        let c = match c {
            Value::Object(m) => m,
            _ => return Err(LoadError::Crash(Crash("TypeError: a component is not an object".into()))),
        };
        let id = match c.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(_) => return Err(LoadError::Crash(Crash("TypeError: a component's id is not text".into()))),
            None => return Err(LoadError::Said("'id'".into())),
        };
        match comps.iter_mut().find(|(i, _)| *i == id) {
            Some(e) => e.1 = c,
            None => comps.push((id, c)),
        }
    }
    let mut presets = vec![];
    match data.get("presets") {
        None => {}
        Some(Value::Object(p)) => {
            for (name, ids) in p {
                let ids = iter_of(Some(ids)).map_err(LoadError::Crash)?;
                let mut out = vec![];
                for i in ids {
                    match &i {
                        Value::String(s) if comps.iter().any(|(x, _)| x == s) => out.push(s.clone()),
                        _ => return Err(LoadError::Said(format!("preset {} names unknown component {}", py::repr(name), py::pyjson::py_repr(&i)))),
                    }
                }
                presets.push((name.clone(), out));
            }
        }
        Some(_) => return Err(LoadError::Crash(Crash("AttributeError: \"presets\" is not an object".into()))),
    }
    Ok(Manifest { comps, presets })
}

/// The names pacman knows a component by (a component with its own build installs that name).
pub fn all_packages(c: &Comp) -> R<Vec<String>> {
    let mut out = texts(c.get("packages"))?;
    out.extend(texts(c.get("aur"))?);
    if let Some(pb) = c.get("pkgbuild").filter(|v| crate::appnet::truthy(v)) {
        out.push(py::pyjson::py_str(pb));
    }
    Ok(out)
}

pub fn state(c: &Comp, installed: &[String]) -> R<&'static str> {
    let pk = all_packages(c)?;
    let have = pk.iter().filter(|p| installed.contains(p)).count();
    Ok(if have == pk.len() {
        "installed"
    } else if have > 0 {
        "partial"
    } else {
        "not installed"
    })
}

/// The names pacman reports installed, or None when pacman itself is missing.
pub fn pacman_installed() -> R<Option<Vec<String>>> {
    match py::run_opts(&["pacman", "-Qq"], py::Run { inherit_stdin: true, ..Default::default() }) {
        Ok(o) => Ok(Some(if o.code == 0 { py::split(&o.stdout).into_iter().map(String::from).collect() } else { vec![] })),
        Err(py::RunError::Os(m)) if m.starts_with("[Errno 2]") => Ok(None),
        Err(e) => Err(Crash(e.message().to_string())),
    }
}

/// AGORA_SUDO (or the older LABYRINTH_SUDO): a program that gains administrator rights through a password box, no terminal.
pub fn gui_sudo() -> Option<String> {
    match py::env("AGORA_SUDO") {
        Some(s) if !s.is_empty() => Some(s),
        _ => py::env("LABYRINTH_SUDO").filter(|s| !s.is_empty()),
    }
}

/// makepkg from Agora: no questions, and Agora's password box instead of sudo (a makepkg.conf in the build's folder).
pub fn makepkg_gui_args(tmp: &str) -> R<Vec<String>> {
    let s = match gui_sudo() {
        Some(s) => s,
        None => return Ok(vec![]),
    };
    let conf = py::join(tmp, "makepkg.conf");
    py::write_file(&conf, &format!("source /etc/makepkg.conf\nPACMAN_AUTH=({})\n", py::shlex_quote(&s)))?;
    Ok(vec!["--noconfirm".into(), "--config".into(), conf])
}

fn sudo(cmd: Vec<String>) -> Vec<String> {
    if py::geteuid() == 0 {
        cmd
    } else {
        let mut v = vec![gui_sudo().unwrap_or_else(|| "sudo".into())];
        v.extend(cmd);
        v
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Whether a repository pacman knows has this package.
pub fn in_repos(name: &str) -> bool {
    py::run_opts(&["pacman", "-Si", name], py::Run { inherit_stdin: true, ..Default::default() }).map(|o| o.code == 0).unwrap_or(false)
}

/// A system that never fetched its package lists (a fresh install: the image carries none).
pub fn never_synced(sync_dir: Option<&str>) -> bool {
    let d = match sync_dir {
        Some(d) if !d.is_empty() => d.to_string(),
        _ => py::env_get("LABYRINTH_PACMAN_SYNC", "/var/lib/pacman/sync"),
    };
    match std::fs::read_dir(&d) {
        Ok(rd) => !rd.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".db")),
        Err(_) => true,
    }
}

/// One step: what it does, the command, and the PKGBUILD folder it is built in (if a build).
pub type Step = (String, Vec<String>, Option<String>);

fn name_of(c: &Comp) -> R<String> {
    c.get("name").map(py::pyjson::py_str).ok_or_else(|| Crash("KeyError: 'name'".into()))
}

pub fn plan_install(chosen: &[String], m: &Manifest, installed: &[String], in_repos: &dyn Fn(&str) -> bool, fresh: bool) -> R<Vec<Step>> {
    let comp = |i: &str| m.get(i).ok_or_else(|| Crash(format!("KeyError: {}", py::repr(i))));
    let mut steps: Vec<Step> = vec![];
    let mut repo: Vec<String> = vec![];
    for i in chosen {
        for p in texts(comp(i)?.get("packages"))? {
            if !installed.contains(&p) && !repo.contains(&p) {
                repo.push(p);
            }
        }
    }
    let mut built_here: Vec<String> = vec![];
    for i in chosen {
        if let Some(pb) = comp(i)?.get("pkgbuild").filter(|v| crate::appnet::truthy(v)) {
            let pb = py::pyjson::py_str(pb);
            if !repo.contains(&pb) {
                if fresh || in_repos(&pb) {
                    repo.push(pb);
                } else if !installed.contains(&pb) {
                    built_here.push(pb);
                }
            }
        }
    }
    if !repo.is_empty() {
        let quiet = if gui_sudo().is_some() { s(&["--noconfirm"]) } else { vec![] };
        let (desc, verb) = if fresh {
            (format!("fetch the package lists, update the system and install {} package(s) with pacman", repo.len()), "-Syu")
        } else {
            (format!("install {} package(s) with pacman", repo.len()), "-S")
        };
        let mut cmd = s(&["pacman", verb, "--needed"]);
        cmd.extend(quiet);
        cmd.extend(repo.iter().cloned());
        steps.push((desc, sudo(cmd), None));
    }
    let mut aur: Vec<String> = vec![];
    for i in chosen {
        for p in texts(comp(i)?.get("aur"))? {
            if !installed.contains(&p) && !aur.contains(&p) {
                aur.push(p);
            }
        }
    }
    if !aur.is_empty() {
        let mut cmd = s(&["yay", "-S", "--needed"]);
        if let Some(g) = gui_sudo() {
            cmd.extend(s(&["--noconfirm", "--sudo", &g]));
        }
        cmd.extend(aur.iter().cloned());
        steps.push((format!("build and install {} package(s) from the AUR ({})", aur.len(), aur.join(", ")), cmd, None));
    }
    for pb in built_here {
        steps.push((format!("build and install {pb} from its PKGBUILD"), s(&["makepkg", "-si"]), Some(pb)));
    }
    for i in chosen {
        let c = comp(i)?;
        for cmd in iter_of(c.get("after_install"))? {
            if let Value::Array(a) = &cmd {
                if !a.is_empty() && a.iter().all(|x| x.is_string()) {
                    let argv: Vec<String> = a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect();
                    steps.push((format!("set up {}: {}", name_of(c)?, argv.join(" ")), sudo(argv), None));
                }
            }
        }
    }
    let user = match py::env("SUDO_USER") {
        Some(u) if !u.is_empty() => u,
        _ => py::env_or("USER", ""),
    };
    for i in chosen {
        let groups = texts(comp(i)?.get("add_groups"))?;
        if !groups.is_empty() && !user.is_empty() {
            let desc = format!("let {user} use the device ({} group; log out and in again)", groups.join(", "));
            steps.push((desc, sudo(s(&["usermod", "-aG", &groups.join(","), &user])), None));
        }
    }
    for i in chosen {
        let c = comp(i)?;
        let units = texts(c.get("enable_global"))?;
        if !units.is_empty() {
            let mut cmd = s(&["systemctl", "--global", "enable"]);
            cmd.extend(units.iter().cloned());
            steps.push((format!("enable {} for every user", units.join(", ")), sudo(cmd), None));
        }
        let units = texts(c.get("enable_system"))?;
        if !units.is_empty() {
            let mut cmd = s(&["systemctl", "enable", "--now"]);
            cmd.extend(units.iter().cloned());
            steps.push((format!("start {}", units.join(", ")), sudo(cmd), None));
        }
    }
    Ok(steps)
}

pub fn plan_remove(chosen: &[String], m: &Manifest, installed: &[String]) -> R<Vec<Step>> {
    let comp = |i: &str| m.get(i).ok_or_else(|| Crash(format!("KeyError: {}", py::repr(i))));
    let mut pk: Vec<String> = vec![];
    for i in chosen {
        for p in all_packages(comp(i)?)? {
            if installed.contains(&p) && !pk.contains(&p) {
                pk.push(p);
            }
        }
    }
    let mut steps: Vec<Step> = vec![];
    for i in chosen {
        let c = comp(i)?;
        let any = all_packages(c)?.iter().any(|p| installed.contains(p));
        let units = texts(c.get("enable_global"))?;
        if !units.is_empty() && any {
            let mut cmd = s(&["systemctl", "--global", "disable"]);
            cmd.extend(units.iter().cloned());
            steps.push((format!("disable {}", units.join(", ")), sudo(cmd), None));
        }
        let units = texts(c.get("enable_system"))?;
        if !units.is_empty() && any {
            let mut cmd = s(&["systemctl", "disable", "--now"]);
            cmd.extend(units.iter().cloned());
            steps.push((format!("stop {}", units.join(", ")), sudo(cmd), None));
        }
    }
    if !pk.is_empty() {
        let mut cmd = s(&["pacman", "-Rs"]);
        cmd.extend(pk.iter().cloned());
        steps.push((format!("remove {} package(s) and their unneeded dependencies", pk.len()), sudo(cmd), None));
    }
    Ok(steps)
}

/// shutil.copytree(src, dst) (links followed, as copytree does by default): the folders with their modes, the files with copy2.
fn copytree(src: &str, dst: &str) -> R<()> {
    py::makedirs(dst)?;
    let mut names: Vec<String> = std::fs::read_dir(src).map_err(|e| py::os_err(e, src))?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for n in names {
        let (s, d) = (py::join(src, &n), py::join(dst, &n));
        if py::isdir(&s) {
            copytree(&s, &d)?;
        } else {
            py::copy2(&s, &d)?;
        }
    }
    let mode = std::fs::metadata(src).map_err(|e| py::os_err(e, src))?.permissions();
    use std::os::unix::fs::PermissionsExt;
    py::chmod(dst, mode.mode() & 0o7777)
}

/// tempfile.TemporaryDirectory(prefix=...): a private folder under TMPDIR (or /tmp), removed afterwards.
fn temp_dir(prefix: &str) -> R<String> {
    let base = py::env("TMPDIR").filter(|d| !d.is_empty() && py::isdir(d)).unwrap_or_else(|| "/tmp".into());
    for _ in 0..100 {
        let p = py::join(&base, &format!("{prefix}{}", &py::random_hex12()[..8]));
        use std::os::unix::fs::DirBuilderExt;
        match std::fs::DirBuilder::new().mode(0o700).create(&p) {
            Ok(()) => return Ok(py::abspath(&p)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(py::os_err(e, &p)),
        }
    }
    Err(Crash("FileExistsError: no usable temporary folder name".into()))
}

pub fn run_steps(steps: &[Step], dry_run: bool) -> R<i32> {
    for (desc, argv, pkgbuild) in steps {
        let shown: Vec<String> = argv.iter().map(|a| py::shlex_quote(a)).collect();
        let tail = pkgbuild.as_ref().map(|p| format!("   (in a copy of {p})")).unwrap_or_default();
        println!("==> {desc}\n    {}{tail}", shown.join(" "));
        if dry_run {
            continue;
        }
        let rc = if let Some(pb) = pkgbuild {
            let src = py::join(&pkgbuilds(), pb);
            if !py::isfile(py::join(&src, "PKGBUILD")) {
                eprintln!("error: no PKGBUILD at {src}");
                return Ok(1);
            }
            let tmp = temp_dir("labyrinth-build-")?;
            let result = (|| -> R<i32> {
                let work = py::join(&tmp, pb);
                copytree(&src, &work)?;
                let mut full = argv.clone();
                full.extend(makepkg_gui_args(&tmp)?);
                let st = std::process::Command::new(&full[0]).args(&full[1..]).current_dir(&work).status().map_err(|e| Crash(py::oserror(&e, Some(&full[0]))))?;
                Ok(st.code().unwrap_or_else(|| {
                    use std::os::unix::process::ExitStatusExt;
                    -st.signal().unwrap_or(1)
                }))
            })();
            py::rmtree_quiet(&tmp);
            result?
        } else {
            py::call(argv)?
        };
        if rc != 0 {
            eprintln!("error: step failed (exit {rc}); stopping");
            return Ok(rc);
        }
    }
    Ok(0)
}

fn run(argv: &[String], installed: Option<Vec<String>>) -> R<i32> {
    let dry = argv.iter().any(|a| a == "--dry-run");
    let args: Vec<String> = argv.iter().filter(|a| *a != "--dry-run").cloned().collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        println!("{DOC}");
        return Ok(0);
    }
    let (cmd, rest) = (args[0].as_str(), &args[1..]);
    let path = manifest();
    let m = match load(&path) {
        Ok(m) => m,
        Err(LoadError::Said(e)) => {
            eprintln!("error: cannot read {path}: {e}");
            return Ok(2);
        }
        Err(LoadError::Crash(c)) => return Err(c),
    };
    let installed = match installed {
        Some(i) => i,
        None => match pacman_installed()? {
            Some(i) => i,
            None => {
                if (cmd == "install" || cmd == "remove") && !dry {
                    eprintln!("error: pacman not found; labyrinth-apps works on Arch-based systems only (use --dry-run to see what it would do)");
                    return Ok(2);
                }
                eprintln!("note: pacman not found; showing everything as not installed");
                vec![]
            }
        },
    };
    if cmd == "list" {
        let w = m.comps.iter().map(|(i, _)| i.chars().count()).max().ok_or_else(|| Crash("ValueError: max() iterable argument is empty".into()))?;
        for (i, c) in &m.comps {
            let flag = if c.get("proprietary").map(crate::appnet::truthy).unwrap_or(false) { " [proprietary]" } else { "" };
            println!("{i:<w$}  {:<13} {}{flag}", state(c, &installed)?, name_of(c)?);
        }
        let ps: Vec<String> = m.presets.iter().map(|(n, v)| format!("{n} ({})", if v.is_empty() { "nothing extra".to_string() } else { v.join(", ") })).collect();
        println!("\npresets: {}", ps.join(", "));
        return Ok(0);
    }
    if cmd == "info" && rest.len() == 1 {
        let c = match m.get(&rest[0]) {
            Some(c) => c,
            None => {
                eprintln!("error: unknown component {}", py::repr(&rest[0]));
                return Ok(2);
            }
        };
        let desc = c.get("description").map(py::pyjson::py_str).ok_or_else(|| Crash("KeyError: 'description'".into()))?;
        println!("{}  ({})\n{desc}", name_of(c)?, state(c, &installed)?);
        let pk = all_packages(c)?;
        let built = match c.get("pkgbuild").filter(|v| crate::appnet::truthy(v)) {
            Some(pb) => format!("  [built from PKGBUILD {}]", py::pyjson::py_str(pb)),
            None => String::new(),
        };
        println!("packages: {}{built}", if pk.is_empty() { "-".to_string() } else { pk.join(", ") });
        if c.get("unverified_packages").map(crate::appnet::truthy).unwrap_or(false) {
            println!("not found in Arch's repos (need CachyOS repo or AUR): {}", texts(c.get("unverified_packages"))?.join(", "));
        }
        if let Some(n) = c.get("notes").filter(|v| crate::appnet::truthy(v)) {
            let n = n.as_str().ok_or_else(|| Crash("TypeError: can only concatenate str (not another type) to str".into()))?;
            println!("after install: {n}");
        }
        return Ok(0);
    }
    if cmd == "install" || cmd == "remove" {
        let mut ids: Vec<String> = rest.to_vec();
        if let Some(k) = ids.iter().position(|a| a == "--preset") {
            let p = ids.get(k + 1).and_then(|n| m.presets.iter().find(|(x, _)| x == n));
            let p = match p {
                Some((_, v)) => v.clone(),
                None => {
                    let names: Vec<&str> = m.presets.iter().map(|(n, _)| n.as_str()).collect();
                    eprintln!("error: --preset needs one of: {}", names.join(", "));
                    return Ok(2);
                }
            };
            let mut n = ids[..k].to_vec();
            n.extend(p);
            n.extend(ids[k + 2..].iter().cloned());
            ids = n;
        }
        if ids.is_empty() {
            eprintln!("error: nothing selected");
            return Ok(2);
        }
        let mut chosen: Vec<String> = vec![];
        for i in ids {
            if m.get(&i).is_none() {
                eprintln!("error: unknown component {}", py::repr(&i));
                return Ok(2);
            }
            if !chosen.contains(&i) {
                chosen.push(i);
            }
        }
        let steps = if cmd == "install" { plan_install(&chosen, &m, &installed, &in_repos, never_synced(None))? } else { plan_remove(&chosen, &m, &installed)? };
        if steps.is_empty() {
            println!("nothing to do");
            return Ok(0);
        }
        let rc = run_steps(&steps, dry)?;
        if rc == 0 && cmd == "install" && !dry {
            for i in &chosen {
                let c = m.get(i).unwrap();
                if let Some(n) = c.get("notes").filter(|v| crate::appnet::truthy(v)) {
                    println!("\n{}: {}", name_of(c)?, py::pyjson::py_str(n));
                }
            }
        }
        return Ok(rc);
    }
    println!("{DOC}");
    Ok(2)
}

/// The program: argv without its own name; `installed` for the tests (else pacman is asked).
pub fn main_with(argv: &[String], installed: Option<Vec<String>>) -> i32 {
    match run(argv, installed) {
        Ok(rc) => rc,
        Err(e) => py::crashed("labyrinth-apps", &e),
    }
}

/// The program: argv[0] is its name.
pub fn main(argv: &[String]) -> i32 {
    main_with(&argv[1..], None)
}

#[cfg(test)]
mod tests;
