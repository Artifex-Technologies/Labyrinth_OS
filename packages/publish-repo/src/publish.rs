//! The publish itself, in the script's order:
//!
//!   1. fresh copies of Labyrinth_OS, Yggdrasil, Sarina, Heimdall and Limbo from GitHub (main as pushed, never a working tree
//!      with unfinished changes), side by side in the work folder;
//!   2. nothing more when none of them changed since the last publish (unless --force);
//!   3. the packages built from them: scripts/build-local-packages.sh (the ISO's packages) and the store's own recipes
//!      (profile/airootfs/usr/share/labyrinth/pkgbuilds); each recipe works out its version and build number
//!      (docs/VERSIONING.md);
//!   4. of each package the newest build; a build whose files and dependencies are the same as what is already published, at
//!      the same version, is not published again (the published one stays, so nobody downloads the same program twice); a
//!      release (a new version) is always published;
//!   5. each package and the database signed with Labyrinth's key (its own keyring, ~/.local/share/labyrinth-signing/gnupg;
//!      the backup and how to restore it are on Proton Drive, Documents/Github/Labyrinth_OS/package-signing-key);
//!   6. uploaded: the packages first, the database last, then the files of the publish before last are removed (the last
//!      publish's stay a day, for systems that read the database just before this one replaced it).
//!
//! Left out on purpose: software whose licence does not allow passing it on (Proton Pass, VS Code: Agora installs those
//! from the AUR, downloaded from Proton and Microsoft). See docs/PACKAGE_REPO.md.

use crate::cmd::{Cmd, R};
use crate::pkg;
use crate::py::{pathlib, state_json, strip};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const ORG: &str = "Artifex-Technologies";
pub const GH_REPO: &str = "Artifex-Technologies/Labyrinth_OS";
pub const TAG: &str = "packages";
pub const REPO_NAME: &str = "labyrinth";
pub const KEY: &str = "C9CE784C8BD9B661042075972D3BD4B1E7CD07A5";
pub const SOURCES: [&str; 5] = ["Labyrinth_OS", "Yggdrasil", "Sarina", "Heimdall", "Limbo"];

pub fn say(msg: &str) {
    // one line at a time into the journal, as it happens
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "==> {msg}");
    let _ = out.flush();
}

/// Where everything happens. Outside the home folder: each package's .BUILDINFO names the folder it was built in, and a path
/// under /home names the account (the folder is made once, owned by the account that publishes: docs/PACKAGE_REPO.md).
pub fn work_dir() -> PathBuf {
    match std::env::var_os("LABYRINTH_PUBLISH_DIR") {
        Some(v) => pathlib(&v),
        None => PathBuf::from("/var/lib/labyrinth-publish"),
    }
}

/// The signing keyring: Labyrinth's own, never the account's GnuPG.
pub fn gnupg_home() -> OsString {
    if let Some(v) = std::env::var_os("LABYRINTH_GNUPGHOME") {
        return v;
    }
    let home = match std::env::var_os("HOME") {
        Some(h) => {
            let b = h.as_bytes();
            let end = b.iter().rposition(|&c| c != b'/').map_or(0, |i| i + 1);
            if end == 0 {
                OsString::from("/")
            } else {
                OsStr::from_bytes(&b[..end]).to_os_string()
            }
        }
        #[allow(deprecated)] // fixed since Rust 1.85: the passwd entry when HOME is not set
        None => std::env::home_dir().map(PathBuf::into_os_string).unwrap_or_default(),
    };
    pathlib(&home).join(".local").join("share").join("labyrinth-signing").join("gnupg").into_os_string()
}

pub struct Options {
    pub force: bool,
    pub dry_run: bool,
}

struct Ctx {
    work: PathBuf,
    gnupg: OsString,
}

impl Ctx {
    fn gpg<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(&self, args: I) -> Cmd {
        Cmd::new(["gpg"]).args(args).env("GNUPGHOME", &self.gnupg)
    }
}

fn shown(p: &Path) -> String {
    p.display().to_string()
}

fn io(what: &str, p: &Path, e: std::io::Error) -> String {
    format!("error: {what} {}: {e}", shown(p))
}

/// mkdir -p, but an empty path is "." (already there)
fn mkdir_p(p: &Path) -> R<()> {
    if p.as_os_str().is_empty() {
        return Ok(());
    }
    fs::create_dir_all(p).map_err(|e| io("cannot make the folder", p, e))
}

/// Path.mkdir(parents=True): the folder must not be there yet
fn mkdir_new(p: &Path) -> R<()> {
    if let Some(parent) = p.parent() {
        mkdir_p(parent)?;
    }
    fs::create_dir(p).map_err(|e| io("cannot make the folder", p, e))
}

/// shutil.rmtree: a folder and everything in it; a link is refused, as rmtree refuses one
fn rmtree(p: &Path) -> R<()> {
    if p.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return Err(format!("error: {} is a link, not a folder: not removed", shown(p)));
    }
    fs::remove_dir_all(p).map_err(|e| io("cannot remove", p, e))
}

/// shutil.copy2: contents, permission bits and times
fn copy2(from: &Path, to: &Path) -> R<()> {
    fs::copy(from, to).map_err(|e| format!("error: cannot copy {} to {}: {e}", shown(from), shown(to)))?;
    copy_times(from, to)
}

fn copy_times(from: &Path, to: &Path) -> R<()> {
    let m = fs::metadata(from).map_err(|e| io("cannot read", from, e))?;
    let mut times = fs::FileTimes::new();
    if let Ok(t) = m.accessed() {
        times = times.set_accessed(t);
    }
    if let Ok(t) = m.modified() {
        times = times.set_modified(t);
    }
    let f = fs::File::options()
        .read(m.is_dir())
        .write(!m.is_dir())
        .open(to)
        .or_else(|_| fs::File::open(to))
        .map_err(|e| io("cannot open", to, e))?;
    f.set_times(times).map_err(|e| io("cannot set the times of", to, e))
}

/// shutil.copytree (links followed): the folder must not be there yet
fn copytree(from: &Path, to: &Path) -> R<()> {
    mkdir_new(to)?;
    for e in fs::read_dir(from).map_err(|e| io("cannot read", from, e))? {
        let e = e.map_err(|e| io("cannot read", from, e))?;
        let src = e.path();
        let dst = to.join(e.file_name());
        if fs::metadata(&src).map(|m| m.is_dir()).unwrap_or(false) {
            copytree(&src, &dst)?;
        } else {
            copy2(&src, &dst)?;
        }
    }
    let m = fs::metadata(from).map_err(|e| io("cannot read", from, e))?;
    fs::set_permissions(to, m.permissions()).map_err(|e| io("cannot set the mode of", to, e))?;
    copy_times(from, to)
}

/// The entries of a folder whose names match, in the folder's own order (as pathlib's glob gives them: hidden names,
/// folders and broken links included; a folder that cannot be read has none).
fn glob(dir: &Path, matches: impl Fn(&[u8]) -> bool) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    rd.filter_map(Result::ok).filter(|e| matches(e.file_name().as_bytes())).map(|e| dir.join(e.file_name())).collect()
}

fn packages_in(dir: &Path) -> Vec<PathBuf> {
    glob(dir, |n| n.ends_with(b".pkg.tar.zst"))
}

/// The names in a folder, sorted
fn names_in(dir: &Path) -> R<Vec<OsString>> {
    let rd = fs::read_dir(dir).map_err(|e| io("cannot read", dir, e))?;
    let mut names = Vec::new();
    for e in rd {
        names.push(e.map_err(|e| io("cannot read", dir, e))?.file_name());
    }
    names.sort();
    Ok(names)
}

fn read_json(p: &Path) -> R<Value> {
    let text = fs::read_to_string(p).map_err(|e| io("cannot read", p, e))?;
    serde_json::from_str(&text).map_err(|e| format!("error: {} is not JSON: {e}", shown(p)))
}

// --- 1, 2: sources -------------------------------------------------------------------------------------------------------

/// Each source repository as it is on GitHub. Returns (name, commit), in SOURCES' order.
fn fresh_copies(src: &Path) -> R<Vec<(String, String)>> {
    mkdir_p(src)?;
    let mut heads = Vec::new();
    for name in SOURCES {
        let d = src.join(name);
        let git = || Cmd::new([OsStr::new("git"), OsStr::new("-C"), d.as_os_str()]);
        if !d.join(".git").is_dir() {
            Cmd::new(["git", "clone", "--quiet", &format!("https://github.com/{ORG}/{name}.git")]).arg(&d).run()?;
        } else {
            git().args(["fetch", "--quiet", "origin"]).run()?;
        }
        let branch = strip(&git().args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).text()?).to_string();
        let Some((_, local)) = branch.split_once('/') else {
            return Err(format!("error: {name}: origin's HEAD is {branch:?}, not a remote branch"));
        };
        // the local branch moves with GitHub: recipes that build from this copy (git+file://, heimdall-git, sarina-git,
        // limbo-git) take its branch, not the checkout, so a detached checkout left them building the first clone's
        // commit for days (2026-11-07: Heimdall two days behind, Sarina too)
        git().args(["checkout", "--quiet", "--force", "-B", local, &branch]).run()?;
        // tracked files exactly as committed; ignored build output (the local repo, makepkg's folders) is kept
        git().args(["clean", "--quiet", "-ffd"]).run()?;
        heads.push((name.to_string(), strip(&git().args(["rev-parse", "HEAD"]).text()?).to_string()));
    }
    Ok(heads)
}

// --- 3: building ---------------------------------------------------------------------------------------------------------

fn build(src: &Path, work: &Path) -> R<PathBuf> {
    let labyrinth = src.join("Labyrinth_OS");
    say("building the ISO's packages (scripts/build-local-packages.sh)");
    // LABYRINTH_SKIP empty: the publish leaves nothing out, whatever the caller's environment says
    Cmd::new([labyrinth.join("scripts").join("build-local-packages.sh")])
        .cwd(&labyrinth)
        .env("YGGDRASIL_DIR", src.join("Yggdrasil"))
        .env("LABYRINTH_SKIP", "")
        .run()?;
    let repo_dir = labyrinth.join("repo").join("x86_64");
    // the store's own recipes (they clone from GitHub); those also built above from a local checkout are not built twice
    let mut built_above: HashSet<OsString> = HashSet::new();
    for d in [labyrinth.join("packages").join("pkgbuilds"), src.join("Yggdrasil").join("packages").join("pkgbuilds")] {
        built_above.extend(names_in(&d)?);
    }
    let store = labyrinth.join("profile/airootfs/usr/share/labyrinth/pkgbuilds");
    for name in names_in(&store)? {
        let d = store.join(&name);
        if !d.join("PKGBUILD").is_file() || built_above.contains(&name) {
            continue;
        }
        say(&format!("building the store's {}", name.to_string_lossy()));
        // always the same folder: a build path ends up inside the programs, and a new random one each time would make every
        // build look changed (and be published again for nothing). makepkg reads the recipe where it is, in the clone (its
        // build number counts the recipe's own commits, docs/VERSIONING.md), and puts everything it makes in that folder:
        // the recipe's folder stays as committed (it goes onto the ISO).
        let w = work.join("store").join(&name);
        if w.exists() {
            rmtree(&w)?;
        }
        mkdir_new(&w)?;
        Cmd::new([OsStr::new("makepkg"), OsStr::new("--config"), labyrinth.join("scripts/makepkg.conf").as_os_str()])
            .args(["-f", "-s", "-c", "--noconfirm"])
            .cwd(&d)
            .env("BUILDDIR", &w)
            .env("SRCDEST", &w)
            .env("PKGDEST", &w)
            .run()?;
        for f in packages_in(&w) {
            copy2(&f, &repo_dir.join(f.file_name().unwrap_or_default()))?;
        }
    }
    Ok(repo_dir)
}

// --- 4: choosing what to publish -----------------------------------------------------------------------------------------

/// A package's name, version and .PKGINFO lines.
type PkgInfo = (String, String, Vec<(String, String)>);

/// The package's .PKGINFO as (name, version, lines).
fn pkginfo(path: &Path) -> R<PkgInfo> {
    let text = Cmd::new([OsStr::new("bsdtar"), OsStr::new("-xOf"), path.as_os_str(), OsStr::new(".PKGINFO")]).text()?;
    let lines = pkg::pkginfo_lines(&text);
    let need = |k: &str| pkg::field(&lines, k).map(str::to_string).ok_or_else(|| format!("error: {}: no {k} in its .PKGINFO", shown(path)));
    let (name, ver) = (need("pkgname")?, need("pkgver")?);
    Ok((name, ver, lines))
}

fn vercmp(a: &str, b: &str) -> R<i64> {
    let t = Cmd::new(["vercmp", a, b]).text()?;
    strip(&t).replace('_', "").parse::<i64>().map_err(|_| format!("error: vercmp {a} {b} answered {t:?}"))
}

/// What makes a package different to the person installing it (pkg::content_digest).
fn content(path: &Path) -> R<String> {
    let mtree = Cmd::new([OsStr::new("bsdtar"), OsStr::new("-xOf"), path.as_os_str(), OsStr::new(".MTREE")]).bytes(None)?;
    let mtree = Cmd::new(["gzip", "-dc"]).bytes(Some(&mtree))?;
    let mtree = String::from_utf8(mtree).map_err(|_| format!("error: {}: its .MTREE is not UTF-8", shown(path)))?;
    let (_, _, lines) = pkginfo(path)?;
    Ok(pkg::content_digest(&mtree, &lines))
}

/// Whether the package was built in a folder under /home or /root (its .BUILDINFO says where); one without a .BUILDINFO
/// was not.
fn built_at_home(path: &Path) -> R<bool> {
    let info = Cmd::new([OsStr::new("bsdtar"), OsStr::new("-xOf"), path.as_os_str(), OsStr::new(".BUILDINFO")]).text_or_none()?;
    Ok(info.is_some_and(|t| pkg::built_at_home(&t)))
}

/// The packages that still have a recipe in the sources of this publish: Labyrinth's own (packages/pkgbuilds), the store's
/// (profile/.../labyrinth/pkgbuilds) and Yggdrasil's (packages/pkgbuilds), by folder name and by every pkgname a recipe
/// makes. Builds of anything else left in the repository folder from earlier publishes are not published: older names
/// (labyrinth-panel became yggdrasil-panel), and software taken out (2026-11-08: huginn-git, Huginn moved to a private
/// repository, was published again from an old build).
fn with_recipe(labyrinth: &Path) -> R<HashSet<String>> {
    let dirs = [
        labyrinth.join("packages").join("pkgbuilds"),
        labyrinth.join("profile/airootfs/usr/share/labyrinth/pkgbuilds"),
        labyrinth.parent().unwrap_or(Path::new("")).join("Yggdrasil").join("packages").join("pkgbuilds"),
    ];
    let mut names = HashSet::new();
    for d in dirs {
        if !d.is_dir() {
            continue;
        }
        for e in fs::read_dir(&d).map_err(|e| io("cannot read", &d, e))? {
            let e = e.map_err(|e| io("cannot read", &d, e))?;
            names.insert(e.file_name().to_string_lossy().into_owned());
            let pk = e.path().join("PKGBUILD");
            if pk.is_file() {
                let text = fs::read(&pk).map_err(|e| io("cannot read", &pk, e))?;
                names.extend(pkg::recipe_pkgnames(&String::from_utf8_lossy(&text)));
            }
        }
    }
    Ok(names)
}

struct Chosen {
    path: PathBuf,
    /// the file already online (from the last publish), not a new build
    published: bool,
}

/// What this publish holds, by package name: of each package the newest build, or the published one when the newest build
/// is the same program at the same version (and the published one was not built at home). A new version (a release) is
/// always published, so every installed system shows it.
fn choose(labyrinth: &Path, repo_dir: &Path, published_dir: &Path) -> R<BTreeMap<String, Chosen>> {
    let mut recipes: Option<HashSet<String>> = None;
    let mut newest: BTreeMap<String, (String, PathBuf)> = BTreeMap::new();
    for f in packages_in(repo_dir) {
        let (name, ver, _) = pkginfo(&f)?;
        if recipes.is_none() {
            recipes = Some(with_recipe(labyrinth)?);
        }
        if !recipes.as_ref().is_some_and(|r| r.contains(&name)) {
            continue;
        }
        let newer = match newest.get(&name) {
            None => true,
            Some((v, _)) => vercmp(&ver, v)? > 0,
        };
        if newer {
            newest.insert(name, (ver, f));
        }
    }
    let mut published: HashMap<String, (String, PathBuf)> = HashMap::new();
    if published_dir.is_dir() {
        for f in packages_in(published_dir) {
            let (name, ver, _) = pkginfo(&f)?;
            published.insert(name, (ver, f));
        }
    }
    let mut chosen = BTreeMap::new();
    for (name, (ver, f)) in newest {
        let pick = match published.get(&name) {
            Some((old_ver, old_f)) if vercmp(&ver, old_ver)? <= 0 => {
                // the build number did not move although the program did: its recipe's _version line misses a folder it is
                // built from, and installed systems would never get this change
                if f != *old_f && content(&f)? != content(old_f)? {
                    say(&format!(
                        "warning: {name} {ver} differs from the published {old_ver} but is not newer: not published (does its \
                         recipe's _version line name every folder it is built from? docs/VERSIONING.md)"
                    ));
                }
                Chosen { path: old_f.clone(), published: true }
            }
            Some((old_ver, old_f))
                if pkg::release_of(&ver) == pkg::release_of(old_ver) && content(&f)? == content(old_f)? && !built_at_home(old_f)? =>
            {
                Chosen { path: old_f.clone(), published: true }
            }
            _ => Chosen { path: f, published: false },
        };
        chosen.insert(name, pick);
    }
    Ok(chosen)
}

// --- 5: signing ----------------------------------------------------------------------------------------------------------

fn stage(ctx: &Ctx, chosen: &BTreeMap<String, Chosen>, stage_dir: &Path) -> R<()> {
    if stage_dir.exists() {
        rmtree(stage_dir)?;
    }
    mkdir_new(stage_dir)?;
    let mut staged_all: Vec<OsString> = Vec::new();
    for c in chosen.values() {
        let f = &c.path;
        let raw = f.file_name().unwrap_or_default();
        let online = match raw.to_str() {
            Some(n) if pkg::safe_name(&pkg::online_name(n)) => pkg::online_name(n),
            _ => return Err(format!("error: {}: GitHub would rename this file; pacman could not find it", raw.to_string_lossy())),
        };
        let staged = stage_dir.join(&online);
        let staged_sig = stage_dir.join(format!("{online}.sig"));
        copy2(f, &staged)?;
        let mut sig_name = raw.to_os_string();
        sig_name.push(".sig");
        let sig = f.with_file_name(sig_name);
        if sig.is_file() && f.parent() != Some(stage_dir) {
            copy2(&sig, &staged_sig)?;
        }
        // A signature found beside the package may belong to an earlier build of the same file name (2026-11-08: the
        // store's apps are copied into the repository after build-local-packages.sh signed what was there): check it, and
        // sign the package itself when there is none or it does not match.
        let verify = || ctx.gpg([OsStr::new("--batch"), OsStr::new("--verify"), staged_sig.as_os_str(), staged.as_os_str()]);
        let good = staged_sig.is_file() && verify().quiet_ok()?;
        if !good {
            ctx.gpg(["--batch", "--yes", "--local-user", KEY, "--detach-sign", "--no-armor", "--output"])
                .arg(&staged_sig)
                .arg(&staged)
                .run()?;
        }
        verify().quiet()?;
        staged_all.push(staged.into_os_string());
    }
    staged_all.sort();
    let db = stage_dir.join(format!("{REPO_NAME}.db.tar.gz"));
    Cmd::new(["repo-add", "--quiet", "--sign", "--key", KEY]).arg(&db).args(&staged_all).env("GNUPGHOME", &ctx.gnupg).run()?;
    // pacman asks for labyrinth.db and labyrinth.files: repo-add makes those as links, and a release holds files only
    for base in [format!("{REPO_NAME}.db"), format!("{REPO_NAME}.files")] {
        for ext in ["", ".sig"] {
            let link = stage_dir.join(format!("{base}{ext}"));
            let target = stage_dir.join(format!("{base}.tar.gz{ext}"));
            if link.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
                fs::remove_file(&link).map_err(|e| io("cannot remove", &link, e))?;
            }
            copy2(&target, &link)?;
            fs::remove_file(&target).map_err(|e| io("cannot remove", &target, e))?;
        }
    }
    for old in glob(stage_dir, |n| n.windows(4).any(|w| w == b".old")) {
        fs::remove_file(&old).map_err(|e| io("cannot remove", &old, e))?;
    }
    Ok(())
}

// --- 6: uploading --------------------------------------------------------------------------------------------------------

fn release_notes() -> String {
    format!(
        "Labyrinth OS's package repository: Labyrinth's own software, signed with the Labyrinth key ({KEY}, in the \
         labyrinth-keyring package). Installed systems use it through [labyrinth] in /etc/pacman.conf. Not meant for \
         downloading by hand. Filled by labyrinth-publish-repo (packages/publish-repo)."
    )
}

fn release_assets() -> R<BTreeSet<String>> {
    let view = Cmd::new(["gh", "release", "view", TAG, "-R", GH_REPO, "--json", "assets"]).text_or_none()?;
    let Some(text) = view else {
        say(&format!("making the release {TAG}"));
        Cmd::new([
            "gh",
            "release",
            "create",
            TAG,
            "-R",
            GH_REPO,
            "--title",
            "Package repository",
            "--notes",
            &release_notes(),
            "--latest=false",
        ])
        .run()?;
        return Ok(BTreeSet::new());
    };
    let bad = || format!("error: gh release view {TAG} answered something else than the release's assets");
    let j: Value = serde_json::from_str(&text).map_err(|_| bad())?;
    let assets = j.get("assets").and_then(Value::as_array).ok_or_else(bad)?;
    assets.iter().map(|a| a.get("name").and_then(Value::as_str).map(str::to_string).ok_or_else(bad)).collect()
}

/// published.json's "files" as the script read them, set(...) of whatever is there.
fn previous_files(state: &Path) -> R<BTreeSet<String>> {
    if !state.is_file() {
        return Ok(BTreeSet::new());
    }
    let bad = || format!("error: {} has no list of files", shown(state));
    match read_json(state)?.get("files").ok_or_else(bad)? {
        Value::Array(a) => {
            let mut s = BTreeSet::new();
            for v in a {
                match v {
                    Value::String(n) => {
                        s.insert(n.clone());
                    }
                    Value::Array(_) | Value::Object(_) => return Err(bad()),
                    _ => {}
                }
            }
            Ok(s)
        }
        Value::String(n) => Ok(n.chars().map(String::from).collect()),
        Value::Object(o) => Ok(o.keys().cloned().collect()),
        _ => Err(bad()),
    }
}

fn upload(stage_dir: &Path, state: &Path) -> R<Vec<String>> {
    let have = release_assets()?;
    let mut files: Vec<String> = Vec::new();
    for n in names_in(stage_dir)? {
        if stage_dir.join(&n).is_file() {
            files.push(n.into_string().map_err(|n| format!("error: {} is not a name GitHub keeps", n.to_string_lossy()))?);
        }
    }
    let is_db = |n: &str| n.starts_with(&format!("{REPO_NAME}.db")) || n.starts_with(&format!("{REPO_NAME}.files"));
    let db_files: Vec<&String> = files.iter().filter(|n| is_db(n)).collect();
    let pkgs: Vec<&String> = files.iter().filter(|n| !is_db(n)).collect();
    let new: Vec<&&String> = pkgs.iter().filter(|n| !have.contains(n.as_str())).collect();
    say(&format!("uploading {} new file(s) of {}", new.len(), pkgs.len()));
    let upload = || Cmd::new(["gh", "release", "upload", TAG, "-R", GH_REPO, "--clobber"]);
    for batch in new.chunks(20) {
        upload().args(batch.iter().map(|n| stage_dir.join(n))).run()?;
    }
    say("uploading the database");
    upload().args(db_files.iter().map(|n| stage_dir.join(n))).run()?;
    // what neither this publish nor the one before it holds goes
    let previous = previous_files(state)?;
    for name in &have {
        if !files.contains(name) && !previous.contains(name) {
            Cmd::new(["gh", "release", "delete-asset", TAG, name, "-R", GH_REPO, "--yes"]).run()?;
        }
    }
    Ok(files)
}

pub fn publish(opts: &Options) -> R<()> {
    let ctx = Ctx { work: work_dir(), gnupg: gnupg_home() };
    if !ctx.gpg(["--batch", "--list-secret-keys", KEY]).quiet_ok()? {
        return Err(format!(
            "error: the signing key {KEY} is not in {} (see the README next to its backup on Proton Drive)",
            ctx.gnupg.to_string_lossy()
        ));
    }
    let work = &ctx.work;
    mkdir_p(work)?;
    let state = work.join("published.json");
    let src = work.join("src");
    let heads = fresh_copies(&src)?;
    if state.is_file() {
        let last = read_json(&state)?;
        let Value::Object(last) = last else { return Err(format!("error: {} is not a JSON object", shown(&state))) };
        let now: serde_json::Map<String, Value> = heads.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
        if last.get("heads") == Some(&Value::Object(now)) && !opts.force {
            say("nothing changed on GitHub since the last publish");
            return Ok(());
        }
    }
    let repo_dir = build(&src, work)?;
    let published_dir = work.join("published");
    let chosen = choose(&src.join("Labyrinth_OS"), &repo_dir, &published_dir)?;
    let stage_dir = work.join("stage");
    stage(&ctx, &chosen, &stage_dir)?;
    let changed: Vec<&str> = chosen.iter().filter(|(_, c)| !c.published).map(|(n, _)| n.as_str()).collect();
    let listed = if changed.is_empty() { "none".to_string() } else { changed.join(", ") };
    say(&format!("{} packages, {} new or changed: {listed}", chosen.len(), changed.len()));
    if opts.dry_run {
        say(&format!("dry run: nothing uploaded; the signed repository is in {}", shown(&stage_dir)));
        return Ok(());
    }
    let files = upload(&stage_dir, &state)?;
    // what is published now becomes what the next publish compares against
    if published_dir.exists() {
        rmtree(&published_dir)?;
    }
    copytree(&stage_dir, &published_dir)?;
    fs::write(&state, state_json(&heads, &files)).map_err(|e| io("cannot write", &state, e))?;
    say(&format!("published: https://github.com/{GH_REPO}/releases/tag/{TAG}"));
    Ok(())
}
