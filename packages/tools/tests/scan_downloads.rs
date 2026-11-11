//! tests/test_scan_downloads.py, carried over: labyrinth-scan-downloads with stand-ins for heimdalld and notify-send.
mod common;
use common::*;

const SCAN: &str = env!("CARGO_BIN_EXE_labyrinth-scan-downloads");

struct Scan {
    d: TempDir,
    home: String,
    log: String,
    heim: String,
    env: Vec<(String, String)>,
}

impl Scan {
    fn new() -> Scan {
        let d = TempDir::new();
        let dp = d.p("");
        let dp = dp.trim_end_matches('/').to_string();
        let home = d.p("home");
        std::fs::create_dir_all(format!("{home}/Downloads")).unwrap();
        let log = d.p("calls.log");
        // heimdalld: records its arguments, reports one infected file and exits 1 like the real one
        let heim = stub(&dp, "heimdalld", &format!("echo \"heimdalld $*\" >> {log}\necho \"INFECTED /home/u/Downloads/bad.exe: Test.Virus\"\necho \"scanned 2 files in 1s: 1 infected, 0 could not be scanned\"\nexit 1"));
        stub(&dp, "notify-send", &format!("echo \"notify $*\" >> {log}"));
        let env = vec![
            ("PATH".into(), format!("{dp}:/usr/bin:/bin")),
            ("HOME".into(), home.clone()),
            ("HEIMDALLD".into(), heim.clone()),
            ("LABYRINTH_RUN_MEDIA".into(), d.p("no-media")),
            ("LABYRINTH_SCAN_SYNC".into(), "1".into()),
        ];
        Scan { d, home, log, heim, env }
    }

    fn calls(&self) -> Vec<String> {
        read(&self.log).lines().map(String::from).collect()
    }

    fn notes(&self) -> Vec<String> {
        self.calls().into_iter().filter(|c| c.starts_with("notify")).collect()
    }
}

#[test]
fn a_find_is_notified_once() {
    let s = Scan::new();
    let r = run(SCAN, &[], &s.env);
    assert_eq!(code(&r), 1);
    let notes = s.notes();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("Possible malware found"));
    assert!(notes[0].contains("bad.exe"));
    run(SCAN, &[], &s.env); // the same file again: no second notification
    assert_eq!(s.notes().len(), 1);
}

#[test]
fn arguments_carry_state_exclusions_and_folders() {
    let s = Scan::new();
    let conf = format!("{}/.config/labyrinth", s.home);
    write(&format!("{conf}/scan-exclude"), "# comment\n*.iso\n~/Downloads/big\n");
    let extra = format!("{}/Extra", s.home);
    std::fs::create_dir_all(&extra).unwrap();
    write(&format!("{conf}/scan-folders"), &format!("{extra}\n"));
    run(SCAN, &[], &s.env);
    let args = s.calls().into_iter().find(|c| c.starts_with("heimdalld")).unwrap();
    assert!(args.contains("scan-files --state"));
    assert!(args.contains("--exclude *.iso"));
    assert!(args.contains(&format!("--exclude {}/Downloads/big", s.home)));
    assert!(args.ends_with(&format!("{}/Downloads {extra}", s.home)), "{args}");
}

#[test]
fn the_weekly_full_scan_covers_home_but_caches_and_reports_only() {
    let s = Scan::new();
    let r = run(SCAN, &["--full"], &s.env);
    assert_eq!(code(&r), 1);
    let args = s.calls().into_iter().find(|c| c.starts_with("heimdalld")).unwrap();
    assert!(args.contains("scan-full-state.json"), "its own state: the Downloads scan's is not mixed in");
    assert!(args.contains("--max-mb 100"));
    for e in
        [format!("{}/.cache", s.home), format!("{}/.local/share/Steam", s.home), "[n]ode_modules".into(), "[.]git".into(), "*.iso".into()]
    {
        assert!(args.contains(&format!("--exclude {e}")), "{e}");
    }
    assert!(args.ends_with(&format!(" {}", s.home)), "{args}");
    // the stand-in's find lies outside this home's download places: a warning, nothing moved
    assert!(s.notes().iter().any(|n| n.contains("Possible malware found")));
}

#[test]
fn a_scanner_that_cannot_run_is_quiet() {
    let s = Scan::new();
    let broken = stub(s.d.p("").trim_end_matches('/'), "heimdalld2", "echo \"heimdalld scan-files: ClamAV is not installed\" >&2; exit 2");
    let r = run(SCAN, &[], &with(&s.env, "HEIMDALLD", &broken));
    assert_eq!(code(&r), 0);
    assert!(s.notes().is_empty());
    let log = read(&format!("{}/.local/state/labyrinth/scan.log", s.home));
    assert!(log.contains("scan did not run"));
    let _ = &s.heim;
}

#[test]
fn no_downloads_folder_is_not_an_error() {
    let s = Scan::new();
    std::fs::remove_dir(format!("{}/Downloads", s.home)).unwrap();
    assert_eq!(code(&run(SCAN, &[], &s.env)), 0);
}

// ---- beyond the Python tests

#[test]
fn a_scanner_that_is_missing_is_logged_as_python_said_it() {
    let s = Scan::new();
    let r = run(SCAN, &[], &with(&s.env, "HEIMDALLD", "no-such-heimdalld"));
    assert_eq!(code(&r), 0);
    let log = read(&format!("{}/.local/state/labyrinth/scan.log", s.home));
    assert!(log.contains("scan did not run: [Errno 2] No such file or directory: 'no-such-heimdalld'"), "{log}");
}

#[test]
fn the_watcher_scans_a_folder_after_new_files_stop_arriving() {
    let s = Scan::new();
    let mut child = std::process::Command::new(SCAN)
        .arg("--watch")
        .env_clear()
        .envs(s.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    std::fs::create_dir_all(format!("{}/Downloads/sub", s.home)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    write(&format!("{}/Downloads/sub/new.exe", s.home), "x");
    let mut scanned = false;
    for _ in 0..80 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if s.calls().iter().any(|c| c.starts_with("heimdalld")) {
            scanned = true;
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    assert!(scanned, "a scan within 8 s of the new file");
    let args = s.calls().into_iter().find(|c| c.starts_with("heimdalld")).unwrap();
    assert!(args.ends_with(&format!(" {}/Downloads", s.home)), "the watched place, not the sub-folder: {args}");
    let log = read(&format!("{}/.local/state/labyrinth/scan.log", s.home));
    assert!(log.contains("watching 1 folder(s) for new files"), "{log}");
}
