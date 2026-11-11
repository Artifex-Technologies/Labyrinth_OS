//! labyrinth-agent run as a program, only where that is harmless wherever the tests run (CI runs them as root): the help, and
//! the refusal when not root. Everything it does as root is tested on made-up answers (src/agent/tests.rs) and against the
//! script in a namespace of its own (parity/cases_agent.py).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_labyrinth-agent");

#[test]
fn help_is_the_scripts_docstring() {
    let doc = include_str!("../src/doc/labyrinth-agent.txt");
    for args in [&[][..], &["-h"], &["--help", "service"]] {
        let o = Command::new(BIN).args(args).env_clear().output().unwrap();
        assert_eq!(o.status.code(), Some(0));
        assert_eq!(String::from_utf8(o.stdout).unwrap(), format!("{doc}\n"));
        assert!(o.stderr.is_empty());
    }
}

#[test]
fn refused_when_not_root() {
    if unsafe { libc::geteuid() } == 0 {
        return; // as root it would act: never here
    }
    let o = Command::new(BIN).args(["service", "status", "heimdall"]).env("PATH", "/nonexistent").output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(String::from_utf8(o.stderr).unwrap(), "labyrinth-agent: runs as root through sudo (claude-sudo labyrinth-agent ...)\n");
    assert!(o.stdout.is_empty());
}
