//! tests/test_quarantine.py, carried over: labyrinth-quarantine, and the quarantine / notify decisions of
//! labyrinth-scan-downloads.
mod common;
use common::*;
use std::os::unix::fs::PermissionsExt;

const Q: &str = env!("CARGO_BIN_EXE_labyrinth-quarantine");
const SCAN: &str = env!("CARGO_BIN_EXE_labyrinth-scan-downloads");

struct Fixture {
    d: TempDir,
    home: String,
    dl: String,
    calls: String,
    env: Vec<(String, String)>,
}

impl Fixture {
    fn new() -> Fixture {
        let d = TempDir::new();
        let home = d.p("home");
        let dl = format!("{home}/Downloads");
        std::fs::create_dir_all(&dl).unwrap();
        let calls = d.p("calls.log");
        let dp = d.0.to_string_lossy().into_owned();
        let env = vec![
            ("PATH".into(), format!("{dp}:/usr/bin:/bin")),
            ("HOME".into(), home.clone()),
            ("LABYRINTH_QUARANTINE".into(), Q.into()),
            ("LABYRINTH_SCAN_SYNC".into(), "1".into()),
            ("LABYRINTH_RUN_MEDIA".into(), d.p("no-media")),
        ];
        stub(&dp, "notify-send", &format!("echo \"notify $*\" >> {calls}\necho \"$NOTIFY_ANSWER\""));
        Fixture { d, home, dl, calls, env }
    }

    fn q(&self, args: &[&str]) -> std::process::Output {
        run(Q, args, &self.env)
    }

    fn file(&self, name: &str, body: &str, folder: Option<&str>) -> String {
        let p = format!("{}/{name}", folder.unwrap_or(&self.dl));
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn scan(&self, infected: &[(&str, &str)], answer: &str, extra_conf: Option<&str>) -> std::process::Output {
        let lines: String = infected.iter().map(|(p, s)| format!("echo \"INFECTED {p}: {s}\"\n")).collect();
        let dp = self.d.0.to_string_lossy().into_owned();
        let heim = stub(&dp, "heimdalld", &format!("{lines}echo \"scanned 3 files in 1s: {} infected, 0 could not be scanned\"\nexit {}", infected.len(), if infected.is_empty() { 0 } else { 1 }));
        if let Some(c) = extra_conf {
            write(&format!("{}/.config/labyrinth/scan.conf", self.home), c);
        }
        run(SCAN, &[], &with(&with(&self.env, "HEIMDALLD", &heim), "NOTIFY_ANSWER", answer))
    }

    fn notes(&self) -> Vec<String> {
        read(&self.calls).lines().map(String::from).collect()
    }

    fn listing(&self) -> serde_json::Value {
        serde_json::from_str(&out(&self.q(&["list", "--json"]))).unwrap()
    }
}

// ---- QuarantineCli

#[test]
fn add_list_restore_delete() {
    let f = Fixture::new();
    let p = f.file("bad.exe", "evil\n", None);
    let r = f.q(&["add", &p, "--signature", "Win.Trojan.Test"]);
    assert_eq!(code(&r), 0, "{}", err(&r));
    let qid = out(&r).trim().to_string();
    assert!(!exists(&p));
    let listing = f.listing();
    let rows: Vec<(String, String, String)> = listing
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["id"].as_str().unwrap().into(), i["signature"].as_str().unwrap().into(), i["original"].as_str().unwrap().into()))
        .collect();
    assert_eq!(rows, vec![(qid.clone(), "Win.Trojan.Test".to_string(), p.clone())]);
    let payload = format!("{}/.local/share/labyrinth/quarantine/{qid}/payload", f.home);
    assert_eq!(std::fs::metadata(&payload).unwrap().permissions().mode() & 0o7777, 0o600); // not executable
    let back = out(&f.q(&["restore", &qid])).trim().to_string();
    assert_eq!(back, p);
    assert_eq!(read(&p), "evil\n");
    assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o111, 0); // comes back without the execute bit
    assert_eq!(f.listing(), serde_json::json!([]));
    assert_eq!(code(&f.q(&["allowed", &sha256_hex(b"evil\n")])), 0); // remembered
    assert_eq!(code(&f.q(&["allowed", &"0".repeat(64)])), 1);
}

#[test]
fn restore_never_overwrites() {
    let f = Fixture::new();
    let p = f.file("a.bin", "evil\n", None);
    let qid = out(&f.q(&["add", &p, "--signature", "X"])).trim().to_string();
    f.file("a.bin", "new file\n", None);
    let back = out(&f.q(&["restore", &qid])).trim().to_string();
    assert_eq!(back, format!("{p}.restored"));
    assert_eq!(read(&p), "new file\n");
}

#[test]
fn delete_and_bad_ids() {
    let f = Fixture::new();
    let p = f.file("b.bin", "evil\n", None);
    let qid = out(&f.q(&["add", &p, "--signature", "X"])).trim().to_string();
    assert_eq!(code(&f.q(&["delete", &qid])), 0);
    assert_eq!(out(&f.q(&["list", "--json"])).trim(), "[]");
    for bad in ["../../etc", "zzzz", "", "123456789012"] {
        assert_ne!(code(&f.q(&["restore", bad])), 0);
        assert_ne!(code(&f.q(&["delete", bad])), 0);
    }
}

#[test]
fn refuses_links_folders_and_missing_files() {
    let f = Fixture::new();
    let target = f.file("t.bin", "evil\n", None);
    let link = format!("{}/link", f.dl);
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert_ne!(code(&f.q(&["add", &link, "--signature", "X"])), 0);
    assert!(exists(&target));
    assert_ne!(code(&f.q(&["add", &f.dl, "--signature", "X"])), 0);
    assert_ne!(code(&f.q(&["add", &format!("{}/nope", f.dl), "--signature", "X"])), 0);
}

// ---- ScanDecisions

#[test]
fn a_certain_find_in_downloads_is_quarantined_and_announced() {
    let f = Fixture::new();
    let p = f.file("invoice.exe", "evil\n", None);
    let r = f.scan(&[(&p, "Win.Trojan.Agent-123")], "", None);
    assert_eq!(code(&r), 1);
    assert!(!exists(&p));
    let note = f.notes().join(" ");
    assert!(note.contains("Malware quarantined"));
    assert!(note.contains("-A restore=Restore"));
    assert!(note.contains("-A delete=Delete"));
    assert!(read(&format!("{}/.local/state/labyrinth/scan.log", f.home)).contains("QUARANTINED"));
}

#[test]
fn restore_from_the_notification_puts_it_back_and_it_is_not_caught_again() {
    let f = Fixture::new();
    let p = f.file("tool.exe", "evil\n", None);
    f.scan(&[(&p, "Win.Trojan.Agent-123")], "restore", None);
    assert!(exists(&p)); // the notification's Restore button
    f.scan(&[(&p, "Win.Trojan.Agent-123")], "", None); // same content: you said keep it
    assert!(exists(&p));
    assert_eq!(f.notes().iter().filter(|n| n.contains("Malware quarantined")).count(), 1);
}

#[test]
fn delete_from_the_notification() {
    let f = Fixture::new();
    let p = f.file("x.exe", "evil\n", None);
    f.scan(&[(&p, "Win.Trojan.Agent-123")], "delete", None);
    assert!(!exists(&p));
    assert_eq!(f.listing(), serde_json::json!([]));
}

#[test]
fn a_guess_is_only_a_warning() {
    let f = Fixture::new();
    for sig in ["PUA.Win.Packer.Upx", "Heuristics.Encrypted.PDF", "Win.Test.EICAR_HDB-1x", "Html.Phishing.Bank-9"] {
        let p = f.file("maybe.bin", sig, None);
        f.scan(&[(&p, sig)], "", None);
        assert!(exists(&p), "{sig}");
    }
    assert!(f.notes().join(" ").contains("Possible malware found"));
}

#[test]
fn mail_attachment_cache_and_drives_are_watched() {
    let f = Fixture::new();
    let cache = format!("{}/.cache/thunderbird/profile", f.home);
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(f.d.p("no-media")).unwrap();
    let p = f.file("attach.scr", "evil\n", Some(&cache));
    f.scan(&[(&p, "Win.Trojan.Agent-9")], "", None);
    assert!(!exists(&p)); // outside Downloads, still quarantined
    assert!(read(&f.d.p("heimdalld")).contains("scanned"));
}

#[test]
fn not_yours_or_outside_the_watched_places_is_not_moved() {
    let f = Fixture::new();
    let elsewhere = f.d.p("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let p = f.file("bad.exe", "evil\n", Some(&elsewhere));
    f.scan(&[(&p, "Win.Trojan.Agent-9")], "", None);
    assert!(exists(&p));
    assert!(f.notes().join(" ").contains("Possible malware found"));
}

#[test]
fn notify_only_setting() {
    let f = Fixture::new();
    let p = f.file("bad2.exe", "evil\n", None);
    f.scan(&[(&p, "Win.Trojan.Agent-9")], "", Some("action = notify\n"));
    assert!(exists(&p));
    assert!(!f.notes().join(" ").contains("quarantined"));
}

// ---- beyond the Python tests

#[test]
fn help_and_bad_arguments_as_the_script() {
    let f = Fixture::new();
    let r = f.q(&[]);
    assert_eq!(code(&r), 2);
    assert!(out(&r).starts_with("labyrinth-quarantine: a safe place"));
    assert!(out(&r).ends_with("do not own.\n\n"), "print(__doc__): the docstring's own newline and print's");
    assert_eq!(code(&f.q(&["--help"])), 0);
    let r = f.q(&["restore"]);
    assert_eq!((code(&r), err(&r)), (2, "labyrinth-quarantine: bad arguments (see --help)\n".to_string()));
    let r = f.q(&["add", &format!("{}/nope", f.dl)]);
    assert_eq!(code(&r), 1);
    assert_eq!(err(&r), format!("labyrinth-quarantine: [Errno 2] No such file or directory: '{}/nope'\n", f.dl));
}

#[test]
fn a_restored_file_comes_back_as_restored2_when_both_names_are_taken() {
    let f = Fixture::new();
    let p = f.file("c.bin", "evil\n", None);
    let qid = out(&f.q(&["add", &p, "--signature", "X"])).trim().to_string();
    f.file("c.bin", "1", None);
    f.file("c.bin.restored", "2", None);
    assert_eq!(out(&f.q(&["restore", &qid])).trim(), format!("{p}.restored2"));
    let allow = read(&format!("{}/.config/labyrinth/scan-allow", f.home));
    assert!(allow.starts_with(&format!("{}  # restored ", sha256_hex(b"evil\n"))) && allow.ends_with(": c.bin\n"), "{allow}");
}

#[test]
fn the_text_listing_and_the_info_file() {
    let f = Fixture::new();
    let p = f.file("d.bin", "evil\n", None);
    let qid = out(&f.q(&["add", &p, "--signature", "Sig.One"])).trim().to_string();
    let info = read(&format!("{}/.local/share/labyrinth/quarantine/{qid}/info.json", f.home));
    assert!(info.starts_with(&format!("{{\n \"id\": \"{qid}\",\n \"original\": ")), "json.dump(indent=1): {info}");
    assert!(info.contains("\"mode\": 493,\n") && info.ends_with("\"source\": \"scan\"\n}"), "{info}");
    let line = out(&f.q(&["list"]));
    assert!(line.starts_with(&format!("{qid}  ")) && line.ends_with(&format!("  Sig.One  {p}\n")), "{line}");
}
