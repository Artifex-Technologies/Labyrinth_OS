//! A desktop notification when a publish does not go as it should. The publish runs from a timer, so its output only
//! reaches the user journal; packages that did not build went unseen for four days (2026-11-18 to 2026-11-21, LAB-53).
//! `notify-send` without actions returns at once. Nothing is shown when there is no desktop session (no session bus).
//! `LABYRINTH_PUBLISH_NOTIFY` names another program to call the same way (the tests' stand-in).

use crate::build::Failure;
use std::process::Command;

/// At most this many packages are named; the rest are counted.
const NAMED: usize = 8;

/// The title and text for packages that did not build: each named with the line of its log that says why.
pub fn failures_text(failed: &[Failure]) -> (String, String) {
    let title = match failed {
        [one] => format!("Labyrinth packages: {} did not build", one.name),
        _ => format!("Labyrinth packages: {} did not build", failed.len()),
    };
    let mut lines: Vec<String> = failed
        .iter()
        .take(NAMED)
        .map(|f| {
            let cause = f.lines.iter().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or(f.why.trim());
            let mut line = format!("{}: {cause}", f.name);
            if line.chars().count() > 160 {
                line = line.chars().take(159).collect::<String>() + "…";
            }
            line
        })
        .collect();
    if failed.len() > NAMED {
        lines.push(format!("and {} more", failed.len() - NAMED));
    }
    lines.push("The others were published. Their logs: journalctl --user -u labyrinth-publish".into());
    (title, lines.join("\n"))
}

/// Shows a notification (or tells the stand-in). False when nothing could be shown.
pub fn send(title: &str, body: &str) -> bool {
    let program = match std::env::var_os("LABYRINTH_PUBLISH_NOTIFY") {
        Some(p) => p,
        None if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() => return false,
        None => "notify-send".into(),
    };
    Command::new(program)
        .args(["--app-name=Labyrinth packages", "--icon=dialog-warning", "--urgency=normal", title, body])
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn failure(name: &str, why: &str, lines: &[&str]) -> Failure {
        Failure { name: name.into(), why: why.into(), lines: lines.iter().map(|s| s.to_string()).collect(), log: PathBuf::new() }
    }

    #[test]
    fn each_failed_package_is_named_with_its_cause() {
        let (t, b) =
            failures_text(&[failure("argus-git", "A failure occurred in prepare().", &["", "fatal: unable to access 'https://...': 403"])]);
        assert_eq!(t, "Labyrinth packages: argus-git did not build");
        assert!(b.starts_with("argus-git: fatal: unable to access"), "{b}");
        let many: Vec<Failure> = (0..10).map(|i| failure(&format!("p{i}"), "A failure occurred in build().", &[])).collect();
        let (t, b) = failures_text(&many);
        assert_eq!(t, "Labyrinth packages: 10 did not build");
        assert!(b.contains("p7: A failure occurred in build().") && !b.contains("p8:") && b.contains("and 2 more"), "{b}");
    }

    #[test]
    fn the_notification_goes_to_the_stand_in_with_title_and_text() {
        let t = std::env::temp_dir().join(format!("lab53-{}", std::process::id()));
        std::fs::create_dir_all(&t).unwrap();
        let script = t.join("notify");
        let out = t.join("args");
        std::fs::write(&script, format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", out.display())).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY: this test is the only one in this crate that sets the variable
        unsafe { std::env::set_var("LABYRINTH_PUBLISH_NOTIFY", &script) };
        let shown = send("Labyrinth packages: argus-git did not build", "argus-git: fatal: 403");
        unsafe { std::env::remove_var("LABYRINTH_PUBLISH_NOTIFY") };
        let args = std::fs::read_to_string(&out).unwrap();
        std::fs::remove_dir_all(&t).unwrap();
        assert!(shown);
        assert!(
            args.contains("--app-name=Labyrinth packages\n") && args.contains("argus-git did not build\nargus-git: fatal: 403\n"),
            "{args}"
        );
    }
}
