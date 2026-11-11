//! For the tests that run the programs as the Python tests ran the scripts: a scratch folder that goes away, stand-in programs
//! (small shell scripts), and the run itself.
#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("labyrinth-tools-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        // the tests compare real paths with what the programs print
        TempDir(std::fs::canonicalize(&p).unwrap())
    }

    /// A path under it, as text.
    pub fn p(&self, rel: &str) -> String {
        self.0.join(rel).to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A stand-in program: `#!/bin/sh` and the body; its path.
pub fn stub(dir: &str, name: &str, body: &str) -> String {
    let p = format!("{dir}/{name}");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

pub fn write(path: &str, text: &str) {
    if let Some(d) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(path, text).unwrap();
}

pub fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

/// Run a program with exactly this environment.
pub fn run(prog: &str, args: &[&str], env: &[(String, String)]) -> Output {
    Command::new(prog).args(args).env_clear().envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str()))).output().unwrap()
}

pub fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

pub fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

pub fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

/// An environment as a list, with one variable set (or replaced).
pub fn with(env: &[(String, String)], k: &str, v: &str) -> Vec<(String, String)> {
    let mut e: Vec<(String, String)> = env.iter().filter(|(x, _)| x != k).cloned().collect();
    e.push((k.into(), v.into()));
    e
}

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}
