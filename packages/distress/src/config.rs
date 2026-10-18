//! The distress password's settings. Stored root-only (0600) in /etc/labyrinth/distress.json; the password itself is only ever kept as a yescrypt hash.

use serde::{Deserialize, Serialize};

/// What the second section does. Every one of these needs the account password typed a second time when saving.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Extra {
    #[default]
    None,
    /// Run the commands in `commands`.
    Commands,
    /// Erase disk encryption keys and secrets, and/or shut down (see `wipe_keys` and `shutdown`).
    WipeKeys,
    /// Remove every account and its data, and the system's settings, then restart into first-boot setup.
    FactoryReset,
    /// Destroy the partition tables and encryption headers of every internal disk, then power off. No OS afterwards.
    WipeSystem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// yescrypt hash of the distress password ("" until one is set).
    pub hash: String,
    /// The password has to be typed `count` times within `window_secs` (count 1 = a single entry).
    pub count: u32,
    pub window_secs: u32,
    /// The password stops working at this time (seconds since 1970); 0 = forever.
    pub expires: i64,
    /// Section 1: log in as normal while a hidden alert is raised.
    pub silent_login: bool,
    /// Section 1: lock the real account and open a decoy session.
    pub decoy: bool,
    /// Section 1 and 2 also apply when the distress password is typed in a terminal or another password prompt (needs the PAM module).
    pub in_prompts: bool,
    pub extra: Extra,
    pub commands: Vec<String>,
    pub wipe_keys: bool,
    pub shutdown: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { enabled: false, hash: String::new(), count: 1, window_secs: 60, expires: 0, silent_login: true, decoy: false, in_prompts: false, extra: Extra::None, commands: vec![], wipe_keys: true, shutdown: true }
    }
}

impl Config {
    /// Whether the distress password is usable at `now`.
    pub fn active(&self, now: i64) -> bool {
        self.enabled && !self.hash.is_empty() && (self.expires == 0 || now < self.expires)
    }

    /// Problems that stop a save. Empty = fine.
    pub fn problems(&self) -> Vec<String> {
        let mut p = vec![];
        if self.count == 0 || self.count > 20 {
            p.push("The number of entries must be between 1 and 20".to_string());
        }
        if self.count > 1 && !(5..=3600).contains(&self.window_secs) {
            p.push("The time window must be between 5 seconds and 1 hour".to_string());
        }
        if self.silent_login && self.decoy {
            p.push("Choose either a silent login or a decoy session, not both".to_string());
        }
        if self.extra == Extra::Commands && self.commands.iter().all(|c| c.trim().is_empty()) {
            p.push("Add at least one command, or choose something else".to_string());
        }
        if self.extra == Extra::WipeKeys && !self.wipe_keys && !self.shutdown {
            p.push("Choose wiping the keys, shutting down, or both".to_string());
        }
        if !self.silent_login && !self.decoy && self.extra == Extra::None {
            p.push("Choose at least one thing for the distress password to do".to_string());
        }
        p
    }

    /// The copy sent to Settings: everything but the hash.
    pub fn public(&self) -> Config {
        Config { hash: if self.hash.is_empty() { String::new() } else { "set".into() }, ..self.clone() }
    }
}

pub fn dir() -> std::path::PathBuf {
    std::env::var_os("LABYRINTH_DISTRESS_DIR").map(Into::into).unwrap_or_else(|| "/etc/labyrinth".into())
}
pub fn state_dir() -> std::path::PathBuf {
    std::env::var_os("LABYRINTH_DISTRESS_STATE").map(Into::into).unwrap_or_else(|| "/var/lib/labyrinth/distress".into())
}

pub fn load() -> Config {
    std::fs::read_to_string(dir().join("distress.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(c: &Config) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    let tmp = dir().join(".distress.json.new");
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(|e| e.to_string())?;
    f.write_all(serde_json::to_string_pretty(c).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir().join("distress.json")).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_something_to_do() {
        let c = Config { silent_login: false, ..Config::default() };
        assert!(!c.problems().is_empty());
        assert!(Config::default().problems().is_empty());
    }

    #[test]
    fn expiry() {
        let c = Config { enabled: true, hash: "x".into(), expires: 100, ..Config::default() };
        assert!(c.active(99));
        assert!(!c.active(100));
        assert!(Config { expires: 0, ..c }.active(1 << 40));
    }

    #[test]
    fn public_hides_hash() {
        let c = Config { hash: "$y$secret".into(), ..Config::default() };
        assert_eq!(c.public().hash, "set");
    }
}
