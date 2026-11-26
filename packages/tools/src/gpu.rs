//! The graphics drivers a computer needs, from the chips in it: every display controller (PCI class 0x03) by its vendor.
//! One place decides, for labyrinth-hardware-setup (at install, and at a start where the chips changed), labyrinth-gaming
//! and the Steam component. Mesa is always kept (it also draws with the processor alone); each vendor adds its own Vulkan
//! and video drivers, NVIDIA its kernel module for every installed kernel; the 32-bit ones only where Steam is installed;
//! GPU switching only with two chips. Everything another vendor would need is unwanted.

use std::collections::BTreeSet;

const NVIDIA: &str = "0x10de";
const AMD: &str = "0x1002";
const INTEL: &str = "0x8086";

/// The graphics chips found, by vendor.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chips {
    pub nvidia: bool,
    pub amd: bool,
    pub intel: bool,
}

impl Chips {
    /// Two vendors' chips: a laptop's own graphics beside a separate card.
    pub fn two(&self) -> bool {
        [self.nvidia, self.amd, self.intel].iter().filter(|x| **x).count() > 1
    }
}

/// Every display controller under `sys` (/sys, or a made-up one in tests) as "vendor:device", sorted: what the boot-time
/// check compares with its record.
pub fn controllers(sys: &str) -> Vec<String> {
    let mut out: Vec<String> = crate::sh::glob(&format!("{sys}/bus/pci/devices"), |_| true)
        .into_iter()
        .filter(|d| crate::sh::cat(&format!("{d}/class")).unwrap_or_default().starts_with("0x03"))
        .map(|d| {
            let v = crate::sh::cat(&format!("{d}/vendor")).unwrap_or_default();
            let p = crate::sh::cat(&format!("{d}/device")).unwrap_or_default();
            format!("{}:{}", v.trim(), p.trim())
        })
        .collect();
    out.sort();
    out
}

/// The vendors among the controllers.
pub fn chips(controllers: &[String]) -> Chips {
    let has = |v: &str| controllers.iter().any(|c| c.split(':').next() == Some(v));
    Chips { nvidia: has(NVIDIA), amd: has(AMD), intel: has(INTEL) }
}

/// The kernels installed, by package base (each /usr/lib/modules/*/pkgbase, as labyrinth-install-cleanup reads them).
pub fn kernels(modules: &str) -> Vec<String> {
    let mut out: Vec<String> = crate::sh::glob(modules, |_| true)
        .into_iter()
        .filter_map(|d| crate::sh::cat(&format!("{d}/pkgbase")))
        .map(|b| b.trim().to_string())
        .filter(|b| crate::install_cleanup::pkgbase_ok(b))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// One vendor's packages; `bits32`: with the 32-bit ones (Steam is installed).
fn vendor(which: &str, kernels: &[String], bits32: bool) -> Vec<String> {
    let mut v: Vec<String> = match which {
        NVIDIA => {
            let mut v: Vec<String> = kernels.iter().map(|k| format!("{k}-nvidia-open")).collect();
            v.extend(["nvidia-utils", "nvidia-settings"].map(String::from));
            v
        }
        AMD => vec!["vulkan-radeon".into()],
        _ => vec!["vulkan-intel".into(), "intel-media-driver".into()],
    };
    if bits32 {
        v.push(
            match which {
                NVIDIA => "lib32-nvidia-utils",
                AMD => "lib32-vulkan-radeon",
                _ => "lib32-vulkan-intel",
            }
            .into(),
        );
    }
    v
}

/// The 32-bit Vulkan drivers this computer's chips need (Steam's games): what the Steam component and labyrinth-gaming
/// install.
pub fn vulkan32(c: Chips) -> Vec<String> {
    let mut out = vec![];
    for (on, v) in [(c.nvidia, NVIDIA), (c.amd, AMD), (c.intel, INTEL)] {
        if on {
            out.extend(vendor(v, &[], true).into_iter().filter(|p| p.starts_with("lib32-")));
        }
    }
    out
}

/// What to install and what to remove: install first, then remove, so a program that needs a driver always has one.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub install: Vec<String>,
    pub remove: Vec<String>,
}

/// The plan for these chips; `installed` says whether a package is installed (pacman -Q).
pub fn plan(c: Chips, kernels: &[String], bits32: bool, installed: &dyn Fn(&str) -> bool) -> Plan {
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    for (on, v) in [(c.nvidia, NVIDIA), (c.amd, AMD), (c.intel, INTEL)] {
        if on {
            wanted.extend(vendor(v, kernels, bits32));
        }
    }
    if c.nvidia && c.two() {
        wanted.insert("nvidia-prime".into());
    }
    let mut every: BTreeSet<String> = BTreeSet::new();
    for v in [NVIDIA, AMD, INTEL] {
        every.extend(vendor(v, kernels, true));
    }
    every.insert("nvidia-prime".into());
    Plan {
        install: wanted.iter().filter(|p| !installed(p)).cloned().collect(),
        remove: every.difference(&wanted).filter(|p| installed(p)).cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(devs: &[(&str, &str, &str)]) -> crate::testutil::TempDir {
        let d = crate::testutil::TempDir::new();
        for (i, (class, vendor, device)) in devs.iter().enumerate() {
            let p = d.p(&format!("bus/pci/devices/0000:0{i}:00.0"));
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(format!("{p}/class"), format!("{class}\n")).unwrap();
            std::fs::write(format!("{p}/vendor"), format!("{vendor}\n")).unwrap();
            std::fs::write(format!("{p}/device"), format!("{device}\n")).unwrap();
        }
        d
    }

    fn chips_of(devs: &[(&str, &str, &str)]) -> Chips {
        let d = sys(devs);
        chips(&controllers(&d.0.to_string_lossy()))
    }

    const K: [&str; 2] = ["linux-cachyos", "linux-cachyos-lts"];

    fn kernels() -> Vec<String> {
        K.map(String::from).to_vec()
    }

    #[test]
    fn chips_from_the_display_controllers_only() {
        let nv = ("0x030000", "0x10de", "0x1e07");
        let intel = ("0x030000", "0x8086", "0xa7a0");
        let amd = ("0x030000", "0x1002", "0x744c");
        let arc = ("0x030000", "0x8086", "0x56a0");
        let nic = ("0x020000", "0x8086", "0x15f3");
        assert_eq!(chips_of(&[nv]), Chips { nvidia: true, ..Chips::default() });
        assert_eq!(chips_of(&[amd]), Chips { amd: true, ..Chips::default() });
        assert_eq!(chips_of(&[intel, nic]), Chips { intel: true, ..Chips::default() }, "a network card is no graphics chip");
        assert_eq!(chips_of(&[arc]), Chips { intel: true, ..Chips::default() }, "Intel Arc is Intel's");
        assert!(chips_of(&[intel, nv]).two());
        assert_eq!(chips_of(&[nic]), Chips::default(), "no graphics chip");
        let d = sys(&[nv, intel]);
        assert_eq!(controllers(&d.0.to_string_lossy()), ["0x10de:0x1e07", "0x8086:0xa7a0"]);
    }

    #[test]
    fn nvidia_only_keeps_its_driver_for_each_kernel_and_removes_the_others() {
        let c = Chips { nvidia: true, ..Chips::default() };
        let all = |_: &str| true;
        let p = plan(c, &kernels(), true, &all);
        assert!(p.install.is_empty());
        assert_eq!(
            p.remove,
            ["intel-media-driver", "lib32-vulkan-intel", "lib32-vulkan-radeon", "nvidia-prime", "vulkan-intel", "vulkan-radeon"]
        );
    }

    #[test]
    fn intel_only_wants_no_nvidia_package() {
        let c = Chips { intel: true, ..Chips::default() };
        let have = ["nvidia-utils", "lib32-nvidia-utils", "linux-cachyos-nvidia-open", "vulkan-intel", "steam"];
        let p = plan(c, &kernels(), true, &|x| have.contains(&x));
        assert_eq!(p.install, ["intel-media-driver", "lib32-vulkan-intel"]);
        assert_eq!(p.remove, ["lib32-nvidia-utils", "linux-cachyos-nvidia-open", "nvidia-utils"]);
        assert_eq!(vulkan32(c), ["lib32-vulkan-intel"]);
    }

    #[test]
    fn amd_only_and_a_swap_from_nvidia() {
        let c = Chips { amd: true, ..Chips::default() };
        // the card swapped: NVIDIA's packages are still there, AMD's 32-bit driver not yet
        let have = [
            "nvidia-utils",
            "nvidia-settings",
            "lib32-nvidia-utils",
            "linux-cachyos-nvidia-open",
            "linux-cachyos-lts-nvidia-open",
            "vulkan-radeon",
        ];
        let p = plan(c, &kernels(), true, &|x| have.contains(&x));
        assert_eq!(p.install, ["lib32-vulkan-radeon"]);
        assert_eq!(
            p.remove,
            ["lib32-nvidia-utils", "linux-cachyos-lts-nvidia-open", "linux-cachyos-nvidia-open", "nvidia-settings", "nvidia-utils"]
        );
    }

    #[test]
    fn intel_beside_nvidia_keeps_both_and_gpu_switching() {
        let c = Chips { nvidia: true, intel: true, ..Chips::default() };
        let p = plan(c, &kernels(), false, &|_| false);
        assert_eq!(
            p.install,
            [
                "intel-media-driver",
                "linux-cachyos-lts-nvidia-open",
                "linux-cachyos-nvidia-open",
                "nvidia-prime",
                "nvidia-settings",
                "nvidia-utils",
                "vulkan-intel"
            ]
        );
        assert!(p.remove.is_empty());
        assert_eq!(vulkan32(c), ["lib32-nvidia-utils", "lib32-vulkan-intel"]);
    }

    #[test]
    fn without_steam_no_32_bit_driver_is_wanted_and_any_is_removed() {
        let c = Chips { intel: true, ..Chips::default() };
        let have = ["vulkan-intel", "intel-media-driver", "lib32-vulkan-intel"];
        let p = plan(c, &kernels(), false, &|x| have.contains(&x));
        assert!(p.install.is_empty());
        assert_eq!(p.remove, ["lib32-vulkan-intel"]);
    }

    #[test]
    fn no_graphics_chip_keeps_mesa_alone() {
        let have = ["vulkan-intel", "vulkan-radeon", "nvidia-utils", "mesa"];
        let p = plan(Chips::default(), &kernels(), false, &|x| have.contains(&x));
        assert!(p.install.is_empty());
        assert_eq!(p.remove, ["nvidia-utils", "vulkan-intel", "vulkan-radeon"], "never mesa");
    }

    #[test]
    fn kernels_by_their_package_base() {
        let d = crate::testutil::TempDir::new();
        for (v, b) in [("7.3.5-2-cachyos", "linux-cachyos"), ("6.18.9-1-cachyos-lts", "linux-cachyos-lts"), ("odd", "bad name!")] {
            std::fs::create_dir_all(d.p(v)).unwrap();
            std::fs::write(d.p(&format!("{v}/pkgbase")), format!("{b}\n")).unwrap();
        }
        assert_eq!(super::kernels(&d.0.to_string_lossy()), K);
    }
}
