//! labyrinth-hardware-setup [--dry-run]: run once by the installer on the installed system; keeps only the drivers and device
//! tools this computer needs (the image carries them all and installs offline): NVIDIA's driver and its sleep services, GPU
//! switching for two graphics chips, ASUS's tools. The bash script of the same name with the same output, log
//! (/var/log/labyrinth-hardware.log), calls and exit code. LABYRINTH_HW_SYS (tests only) is another /sys, as before.
//!
//! pacman and systemctl are still those programs; the PCI and DMI files are read here (cat) and the log written here.
//!
//! Not 1:1: when the log cannot be written its lines are skipped quietly (bash printed its redirection error for each).

use crate::sh::{self, Io};

const LOG: &str = "/var/log/labyrinth-hardware.log";

struct Ctx {
    dry: bool,
}

impl Ctx {
    fn say(&self, m: &str) {
        println!("==> {m}");
        if !self.dry {
            let _ = sh::append(LOG, format!("{} {m}\n", sh::date_iso()).as_bytes());
        }
    }

    /// The command, its output and errors into the log (`"$@" >>LOG 2>&1`); with --dry-run only said.
    fn run(&self, argv: &[&str]) {
        if self.dry {
            println!("    would run: {}", argv.join(" "));
            return;
        }
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let log = std::fs::OpenOptions::new().append(true).create(true).open(LOG);
        let r = match log.and_then(|f| Ok((f.try_clone()?, f))) {
            Ok((o, e)) => std::process::Command::new(argv[0]).args(&argv[1..]).stdout(o).stderr(e).status().map(|_| ()),
            Err(_) => {
                sh::run(argv, Io::Null, Io::Null);
                Ok(())
            }
        };
        if let Err(e) = r {
            let _ = sh::append(LOG, format!("{}: {e}\n", argv[0]).as_bytes());
        }
    }
}

/// The packages of this list that are installed (pacman -Q each).
fn installed(pkgs: &[&'static str]) -> Vec<&'static str> {
    pkgs.iter().copied().filter(|p| sh::quiet(&["pacman", "-Q", p])).collect()
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-hardware-setup");
    let sys = match std::env::var("LABYRINTH_HW_SYS") {
        Ok(s) if !s.is_empty() => s,
        _ => "/sys".into(),
    };
    let c = Ctx { dry: args.get(1).map(|a| a == "--dry-run").unwrap_or(false) };

    // graphics chips, by the PCI vendor of every display controller (class 0x03xxxx)
    let (mut nvidia, mut other) = (false, false);
    for d in sh::glob(&format!("{sys}/bus/pci/devices"), |_| true) {
        if !sh::cat(&format!("{d}/class")).unwrap_or_default().starts_with("0x03") {
            continue;
        }
        match sh::cat(&format!("{d}/vendor")).unwrap_or_default().as_str() {
            "0x10de" => nvidia = true,
            "0x8086" | "0x1002" => other = true,
            _ => {}
        }
    }
    let vendor = sh::cat(&format!("{sys}/class/dmi/id/sys_vendor")).unwrap_or_default();

    if nvidia {
        c.say("NVIDIA graphics found: keeping its driver");
        // NVIDIA keeps video memory across suspend and hibernate only if asked to
        if c.dry {
            println!("    would write /etc/modprobe.d/labyrinth-nvidia-power.conf");
        } else {
            if let Err(e) = sh::install_dir("/etc/modprobe.d", 0o755, None) {
                eprintln!("labyrinth-hardware-setup: cannot make /etc/modprobe.d: {e}");
            }
            if let Err(e) =
                sh::write("/etc/modprobe.d/labyrinth-nvidia-power.conf", b"options nvidia NVreg_PreserveVideoMemoryAllocations=1\n")
            {
                eprintln!("labyrinth-hardware-setup: cannot write /etc/modprobe.d/labyrinth-nvidia-power.conf: {e}");
            }
        }
        c.run(&["systemctl", "enable", "nvidia-suspend.service", "nvidia-hibernate.service", "nvidia-resume.service"]);
    } else {
        let pk = installed(&[
            "linux-cachyos-nvidia-open",
            "linux-cachyos-lts-nvidia-open",
            "lib32-nvidia-utils",
            "nvidia-settings",
            "nvidia-prime",
            "nvidia-utils",
        ]);
        if !pk.is_empty() {
            c.say(&format!("no NVIDIA graphics: removing {}", pk.join(" ")));
            let mut argv = vec!["pacman", "-Rns", "--noconfirm"];
            argv.extend(&pk);
            c.run(&argv);
        }
    }

    if nvidia && other {
        c.say("two graphics chips: GPU switching on (apps can be started on the NVIDIA chip)");
        c.run(&["systemctl", "enable", "switcheroo-control.service"]);
    } else {
        c.run(&["systemctl", "disable", "switcheroo-control.service"]);
    }

    if vendor.starts_with("ASUSTeK") {
        c.say(&format!("ASUS computer ({vendor}): keeping ASUS tools"));
    } else {
        let pk = installed(&["rog-control-center", "asusctl"]);
        if !pk.is_empty() {
            c.say(&format!("not an ASUS computer: removing {}", pk.join(" ")));
            let mut argv = vec!["pacman", "-Rns", "--noconfirm"];
            argv.extend(&pk);
            c.run(&argv);
        }
    }
    let maker = if vendor.is_empty() { "unknown" } else { &vendor };
    c.say(&format!("hardware setup done (nvidia={}, other graphics={}, maker={maker})", nvidia as u8, other as u8));
    0
}
