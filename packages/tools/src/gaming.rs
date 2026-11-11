//! labyrinth-gaming check|setup: are Steam and Proton installed and working (check: PASS/WARN/FAIL per item, exit code 1 on
//! any FAIL), and install what is missing, Steam Play for all titles, then check (setup). The bash script of the same name with
//! the same output, calls and exit codes.
//!
//! pacman, sudo, nvidia-smi, vulkaninfo and steam_playall are still those programs. Read here: the package list's answer, /etc/
//! pacman.conf, /sys and /proc, the open-file limit (getrlimit, the script's `ulimit -Hn`), the Proton folders (ls | xargs
//! basename), Steam's config.vdf (the script ran Python for it), whether Steam runs (/proc/PID/comm, the script's pgrep -x).
//!
//! Fixed in both (script and port): `setup` looked for Proton-CachyOS with one `ls` over both places, which fails when either
//! has none, so the system-wide Proton-CachyOS (the proton-cachyos package) was never chosen; each place is looked at on its own
//! now (check had been fixed so already).
//!
//! Not 1:1: a Proton folder name with two spaces in a row or a quote is shown as it is (xargs split names at blanks and
//! stopped at quotes); nvidia-smi -L runs once for the check and its line (twice in the script); a limit that is not a number
//! is said without bash's line number.

use crate::sh::{self, Io};

const REQUIRED: [&str; 7] =
    ["steam", "lib32-nvidia-utils", "nvidia-utils", "lib32-vulkan-icd-loader", "vulkan-icd-loader", "gamemode", "mangohud"];
const OPTIONAL: [&str; 4] = ["proton-cachyos", "protonup-qt", "protontricks", "umu-launcher"];
const HELPER: &str = "/usr/bin/steam_playall";
const SYSTEM_TOOLS: &str = "/usr/share/steam/compatibilitytools.d";

fn pkg(p: &str) -> bool {
    sh::quiet(&["pacman", "-Q", p])
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

/// Steam's data folder: the first of ~/.local/share/Steam and ~/.steam/steam that has steamapps.
fn steam_root() -> Option<String> {
    let h = home();
    [format!("{h}/.local/share/Steam"), format!("{h}/.steam/steam")].into_iter().find(|d| sh::is_dir(&format!("{d}/steamapps")))
}

/// Is a process named exactly `name` running (pgrep -x: the kernel's name of the process)?
pub fn running(name: &str) -> bool {
    sh::processes().iter().any(|pid| {
        std::fs::read(format!("/proc/{pid}/comm")).map(|c| c.strip_suffix(b"\n").unwrap_or(&c) == name.as_bytes()).unwrap_or(false)
    })
}

/// Python's re.search(r'"CompatToolMapping"\s*\{', t) and then r'^\s*"0"\s*$' (multi-line) in what follows: Steam Play is on
/// for all titles. `t` as Python read it (universal newlines).
pub fn play_all_on(t: &str) -> bool {
    let ws = crate::pycompat::isspace;
    let mut from = 0;
    let rest = loop {
        let Some(i) = t[from..].find("\"CompatToolMapping\"") else { return false };
        let after = from + i + "\"CompatToolMapping\"".len();
        let j = after + t[after..].len() - t[after..].trim_start_matches(ws).len();
        if t[j..].starts_with('{') {
            break &t[j + 1..];
        }
        from = after;
    };
    let mut start = 0;
    while let Some(i) = rest[start..].find("\"0\"") {
        let q = start + i;
        let before = &rest[..q];
        let run = &before[before.trim_end_matches(ws).len()..];
        let line_start = run.len() == before.len() || run.contains('\n');
        let after = &rest[q + 3..];
        let trail = &after[..after.len() - after.trim_start_matches(ws).len()];
        let line_end = trail.len() == after.len() || trail.contains('\n');
        if line_start && line_end {
            return true;
        }
        start = q + 1;
    }
    false
}

/// The names in these folders matching a test, each folder on its own, in ls's order (the script's ls -d ... | xargs basename).
fn names_in(dirs: &[&str], test: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out = vec![];
    for d in dirs {
        out.extend(sh::glob(d, &test).iter().map(|p| sh::base(p).to_string()));
    }
    out
}

struct Check {
    fails: usize,
}

impl Check {
    fn pass(&self, m: &str) {
        println!("  [PASS] {m}");
    }
    fn warn(&self, m: &str) {
        println!("  [WARN] {m}");
    }
    fn fail(&mut self, m: &str) {
        println!("  [FAIL] {m}");
        self.fails += 1;
    }
}

fn check() -> i32 {
    let mut c = Check { fails: 0 };
    println!("Packages");
    for p in REQUIRED {
        if pkg(p) {
            c.pass(&format!("{p} installed"));
        } else {
            c.fail(&format!("{p} missing"));
        }
    }
    for p in OPTIONAL {
        if pkg(p) {
            c.pass(&format!("{p} installed"));
        } else {
            c.warn(&format!("{p} not installed (optional)"));
        }
    }

    println!("32-bit support (Proton needs it)");
    if sh::grep("/etc/pacman.conf", |l| l.starts_with(b"[multilib]")) {
        c.pass("multilib repo enabled");
    } else {
        c.fail("multilib repo not enabled in /etc/pacman.conf");
    }

    println!("GPU / Vulkan");
    let gpu = if sh::have("nvidia-smi") { Some(sh::capture(&["nvidia-smi", "-L"], Io::Null)) } else { None };
    match gpu {
        Some((0, out)) => c.pass(&format!("NVIDIA driver loaded: {}", sh::subst(out.split(|&b| b == b'\n').next().unwrap_or(&[])))),
        _ => c.fail("nvidia-smi cannot talk to the driver"),
    }
    if sh::have("vulkaninfo") {
        let (_, out) = sh::capture(&["vulkaninfo", "--summary"], Io::Null);
        let seen = String::from_utf8_lossy(&out).split('\n').any(|l| {
            let l = l.to_lowercase();
            l.find("devicename").map(|i| l[i..].contains("nvidia")).unwrap_or(false)
        });
        if seen {
            c.pass("Vulkan sees the NVIDIA GPU");
        } else {
            c.fail("Vulkan does not list an NVIDIA device");
        }
    } else {
        c.warn("vulkaninfo not installed (package vulkan-tools); cannot check Vulkan");
    }
    let modeset = "/sys/module/nvidia_drm/parameters/modeset";
    if sh::is_file(modeset) {
        if sh::cat(modeset).as_deref() == Some("Y") {
            c.pass("nvidia_drm.modeset=1");
        } else {
            c.warn("nvidia_drm.modeset is off (Wayland needs it)");
        }
    }

    println!("Kernel limits some games need");
    let mm = sh::cat("/proc/sys/vm/max_map_count").unwrap_or_else(|| "0".into());
    match test_ge(&mm, 1048576) {
        Some(true) => c.pass(&format!("vm.max_map_count={mm}")),
        _ => c.warn(&format!("vm.max_map_count={mm} (some games crash below 1048576)")),
    }
    let mut rl = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    let nf = if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) } != 0 {
        "0".to_string()
    } else if rl.rlim_max == libc::RLIM_INFINITY {
        "unlimited".to_string()
    } else {
        rl.rlim_max.to_string()
    };
    if nf == "unlimited" || test_ge(&nf, 524288) == Some(true) {
        c.pass(&format!("open-file hard limit {nf} (esync ok)"));
    } else {
        c.warn(&format!("open-file hard limit {nf} (esync wants 524288)"));
    }

    println!("Steam and Proton");
    match steam_root() {
        None => c.warn("Steam has not been run yet (no steamapps folder). Start Steam once, sign in, then re-run."),
        Some(root) => {
            c.pass(&format!("Steam data at {root}"));
            let proton = names_in(&[&format!("{root}/steamapps/common")], |n| n.starts_with("Proton"));
            if proton.is_empty() {
                c.warn("no Valve Proton downloaded yet (Steam fetches it the first time a game needs it)");
            } else {
                c.pass(&format!("Valve Proton present: {}", spaced(&proton)));
            }
            let extra = names_in(&[&format!("{root}/compatibilitytools.d"), SYSTEM_TOOLS], |_| true);
            if extra.is_empty() {
                c.warn("no extra tools (Proton-CachyOS/GE); optional");
            } else {
                c.pass(&format!("extra compatibility tools installed: {}", spaced(&extra)));
            }
            let cfg = format!("{root}/config/config.vdf");
            let on = sh::is_file(&cfg)
                && std::fs::read(&cfg)
                    .ok()
                    .and_then(|b| String::from_utf8(b).ok())
                    .map(|t| play_all_on(&crate::py::universal(&t)))
                    .unwrap_or(false);
            if on {
                c.pass("Steam Play enabled for all titles");
            } else {
                c.warn("Steam Play for all titles not enabled yet (run: labyrinth-gaming setup)");
            }
        }
    }
    if running("steam") {
        println!("  (Steam is running)");
    }
    println!();
    if c.fails == 0 {
        println!("Result: OK");
        0
    } else {
        println!("Result: {} FAIL", c.fails);
        1
    }
}

/// `[ "$x" -ge N ]`: None when x is not an integer (test says so on standard error).
fn test_ge(x: &str, n: i64) -> Option<bool> {
    let t = x.trim_matches([' ', '\t', '\n']);
    let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
    match (sh::digits(digits), t.parse::<i64>()) {
        (true, Ok(v)) => Some(v >= n),
        _ => {
            eprintln!("labyrinth-gaming: [: {x}: integer expression expected");
            None
        }
    }
}

/// The names joined by spaces, with the trailing one the script's `tr '\n' ' '` left.
fn spaced(names: &[String]) -> String {
    names.iter().map(|n| format!("{n} ")).collect()
}

fn setup() -> i32 {
    let missing: Vec<&str> = REQUIRED.iter().chain(OPTIONAL.iter()).copied().filter(|p| !pkg(p)).collect();
    if !missing.is_empty() {
        println!("Installing: {}", missing.join(" "));
        let mut argv = vec!["sudo", "pacman", "-S", "--needed"];
        argv.extend(&missing);
        if sh::status(&argv) != 0 {
            println!("pacman failed for some packages; see output above");
        }
    }
    match steam_root() {
        None => println!("Steam has not been run yet: start it once and sign in, then run this again to enable Steam Play."),
        Some(_) if running("steam") => {
            println!("Steam is running: close it fully, then run setup again so its config can be edited safely.")
        }
        Some(root) => {
            let cfg = format!("{root}/config/config.vdf");
            if sh::is_file(&cfg) && sh::executable(HELPER) {
                // Proton-CachyOS when there is one (yours first, then the system's), else Valve's Proton Experimental
                let user = format!("{root}/compatibilitytools.d");
                let mut found = sh::glob(&user, |n| n.starts_with("proton-cachyos"));
                found.extend(sh::glob(SYSTEM_TOOLS, |n| n.starts_with("proton-cachyos")));
                let tool = found.first().map(|p| sh::base(p).to_string()).unwrap_or_else(|| "proton_experimental".into());
                sh::status(&[HELPER, &cfg, &tool]);
            }
        }
    }
    println!();
    check()
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-gaming");
    match args.get(1).map(|s| s.as_str()).filter(|s| !s.is_empty()).unwrap_or("check") {
        "check" => check(),
        "setup" => setup(),
        _ => {
            println!("usage: labyrinth-gaming check|setup");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::play_all_on;

    #[test]
    fn steam_play() {
        assert!(play_all_on("\"CompatToolMapping\"\n{\n\t\"0\"\n\t{\n}\n}"));
        assert!(play_all_on("\"CompatToolMapping\"{\"0\"\n"));
        assert!(play_all_on("\"CompatToolMapping\" \n {\n\"0\""));
        assert!(!play_all_on("\"CompatToolMapping\"\n{\n\"620\"\n{\n\"name\" \"0\"\n}\n}"));
        assert!(!play_all_on("\"0\"\n\"CompatToolMapping\"\n{\n}"));
        assert!(!play_all_on("\"CompatToolMapping\" x {\n\"0\"\n"));
        assert!(play_all_on("\"CompatToolMapping\" x\n\"CompatToolMapping\"{\n  \"0\"  \n"));
        assert!(!play_all_on("\"CompatToolMapping\"{\n x\"0\"\n"));
        assert!(!play_all_on("\"CompatToolMapping\"{\n\"0\" x\n"));
    }
}
