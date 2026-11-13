//! labyrinth-os-release PACKAGE-VERSION [FILE]: writes the system version into /etc/os-release (docs/VERSIONING.md). The
//! labyrinth-tools package's version is Labyrinth OS's, so each update runs this from its scriptlet: VERSION ("1.0", "1.1",
//! "1.1.1": the patch only when there is one) and VERSION_ID (1.0.0), which Settings > About and every program that asks what the
//! system is read. The rest of the file stays as it is. Only on Labyrinth (an ID=labyrinth line): another system using these
//! packages keeps its own, and a link to the distribution's /usr/lib/os-release is left alone. FILE is for tests.
//!
//! As the scriptlet's awk program did (until 2026-11-12): the epoch and the build number dropped from the package's version,
//! the old VERSION and VERSION_ID lines removed, the new ones after the first PRETTY_NAME line (or at the end), every line
//! ending in a newline, the file written whole as 0644 and renamed into place. It always exits 0, as the scriptlet did.

/// ARGS as the program got them (its own name first).
pub fn main(args: &[String]) -> i32 {
    let args = args.get(1..).unwrap_or_default();
    let Some(pkgver) = args.first() else {
        eprintln!("usage: labyrinth-os-release PACKAGE-VERSION [FILE]");
        return 2;
    };
    let file = args.get(1).map(String::as_str).unwrap_or("/etc/os-release");
    let Ok(meta) = std::fs::symlink_metadata(file) else { return 0 };
    if !meta.is_file() {
        return 0;
    }
    let Ok(text) = std::fs::read(file) else { return 0 };
    let text = String::from_utf8_lossy(&text);
    if !text.split('\n').any(|l| matches!(l, "ID=labyrinth" | "ID=\"labyrinth\"" | "ID=\"labyrinth" | "ID=labyrinth\"")) {
        return 0;
    }
    let (show, id) = versions(pkgver);
    if let Err(e) = crate::sh::replace_file(file, edit(&text, &show, &id).as_bytes(), 0o644, None) {
        eprintln!("labyrinth-os-release: {file}: {e}");
    }
    0
}

/// ("1.0", "1.0.0") from "1:1.0.0-3": the epoch and the build number off, and a ".0" patch off the shown version.
pub fn versions(pkgver: &str) -> (String, String) {
    let v = pkgver.split_once(':').map_or(pkgver, |(_, v)| v);
    let v = v.rsplit_once('-').map_or(v, |(v, _)| v);
    (v.strip_suffix(".0").unwrap_or(v).to_string(), v.to_string())
}

/// The file with VERSION and VERSION_ID replaced: awk's lines (a last line without a newline gets one).
pub fn edit(text: &str, show: &str, id: &str) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') || text.is_empty() {
        lines.pop();
    }
    let ours = format!("VERSION=\"{show}\"\nVERSION_ID={id}\n");
    let mut out = String::new();
    let mut done = false;
    for l in lines {
        if l.starts_with("VERSION=") || l.starts_with("VERSION_ID=") {
            continue;
        }
        out.push_str(l);
        out.push('\n');
        if l.starts_with("PRETTY_NAME=") && !done {
            out.push_str(&ours);
            done = true;
        }
    }
    if !done {
        out.push_str(&ours);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_versions() {
        assert_eq!(versions("1.0.0-3"), ("1.0".into(), "1.0.0".into()));
        assert_eq!(versions("1:1.1.2-10"), ("1.1.2".into(), "1.1.2".into()));
        assert_eq!(versions("1.10-1"), ("1.10".into(), "1.10".into()));
        assert_eq!(versions("2.0.0"), ("2.0".into(), "2.0.0".into()));
    }

    #[test]
    fn lines_as_awk_wrote_them() {
        let t = "NAME=\"Labyrinth OS\"\nVERSION=\"0.9\"\nPRETTY_NAME=\"Labyrinth OS\"\nID=labyrinth\nVERSION_ID=0.9.0\nPRETTY_NAME=x";
        assert_eq!(
            edit(t, "1.0", "1.0.0"),
            "NAME=\"Labyrinth OS\"\nPRETTY_NAME=\"Labyrinth OS\"\nVERSION=\"1.0\"\nVERSION_ID=1.0.0\nID=labyrinth\nPRETTY_NAME=x\n"
        );
        assert_eq!(edit("ID=labyrinth\n", "1.1", "1.1.0"), "ID=labyrinth\nVERSION=\"1.1\"\nVERSION_ID=1.1.0\n");
        assert_eq!(edit("", "1.1", "1.1.0"), "VERSION=\"1.1\"\nVERSION_ID=1.1.0\n");
    }

    fn run(text: &str, version: &str) -> (String, Vec<String>) {
        let d = std::env::temp_dir().join(format!("os-release-test-{}-{version}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("os-release");
        std::fs::write(&f, text).unwrap();
        assert_eq!(main(&["labyrinth-os-release".into(), version.into(), f.to_string_lossy().into_owned()]), 0);
        let out = std::fs::read_to_string(&f).unwrap();
        let mut files: Vec<String> =
            std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        files.sort();
        let _ = std::fs::remove_dir_all(&d);
        (out, files)
    }

    // was tests/test_package_version.py, for the scriptlet's awk program
    #[test]
    fn written_on_an_update() {
        let before = "NAME=\"Labyrinth OS\"\nPRETTY_NAME=\"Labyrinth OS\"\nID=labyrinth\nBUILD_ID=rolling\nIMAGE_VERSION=2026.10.13\n";
        let (after, files) = run(before, "1.0.0-3");
        assert_eq!(
            after,
            "NAME=\"Labyrinth OS\"\nPRETTY_NAME=\"Labyrinth OS\"\nVERSION=\"1.0\"\nVERSION_ID=1.0.0\nID=labyrinth\nBUILD_ID=rolling\nIMAGE_VERSION=2026.10.13\n"
        );
        assert_eq!(files, ["os-release"], "no temporary file left");
        let (again, _) = run(&after, "1:1.1.1-1");
        assert!(again.contains("VERSION=\"1.1.1\"\nVERSION_ID=1.1.1\n"));
        assert_eq!(again.matches("VERSION=").count(), 2, "replaced, not added (IMAGE_VERSION stays)");
    }

    #[test]
    fn another_system_keeps_its_own() {
        let other = "NAME=\"LykanOS\"\nID=lykanos\nVERSION=\"2\"\n";
        assert_eq!(run(other, "1.0.0-1").0, other);
    }
}
