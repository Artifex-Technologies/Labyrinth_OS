//! Building the packages: the publish's step 3, and the ISO's local repository (`labyrinth-publish-repo build`, which
//! replaced scripts/build-local-packages.sh on 2026-11-21). Every recipe is built on its own, a few at a time, each under
//! its own memory cap.
//!
//! - A recipe is built again only when something it is built from changed since its last good build. Its key is the files
//!   makepkg says it makes (version and build number, docs/VERSIONING.md), the recipe as committed, the commit each of its
//!   git sources points at now, scripts/makepkg.conf, and the uncommitted changes of the checkout it is in. The keys of the
//!   last good builds are kept in `repo/builds.json`, beside the packages.
//! - A build that fails is named with what makepkg was doing, the lines of its log that say why, and where the whole log is
//!   (`repo/logs/NAME.log`); the other builds go on. The script stopped at the first failure: from 2026-11-18 one failing
//!   package a day kept every other change from installed systems for four days, and the journal said only "exit status 4".
//! - makepkg never installs anything (no `-s`): a missing dependency is a failure that names it.

use crate::cmd::{Cmd, R};
use crate::publish::{copy2, say, KEY};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Mutex};
use std::time::Instant;

/// The store's recipes: copied onto every system, built by the publish only (they clone from GitHub).
pub const STORE: &str = "profile/airootfs/usr/share/labyrinth/pkgbuilds";
/// Left out of the ISO's local build unless named (LABYRINTH_SKIP replaces the list): Daedalus CAD and Icarus are only in
/// the Marketplace, Argus is LykanOS's (its firmware build fetches Flipper's ARM toolchain), yara-x is for Limbo's analysis
/// guest image. The publish builds them all.
pub const NOT_ON_THE_ISO: &str = "daedalus-cad-git icarus-git argus-git yara-x";
/// The most lines of a log shown for a failure; the log has the rest.
const SHOWN: usize = 12;

pub struct Recipe {
    pub name: String,
    pub dir: PathBuf,
    /// the git checkout the recipe is in (its committed state and its uncommitted changes are part of the key)
    pub checkout: PathBuf,
    /// for the store's recipes, the folder makepkg builds in and puts the packages in: always the same one, since a build
    /// path ends up inside the programs, and a new random one each time would make every build look changed. None: the
    /// recipe's own folder, as a `makepkg` there would.
    pub out: Option<PathBuf>,
}

fn folders_with_a_recipe(dir: &Path) -> R<Vec<String>> {
    let rd = fs::read_dir(dir).map_err(|e| format!("error: cannot read {}: {e}", dir.display()))?;
    let mut names = Vec::new();
    for e in rd {
        let e = e.map_err(|e| format!("error: cannot read {}: {e}", dir.display()))?;
        if e.path().join("PKGBUILD").is_file() {
            let n = e.file_name();
            names.push(n.into_string().map_err(|n| format!("error: {}: a recipe folder's name is not UTF-8", n.to_string_lossy()))?);
        }
    }
    names.sort();
    Ok(names)
}

/// The recipes: Labyrinth's (packages/pkgbuilds), Yggdrasil's (its packages/pkgbuilds) and, given `store`, the store's
/// own that are not also among the first two, each built in `store`/NAME.
pub fn recipes(labyrinth: &Path, yggdrasil: &Path, store: Option<&Path>) -> R<Vec<Recipe>> {
    let mut all: Vec<Recipe> = Vec::new();
    for checkout in [labyrinth, yggdrasil] {
        let dir = checkout.join("packages").join("pkgbuilds");
        for name in folders_with_a_recipe(&dir)? {
            if let Some(other) = all.iter().find(|r| r.name == name) {
                return Err(format!("error: two recipes are named {name}: {} and {}", other.dir.display(), dir.join(&name).display()));
            }
            all.push(Recipe { dir: dir.join(&name), name, checkout: checkout.to_path_buf(), out: None });
        }
    }
    if let Some(store) = store {
        let dir = labyrinth.join(STORE);
        for name in folders_with_a_recipe(&dir)? {
            if all.iter().any(|r| r.name == name) {
                continue;
            }
            all.push(Recipe { dir: dir.join(&name), out: Some(store.join(&name)), name, checkout: labyrinth.to_path_buf() });
        }
    }
    Ok(all)
}

pub struct Settings {
    pub makepkg_conf: PathBuf,
    /// where the packages go (repo/x86_64); builds.json and logs/ are in the folder above it
    pub repo_dir: PathBuf,
    /// build every recipe, changed or not
    pub force: bool,
    /// builds at the same time
    pub at_once: usize,
    /// compile jobs of each build (CARGO_BUILD_JOBS, MAKEFLAGS)
    pub jobs: usize,
    /// each build's memory cap, through `systemd-run --user --scope`; None where the user's systemd cannot make one
    pub memory: Option<String>,
    /// the makepkg to run (the tests' is a stand-in)
    pub makepkg: OsString,
}

fn env_number(name: &str) -> Option<usize> {
    std::env::var(name).ok().and_then(|v| v.trim().parse().ok()).filter(|n| *n > 0)
}

impl Settings {
    /// Three builds at once, each with at most 4 compile jobs and 8 GB (LABYRINTH_BUILDS_AT_ONCE, LABYRINTH_BUILD_JOBS and
    /// LABYRINTH_BUILD_MEMORY change them; an empty LABYRINTH_BUILD_MEMORY is no cap).
    pub fn new(makepkg_conf: PathBuf, repo_dir: PathBuf, force: bool) -> Settings {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let at_once = env_number("LABYRINTH_BUILDS_AT_ONCE").unwrap_or(3);
        let jobs = env_number("LABYRINTH_BUILD_JOBS").unwrap_or((threads / at_once).clamp(1, 4));
        let size = std::env::var("LABYRINTH_BUILD_MEMORY").unwrap_or_else(|_| "8G".into());
        let memory = (!size.trim().is_empty() && can_cap(size.trim())).then(|| size.trim().to_string());
        Settings { makepkg_conf, repo_dir, force, at_once, jobs, memory, makepkg: "makepkg".into() }
    }

    fn state_file(&self) -> PathBuf {
        self.repo_dir.parent().unwrap_or(Path::new(".")).join("builds.json")
    }

    fn logs(&self) -> PathBuf {
        self.repo_dir.parent().unwrap_or(Path::new(".")).join("logs")
    }

    fn makepkg(&self, r: &Recipe) -> Cmd {
        let c = Cmd::new([&self.makepkg]).arg("--config").arg(&self.makepkg_conf).cwd(&r.dir);
        match &r.out {
            Some(o) => c.env("BUILDDIR", o).env("SRCDEST", o).env("PKGDEST", o),
            None => c.env("PKGDEST", &r.dir),
        }
    }
}

/// Whether the user's systemd makes a scope with a memory cap here (it does under the timer and in a desktop session; not in
/// a container without one).
fn can_cap(size: &str) -> bool {
    Command::new("systemd-run")
        .args(["--user", "--scope", "-q", "-p", &format!("MemoryMax={size}"), "-p", "MemorySwapMax=0", "--", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn hex(d: impl AsRef<[u8]>) -> String {
    d.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

fn add(h: &mut Sha256, what: &str, value: &str) {
    h.update(what.as_bytes());
    h.update(b" ");
    h.update(value.as_bytes());
    h.update(b"\n");
}

/// The checkout's uncommitted changes, as one digest: the diff of the tracked files and every untracked file that git does
/// not ignore. A fresh copy from GitHub has none.
fn uncommitted(checkout: &Path) -> R<String> {
    let git = || Cmd::new([OsStr::new("git"), OsStr::new("-C"), checkout.as_os_str()]);
    let mut h = Sha256::new();
    h.update(git().args(["diff", "HEAD", "--binary"]).bytes(None)?);
    let others = git().args(["ls-files", "--others", "--exclude-standard", "-z"]).bytes(None)?;
    for name in others.split(|&b| b == 0).filter(|n| !n.is_empty()) {
        h.update(name);
        h.update([0]);
        if let Ok(data) = fs::read(checkout.join(OsStr::from_bytes(name))) {
            h.update(Sha256::digest(&data));
        }
    }
    Ok(hex(h.finalize()))
}

#[derive(Debug, PartialEq)]
pub struct GitSource {
    pub url: String,
    /// branch, tag or commit, and its value (makepkg's #branch=main)
    pub pick: Option<(String, String)>,
}

/// The git sources a recipe's .SRCINFO lists (source = name::git+URL#branch=b?signed).
pub fn git_sources(srcinfo: &str) -> Vec<GitSource> {
    let mut found = Vec::new();
    for line in srcinfo.lines() {
        let Some((k, v)) = line.trim().split_once(" = ") else { continue };
        if k != "source" && !k.starts_with("source_") {
            continue;
        }
        let v = v.split_once("::").map_or(v, |(_, u)| u);
        let Some(u) = v.strip_prefix("git+") else { continue };
        let (u, pick) = match u.split_once('#') {
            Some((u, f)) => (u, f.split_once('=').map(|(a, b)| (a.to_string(), b.to_string()))),
            None => (u, None),
        };
        let u = u.split_once('?').map_or(u, |(a, _)| a);
        found.push(GitSource { url: u.to_string(), pick });
    }
    found
}

/// The commit a git source points at now: a local checkout's branch, or what GitHub says. Unknown (a network failure, a
/// branch that is gone) gives a value that never matches, so the recipe is built and makepkg says what is wrong.
fn commit_of(s: &GitSource) -> String {
    let (kind, value) = s.pick.clone().unwrap_or_default();
    if kind == "commit" {
        return value;
    }
    let first_word = |t: Option<String>| t.and_then(|t| t.split_whitespace().next().map(str::to_string));
    let found = if let Some(path) = s.url.strip_prefix("file://") {
        let rev = if kind == "branch" || kind == "tag" { value } else { "HEAD".into() };
        Cmd::new(["git", "-C", path, "rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")]).text_or_none().ok().flatten()
    } else {
        let name = match kind.as_str() {
            "branch" => format!("refs/heads/{value}"),
            "tag" => format!("refs/tags/{value}"),
            _ => "HEAD".into(),
        };
        Cmd::new(["git", "ls-remote", "--", &s.url, &name]).env("GIT_TERMINAL_PROMPT", "0").text_or_none().ok().flatten()
    };
    first_word(found).unwrap_or_else(|| format!("unknown at {:?}", std::time::SystemTime::now()))
}

/// The recipe's key, and the files makepkg says it makes.
fn key(s: &Settings, r: &Recipe, conf: &str, dirty: &str) -> R<(String, Vec<String>)> {
    let listed: Vec<String> = s
        .makepkg(r)
        .arg("--packagelist")
        .text()?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| Path::new(l.trim()).file_name().map_or_else(|| l.trim().to_string(), |n| n.to_string_lossy().into_owned()))
        .collect();
    let srcinfo = s.makepkg(r).arg("--printsrcinfo").text()?;
    let mut h = Sha256::new();
    add(&mut h, "makepkg.conf", conf);
    for f in &listed {
        add(&mut h, "makes", f);
    }
    let inside = r.dir.strip_prefix(&r.checkout).unwrap_or(&r.dir);
    let tree = Cmd::new([OsStr::new("git"), OsStr::new("-C"), r.checkout.as_os_str(), OsStr::new("rev-parse")])
        .arg(format!("HEAD:{}", inside.display()))
        .text_or_none()?;
    add(&mut h, "recipe", tree.as_deref().map_or("not committed", str::trim));
    add(&mut h, "uncommitted", dirty);
    for g in git_sources(&srcinfo) {
        add(&mut h, "source", &format!("{} {:?} {}", g.url, g.pick, commit_of(&g)));
    }
    Ok((hex(h.finalize()), listed))
}

#[derive(Clone, Debug, PartialEq)]
struct Good {
    key: String,
    files: Vec<String>,
    secs: u64,
}

fn load(path: &Path) -> R<HashMap<String, Good>> {
    let Ok(text) = fs::read_to_string(path) else { return Ok(HashMap::new()) };
    let bad = || format!("error: {} is not a list of builds (remove it, and every recipe is built once)", path.display());
    let Value::Object(all) = serde_json::from_str(&text).map_err(|_| bad())? else { return Err(bad()) };
    let mut out = HashMap::new();
    for (name, v) in all {
        let key = v.get("key").and_then(Value::as_str).ok_or_else(bad)?.to_string();
        let files = v.get("files").and_then(Value::as_array).ok_or_else(bad)?;
        let files = files.iter().map(|f| f.as_str().map(str::to_string).ok_or_else(bad)).collect::<R<Vec<_>>>()?;
        let secs = v.get("secs").and_then(Value::as_u64).unwrap_or(0);
        out.insert(name, Good { key, files, secs });
    }
    Ok(out)
}

fn save(path: &Path, all: &HashMap<String, Good>) -> R<()> {
    let mut names: Vec<&String> = all.keys().collect();
    names.sort();
    let mut o = Map::new();
    for n in names {
        let g = &all[n];
        let mut e = Map::new();
        e.insert("key".into(), Value::String(g.key.clone()));
        e.insert("files".into(), Value::Array(g.files.iter().cloned().map(Value::String).collect()));
        e.insert("secs".into(), Value::from(g.secs));
        o.insert(n.clone(), Value::Object(e));
    }
    let text = serde_json::to_string_pretty(&Value::Object(o)).unwrap_or_default() + "\n";
    let tmp = path.with_extension("json.new");
    fs::write(&tmp, text).map_err(|e| format!("error: cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("error: cannot write {}: {e}", path.display()))
}

#[derive(Debug)]
pub struct Failure {
    pub name: String,
    /// what makepkg was doing ("A failure occurred in check().")
    pub why: String,
    /// the lines of the log that say why
    pub lines: Vec<String>,
    pub log: PathBuf,
}

#[derive(Default, Debug)]
pub struct Report {
    pub built: Vec<String>,
    pub unchanged: Vec<String>,
    pub failed: Vec<Failure>,
}

fn without_colour(l: &str) -> String {
    let mut out = String::with_capacity(l.len());
    let mut chars = l.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A line that says why a build failed: the compiler's and linker's errors, a test's failure and its assertion, a missing
/// file or dependency, a download or git failure, a kill.
fn telling(t: &str) -> bool {
    const STARTS: [&str; 7] = ["error", "fatal:", "left:", "right:", "assertion", "curl:", "Killed"];
    const HAS: [&str; 12] = [
        "panicked at",
        " ... FAILED",
        "undefined symbol",
        "undefined reference",
        "No such file or directory",
        "Permission denied",
        "No space left",
        "SIGKILL",
        "command not found",
        "out of memory",
        "One or more files did not pass the validity check",
        "Failure while downloading",
    ];
    STARTS.iter().any(|s| t.starts_with(s)) || HAS.iter().any(|s| t.contains(s))
}

/// What makepkg said it was doing when it stopped, and the lines of the log that say why (the first ones, at most SHOWN, and
/// a test run's verdict).
pub fn cause(log: &str) -> (String, Vec<String>) {
    let lines: Vec<String> = log.lines().map(|l| without_colour(l).trim_end().to_string()).collect();
    let why = lines
        .iter()
        .rev()
        .find_map(|l| l.trim_start().strip_prefix("==> ERROR: ").map(str::to_string))
        .unwrap_or_else(|| "makepkg stopped without saying why".into());
    let mut picked: Vec<String> = Vec::new();
    let mut verdict: Option<String> = None;
    let (mut in_deps, mut after_panic, mut after_error) = (false, false, false);
    for l in &lines {
        let t = l.trim_start();
        let take = if t.starts_with("==> Missing dependencies") || t.starts_with("==> ERROR: Could not resolve") {
            in_deps = true;
            false
        } else if in_deps && t.starts_with("-> ") {
            true
        } else {
            in_deps = false;
            // a panic's message is the line after "panicked at", a compiler error's place the "-->" line after it
            let take = !t.starts_with("==> ") && (telling(t) || after_panic || (after_error && t.starts_with("--> ")));
            after_panic = t.contains("panicked at");
            after_error = t.starts_with("error");
            take
        };
        if t.starts_with("test result: FAILED") {
            verdict = Some(t.to_string());
        }
        if take && !t.is_empty() && !picked.iter().any(|p| p == t) {
            let mut t = t.to_string();
            if t.len() > 300 {
                let mut cut = 300;
                while !t.is_char_boundary(cut) {
                    cut -= 1;
                }
                t.truncate(cut);
                t.push_str(" ...");
            }
            picked.push(t);
        }
    }
    picked.truncate(SHOWN);
    if let Some(v) = verdict {
        if !picked.contains(&v) {
            picked.push(v);
        }
    }
    if picked.iter().any(|p| p.contains("SIGKILL") || p.starts_with("Killed")) {
        picked.push("(killed: most likely out of memory; a build may use at most the cap it was given)".into());
    }
    (why, picked)
}

fn remove(p: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(p),
        Ok(_) => fs::remove_file(p),
        Err(_) => Ok(()),
    }
}

fn packages_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> =
        rd.filter_map(Result::ok).filter(|e| e.file_name().as_bytes().ends_with(b".pkg.tar.zst")).map(|e| e.path()).collect();
    v.sort();
    v
}

/// One build; the names of the packages it put in the repository folder.
fn build_one(s: &Settings, r: &Recipe, log: &Path) -> Result<Vec<String>, (String, Vec<String>)> {
    let io = |what: &str, p: &Path, e: std::io::Error| (format!("{what} {}: {e}", p.display()), Vec::new());
    let out = match &r.out {
        Some(o) => {
            remove(o).map_err(|e| io("cannot remove", o, e))?;
            fs::create_dir_all(o).map_err(|e| io("cannot make", o, e))?;
            o.clone()
        }
        None => {
            // as a fresh makepkg there: no folders or packages of an earlier build
            for p in [r.dir.join("pkg"), r.dir.join("src")].into_iter().chain(packages_in(&r.dir)) {
                remove(&p).map_err(|e| io("cannot remove", &p, e))?;
            }
            r.dir.clone()
        }
    };
    let file = fs::File::create(log).map_err(|e| io("cannot write", log, e))?;
    let err = file.try_clone().map_err(|e| io("cannot write", log, e))?;
    let mut c = match &s.memory {
        Some(size) => {
            let mut c = Command::new("systemd-run");
            c.args(["--user", "--scope", "-q", "-p", &format!("MemoryMax={size}"), "-p", "MemorySwapMax=0", "--"]).arg(&s.makepkg);
            c
        }
        None => Command::new(&s.makepkg),
    };
    // -c: makepkg removes its src/ and pkg/ folders after a good build (they hold gigabytes of build files)
    c.arg("--config").arg(&s.makepkg_conf).args(["-f", "-c", "--noconfirm", "--nocolor"]).current_dir(&r.dir);
    c.env("PKGDEST", &out).env("CARGO_BUILD_JOBS", s.jobs.to_string()).env("MAKEFLAGS", format!("-j{}", s.jobs));
    if r.out.is_some() {
        c.env("BUILDDIR", &out).env("SRCDEST", &out);
    }
    let status = c.stdin(Stdio::null()).stdout(file).stderr(err).status().map_err(|e| (format!("cannot run makepkg: {e}"), Vec::new()))?;
    if !status.success() {
        let text = fs::read(log).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let (mut why, mut lines) = cause(&text);
        if let Some(sig) = status.signal() {
            why = format!("makepkg was killed (signal {sig}): {why}");
            if sig == 9 {
                lines.push("(killed: most likely out of memory; a build may use at most the cap it was given)".into());
            }
        }
        return Err((why, lines));
    }
    let made = packages_in(&out);
    if made.is_empty() {
        return Err(("makepkg ended well but made no package".into(), Vec::new()));
    }
    let mut names = Vec::new();
    for f in made {
        let name = f.file_name().unwrap_or_default().to_os_string();
        copy2(&f, &s.repo_dir.join(&name)).map_err(|e| (e, Vec::new()))?;
        names.push(name.to_string_lossy().into_owned());
    }
    Ok(names)
}

enum Event {
    Started(String),
    Unchanged(String),
    Built { name: String, key: String, files: Vec<String>, secs: u64 },
    Failed(Failure),
}

fn work(s: &Settings, r: &Recipe, before: Option<&Good>, conf: &str, dirty: &str, logs: &Path, tx: &mpsc::Sender<Event>) -> Event {
    let log = logs.join(format!("{}.log", r.name));
    let failed = |why: String, lines: Vec<String>| Event::Failed(Failure { name: r.name.clone(), why, lines, log: log.clone() });
    let key = match key(s, r, conf, dirty) {
        Ok((k, _)) => k,
        Err(e) => return failed("makepkg could not read the recipe".into(), vec![e]),
    };
    if !s.force {
        if let Some(g) = before {
            if g.key == key && !g.files.is_empty() && g.files.iter().all(|f| s.repo_dir.join(f).is_file()) {
                return Event::Unchanged(r.name.clone());
            }
        }
    }
    let _ = tx.send(Event::Started(r.name.clone()));
    let started = Instant::now();
    match build_one(s, r, &log) {
        Ok(files) => Event::Built { name: r.name.clone(), key, files, secs: started.elapsed().as_secs() },
        Err((why, lines)) => failed(why, lines),
    }
}

fn took(secs: u64) -> String {
    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

/// Builds what changed, a few at a time; a failure is said as it happens and the others go on.
pub fn build_all(recipes: Vec<Recipe>, s: &Settings) -> R<Report> {
    fs::create_dir_all(&s.repo_dir).map_err(|e| format!("error: cannot make {}: {e}", s.repo_dir.display()))?;
    let logs = s.logs();
    fs::create_dir_all(&logs).map_err(|e| format!("error: cannot make {}: {e}", logs.display()))?;
    let state_file = s.state_file();
    let mut good = load(&state_file)?;
    let before = good.clone();
    let conf = hex(Sha256::digest(fs::read(&s.makepkg_conf).map_err(|e| format!("error: cannot read {}: {e}", s.makepkg_conf.display()))?));
    let mut dirty: HashMap<PathBuf, String> = HashMap::new();
    for r in &recipes {
        if !dirty.contains_key(&r.checkout) {
            dirty.insert(r.checkout.clone(), uncommitted(&r.checkout)?);
        }
    }
    let mut queue = recipes;
    // the longest builds first, so the last one still running is a short one; a recipe never built yet counts as long
    queue.sort_by_key(|r| std::cmp::Reverse(before.get(&r.name).map_or(u64::MAX, |g| g.secs)));
    let cap = s.memory.as_ref().map_or_else(|| ", no memory cap here".to_string(), |m| format!(", {m} each"));
    say(&format!(
        "{} recipes: building what changed, {} at a time ({} compile jobs{cap}); logs in {}",
        queue.len(),
        s.at_once,
        s.jobs,
        logs.display()
    ));
    let workers = s.at_once.min(queue.len()).max(1);
    let queue = Mutex::new(VecDeque::from(queue));
    let mut report = Report::default();
    let mut saving: R<()> = Ok(());
    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::channel::<Event>();
        for _ in 0..workers {
            let (tx, queue, before, conf, dirty, logs) = (tx.clone(), &queue, &before, &conf, &dirty, &logs);
            scope.spawn(move || loop {
                let next = queue.lock().map(|mut q| q.pop_front()).unwrap_or(None);
                let Some(r) = next else { break };
                let ev = work(s, &r, before.get(&r.name), conf, &dirty[&r.checkout], logs, &tx);
                if tx.send(ev).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        for ev in rx {
            match ev {
                Event::Started(name) => say(&format!("{name}: building")),
                Event::Unchanged(name) => {
                    say(&format!("{name}: unchanged since its last build"));
                    report.unchanged.push(name);
                }
                Event::Built { name, key, files, secs } => {
                    say(&format!("{name}: built in {}: {}", took(secs), files.join(", ")));
                    good.insert(name.clone(), Good { key, files, secs });
                    // kept after every build, so a publish stopped halfway does not build the same again
                    if saving.is_ok() {
                        saving = save(&state_file, &good);
                    }
                    report.built.push(name);
                }
                Event::Failed(f) => {
                    say(&format!("{}: FAILED: {} (the whole log: {})", f.name, f.why, f.log.display()));
                    for l in &f.lines {
                        println!("      {l}");
                    }
                    report.failed.push(f);
                }
            }
        }
    });
    saving?;
    let names: Vec<&str> = report.failed.iter().map(|f| f.name.as_str()).collect();
    say(&format!(
        "builds: {} built, {} unchanged, {} failed{}",
        report.built.len(),
        report.unchanged.len(),
        names.len(),
        if names.is_empty() { String::new() } else { format!(" ({})", names.join(", ")) }
    ));
    Ok(report)
}

// --- the ISO's local repository ------------------------------------------------------------------------------------------

/// The Labyrinth_OS checkout this is run in (or below).
fn labyrinth_here() -> R<PathBuf> {
    let start = std::env::current_dir().map_err(|e| format!("error: cannot tell the current folder: {e}"))?;
    start
        .ancestors()
        .find(|d| d.join("scripts").join("package-version.sh").is_file() && d.join("packages").join("pkgbuilds").is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| "error: run this inside a Labyrinth_OS checkout (it builds that checkout's recipes)".to_string())
}

/// `labyrinth-publish-repo build [--force] [NAME...]`: the recipes of this checkout and of the Yggdrasil checkout next to it
/// (or YGGDRASIL_DIR) built into repo/x86_64, signed, and its database made, for scripts/build.sh. Named recipes are always
/// built; without names, what changed, and not the ones that are not on the ISO. Returns how many failed.
pub fn local(names: &[String], force: bool) -> R<usize> {
    if fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(1) == 0 {
        return Err("error: makepkg refuses to run as root; run this as a regular user".into());
    }
    let root = labyrinth_here()?;
    let ygg = std::env::var_os("YGGDRASIL_DIR").map_or_else(|| root.join("..").join("Yggdrasil"), PathBuf::from);
    if !ygg.join("packages").join("pkgbuilds").is_dir() {
        return Err(format!(
            "error: the Yggdrasil repository is not at {} (git clone https://github.com/Artifex-Technologies/Yggdrasil, or set \
             YGGDRASIL_DIR)",
            ygg.display()
        ));
    }
    let mut all = recipes(&root, &ygg, None)?;
    if names.is_empty() {
        let skip = std::env::var("LABYRINTH_SKIP").unwrap_or_else(|_| NOT_ON_THE_ISO.into());
        let skip: Vec<&str> = skip.split_whitespace().collect();
        all.retain(|r| {
            let keep = !skip.contains(&r.name.as_str());
            if !keep {
                say(&format!("{}: not on the ISO, not built (name it to build it)", r.name));
            }
            keep
        });
    } else {
        let unknown: Vec<&String> = names.iter().filter(|n| !all.iter().any(|r| &&r.name == n)).collect();
        if !unknown.is_empty() {
            return Err(format!("error: no recipe named {}", unknown.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")));
        }
        all.retain(|r| names.contains(&r.name));
    }
    let repo_dir = root.join("repo").join("x86_64");
    let s = Settings::new(root.join("scripts").join("makepkg.conf"), repo_dir.clone(), force || !names.is_empty());
    let report = build_all(all, &s)?;
    repository(&repo_dir)?;
    Ok(report.failed.len())
}

/// Every package in the folder and the database signed with Labyrinth's key (its own keyring, docs/PACKAGE_REPO.md), so
/// the ISO build checks them like any other repository; LABYRINTH_UNSIGNED=1 on a host without the key, and scripts/build.sh
/// then trusts them unsigned, saying so. Of each package the database lists the newest by version (--prevent-downgrade),
/// not the last file name in the folder's order.
fn repository(repo_dir: &Path) -> R<()> {
    let unsigned = std::env::var("LABYRINTH_UNSIGNED").is_ok_and(|v| v == "1");
    let gnupg = crate::publish::gnupg_home();
    let mut sign: Vec<&str> = Vec::new();
    if !unsigned {
        if !Cmd::new(["gpg", "--batch", "--list-secret-keys", KEY]).env("GNUPGHOME", &gnupg).quiet_ok()? {
            return Err(format!(
                "error: Labyrinth's signing key is not in {} (restore it, or set LABYRINTH_UNSIGNED=1)",
                gnupg.to_string_lossy()
            ));
        }
        for f in packages_in(repo_dir) {
            let mut sig = f.clone().into_os_string();
            sig.push(".sig");
            let sig = PathBuf::from(sig);
            let newer = |a: &Path, b: &Path| match (fs::metadata(a).and_then(|m| m.modified()), fs::metadata(b).and_then(|m| m.modified()))
            {
                (Ok(a), Ok(b)) => a > b,
                _ => false,
            };
            if sig.is_file() && newer(&sig, &f) {
                continue;
            }
            Cmd::new(["gpg", "--batch", "--yes", "--local-user", KEY, "--detach-sign", "--no-armor", "--output"])
                .arg(&sig)
                .arg(&f)
                .env("GNUPGHOME", &gnupg)
                .quiet()?;
        }
        sign = vec!["--sign", "--key", KEY];
    }
    let Ok(rd) = fs::read_dir(repo_dir) else { return Err(format!("error: cannot read {}", repo_dir.display())) };
    for e in rd.filter_map(Result::ok) {
        let n = e.file_name();
        if n.as_bytes().starts_with(b"labyrinth.db") || n.as_bytes().starts_with(b"labyrinth.files") {
            let _ = fs::remove_file(e.path());
        }
    }
    let packages = packages_in(repo_dir);
    if packages.is_empty() {
        return Err(format!("error: no packages in {}", repo_dir.display()));
    }
    Cmd::new(["repo-add", "--quiet", "--prevent-downgrade"])
        .args(&sign)
        .arg(repo_dir.join("labyrinth.db.tar.gz"))
        .args(&packages)
        .env("GNUPGHOME", &gnupg)
        .run()?;
    say(&format!("the local repository: {}{}", repo_dir.display(), if unsigned { " (unsigned)" } else { " (signed)" }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_failed_test_run_says_which_tests_and_why() {
        let log = "\
==> Starting check()...
   Compiling yggdrasil-installer v0.1.0
running 32 tests
test network::tests::signal_bars ... ok
test tests::without_a_wifi_adapter_the_network_step_is_skipped ... FAILED
test tests::online_by_cable_skips_the_network_step ... FAILED
failures:
---- tests::without_a_wifi_adapter_the_network_step_is_skipped stdout ----
thread 'tests::without_a_wifi_adapter_the_network_step_is_skipped' (1) panicked at src/main.rs:1721:13:
assertion `left == right` failed: no Wi-Fi adapter: nothing to offer, cable or not
  left: Some(Font)
 right: Some(Keyboard)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
failures:
    tests::without_a_wifi_adapter_the_network_step_is_skipped
test result: FAILED. 27 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s
error: test failed, to rerun pass `--bin yggdrasil-installer`
==> ERROR: A failure occurred in check().
    Aborting...
";
        let (why, lines) = cause(log);
        assert_eq!(why, "A failure occurred in check().");
        assert_eq!(
            lines,
            [
                "test tests::without_a_wifi_adapter_the_network_step_is_skipped ... FAILED",
                "test tests::online_by_cable_skips_the_network_step ... FAILED",
                "thread 'tests::without_a_wifi_adapter_the_network_step_is_skipped' (1) panicked at src/main.rs:1721:13:",
                "assertion `left == right` failed: no Wi-Fi adapter: nothing to offer, cable or not",
                "left: Some(Font)",
                "right: Some(Keyboard)",
                "error: test failed, to rerun pass `--bin yggdrasil-installer`",
                "test result: FAILED. 27 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s",
            ]
        );
    }

    #[test]
    fn a_link_error_a_compile_error_and_missing_dependencies() {
        let (why, lines) = cause(
            "error: linking with `cc` failed: exit status: 1\n  = note: ld.lld: error: undefined symbol: onig_free\n\
             \x1b[1m\x1b[31merror[E0425]\x1b[0m: cannot find value `x` in this scope\n   --> src/main.rs:5:13\n    |\n\
             ==> ERROR: A failure occurred in build().\n",
        );
        assert_eq!(why, "A failure occurred in build().");
        assert_eq!(
            lines,
            [
                "error: linking with `cc` failed: exit status: 1",
                "= note: ld.lld: error: undefined symbol: onig_free",
                "error[E0425]: cannot find value `x` in this scope",
                "--> src/main.rs:5:13",
            ]
        );
        let (why, lines) =
            cause("==> Making package: x 1-1\n==> Checking runtime dependencies...\n==> Missing dependencies:\n  -> libfoo\n  -> bar>=2\n==> ERROR: Could not resolve all dependencies.\n");
        assert_eq!(why, "Could not resolve all dependencies.");
        assert_eq!(lines, ["-> libfoo", "-> bar>=2"]);
        let (why, lines) =
            cause("error: could not compile `x` (bin \"x\")\nCaused by:\n  process didn't exit successfully (signal: 9, SIGKILL: kill)\n");
        assert_eq!(why, "makepkg stopped without saying why");
        assert!(lines.last().unwrap().contains("out of memory"), "{lines:?}");
    }

    #[test]
    fn git_sources_as_srcinfo_lists_them() {
        let info = "pkgbase = sarina-git\n\tpkgver = 1.0.0\n\tsource = sarina::git+file:///w/src/Sarina\n\t\
                    source = icarus::git+https://github.com/o/Icarus.git#branch=main\n\tsource = x.tar.gz::https://h/x.tar.gz\n\t\
                    source_x86_64 = git+https://h/y.git?signed#tag=v1\n\tsha256sums = SKIP\n";
        assert_eq!(
            git_sources(info),
            [
                GitSource { url: "file:///w/src/Sarina".into(), pick: None },
                GitSource { url: "https://github.com/o/Icarus.git".into(), pick: Some(("branch".into(), "main".into())) },
                GitSource { url: "https://h/y.git".into(), pick: Some(("tag".into(), "v1".into())) },
            ]
        );
    }

    /// A made-up world: a git checkout with three recipes, and a stand-in makepkg that "builds" by writing the package file
    /// named in the recipe, or fails the way a test run does when the recipe says so.
    struct World {
        dir: PathBuf,
    }

    impl World {
        fn new(tag: &str) -> World {
            let dir = std::env::temp_dir().join(format!("publish-build-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            let w = World { dir };
            fs::create_dir_all(w.checkout().join("packages/pkgbuilds")).unwrap();
            fs::create_dir_all(w.checkout().join("scripts")).unwrap();
            fs::write(w.checkout().join("scripts/makepkg.conf"), "# conf\n").unwrap();
            fs::write(w.checkout().join(".gitignore"), "repo/\n*.pkg.tar.zst\n").unwrap();
            for (name, ver) in [("aaa", "1-1"), ("bbb", "1-1"), ("ccc", "1-1")] {
                w.recipe(name, ver, false);
            }
            fs::create_dir_all(w.dir.join("ygg/packages/pkgbuilds")).unwrap();
            w.git(&["init", "-q"]);
            w.commit();
            let fake = w.dir.join("makepkg");
            fs::write(
                &fake,
                "#!/bin/sh\n. ./PKGBUILD\nfile=\"$pkgname-$pkgver-any.pkg.tar.zst\"\nfor a; do case $a in\n\
                 --packagelist) echo \"$PKGDEST/$file\"; exit 0;;\n--printsrcinfo) echo \"pkgbase = $pkgname\"; exit 0;;\nesac; done\n\
                 echo \"==> Making package: $pkgname $pkgver\"\n\
                 if [ -n \"$fail\" ]; then echo 'test tests::it_works ... FAILED'; echo 'test result: FAILED. 0 passed; 1 failed'; \
                 echo '==> ERROR: A failure occurred in check().'; exit 4; fi\necho built > \"$PKGDEST/$file\"\n",
            )
            .unwrap();
            fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
            w
        }

        fn checkout(&self) -> PathBuf {
            self.dir.join("lab")
        }

        fn git(&self, args: &[&str]) {
            let st = Command::new("git").arg("-C").arg(self.checkout()).args(args).stdout(Stdio::null()).status().unwrap();
            assert!(st.success(), "git {args:?}");
        }

        fn commit(&self) {
            self.git(&["add", "."]);
            self.git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "recipes"]);
        }

        fn recipe(&self, name: &str, ver: &str, fail: bool) {
            let d = self.checkout().join("packages/pkgbuilds").join(name);
            fs::create_dir_all(&d).unwrap();
            let fail = if fail { "fail=1\n" } else { "" };
            fs::write(d.join("PKGBUILD"), format!("pkgname={name}\npkgver={ver}\n{fail}")).unwrap();
        }

        fn settings(&self) -> Settings {
            Settings {
                makepkg_conf: self.checkout().join("scripts/makepkg.conf"),
                repo_dir: self.checkout().join("repo/x86_64"),
                force: false,
                at_once: 2,
                jobs: 1,
                memory: None,
                makepkg: self.dir.join("makepkg").into_os_string(),
            }
        }

        fn build(&self) -> Report {
            build_all(recipes(&self.checkout(), &self.dir.join("ygg"), None).unwrap(), &self.settings()).unwrap()
        }
    }

    impl Drop for World {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    #[test]
    fn only_what_changed_is_built_and_a_failure_does_not_stop_the_others() {
        let w = World::new("changed");
        let first = w.build();
        assert_eq!(sorted(first.built), ["aaa", "bbb", "ccc"]);
        assert!(w.checkout().join("repo/x86_64/bbb-1-1-any.pkg.tar.zst").is_file());
        // nothing changed: nothing built
        let again = w.build();
        assert!(again.built.is_empty() && again.failed.is_empty());
        assert_eq!(sorted(again.unchanged), ["aaa", "bbb", "ccc"]);
        // a new version of one, and one that now fails its tests (committed, as in the publish's fresh copies)
        w.recipe("aaa", "1-2", false);
        w.recipe("bbb", "1-1", true);
        w.commit();
        let third = w.build();
        assert_eq!(third.built, ["aaa"]);
        assert_eq!(third.unchanged, ["ccc"]);
        assert_eq!(third.failed.len(), 1);
        let f = &third.failed[0];
        assert_eq!((f.name.as_str(), f.why.as_str()), ("bbb", "A failure occurred in check()."));
        assert_eq!(f.lines, ["test tests::it_works ... FAILED", "test result: FAILED. 0 passed; 1 failed"]);
        assert!(fs::read_to_string(&f.log).unwrap().contains("Making package: bbb"));
        assert!(w.checkout().join("repo/x86_64/aaa-1-2-any.pkg.tar.zst").is_file());
        // its last good build is still there, and the failed one is tried again next time
        assert!(w.checkout().join("repo/x86_64/bbb-1-1-any.pkg.tar.zst").is_file());
        let fourth = w.build();
        assert_eq!(fourth.failed.len(), 1);
        assert_eq!(sorted(fourth.unchanged), ["aaa", "ccc"]);
        // a package removed from the folder is built again; a recipe put back as it was at its last good build is not
        // (that build is still there)
        fs::remove_file(w.checkout().join("repo/x86_64/ccc-1-1-any.pkg.tar.zst")).unwrap();
        w.recipe("bbb", "1-1", false);
        w.commit();
        assert_eq!(w.build().built, ["ccc"]);
        // an uncommitted change anywhere in the checkout: which recipe it feeds cannot be told, so all of its recipes are
        // built (a hand build of the ISO from a working tree), and once only
        fs::write(w.checkout().join("notes.txt"), "a change\n").unwrap();
        assert_eq!(sorted(w.build().built), ["aaa", "bbb", "ccc"]);
        assert!(w.build().built.is_empty());
        // and --force builds everything
        let s = Settings { force: true, ..w.settings() };
        assert_eq!(sorted(build_all(recipes(&w.checkout(), &w.dir.join("ygg"), None).unwrap(), &s).unwrap().built), ["aaa", "bbb", "ccc"]);
    }

    #[test]
    fn the_stores_recipes_come_after_and_are_not_built_twice() {
        let w = World::new("store");
        for (dir, name) in [(STORE, "aaa"), (STORE, "zzz")] {
            let d = w.checkout().join(dir).join(name);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("PKGBUILD"), format!("pkgname={name}\npkgver=1-1\n")).unwrap();
        }
        let store = w.dir.join("store");
        let all = recipes(&w.checkout(), &w.dir.join("ygg"), Some(&store)).unwrap();
        let names: Vec<&str> = all.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["aaa", "bbb", "ccc", "zzz"]);
        assert_eq!(all[3].out.as_deref(), Some(store.join("zzz").as_path()));
        assert!(all[0].out.is_none());
        assert!(recipes(&w.checkout(), &w.dir.join("ygg"), None).unwrap().iter().all(|r| r.name != "zzz"));
        let report = build_all(all, &w.settings()).unwrap();
        assert_eq!(sorted(report.built), ["aaa", "bbb", "ccc", "zzz"]);
        assert!(store.join("zzz/zzz-1-1-any.pkg.tar.zst").is_file(), "the store's builds are made in their own folder");
        assert!(w.checkout().join("repo/x86_64/zzz-1-1-any.pkg.tar.zst").is_file());
    }
}
