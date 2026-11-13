//! What the publish reads in a package and a recipe, without starting anything: a package's .PKGINFO, what makes two builds
//! the same program (its .MTREE and dependencies), whether it was built under a home folder (.BUILDINFO), the names a recipe
//! makes, and the name a file gets online.

use crate::py::{split_ws, splitlines, universal_newlines};
use sha2::{Digest, Sha256};

/// A .PKGINFO's "key = value" lines, in order.
pub fn pkginfo_lines(text: &str) -> Vec<(String, String)> {
    splitlines(text).into_iter().filter_map(|l| l.split_once(" = ").map(|(k, v)| (k.to_string(), v.to_string()))).collect()
}

/// The first value of a key (a .PKGINFO lists depend, provides ... once per value; pkgname and pkgver once).
pub fn field<'a>(lines: &'a [(String, String)], key: &str) -> Option<&'a str> {
    lines.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// What makes a package different to the person installing it: its files' digests, modes, types and link targets (from its
/// .MTREE, already unpacked), its install script and what it depends on, provides, conflicts with, replaces and keeps.
/// Not its version, build date or size.
pub fn content_digest(mtree: &str, pkginfo: &[(String, String)]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for l in splitlines(mtree) {
        let parts = split_ws(l);
        let Some(first) = parts.first() else { continue };
        if first.starts_with('/') || matches!(*first, "./.PKGINFO" | "./.BUILDINFO" | "./.MTREE") {
            continue;
        }
        let mut keep: Vec<&str> =
            parts[1..].iter().copied().filter(|p| ["sha256digest=", "mode=", "type=", "link="].iter().any(|k| p.starts_with(k))).collect();
        keep.sort_unstable();
        lines.push(format!("{} {}", first, keep.join(" ")));
    }
    lines.sort_unstable();
    let mut meta: Vec<String> = pkginfo
        .iter()
        .filter(|(k, _)| matches!(k.as_str(), "depend" | "optdepend" | "provides" | "conflict" | "replaces" | "backup" | "arch"))
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    meta.sort_unstable();
    lines.extend(meta);
    let digest = Sha256::digest(lines.join("\n").as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether a .BUILDINFO says the package was built in a folder under /home or /root: such a package names the account that
/// built it, so it is not kept online even when the program is the same.
pub fn built_at_home(buildinfo: &str) -> bool {
    splitlines(buildinfo).into_iter().any(|l| {
        (l.starts_with("builddir = ") || l.starts_with("startdir = "))
            && l.split_once(" = ").is_some_and(|(_, v)| v.starts_with("/home/") || v.starts_with("/root/"))
    })
}

/// The names a recipe makes: its first `pkgname=` line, a single name or a (list), quotes taken off. The text is the
/// PKGBUILD as read (any line endings).
pub fn recipe_pkgnames(pkgbuild: &str) -> Vec<String> {
    let text = universal_newlines(pkgbuild);
    let Some(line) = text.split('\n').find(|l| l.starts_with("pkgname=")) else { return Vec::new() };
    let rest = &line["pkgname=".len()..];
    let rest = rest.strip_prefix('(').unwrap_or(rest);
    let list = rest.split(')').next().unwrap_or("");
    split_ws(list).into_iter().map(|n| n.trim_matches(['\'', '"', ' '])).filter(|n| !n.is_empty()).map(str::to_string).collect()
}

/// The file's name in the release. A package with an epoch (the store's apps have epoch=1 since 2026-11-07) is named
/// name-1:ver-rel-arch by makepkg, and GitHub turns the colon into a dot; so the repository gives it that name itself (pacman
/// downloads the name the database lists, and the version inside the package keeps its epoch).
pub fn online_name(name: &str) -> String {
    name.replace(':', ".")
}

/// Names GitHub keeps as they are (anything else it renames, and pacman would ask for a file that is not there).
pub fn safe_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// The version without its build number: 1:1.0.0-37 -> 1:1.0.0.
pub fn release_of(version: &str) -> &str {
    version.rsplit_once('-').map_or(version, |(r, _)| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_epoch_gets_a_name_github_keeps() {
        let f = "yggdrasil-2048-git-1:0.1.0.r12.gabc1234-1-x86_64.pkg.tar.zst";
        let name = online_name(f);
        assert_eq!(name, "yggdrasil-2048-git-1.0.1.0.r12.gabc1234-1-x86_64.pkg.tar.zst");
        assert!(safe_name(&name));
        assert!(!safe_name(f), "the colon is what GitHub would rename");
    }

    #[test]
    fn other_names_stay() {
        let f = "saga-md-0.1.0.r467-1-x86_64.pkg.tar.zst";
        assert_eq!(online_name(f), f);
    }

    #[test]
    fn unsafe_names() {
        for n in ["", "a+b.pkg.tar.zst", "a b", "a.pkg.tar.zst\n", "\u{e9}"] {
            assert!(!safe_name(n), "{n:?}");
        }
    }

    #[test]
    fn recipe_names() {
        assert_eq!(recipe_pkgnames("pkgbase=x\npkgname=('a' \"b\" c)\npkgname=d\n"), vec!["a", "b", "c"]);
        assert_eq!(recipe_pkgnames("pkgname=yggdrasil-panel\npkgver=1\n"), vec!["yggdrasil-panel"]);
        assert_eq!(recipe_pkgnames("pkgname=(a\r\nb)\r\n"), vec!["a"]);
        assert_eq!(recipe_pkgnames("  pkgname=x\n"), Vec::<String>::new());
        assert_eq!(recipe_pkgnames("pkgname=\"\"\n"), Vec::<String>::new());
    }

    #[test]
    fn versions_and_fields() {
        assert_eq!(release_of("1:1.0.0-37"), "1:1.0.0");
        assert_eq!(release_of("1.0"), "1.0");
        let l = pkginfo_lines("# x\npkgname = a\npkgver = 1-1\ndepend = b = 2\ndepend = c\n");
        assert_eq!(field(&l, "pkgname"), Some("a"));
        assert_eq!(field(&l, "depend"), Some("b = 2"));
        assert_eq!(field(&l, "nope"), None);
    }

    #[test]
    fn home_builds() {
        assert!(built_at_home("format = 2\nbuilddir = /home/x/build\n"));
        assert!(built_at_home("startdir = /root/r\n"));
        assert!(!built_at_home("builddir = /build\nstartdir = /var/lib/labyrinth-publish/x\n"));
        assert!(!built_at_home("builddir= /home/x\n"));
    }

    #[test]
    fn content_ignores_version_date_and_size() {
        let a = "#mtree\n/set type=file\n./.PKGINFO time=1 size=3\n./usr/bin/x time=5 size=9 mode=755 sha256digest=aa\n";
        let b = "#mtree\n./usr/bin/x mode=755 sha256digest=aa time=6 size=10\n";
        let pa = pkginfo_lines("pkgname = x\npkgver = 1-1\nbuilddate = 1\ndepend = glibc\n");
        let pb = pkginfo_lines("pkgname = x\npkgver = 1-2\nbuilddate = 2\ndepend = glibc\n");
        assert_eq!(content_digest(a, &pa), content_digest(b, &pb));
        let c = "./usr/bin/x mode=755 sha256digest=bb\n";
        assert_ne!(content_digest(a, &pa), content_digest(c, &pa));
        // the digest of the same text the script hashed
        assert_eq!(content_digest("", &[]), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }
}
