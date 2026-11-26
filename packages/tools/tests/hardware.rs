//! labyrinth-hardware-setup on made-up computers: a made-up /sys and kernel list, and stand-ins for pacman and systemctl that
//! write down what they were asked.
mod common;
use common::*;

const HW: &str = env!("CARGO_BIN_EXE_labyrinth-hardware-setup");

struct World {
    d: TempDir,
    env: Vec<(String, String)>,
}

/// `chips`: (vendor, device) of each display controller; `installed`: what pacman -Q finds; `install_works`: pacman -S.
fn world(chips: &[(&str, &str)], installed: &[&str], install_works: bool) -> World {
    let d = TempDir::new();
    for (i, (v, p)) in chips.iter().enumerate() {
        let dev = d.p(&format!("sys/bus/pci/devices/0000:0{i}:00.0"));
        write(&format!("{dev}/class"), "0x030000\n");
        write(&format!("{dev}/vendor"), &format!("{v}\n"));
        write(&format!("{dev}/device"), &format!("{p}\n"));
    }
    write(&d.p("modules/7.3.5-2-cachyos/pkgbase"), "linux-cachyos\n");
    let bin = d.p("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let calls = d.p("calls");
    let have = installed.iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(" ");
    let s_code = if install_works { 0 } else { 1 };
    stub(
        &bin,
        "pacman",
        &format!(
            "case \"$1\" in\n  -Q) for p in {have}; do [ \"$p\" = \"$2\" ] && exit 0; done; exit 1;;\n  -S) echo \"pacman $*\" >> {calls}; exit {s_code};;\n  *) echo \"pacman $*\" >> {calls};;\nesac"
        ),
    );
    stub(&bin, "systemctl", &format!("echo \"systemctl $*\" >> {calls}"));
    stub(&bin, "mkinitcpio", &format!("echo \"mkinitcpio $*\" >> {calls}"));
    let env = vec![
        ("PATH".into(), bin),
        ("LABYRINTH_HW_SYS".into(), d.p("sys")),
        ("LABYRINTH_HW_MODULES".into(), d.p("modules")),
        ("LABYRINTH_HW_RECORD".into(), d.p("record")),
    ];
    World { d, env }
}

impl World {
    fn run(&self, args: &[&str]) -> String {
        let o = run(HW, args, &self.env);
        assert_eq!(code(&o), 0, "{}", err(&o));
        out(&o)
    }

    fn calls(&self) -> Vec<String> {
        read(&self.d.p("calls")).lines().map(String::from).collect()
    }
}

const NVIDIA: (&str, &str) = ("0x10de", "0x1e07");
const INTEL: (&str, &str) = ("0x8086", "0xa7a0");

/// The laptop: Intel graphics only, with NVIDIA's libraries a gaming set-up put there and Steam installed.
#[test]
fn intel_only_gets_its_32_bit_driver_first_and_loses_nvidias() {
    let w = world(
        &[INTEL],
        &["steam", "nvidia-utils", "lib32-nvidia-utils", "linux-cachyos-nvidia-open", "vulkan-intel", "intel-media-driver"],
        true,
    );
    w.run(&[]);
    let calls = w.calls();
    let install =
        calls.iter().position(|c| c == "pacman -S --needed --noconfirm lib32-vulkan-intel").unwrap_or_else(|| panic!("{calls:?}"));
    let off = calls.iter().position(|c| c.starts_with("systemctl disable nvidia-suspend.service")).expect("the sleep services off");
    let remove = calls
        .iter()
        .position(|c| c == "pacman -Rns --noconfirm lib32-nvidia-utils linux-cachyos-nvidia-open nvidia-utils")
        .unwrap_or_else(|| panic!("{calls:?}"));
    assert!(install < off && off < remove, "install, then the services, then the removal: {calls:?}");
    assert_eq!(read(&w.d.p("record")), "0x8086:0xa7a0\n");
}

#[test]
fn the_start_does_nothing_while_the_chips_match_the_record() {
    let w = world(&[NVIDIA], &["nvidia-utils", "nvidia-settings", "linux-cachyos-nvidia-open"], true);
    write(&w.d.p("record"), "0x10de:0x1e07\n");
    assert_eq!(w.run(&["--if-changed"]), "");
    assert!(w.calls().is_empty());
    // the card swapped for Intel's: the next start acts
    let w2 = world(&[INTEL], &["nvidia-utils"], true);
    write(&w2.d.p("record"), "0x10de:0x1e07\n");
    assert!(w2.run(&["--if-changed"]).contains("removing the drivers no chip here needs: nvidia-utils"));
    assert_eq!(w2.calls().last().map(String::as_str), Some("mkinitcpio -P"), "the start-up images follow the new chips");
}

#[test]
fn without_the_internet_nothing_is_removed_and_the_next_start_tries_again() {
    let w = world(&[INTEL], &["nvidia-utils"], false);
    let said = w.run(&["--if-changed"]);
    assert!(said.contains("could not install them (no internet?)"), "{said}");
    assert!(!w.calls().iter().any(|c| c.starts_with("pacman -Rns")), "{:?}", w.calls());
    assert!(!exists(&w.d.p("record")), "no record: the next start tries again");
}

#[test]
fn a_dry_run_changes_nothing() {
    let w = world(&[NVIDIA, INTEL], &["vulkan-radeon"], true);
    let said = w.run(&["--dry-run"]);
    assert!(said.contains("two graphics chips: GPU switching on"), "{said}");
    assert!(said.contains("would run: pacman -Rns --noconfirm vulkan-radeon"), "{said}");
    assert!(w.calls().is_empty());
    assert!(!exists(&w.d.p("record")));
}
