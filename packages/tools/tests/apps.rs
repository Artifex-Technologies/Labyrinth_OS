//! tests/test_labyrinth_apps.py's Commands tests that read the output, carried over: the program runs with a stand-in pacman
//! that reports what is installed (the Python tests passed that set to main()).
mod common;
use common::*;

const APPS: &str = env!("CARGO_BIN_EXE_labyrinth-apps");

struct World {
    _d: TempDir,
    env: Vec<(String, String)>,
}

fn world(installed: &[&str], with_pacman: bool) -> World {
    let d = TempDir::new();
    let bin = d.p("bin");
    std::fs::create_dir_all(&bin).unwrap();
    if with_pacman {
        stub(&bin, "pacman", &format!("case \"$1\" in -Qq) printf '%s\\n' {};; -Si) exit 1;; esac", installed.iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(" ")));
    }
    std::fs::create_dir_all(d.p("sync")).unwrap();
    std::fs::write(d.p("sync/core.db"), "").unwrap();
    let manifest = format!("{}/../../profile/airootfs/usr/share/labyrinth/components.json", env!("CARGO_MANIFEST_DIR"));
    let env = vec![
        ("PATH".into(), bin.clone()),
        ("LABYRINTH_COMPONENTS".into(), manifest),
        ("LABYRINTH_PACMAN_SYNC".into(), d.p("sync")),
        ("USER".into(), "tester".into()),
    ];
    World { _d: d, env }
}

fn run_apps(w: &World, args: &[&str]) -> (i32, String, String) {
    let o = run(APPS, args, &w.env);
    (code(&o), out(&o), err(&o))
}

fn has_row(out: &str, id: &str, state: &str) -> bool {
    out.lines().any(|l| l.starts_with(id) && l[id.len()..].trim_start().starts_with(state))
}

#[test]
fn list_shows_state() {
    let w = world(&["obsidian", "rclone"], true);
    let (rc, out, _) = run_apps(&w, &["list"]);
    assert_eq!(rc, 0);
    assert!(has_row(&out, "obsidian", "installed"));
    assert!(has_row(&out, "proton-drive", "partial"));
    assert!(has_row(&out, "steam", "not installed"));
}

#[test]
fn dry_run_install_skips_installed_and_runs_nothing() {
    let w = world(&["obsidian"], true);
    let (rc, out, _) = run_apps(&w, &["install", "obsidian", "spotify", "--dry-run"]);
    assert_eq!(rc, 0);
    assert!(out.contains("spotify-launcher"));
    assert!(!out.split("==>").nth(1).unwrap().contains("obsidian"));
    assert!(out.contains("pacman -S --needed"));
}

#[test]
fn proton_and_vscode_come_from_proton_and_microsoft_through_the_aur() {
    let w = world(&[], true);
    for (comp, pkg) in [("proton-pass", "proton-pass-bin"), ("proton-mail", "proton-mail-bin"), ("vscode", "visual-studio-code-bin")] {
        let (rc, out, _) = run_apps(&w, &["install", comp, "--dry-run"]);
        assert_eq!(rc, 0);
        assert!(out.contains(&format!("yay -S --needed {pkg}")), "{out}");
        assert!(!out.contains("makepkg"));
    }
}

#[test]
fn preset_expands() {
    let w = world(&[], true);
    let (_, out, _) = run_apps(&w, &["install", "--preset", "gaming", "--dry-run"]);
    assert!(out.contains("steam"));
    assert!(!out.contains("spotify"));
}

#[test]
fn nothing_to_do_when_all_installed() {
    let w = world(&["obsidian"], true);
    let (rc, out, _) = run_apps(&w, &["install", "obsidian", "--dry-run"]);
    assert_eq!((rc, out.trim()), (0, "nothing to do"));
}

#[test]
fn unknown_component_and_preset() {
    let w = world(&[], true);
    assert_eq!(run_apps(&w, &["install", "nope", "--dry-run"]).0, 2);
    assert_eq!(run_apps(&w, &["install", "--preset", "nope", "--dry-run"]).0, 2);
    assert_eq!(run_apps(&w, &["install", "--dry-run"]).0, 2);
}

#[test]
fn remove_only_touches_installed_packages() {
    let w = world(&["rclone"], true);
    let (_, out, _) = run_apps(&w, &["remove", "proton-drive", "--dry-run"]);
    assert!(out.contains("pacman -Rs rclone"));
    assert!(!out.contains("fuse3"));
}

#[test]
fn weather_enables_its_timer_and_removing_it_disables_the_timer_first() {
    let w = world(&[], true);
    let (_, out, _) = run_apps(&w, &["install", "weather", "--dry-run"]);
    assert!(out.contains("makepkg -si"));
    assert!(out.contains("systemctl --global enable yggdrasil-weather-record.timer"));
    let w = world(&["yggdrasil-weather"], true);
    let (_, out, _) = run_apps(&w, &["remove", "weather", "--dry-run"]);
    assert!(out.find("--global disable").unwrap() < out.find("pacman -Rs yggdrasil-weather").unwrap());
}

#[test]
fn without_pacman_it_explains_instead_of_crashing() {
    let w = world(&[], false);
    let (rc, _, e1) = run_apps(&w, &["install", "obsidian"]);
    assert_eq!(rc, 2);
    assert_eq!(run_apps(&w, &["install", "obsidian", "--dry-run"]).0, 0);
    assert_eq!(run_apps(&w, &["list"]).0, 0);
    assert!(e1.contains("pacman not found"));
}

#[test]
fn info_mentions_unverified_packages() {
    let w = world(&[], true);
    let (_, out, _) = run_apps(&w, &["info", "steam"]);
    assert!(out.contains("proton-cachyos"));
}
