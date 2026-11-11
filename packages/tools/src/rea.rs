//! labyrinth-rea and labyrinth-rea-setup: REA (reverse engineering for AI agents) in a bubblewrap sandbox for files that may be
//! hostile, and its installation (REA pinned, installed without its scripts; Ghidra checked against its published SHA-256), in
//! the home folder, no root. The bash scripts of the same names with the same options, output, calls and exit codes; the help
//! of labyrinth-rea is src/doc/labyrinth-rea.txt.
//!
//! bwrap, node, npm, curl, unzip and java are still those programs; the sandbox's arguments are built here as a list (never a
//! command line), Ghidra's SHA-256 is computed here (sha256sum -c).
//!
//! Fixed in both (scripts and ports): labyrinth-rea-setup took REA_VERSION, GHIDRA_TAG, GHIDRA_ZIP and GHIDRA_SHA256 from the
//! environment unchecked: REA_VERSION is a version (1.2.3) now, not any npm spec (a git URL or a file installed whatever it
//! named), the tag and the file name are plain names (no `/`, `?` or `#` in the download address), the hash is 64 hex digits.
//!
//! Not 1:1: `--samples` or `--work` without a value is a usage error (exit code 2; bash stopped on the unbound $2, exit code 1).

use crate::sh::{self, Io};

pub const DOC: &str = include_str!("doc/labyrinth-rea.txt");

fn env_or(k: &str, default: impl FnOnce() -> String) -> String {
    std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(default)
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn mkdir_err(p: &str, e: std::io::Error) -> i32 {
    eprintln!("mkdir: cannot create directory '{p}': {}", crate::py::strerror(e.raw_os_error().unwrap_or(0)));
    1
}

/// The sandbox's arguments, without the command (see the script's header for what it is and is not).
#[allow(clippy::too_many_arguments)]
pub fn bwrap_args(
    apps: &str,
    samples: &str,
    work: &str,
    home: &str,
    net: bool,
    ghidra: Option<&str>,
    jvm: Option<&str>,
    etc: &[String],
    quarantine: Option<&str>,
) -> Vec<String> {
    let mut a: Vec<String> = [
        "--die-with-parent",
        "--new-session",
        "--unshare-all",
        "--ro-bind",
        "/usr",
        "/usr",
        "--symlink",
        "usr/bin",
        "/bin",
        "--symlink",
        "usr/lib",
        "/lib",
        "--symlink",
        "usr/lib64",
        "/lib64",
        "--symlink",
        "usr/bin",
        "/sbin",
        "--ro-bind",
        "/etc/ssl",
        "/etc/ssl",
        "--ro-bind",
        "/etc/passwd",
        "/etc/passwd",
        "--ro-bind",
        "/etc/group",
        "/etc/group",
        "--ro-bind",
        "/etc/ld.so.cache",
        "/etc/ld.so.cache",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut push = |v: &[&str]| a.extend(v.iter().map(|s| s.to_string()));
    push(&[
        "--bind",
        &format!("{work}/tmp"),
        "/tmp",
        "--ro-bind",
        &format!("{apps}/rea"),
        "/opt/rea",
        "--bind",
        work,
        "/work",
        "--ro-bind",
        samples,
        "/samples",
    ]);
    // Ghidra looks up the home folder in /etc/passwd: it gets this empty one under the real name
    push(&["--bind", &format!("{work}/home"), home]);
    push(&["--setenv", "HOME", home, "--setenv", "PATH", "/usr/bin:/bin", "--setenv", "TMPDIR", "/tmp", "--setenv", "NO_COLOR", "1"]);
    if net {
        push(&["--share-net", "--ro-bind", "/etc/resolv.conf", "/etc/resolv.conf"]);
    }
    if let Some(g) = ghidra {
        push(&["--ro-bind", g, "/opt/ghidra", "--setenv", "GHIDRA_INSTALL_DIR", "/opt/ghidra"]);
    }
    if let Some(j) = jvm {
        push(&["--setenv", "JAVA_HOME", j]);
    }
    for d in etc {
        push(&["--ro-bind", d, d]);
    }
    if let Some(q) = quarantine {
        push(&["--ro-bind", q, "/quarantine"]);
    }
    a
}

pub fn rea(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-rea");
    let h = home();
    let apps = env_or("LABYRINTH_APPS", || format!("{h}/.local/share/labyrinth/apps"));
    let mut samples = env_or("LABYRINTH_REA_SAMPLES", || format!("{h}/Documents/Samples"));
    let mut work = env_or("LABYRINTH_REA_WORK", || format!("{h}/.local/share/labyrinth/rea-work"));
    let (mut net, mut exec_cmd) = (false, false);
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            o @ ("--samples" | "--work") => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("labyrinth-rea: {o} needs a folder");
                    return 2;
                };
                if o == "--samples" {
                    samples = v.clone();
                } else {
                    work = v.clone();
                }
                i += 2;
            }
            "--net" => {
                net = true;
                i += 1;
            }
            "--exec" => {
                exec_cmd = true;
                i += 1;
                break;
            }
            "-h" | "--help" => {
                print!("{DOC}");
                return 0;
            }
            _ => break,
        }
    }
    let rest = &args[i.min(args.len())..];
    if !sh::is_file(&format!("{apps}/rea/node_modules/rea-agents/scripts/rea.mjs")) {
        eprintln!("labyrinth-rea: REA is not installed: run labyrinth-rea-setup");
        return 1;
    }
    if !sh::have("bwrap") {
        eprintln!("labyrinth-rea: bubblewrap is needed (pacman -S bubblewrap)");
        return 1;
    }
    let jvm = sh::glob("/usr/lib/jvm", |n| {
        n.len() == 15 && n.starts_with("java-2") && matches!(n.as_bytes()[6], b'1'..=b'9') && n.ends_with("-openjdk")
    })
    .into_iter()
    .next();
    let ghidra =
        sh::glob(&format!("{apps}/ghidra"), |n| n.len() >= 14 && n.starts_with("ghidra_") && n.ends_with("_PUBLIC")).into_iter().next();
    for d in [format!("{work}/home"), format!("{work}/tmp"), samples.clone()] {
        if let Err(e) = sh::mkdir_p(&d) {
            return mkdir_err(&d, e);
        }
    }
    // the JDK keeps its configuration in /etc on Arch
    let mut etc: Vec<String> = sh::glob("/etc", |n| n.starts_with("java"));
    etc.extend(["/etc/fonts".to_string(), "/etc/ca-certificates".to_string()]);
    let etc: Vec<String> = etc.into_iter().filter(|d| sh::is_dir(d)).collect();
    let q = format!("{h}/.local/share/labyrinth/quarantine");
    let q = sh::is_dir(&q).then_some(q);
    let mut argv = vec!["bwrap".to_string()];
    argv.extend(bwrap_args(&apps, &samples, &work, &h, net, ghidra.as_deref(), jvm.as_deref(), &etc, q.as_deref()));
    if !exec_cmd {
        argv.extend(["node".to_string(), "/opt/rea/node_modules/rea-agents/scripts/rea.mjs".to_string()]);
    }
    argv.extend(rest.iter().cloned());
    sh::exec(&argv)
}

// ---- labyrinth-rea-setup

/// ^[0-9]+\.[0-9]+\.[0-9]+$
pub fn version_ok(v: &str) -> bool {
    let p: Vec<&str> = v.split('.').collect();
    p.len() == 3 && p.iter().all(|x| sh::digits(x))
}

/// ^[A-Za-z0-9._-]+$, and not . or ..
pub fn plain_name(v: &str) -> bool {
    !v.is_empty() && v != "." && v != ".." && v.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn setup_check(apps: &str) {
    let rea = format!("{apps}/rea/node_modules/rea-agents/scripts/rea.mjs");
    if sh::is_file(&rea) {
        println!("REA: {}", sh::output(&["node", &rea, "--version"]));
    } else {
        println!("REA: not installed");
    }
    let g = format!("{apps}/ghidra");
    if sh::is_dir(&g) {
        let first = sh::glob(&g, |_| true).into_iter().next().map(|p| sh::base(&p).to_string()).unwrap_or_default();
        println!("Ghidra: {first}");
    } else {
        println!("Ghidra: not installed");
    }
    if sh::have("java") {
        let (_, out) = sh::combined(&["java", "-version"]);
        println!("Java: {}", sh::subst(out.split(|&c| c == b'\n').next().unwrap_or(&[])));
    } else {
        println!("Java: not installed (pacman -S jdk21-openjdk)");
    }
}

pub fn setup(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-rea-setup");
    let h = home();
    let apps = env_or("LABYRINTH_APPS", || format!("{h}/.local/share/labyrinth/apps"));
    let version = env_or("REA_VERSION", || "6.5.0".into());
    let tag = env_or("GHIDRA_TAG", || "Ghidra_12.1.4_build".into());
    let zip = env_or("GHIDRA_ZIP", || "ghidra_12.1.4_PUBLIC_20260921.zip".into());
    let sha = env_or("GHIDRA_SHA256", || "ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db".into());
    if !version_ok(&version)
        || !plain_name(&tag)
        || !plain_name(&zip)
        || sha.len() != 64
        || !sha.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        eprintln!("labyrinth-rea-setup: REA_VERSION must be a version (1.2.3), GHIDRA_TAG and GHIDRA_ZIP plain names, GHIDRA_SHA256 64 hex digits");
        return 2;
    }
    if args.get(1).map(|a| a == "--check").unwrap_or(false) {
        setup_check(&apps);
        return 0;
    }
    if !sh::have("node") {
        println!("Node.js is needed (pacman -S nodejs npm)");
        return 1;
    }
    let rea_dir = format!("{apps}/rea");
    if let Err(e) = sh::mkdir_p(&rea_dir) {
        return mkdir_err(&rea_dir, e);
    }
    if let Err(e) = std::env::set_current_dir(&rea_dir) {
        eprintln!("labyrinth-rea-setup: cd: {rea_dir}: {e}");
        return 1;
    }
    if !sh::is_file("package.json") {
        let c = sh::run(&["npm", "init", "-y"], Io::Null, Io::Null);
        if c != 0 {
            return c;
        }
    }
    let c =
        sh::status(&["npm", "install", &format!("rea-agents@{version}"), "--ignore-scripts", "--omit=optional", "--no-audit", "--no-fund"]);
    if c != 0 {
        return c;
    }
    let ghidra = format!("{apps}/ghidra");
    if !sh::is_dir(&ghidra) {
        let tmpdir = env_or("TMPDIR", || "/tmp".into());
        let tmp = match sh::mkdtemp(&format!("{tmpdir}/tmp.XXXXXXXXXX")) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("mktemp: {e}");
                return 1;
            }
        };
        let code = fetch_ghidra(&tmp, &tag, &zip, &sha, &ghidra);
        let _ = std::fs::remove_dir_all(&tmp);
        if code != 0 {
            return code;
        }
    }
    setup_check(&apps);
    0
}

fn fetch_ghidra(tmp: &str, tag: &str, zip: &str, sha: &str, ghidra: &str) -> i32 {
    let file = format!("{tmp}/g.zip");
    let url = format!("https://github.com/NationalSecurityAgency/ghidra/releases/download/{tag}/{zip}");
    let c = sh::status(&["curl", "-fsSL", "-o", &file, &url]);
    if c != 0 {
        return c;
    }
    let got = {
        use sha2::{Digest, Sha256};
        std::fs::read(&file).map(|b| Sha256::digest(&b).iter().map(|x| format!("{x:02x}")).collect::<String>())
    };
    match got {
        Ok(h) if h == sha => println!("{file}: OK"),
        Ok(_) => {
            println!("{file}: FAILED");
            eprintln!("sha256sum: WARNING: 1 computed checksum did NOT match");
            println!("Ghidra's hash does not match: not installed");
            return 1;
        }
        Err(e) => {
            eprintln!("sha256sum: {file}: {e}");
            println!("Ghidra's hash does not match: not installed");
            return 1;
        }
    }
    if let Err(e) = sh::mkdir_p(ghidra) {
        return mkdir_err(ghidra, e);
    }
    sh::status(&["unzip", "-q", &file, "-d", ghidra])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks() {
        assert!(
            version_ok("6.5.0")
                && !version_ok("latest")
                && !version_ok("6.5")
                && !version_ok("git+https://x/y")
                && !version_ok("1.2.3-beta")
        );
        assert!(plain_name("Ghidra_12.1.4_build") && !plain_name("a/b") && !plain_name("..") && !plain_name("x?y") && !plain_name(""));
    }

    #[test]
    fn sandbox() {
        let a = bwrap_args("/a", "/s", "/w", "/h", true, Some("/g"), Some("/j"), &["/etc/fonts".into()], Some("/q"));
        let s = a.join(" ");
        assert!(s.starts_with("--die-with-parent --new-session --unshare-all --ro-bind /usr /usr"));
        assert!(s.contains(
            "--bind /w/tmp /tmp --ro-bind /a/rea /opt/rea --bind /w /work --ro-bind /s /samples --bind /w/home /h --setenv HOME /h"
        ));
        assert!(s.ends_with("--share-net --ro-bind /etc/resolv.conf /etc/resolv.conf --ro-bind /g /opt/ghidra --setenv GHIDRA_INSTALL_DIR /opt/ghidra --setenv JAVA_HOME /j --ro-bind /etc/fonts /etc/fonts --ro-bind /q /quarantine"));
    }
}
