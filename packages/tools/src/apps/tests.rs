//! tests/test_labyrinth_apps.py, carried over: the Manifest tests and the Commands tests that call the planning directly (the
//! ones that read the program's output are in tests/apps.rs; the Generator tests are of scripts/gen-packages.py, which stays
//! Python).
use super::*;
use crate::testutil::{env_lock, TempDir};

fn manifest_path() -> String {
    format!("{}/../../profile/airootfs/usr/share/labyrinth/components.json", env!("CARGO_MANIFEST_DIR"))
}

fn load_ok(p: &str) -> Manifest {
    match load(p) {
        Ok(m) => m,
        _ => panic!("components.json does not load"),
    }
}

#[test]
fn presets_reference_real_components() {
    let m = load_ok(&manifest_path());
    assert!(m.presets.iter().any(|(n, _)| n == "everything"));
    assert_eq!(m.presets.iter().find(|(n, _)| n == "minimal").unwrap().1, Vec::<String>::new());
}

#[test]
fn every_component_installs_something() {
    let m = load_ok(&manifest_path());
    for (i, c) in &m.comps {
        assert!(!all_packages(c).unwrap().is_empty(), "{i}");
    }
}

#[test]
fn unknown_preset_member_is_rejected() {
    let d = TempDir::new();
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(manifest_path()).unwrap()).unwrap();
    v["presets"]["bad"] = serde_json::json!(["nope"]);
    let p = d.p("c.json");
    std::fs::write(&p, v.to_string()).unwrap();
    assert!(matches!(load(&p), Err(LoadError::Said(m)) if m == "preset 'bad' names unknown component 'nope'"));
}

#[test]
fn pkgbuild_component_uses_makepkg_when_no_repository_has_it() {
    let _g = env_lock();
    let m = load_ok(&manifest_path());
    let steps = plan_install(&["limbo".to_string()], &m, &[], &|_| false, false).unwrap();
    let built: Vec<&Vec<String>> = steps.iter().filter(|s| s.2.is_some()).map(|s| &s.1).collect();
    assert_eq!(built, vec![&vec!["makepkg".to_string(), "-si".to_string()]]);
    assert_eq!(steps[1].2.as_deref(), Some("limbo-git"));
    assert!(steps[0].1.contains(&"base-devel".to_string()), "the build tools first: {:?}", steps[0].1);
}

#[test]
fn pkgbuild_component_comes_from_the_repository_and_updates_there() {
    let _g = env_lock();
    let m = load_ok(&manifest_path());
    let steps = plan_install(&["limbo".to_string()], &m, &["limbo-git".to_string()], &|n| n == "limbo-git", false).unwrap();
    assert!(!steps.iter().any(|s| s.2.is_some()), "nothing is built on this machine");
    let pacman: Vec<&Vec<String>> = steps.iter().map(|s| &s.1).filter(|a| a.contains(&"pacman".to_string())).collect();
    assert_eq!(pacman.len(), 1);
    assert!(pacman[0].contains(&"--needed".to_string()));
    assert!(pacman[0].contains(&"limbo-git".to_string()));
}

#[test]
fn a_fresh_install_fetches_the_package_lists_and_takes_builds_from_the_repository() {
    let _g = env_lock();
    let m = load_ok(&manifest_path());
    let steps = plan_install(&["limbo".to_string(), "obsidian".to_string()], &m, &[], &|_| false, true).unwrap();
    assert!(!steps.iter().any(|s| s.2.is_some()));
    let pacman: Vec<&Vec<String>> = steps.iter().map(|s| &s.1).filter(|a| a.contains(&"pacman".to_string())).collect();
    assert_eq!(pacman.len(), 1);
    assert!(pacman[0].contains(&"-Syu".to_string()));
    assert!(pacman[0].contains(&"limbo-git".to_string()));
    let d = TempDir::new();
    assert!(never_synced(Some(&d.p(""))));
    assert!(never_synced(Some(&d.p("missing"))));
    std::fs::write(d.p("core.db"), "").unwrap();
    assert!(!never_synced(Some(&d.p(""))));
}

#[test]
fn makepkg_from_agora_asks_through_its_password_box() {
    let _g = env_lock();
    let d = TempDir::new();
    std::env::remove_var("AGORA_SUDO");
    std::env::remove_var("LABYRINTH_SUDO");
    assert_eq!(makepkg_gui_args(&d.p("")).unwrap(), Vec::<String>::new());
    std::env::set_var("AGORA_SUDO", "/usr/lib/agora/sudo");
    let args = makepkg_gui_args(d.p("").trim_end_matches('/')).unwrap();
    std::env::remove_var("AGORA_SUDO");
    assert_eq!(args[..2], ["--noconfirm".to_string(), "--config".to_string()]);
    let conf = std::fs::read_to_string(&args[2]).unwrap();
    assert!(conf.contains("source /etc/makepkg.conf"));
    assert!(conf.contains("PACMAN_AUTH=(/usr/lib/agora/sudo)"));
}

#[test]
fn weather_is_optional() {
    let m = load_ok(&manifest_path());
    assert!(m.get("weather").is_some());
    for p in ["minimal", "gaming", "creative"] {
        assert!(!m.presets.iter().find(|(n, _)| n == p).unwrap().1.contains(&"weather".to_string()), "{p}");
    }
}

fn plain_env() -> std::sync::MutexGuard<'static, ()> {
    let g = env_lock();
    for k in ["AGORA_SUDO", "LABYRINTH_SUDO", "LABYRINTH_APPS"] {
        std::env::remove_var(k);
    }
    g
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// A root step as planned: behind `password_box` for a user, as it is for root (CI's container runs the tests as root).
fn root_step(password_box: &str, v: &[&str]) -> Vec<String> {
    let mut out = if py::geteuid() == 0 { vec![] } else { ids(&[password_box]) };
    out.extend(ids(v));
    out
}

#[test]
fn a_components_root_steps_are_one_setup_step_so_the_password_is_asked_once() {
    let _g = plain_env();
    let m = load_ok(&manifest_path());
    let steps = plan_install(&ids(&["waydroid"]), &m, &[], &|_| true, false).unwrap();
    assert_eq!(steps.len(), 2, "the packages, then one setup step: {steps:?}");
    assert_eq!(steps[1].1, root_step("sudo", &["/usr/bin/labyrinth-apps", "setup", "waydroid"]));
    let what = &steps[1].0;
    assert!(what.contains("waydroid init") && what.contains("systemctl enable --now waydroid-container.service"), "{what}");
    // a component with nothing to set up has no setup step
    let steps = plan_install(&ids(&["obsidian"]), &m, &[], &|_| true, false).unwrap();
    assert!(!steps.iter().any(|s| s.1.contains(&"setup".to_string())), "{steps:?}");
}

#[test]
fn the_setup_is_the_components_commands_groups_and_services_run_as_root() {
    let _g = plain_env();
    let m = load_ok(&manifest_path());
    let steps = plan_setup(&ids(&["razer", "waydroid"]), &m, "tester").unwrap();
    let argvs: Vec<String> = steps.iter().map(|s| s.1.join(" ")).collect();
    assert_eq!(
        argvs,
        [
            "waydroid init",
            "usermod -aG openrazer tester",
            "systemctl enable --now input-remapper.service",
            "systemctl enable --now waydroid-container.service",
        ],
        "in the old order (setup commands, groups, services), without sudo"
    );
    // without the user who asked (root's own shell) no one is added to a group
    let steps = plan_setup(&ids(&["razer"]), &m, "").unwrap();
    assert!(!steps.iter().any(|s| s.1[0] == "usermod"), "{steps:?}");
}

#[test]
fn removing_undoes_the_setup_first_in_one_step() {
    let _g = plain_env();
    let m = load_ok(&manifest_path());
    let steps = plan_remove(&ids(&["weather"]), &m, &ids(&["yggdrasil-weather"])).unwrap();
    assert_eq!(steps.len(), 2, "{steps:?}");
    assert_eq!(steps[0].1, root_step("sudo", &["/usr/bin/labyrinth-apps", "teardown", "weather"]));
    assert!(steps[1].1.contains(&"pacman".to_string()) && steps[1].1.contains(&"-Rs".to_string()), "{steps:?}");
    let down = plan_teardown(&ids(&["weather"]), &m, &ids(&["yggdrasil-weather"])).unwrap();
    assert_eq!(
        down.iter().map(|s| s.1.join(" ")).collect::<Vec<_>>(),
        ["systemctl --global disable yggdrasil-weather-record.timer yggdrasil-weather-nowcast.timer"]
    );
    assert!(plan_teardown(&ids(&["weather"]), &m, &[]).unwrap().is_empty(), "nothing installed, nothing to switch off");
}

#[test]
fn the_setup_step_names_this_program_where_a_test_puts_it() {
    let _g = plain_env();
    let m = load_ok(&manifest_path());
    std::env::set_var("LABYRINTH_APPS", "/x/labyrinth-apps");
    std::env::set_var("AGORA_SUDO", "/usr/lib/agora/sudo");
    let steps = plan_install(&ids(&["waydroid"]), &m, &ids(&["waydroid"]), &|_| true, false).unwrap();
    std::env::remove_var("LABYRINTH_APPS");
    std::env::remove_var("AGORA_SUDO");
    assert_eq!(steps.len(), 1, "the package is there; only the setup is left: {steps:?}");
    assert_eq!(steps[0].1, root_step("/usr/lib/agora/sudo", &["/x/labyrinth-apps", "setup", "waydroid"]));
}

#[test]
fn the_user_who_asked_is_sudos_or_pkexecs() {
    let _g = plain_env();
    std::env::set_var("SUDO_USER", "tester");
    std::env::set_var("PKEXEC_UID", "0");
    assert_eq!(asking_user(), "tester");
    std::env::remove_var("SUDO_USER");
    assert_eq!(asking_user(), "root", "run through plain pkexec (an older sudo stand-in): its PKEXEC_UID");
    std::env::set_var("PKEXEC_UID", "x");
    assert_eq!(asking_user(), "");
    std::env::remove_var("PKEXEC_UID");
    assert_eq!(asking_user(), "", "root's own shell: no one");
}
