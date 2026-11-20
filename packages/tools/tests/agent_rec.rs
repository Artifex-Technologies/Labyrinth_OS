//! labyrinth-agent-rec run as a program, only where that is harmless wherever the tests run (CI runs them as root): the help,
//! and the refusals before anything is run or recorded. The sessions themselves are tested on stand-ins (src/agent_rec/tests.rs).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_labyrinth-agent-rec");
const DOC: &str = include_str!("../src/doc/labyrinth-agent-rec.txt");

#[test]
fn help() {
    for a in ["-h", "--help"] {
        let o = Command::new(BIN).arg(a).env_clear().output().unwrap();
        assert_eq!(o.status.code(), Some(0));
        assert_eq!(String::from_utf8(o.stdout).unwrap(), DOC);
    }
    let o = Command::new(BIN).env_clear().output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(String::from_utf8(o.stderr).unwrap(), DOC);
}

#[test]
fn refused_before_anything_runs() {
    let root = unsafe { libc::geteuid() } == 0;
    // as root (CI) without sudo's variables it stops on those; as anyone else on not being root
    let o = Command::new(BIN).args(["--", "/usr/bin/true"]).env_clear().output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    let want = if root {
        "labyrinth-agent-rec: runs through sudo (SUDO_USER and SUDO_UID are not set)\n"
    } else {
        "labyrinth-agent-rec: runs as root through sudo (claude-sudo CMD...)\n"
    };
    assert_eq!(String::from_utf8(o.stderr).unwrap(), want);
    assert!(o.stdout.is_empty());
    // the agent may not read the records (here: a caller whose rules account exists, or not root)
    if !root {
        for args in [&["list"][..], &["replay", "20261119T000000.000Z-1"]] {
            let o = Command::new(BIN).args(args).env_clear().output().unwrap();
            assert_eq!(o.status.code(), Some(1));
            assert_eq!(
                String::from_utf8(o.stderr).unwrap(),
                "labyrinth-agent-rec: only root can read the records: sudo labyrinth-agent-rec list|replay\n"
            );
        }
    }
    for args in [&["list", "x"][..], &["replay"], &["replay", "a", "b"], &["replay", "--slow", "a"]] {
        let o = Command::new(BIN).args(args).env_clear().output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{args:?}");
    }
}
