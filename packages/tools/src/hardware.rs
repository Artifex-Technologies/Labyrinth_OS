//! labyrinth-hardware-setup [--dry-run] [--if-changed]: the drivers and device tools this computer needs, and no others: the
//! graphics drivers its chips need (the gpu module decides: installed first, then the other vendors' removed), NVIDIA's
//! sleep services and power setting only with NVIDIA, GPU switching for two graphics chips, ASUS's tools. The installer runs
//! it on the installed system (the image carries every driver and installs offline); labyrinth-hardware-setup.service runs
//! it at each start with --if-changed, which does nothing while the chips match the record of the last run
//! (/var/lib/labyrinth/hardware-chips), so a swapped card gets its driver and loses the old one. Output and calls go to
//! /var/log/labyrinth-hardware.log. LABYRINTH_HW_SYS, LABYRINTH_HW_MODULES and LABYRINTH_HW_RECORD (tests only) are another
//! /sys, /usr/lib/modules and record.

use crate::gpu;
use crate::sh::{self, Io};

const LOG: &str = "/var/log/labyrinth-hardware.log";
/// The display controllers at the last run that finished.
const RECORD: &str = "/var/lib/labyrinth/hardware-chips";
const NVIDIA_POWER: &str = "/etc/modprobe.d/labyrinth-nvidia-power.conf";

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

    /// The command, its output and errors into the log (`"$@" >>LOG 2>&1`); with --dry-run only said. Whether it worked.
    fn run(&self, argv: &[&str]) -> bool {
        if self.dry {
            println!("    would run: {}", argv.join(" "));
            return true;
        }
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let log = std::fs::OpenOptions::new().append(true).create(true).open(LOG);
        let r = match log.and_then(|f| Ok((f.try_clone()?, f))) {
            Ok((o, e)) => std::process::Command::new(argv[0]).args(&argv[1..]).stdout(o).stderr(e).status().map(|s| s.success()),
            Err(_) => Ok(sh::run(argv, Io::Null, Io::Null) == 0),
        };
        r.unwrap_or_else(|e| {
            let _ = sh::append(LOG, format!("{}: {e}\n", argv[0]).as_bytes());
            false
        })
    }
}

/// The packages of this list that are installed (pacman -Q each).
fn installed(pkgs: &[&'static str]) -> Vec<&'static str> {
    pkgs.iter().copied().filter(|p| sh::quiet(&["pacman", "-Q", p])).collect()
}

fn env_or(name: &str, default: &str) -> String {
    match std::env::var(name) {
        Ok(s) if !s.is_empty() => s,
        _ => default.into(),
    }
}

pub fn main(args: &[String]) -> i32 {
    sh::set_prog("labyrinth-hardware-setup");
    let sys = env_or("LABYRINTH_HW_SYS", "/sys");
    let modules = env_or("LABYRINTH_HW_MODULES", "/usr/lib/modules");
    let record = env_or("LABYRINTH_HW_RECORD", RECORD);
    let flags: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let c = Ctx { dry: flags.contains(&"--dry-run") };

    // graphics chips, by the PCI vendor of every display controller (class 0x03xxxx)
    let controllers = gpu::controllers(&sys);
    let signature = controllers.join("\n") + "\n";
    // (cat drops the last line break, as $(cat) did)
    if flags.contains(&"--if-changed") && sh::cat(&record).as_deref() == Some(signature.trim_end()) {
        return 0;
    }
    let chips = gpu::chips(&controllers);
    let vendor = sh::cat(&format!("{sys}/class/dmi/id/sys_vendor")).unwrap_or_default();

    if chips.nvidia {
        c.say("NVIDIA graphics found: keeping its driver");
        // NVIDIA keeps video memory across suspend and hibernate only if asked to
        if c.dry {
            println!("    would write {NVIDIA_POWER}");
        } else {
            if let Err(e) = sh::install_dir("/etc/modprobe.d", 0o755, None) {
                eprintln!("labyrinth-hardware-setup: cannot make /etc/modprobe.d: {e}");
            }
            if let Err(e) = sh::write(NVIDIA_POWER, b"options nvidia NVreg_PreserveVideoMemoryAllocations=1\n") {
                eprintln!("labyrinth-hardware-setup: cannot write {NVIDIA_POWER}: {e}");
            }
        }
    }

    // what these chips need, installed first, then what another vendor would need removed: a program that needs a driver
    // (Steam's games) always has one. Without the internet nothing is removed and the next start tries again.
    let bits32 = sh::quiet(&["pacman", "-Q", "steam"]);
    let plan = gpu::plan(chips, &gpu::kernels(&modules), bits32, &|p| sh::quiet(&["pacman", "-Q", p]));
    let mut done = true;
    if !plan.install.is_empty() {
        c.say(&format!("installing the drivers these chips need: {}", plan.install.join(" ")));
        let mut argv = vec!["pacman", "-S", "--needed", "--noconfirm"];
        argv.extend(plan.install.iter().map(String::as_str));
        if !c.run(&argv) {
            c.say("could not install them (no internet?): nothing is removed, and the next start tries again");
            done = false;
        }
    }
    if chips.nvidia {
        c.run(&["systemctl", "enable", "nvidia-suspend.service", "nvidia-hibernate.service", "nvidia-resume.service"]);
    } else if done && plan.remove.iter().any(|p| p == "nvidia-utils") {
        // its sleep services and power setting go with it (the units are nvidia-utils')
        c.run(&["systemctl", "disable", "nvidia-suspend.service", "nvidia-hibernate.service", "nvidia-resume.service"]);
        if c.dry {
            println!("    would remove {NVIDIA_POWER}");
        } else {
            let _ = std::fs::remove_file(NVIDIA_POWER);
        }
    }
    if done && !plan.remove.is_empty() {
        c.say(&format!("removing the drivers no chip here needs: {}", plan.remove.join(" ")));
        let mut argv = vec!["pacman", "-Rns", "--noconfirm"];
        argv.extend(plan.remove.iter().map(String::as_str));
        if !c.run(&argv) {
            // something else needs one of them: each on its own, so the rest still goes
            for p in &plan.remove {
                if !c.run(&["pacman", "-Rns", "--noconfirm", p]) {
                    c.say(&format!("kept {p}: another package needs it"));
                }
            }
        }
    }

    if chips.nvidia && chips.two() {
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
    if done && !c.dry {
        let _ = sh::install_dir(&record[..record.rfind('/').unwrap_or(0)], 0o755, None);
        if let Err(e) = sh::write(&record, signature.as_bytes()) {
            eprintln!("labyrinth-hardware-setup: cannot write {record}: {e}");
        }
    }
    let maker = if vendor.is_empty() { "unknown" } else { &vendor };
    let other = chips.amd || chips.intel;
    c.say(&format!("hardware setup done (nvidia={}, other graphics={}, maker={maker})", chips.nvidia as u8, other as u8));
    0
}
