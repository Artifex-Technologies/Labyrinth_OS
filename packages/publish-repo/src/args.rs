//! The command line, as the script's argparse read it: --force, --dry-run, -h/--help, a long option shortened as long as it
//! stays unambiguous (--dry), anything else refused with exit code 2.

use std::ffi::OsString;

#[derive(Debug, PartialEq)]
pub enum Parsed {
    Run {
        force: bool,
        dry_run: bool,
    },
    Help(String),
    /// What goes to standard error; exit code 2.
    Refused(String),
}

const LONG: [&str; 3] = ["--help", "--force", "--dry-run"];

pub fn usage(prog: &str) -> String {
    format!("usage: {prog} [-h] [--force] [--dry-run]\n")
}

pub fn help(prog: &str) -> String {
    format!(
        "{}\nBuild what changed in Labyrinth's sources on GitHub, sign it with Labyrinth's key and publish it in the\n\
         release \"packages\" (Labyrinth's pacman repository).\n\n\
         options:\n  -h, --help  show this help message and exit\n  --force     publish even when no source changed\n  \
         --dry-run   build and sign, upload nothing\n",
        usage(prog)
    )
}

pub fn parse(prog: &str, args: &[OsString]) -> Parsed {
    let (mut force, mut dry_run, mut want_help) = (false, false, false);
    let mut extras: Vec<String> = Vec::new();
    let mut only_positional = false;
    for a in args {
        let a = a.to_string_lossy().into_owned();
        if only_positional || !a.starts_with('-') || a == "-" {
            extras.push(a);
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a == "-h" {
            want_help = true;
            continue;
        }
        if !a.starts_with("--") {
            extras.push(a);
            continue;
        }
        let (name, value) = match a.split_once('=') {
            Some((n, v)) => (n.to_string(), Some(v.to_string())),
            None => (a.clone(), None),
        };
        let matched: Vec<&str> = match LONG.iter().find(|o| **o == name) {
            Some(o) => vec![*o],
            None => LONG.iter().copied().filter(|o| o.starts_with(&name)).collect(),
        };
        let Some(&opt) = matched.first() else {
            extras.push(a);
            continue;
        };
        if let Some(v) = value {
            let shown = if opt == "--help" { "-h/--help" } else { opt };
            return Parsed::Refused(format!("{}{prog}: error: argument {shown}: ignored explicit argument '{v}'\n", usage(prog)));
        }
        match opt {
            "--help" => want_help = true,
            "--force" => force = true,
            _ => dry_run = true,
        }
    }
    if want_help {
        return Parsed::Help(help(prog));
    }
    if !extras.is_empty() {
        return Parsed::Refused(format!("{}{prog}: error: unrecognized arguments: {}\n", usage(prog), extras.join(" ")));
    }
    Parsed::Run { force, dry_run }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Parsed {
        parse("p", &a.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn options() {
        assert_eq!(p(&[]), Parsed::Run { force: false, dry_run: false });
        assert_eq!(p(&["--force", "--dry"]), Parsed::Run { force: true, dry_run: true });
        assert_eq!(p(&["--f", "--dry-run", "--force"]), Parsed::Run { force: true, dry_run: true });
        assert!(matches!(p(&["--he"]), Parsed::Help(_)));
        assert!(matches!(p(&["x", "-h"]), Parsed::Help(_)));
    }

    #[test]
    fn refusals() {
        assert_eq!(
            p(&["x", "-f"]),
            Parsed::Refused("usage: p [-h] [--force] [--dry-run]\np: error: unrecognized arguments: x -f\n".into())
        );
        assert!(matches!(p(&["--force=1", "--help"]), Parsed::Refused(m) if m.contains("argument --force: ignored explicit argument '1'")));
        assert!(matches!(p(&["--", "--force"]), Parsed::Refused(m) if m.ends_with("unrecognized arguments: --force\n")));
        assert!(matches!(p(&["--forced"]), Parsed::Refused(_)));
    }
}
