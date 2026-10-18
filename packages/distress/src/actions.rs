//! What happens when the distress password fires. Everything is planned first as a list of steps (so it can be shown, tested and
//! dry-run) and only then carried out. Nothing here runs unless the user switched the distress password on and saved it.

use crate::config::{Config, Extra};

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Run a program with arguments.
    Run(Vec<String>),
    /// Run a shell command line.
    Sh(String),
    /// Delete a file or folder.
    Remove(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    pub why: String,
    pub step: Step,
}

/// What the plan needs to know about this computer.
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    /// The account the password belongs to.
    pub user: String,
    /// Every normal account: (name, home).
    pub users: Vec<(String, String)>,
    /// Encrypted partitions (e.g. /dev/nvme0n1p2).
    pub luks: Vec<String>,
    /// Internal disks (e.g. /dev/nvme0n1).
    pub disks: Vec<String>,
}

impl Ctx {
    pub fn detect(user: &str) -> Ctx {
        let mut c = Ctx { user: user.to_string(), ..Ctx::default() };
        for l in std::fs::read_to_string("/etc/passwd").unwrap_or_default().lines() {
            let p: Vec<&str> = l.split(':').collect();
            if p.len() > 5 && p[2].parse::<u32>().map(|u| (1000..60000).contains(&u)).unwrap_or(false) {
                c.users.push((p[0].to_string(), p[5].to_string()));
            }
        }
        if let Ok(o) = std::process::Command::new("lsblk").args(["-rno", "PATH,FSTYPE"]).output() {
            for l in String::from_utf8_lossy(&o.stdout).lines() {
                let mut w = l.split_whitespace();
                if let (Some(p), Some("crypto_LUKS")) = (w.next(), w.next()) {
                    c.luks.push(p.to_string());
                }
            }
        }
        if let Ok(o) = std::process::Command::new("lsblk").args(["-dnro", "PATH,TYPE,RM"]).output() {
            for l in String::from_utf8_lossy(&o.stdout).lines() {
                let w: Vec<&str> = l.split_whitespace().collect();
                // internal, non-removable disks only: the live USB and the thumb drive in the pocket are left alone
                if w.len() == 3 && w[1] == "disk" && w[2] == "0" && !["/dev/zram", "/dev/loop", "/dev/ram", "/dev/nbd"].iter().any(|p| w[0].starts_with(p)) {
                    c.disks.push(w[0].to_string());
                }
            }
        }
        c
    }
}

fn run(why: &str, args: &[&str]) -> Planned {
    Planned { why: why.into(), step: Step::Run(args.iter().map(|s| s.to_string()).collect()) }
}

/// Section 1 when the real account is locked and the decoy session is used.
pub fn decoy_steps(ctx: &Ctx) -> Vec<Planned> {
    vec![
        run("Lock your real account", &["passwd", "-l", &ctx.user]),
        run("Block other ways into your real account", &["usermod", "--expiredate", "1", &ctx.user]),
    ]
}

/// Section 2.
pub fn extra_steps(cfg: &Config, ctx: &Ctx) -> Vec<Planned> {
    let mut v = vec![];
    match cfg.extra {
        Extra::None => {}
        Extra::Commands => {
            for c in cfg.commands.iter().filter(|c| !c.trim().is_empty()) {
                v.push(Planned { why: format!("Run: {}", c.trim()), step: Step::Sh(c.trim().to_string()) });
            }
        }
        Extra::WipeKeys => {
            if cfg.wipe_keys {
                for (_, home) in &ctx.users {
                    for rel in [".ssh", ".gnupg", ".local/share/keyrings", ".local/share/kwalletd", ".config/labyrinth/vault"] {
                        v.push(Planned { why: format!("Delete {home}/{rel}"), step: Step::Remove(format!("{home}/{rel}")) });
                    }
                }
                for d in &ctx.luks {
                    v.push(run(&format!("Erase the encryption keys of {d}; its data can never be read again"), &["cryptsetup", "luksErase", "--batch-mode", d]));
                }
            }
            if cfg.shutdown {
                v.push(run("Shut down", &["systemctl", "poweroff", "-f"]));
            }
        }
        Extra::FactoryReset => {
            for (u, home) in &ctx.users {
                v.push(Planned { why: format!("Remove the account {u} and everything in {home}"), step: Step::Sh(format!("userdel -rf {u} 2>/dev/null; rm -rf {home}")) });
            }
            for p in ["/etc/labyrinth", "/var/lib/labyrinth", "/var/lib/AccountsService", "/etc/NetworkManager/system-connections", "/var/lib/bluetooth", "/var/lib/flatpak", "/var/cache"] {
                v.push(Planned { why: format!("Reset {p}"), step: Step::Remove(p.to_string()) });
            }
            v.push(run("Start the first-time setup again at the next start", &["touch", "/etc/labyrinth-first-boot"]));
            v.push(run("Restart", &["systemctl", "reboot", "-f"]));
        }
        Extra::WipeSystem => {
            for d in ctx.luks.iter() {
                v.push(run(&format!("Erase the encryption keys of {d}"), &["cryptsetup", "luksErase", "--batch-mode", d]));
            }
            for d in &ctx.disks {
                v.push(Planned { why: format!("Destroy the partition table and the start of {d}"), step: Step::Sh(format!("wipefs -af {d}; dd if=/dev/zero of={d} bs=1M count=64 conv=fsync 2>/dev/null; blkdiscard -f {d} 2>/dev/null; true")) });
            }
            v.push(run("Power off", &["systemctl", "poweroff", "-f"]));
        }
    }
    v
}

/// Plain-language description of section 2, shown before saving.
pub fn describe(cfg: &Config, ctx: &Ctx) -> Vec<String> {
    extra_steps(cfg, ctx).into_iter().map(|p| p.why).collect()
}

/// Carries the steps out one after another; a step that fails does not stop the ones after it. Returns one line per step.
pub fn execute(steps: &[Planned], dry: bool) -> Vec<String> {
    let mut log = vec![];
    for p in steps {
        if dry {
            log.push(format!("would: {} ({:?})", p.why, p.step));
            continue;
        }
        let ok = match &p.step {
            Step::Run(a) if !a.is_empty() => std::process::Command::new(&a[0]).args(&a[1..]).status().map(|s| s.success()).unwrap_or(false),
            Step::Sh(c) => std::process::Command::new("sh").args(["-c", c]).status().map(|s| s.success()).unwrap_or(false),
            Step::Remove(path) => {
                let r = std::path::Path::new(path);
                // never follow the path out of where it points: refuse anything that is not an absolute path of some depth
                r.is_absolute() && r.components().count() >= 3 && (std::fs::remove_dir_all(r).is_ok() || std::fs::remove_file(r).is_ok() || !r.exists())
            }
            _ => false,
        };
        log.push(format!("{}: {}", if ok { "done" } else { "failed" }, p.why));
    }
    log
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx { user: "me".into(), users: vec![("me".into(), "/home/me".into())], luks: vec!["/dev/nvme0n1p2".into()], disks: vec!["/dev/nvme0n1".into()] }
    }

    #[test]
    fn nothing_extra_by_default() {
        assert!(extra_steps(&Config::default(), &ctx()).is_empty());
    }

    #[test]
    fn wipe_keys_without_shutdown() {
        let c = Config { extra: Extra::WipeKeys, shutdown: false, ..Config::default() };
        let s = extra_steps(&c, &ctx());
        assert!(s.iter().any(|p| p.why.contains("encryption keys")));
        assert!(!s.iter().any(|p| p.why == "Shut down"));
    }

    #[test]
    fn wipe_system_ends_with_power_off_and_hits_every_disk() {
        let c = Config { extra: Extra::WipeSystem, ..Config::default() };
        let s = extra_steps(&c, &ctx());
        assert_eq!(s.last().unwrap().why, "Power off");
        assert!(s.iter().any(|p| matches!(&p.step, Step::Sh(c) if c.contains("/dev/nvme0n1"))));
    }

    #[test]
    fn dry_run_touches_nothing() {
        let c = Config { extra: Extra::FactoryReset, ..Config::default() };
        let log = execute(&extra_steps(&c, &ctx()), true);
        assert!(log.iter().all(|l| l.starts_with("would:")));
    }

    #[test]
    fn shallow_removals_are_refused() {
        let s = [Planned { why: "x".into(), step: Step::Remove("/etc".into()) }];
        assert!(execute(&s, false)[0].starts_with("failed"));
    }
}
