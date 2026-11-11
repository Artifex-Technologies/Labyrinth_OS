//! tests/test_labyrinth_agent.py carried over, a test for each hole closed on 2026-11-11, and random command lines checked
//! against the allow-list of audit.rs.

use super::audit::{self, standard_mock, Mock};
use super::*;
use crate::testutil::TempDir;

fn b(s: &str) -> B {
    s.as_bytes().to_vec()
}

fn bs(v: &[&str]) -> Vec<B> {
    v.iter().map(|s| b(s)).collect()
}

fn refusal(r: Result<impl std::fmt::Debug, Fail>) -> String {
    match r {
        Err(Fail::Refused(m)) => show(&m),
        other => panic!("not refused: {other:?}"),
    }
}

fn mock_with(files: &[(&str, &str)]) -> Mock {
    Mock { files: files.iter().map(|(p, n)| (b(p), b(n))).collect(), ..Mock::default() }
}

// ---- the Python tests, carried over

#[test]
fn only_our_own_packages_are_installed() {
    let ours = "/d/yggdrasil-settings-1-1-x86_64.pkg.tar.zst";
    let mut m = mock_with(&[(ours, "yggdrasil-settings")]);
    assert_eq!(check_pkg_files(&bs(&[ours]), &mut m).unwrap(), bs(&["yggdrasil-settings"]));
    for name in ["sudo", "systemd", "evil-yggdrasil", "openssh"] {
        let p = format!("/d/{name}-1-1-x86_64.pkg.tar.zst");
        let mut m = mock_with(&[(&p, name)]);
        assert!(matches!(check_pkg_files(&bs(&[&p]), &mut m), Err(Fail::Refused(_))), "{name}");
    }
    // a link is not a regular file
    let mut m = mock_with(&[]);
    assert!(refusal(check_pkg_files(&bs(&["/d/link-1-1-x86_64.pkg.tar.zst"]), &mut m)).ends_with("not a regular file"));
    assert!(refusal(check_pkg_files(&bs(&["relative.pkg.tar.zst"]), &mut m)).ends_with("not an absolute path to a package file"));
}

#[test]
fn repository_packages_but_never_remote_access_tools() {
    check_pkg_names(&bs(&["gnu-efi", "python-pillow"])).unwrap();
    for bad in ["tailscale", "ngrok", "cloudflared", "anydesk-bin", "openssh", "x11vnc", "samba", "--overwrite=*", "a b"] {
        assert!(check_pkg_names(&bs(&[bad])).is_err(), "{bad}");
    }
}

#[test]
fn services_of_the_ecosystem_only() {
    assert_eq!(service_argv(b"restart", &bs(&["heimdall.service"])).unwrap(), bs(&["systemctl", "restart", "heimdall.service"]));
    assert_eq!(service_argv(b"enable-now", &bs(&["yggdrasil-tune"])).unwrap(), bs(&["systemctl", "enable", "--now", "yggdrasil-tune"]));
    assert_eq!(
        service_argv(b"user-enable", &bs(&["labyrinth-voiced.service"])).unwrap(),
        bs(&["systemctl", "--global", "enable", "labyrinth-voiced.service"])
    );
    assert_eq!(service_argv(b"daemon-reload", &[]).unwrap(), bs(&["systemctl", "daemon-reload"]));
    for (action, units) in [
        ("restart", vec!["sshd"]),
        ("edit", vec!["heimdall"]),
        ("link", vec!["/tmp/x.service"]),
        ("restart", vec!["heimdall;id"]),
        ("restart", vec![]),
        ("mask", vec!["ufw"]),
        ("stop", vec!["ufw.service"]),
    ] {
        assert!(service_argv(action.as_bytes(), &bs(&units)).is_err(), "{action} {units:?}");
    }
}

#[test]
fn the_journal_is_only_read() {
    assert_eq!(
        logs_argv(&bs(&["-u", "heimdall", "-n", "50", "--since", "-1h"])).unwrap(),
        bs(&["journalctl", "--no-pager", "-u", "heimdall", "-n", "50", "--since", "-1h"])
    );
    assert_eq!(logs_argv(&bs(&["-b", "-1", "-k"])).unwrap(), bs(&["journalctl", "--no-pager", "-b", "-1", "-k"]));
    assert_eq!(logs_argv(&bs(&["_COMM=sudo", "--output=cat"])).unwrap(), bs(&["journalctl", "--no-pager", "_COMM=sudo", "--output=cat"]));
    for bad in [
        vec!["--vacuum-time=1s"],
        vec!["--rotate"],
        vec!["--flush"],
        vec!["--setup-keys"],
        vec!["-D", "/tmp/x"],
        vec!["--file", "/x"],
        vec!["-u"],
    ] {
        assert!(logs_argv(&bs(&bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn limbo_manifests_from_the_agents_folders_only() {
    assert_eq!(
        limbo_argv(b"up", b"/var/lib/limbo/build-agent/test.json").unwrap(),
        bs(&["limbo", "up", "/var/lib/limbo/build-agent/test.json"])
    );
    assert_eq!(limbo_argv(b"doctor", b"").unwrap(), bs(&["limbo", "doctor"]));
    for (verb, mf) in [("up", "/home/alex/evil.json"), ("up", "/var/lib/limbo/build-agent/../../../etc/x.json"), ("hold", ""), ("up", "")] {
        assert!(limbo_argv(verb.as_bytes(), mf.as_bytes()).is_err(), "{verb} {mf}");
    }
}

#[test]
fn deploy_only_to_user_side_desktop_files() {
    assert_eq!(deploy_mode(b"/usr/bin/yggdrasil-settings").unwrap(), 0o755);
    assert_eq!(deploy_mode(b"/usr/share/yggdrasil/theme/x.png").unwrap(), 0o644);
    assert_eq!(deploy_mode(b"/etc/yggdrasil/tuning.json").unwrap(), 0o644);
    for bad in [
        "/usr/lib/yggdrasil/tune",
        "/usr/local/bin/labyrinth-x",
        "/etc/sudoers.d/x",
        "/usr/bin/sudo",
        "/etc/systemd/system/x.service",
        "/usr/share/yggdrasil/../../../etc/passwd",
        "/usr/share/polkit-1/actions/x.policy",
        "/etc/yggdrasil/../sudoers",
    ] {
        assert!(deploy_mode(bad.as_bytes()).is_err(), "{bad}");
    }
}

// ---- the holes closed on 2026-11-11 (each also a test in tests/test_labyrinth_agent.py)

/// journalctl's -n takes the next word only when it is a count: `-n --vacuum-time=1s` vacuumed the journal as root.
#[test]
fn a_count_after_n_or_nothing() {
    for (args, want) in [
        (vec!["-n", "5"], vec!["-n", "5"]),
        (vec!["-n", "+5"], vec!["-n", "+5"]),
        (vec!["--lines", "all"], vec!["--lines", "all"]),
        (vec!["-n", "-k"], vec!["-n", "-k"]),
        (vec!["-n"], vec!["-n"]),
        (vec!["--lines=20", "-q"], vec!["--lines=20", "-q"]),
    ] {
        let mut full = bs(&["journalctl", "--no-pager"]);
        full.extend(bs(&want));
        assert_eq!(logs_argv(&bs(&args)).unwrap(), full, "{args:?}");
    }
    for bad in
        ["--vacuum-time=1s", "--vacuum-size=1", "--rotate", "--flush", "--setup-keys", "--cursor-file=/etc/x", "--relinquish-var", "-D"]
    {
        for n in ["-n", "--lines"] {
            assert!(refusal(logs_argv(&bs(&[n, bad]))).contains("not a read-only journal option"), "{n} {bad}");
        }
    }
}

/// Every pattern is matched whole: a trailing newline used to pass Python's `$`.
#[test]
fn a_newline_at_the_end_is_not_a_name() {
    assert!(!ours(b"yggdrasil-x\n") && ours(b"yggdrasil-x"));
    assert!(check_pkg_names(&bs(&["htop\n"])).is_err());
    assert!(service_argv(b"restart", &bs(&["heimdall\n"])).is_err());
    assert!(deploy_mode(b"/usr/bin/yggdrasil-x\n").is_err());
    assert!(deploy_mode(b"/etc/yggdrasil/x.json\n").is_err());
    assert!(logs_argv(&bs(&["_COMM=x\n"])).is_err());
    assert!(!boot_id(b"5\n") && boot_id(b"5") && boot_id(b"-1") && boot_id(&[b'a'; 32]));
}

/// The agent window's own units (its closing timer) are the owner's; Heimdall is started, not stopped.
#[test]
fn the_window_and_the_watcher() {
    for action in ["stop", "restart", "start", "disable", "status"] {
        assert!(refusal(service_argv(action.as_bytes(), &bs(&["labyrinth-agent-window-close.timer"]))).contains("the owner's"), "{action}");
    }
    for action in ["stop", "disable", "disable-now", "user-disable"] {
        assert!(refusal(service_argv(action.as_bytes(), &bs(&["heimdall.service"]))).contains("Heimdall watches the agent"));
        assert!(service_argv(action.as_bytes(), &bs(&["limbo.service"])).is_ok());
    }
    for action in ["start", "restart", "reload", "status", "is-active", "enable", "enable-now", "user-enable"] {
        assert!(service_argv(action.as_bytes(), &bs(&["heimdall.service"])).is_ok(), "{action}");
    }
    // a unit is never an option or a path
    assert!(service_argv(b"stop", &bs(&["--all"])).is_err());
    assert!(service_argv(b"stop", &bs(&["labyrinth-x/../y"])).is_err());
}

/// What pkg-add would install with a name is checked too: sshfs brings openssh.
#[test]
fn remote_access_also_not_as_a_dependency() {
    let mut m = standard_mock();
    assert!(refusal(decide(b"pkg-add", &bs(&["sshfs"]), &mut m)).starts_with("openssh, which sshfs would install"));
    assert!(refusal(decide(b"pkg-add", &bs(&["htop", "remmina"]), &mut m)).starts_with("gnome-remote-desktop, which htop, remmina"));
    assert!(refusal(decide(b"pkg-add", &bs(&["python-lsp"]), &mut m)).starts_with("python-lsp-server"));
    assert_eq!(
        decide(b"pkg-add", &bs(&["htop"]), &mut m).unwrap(),
        Plan::Runs(vec![bs(&["pacman", "-S", "--needed", "--noconfirm", "htop"])])
    );
    m.sp_fails = true;
    assert_eq!(refusal(decide(b"pkg-add", &bs(&["htop"]), &mut m)), "pacman cannot say what htop would install");
    // Heimdall's catalogue
    for n in [
        "headscale",
        "netbird",
        "netbird-bin",
        "frpc",
        "frps",
        "bore",
        "bore-cli",
        "tor",
        "cloudflared-bin",
        "gnome-remote-desktop",
        "krfb",
        "ttyd",
    ] {
        assert!(never_add(n.as_bytes()), "{n}");
    }
    assert!(check_transaction(&bs(&["x"]), b"fuse3\n  htop  \n\n").is_ok());
}

#[test]
fn limbo_reads_a_manifest_only_from_the_agents_folders() {
    for verb in ["doctor", "status"] {
        assert!(refusal(limbo_argv(verb.as_bytes(), b"/etc/shadow")).contains("a manifest only from under"));
        assert!(limbo_argv(verb.as_bytes(), b"--state-dir=/tmp/x").is_err());
        assert_eq!(
            limbo_argv(verb.as_bytes(), b"/etc/limbo/validation/a.json").unwrap(),
            bs(&["limbo", verb, "/etc/limbo/validation/a.json"])
        );
    }
}

#[test]
fn rollback_counts_in_ascii_digits() {
    let mut m = Mock::default();
    assert!(decide(b"rollback", &bs(&["dry-run", "12"]), &mut m).is_ok());
    for n in ["²", "١٢", "𝟏", "", "1a", "-1"] {
        assert!(decide(b"rollback", &bs(&["dry-run", n]), &mut m).is_err(), "{n}");
    }
}

#[test]
fn deploy_paths_are_clean_and_not_where_root_looks() {
    for bad in ["/usr/share/yggdrasil/./x", "/usr/share/yggdrasil/x/", "/usr/share/yggdrasil//x", "/usr/share/yggdrasil/."] {
        assert!(refusal(deploy_mode(bad.as_bytes())).ends_with("not a clean path"), "{bad}");
    }
    for bad in [
        "/usr/share/yggdrasil/gpu-unlock/supported.json",
        "/usr/share/yggdrasil/gpu-unlock",
        "/usr/share/labyrinth/pkgbuilds/yggdrasil-x/PKGBUILD",
        "/usr/share/labyrinth/grub-evodevo/install.sh",
    ] {
        assert!(refusal(deploy_mode(bad.as_bytes())).contains("root uses what is there"), "{bad}");
    }
    assert_eq!(deploy_mode(b"/usr/share/yggdrasil/gpu-unlocked.json").unwrap(), 0o644);
    let mut m = standard_mock();
    assert_eq!(
        refusal(decide(b"deploy", &bs(&["/tmp/x", "/usr/bin/yggdrasil-spell"]), &mut m)),
        "/usr/bin/yggdrasil-spell: root runs it (/usr/lib/systemd/system/made-up.service)"
    );
    assert!(decide(b"deploy", &bs(&["/tmp/x", "/usr/bin/yggdrasil-spellcheck"]), &mut m).is_ok());
}

#[test]
fn a_program_is_named_whole() {
    assert!(names(b"ExecStart=/usr/bin/yggdrasil-spell global merge\n", b"yggdrasil-spell"));
    assert!(names(b"Exec=yggdrasil-spell", b"yggdrasil-spell"));
    assert!(names(b"yggdrasil-spell", b"yggdrasil-spell"));
    assert!(!names(b"Requires=yggdrasil-spell-global.service yggdrasil-spell.socket", b"yggdrasil-spell"));
    assert!(!names(b"/usr/bin/yggdrasil-spells", b"yggdrasil-spell"));
    assert!(!names(b"", b"x") && !names(b"abc", b""));
}

#[test]
fn root_run_places_are_searched_in_order() {
    let d = TempDir::new();
    let a = d.p("a");
    let b2 = d.p("b");
    std::fs::create_dir_all(format!("{a}/x.service.d")).unwrap();
    std::fs::create_dir_all(format!("{a}/deep/er")).unwrap();
    std::fs::create_dir_all(&b2).unwrap();
    std::fs::write(format!("{a}/x.service.d/override.conf"), "ExecStart=\nExecStart=/usr/bin/ygg-one\n").unwrap();
    std::fs::write(format!("{a}/deep/er/z.conf"), "ygg-two\n").unwrap();
    std::fs::write(format!("{b2}/hook.hook"), "Exec = /usr/bin/ygg-one --x\n").unwrap();
    std::os::unix::fs::symlink("/dev/null", format!("{a}/masked.service")).unwrap();
    let places = [a.as_str(), b2.as_str(), "/nonexistent-place"];
    assert_eq!(root_runs_in(&places, b"ygg-one"), Some(format!("{a}/x.service.d/override.conf").into_bytes()));
    assert_eq!(root_runs_in(&places, b"ygg-two"), None, "one level of subfolders only");
    assert_eq!(root_runs_in(&places, b"ygg-three"), None);
}

#[test]
fn programs_get_a_clean_environment() {
    let vars: Vec<(OsString, OsString)> = [
        ("PATH", "/home/agent/bin:/usr/bin"),
        ("LD_PRELOAD", "/home/agent/x.so"),
        ("SYSTEMD_PAGER", "sh"),
        ("LIMBO_GO_BINARY", "/home/agent/x"),
        ("HOME", "/home/agent"),
        ("SUDO_USER", "claude-agent"),
        ("SUDO_UID", "1001"),
        ("TERM", "xterm-256color"),
        ("USER", "root"),
        ("SUDO_UID", "0"),
    ]
    .iter()
    .map(|(k, v)| (OsString::from(k), OsString::from(v)))
    .collect();
    let env = child_env(vars);
    let want: Vec<(OsString, OsString)> = [
        ("PATH", "/usr/bin"),
        ("HOME", "/root"),
        ("USER", "root"),
        ("SUDO_UID", "1001"),
        ("SUDO_USER", "claude-agent"),
        ("TERM", "xterm-256color"),
    ]
    .iter()
    .map(|(k, v)| (OsString::from(k), OsString::from(v)))
    .collect();
    assert_eq!(env, want);
    let env = child_env([(OsString::from("TERM"), OsString::from("../../tmp/x"))]);
    assert!(!env.iter().any(|(k, _)| k == "TERM"));
}

#[test]
fn long_journal_lines_are_cut() {
    assert_eq!(log_bytes(b"short"), b"short");
    let long = vec![b'x'; LOG_MAX + 10];
    let cut = log_bytes(&long);
    assert_eq!(cut.len(), LOG_MAX + " [10 bytes more]".len());
    assert!(cut.ends_with(b" [10 bytes more]"));
}

#[test]
fn the_caller_from_sudo() {
    let o = |s: &str| Some(OsString::from(s));
    assert_eq!(caller(None, None).unwrap(), None);
    assert_eq!(caller(o("0"), o("0")).unwrap(), None);
    for (u, g) in [(o(""), o("1")), (o("1"), None), (None, o("1")), (o("x"), o("1")), (o("4294967295"), o("1")), (o("12345678901"), o("1"))]
    {
        assert_eq!(refusal(caller(u, g)), "SUDO_UID and SUDO_GID do not name an account");
    }
    // an id no account has: its number, its own group only
    let c = caller(o("3999999"), o("3999998")).unwrap().unwrap();
    assert_eq!((c.uid, c.gid, c.groups.clone(), c.name.clone()), (3999999, 3999998, vec![3999998], b("3999999")));
}

/// read_as as root of nobody: what it is given, refusals for links and folders (the child is the test process itself).
#[test]
fn the_source_is_read_by_a_child() {
    let d = TempDir::new();
    std::fs::write(d.p("f"), b"data\xff").unwrap();
    std::os::unix::fs::symlink(d.p("f"), d.p("l")).unwrap();
    assert_eq!(read_as(d.p("f").as_bytes(), None).unwrap(), b"data\xff");
    assert!(refusal(read_as(d.p("l").as_bytes(), None)).ends_with("not a regular file"));
    assert!(refusal(read_as(d.0.to_string_lossy().as_bytes(), None)).ends_with("not a regular file"));
    let missing = refusal(read_as(d.p("none").as_bytes(), None));
    assert!(missing.ends_with("root cannot read it (No such file or directory)"), "{missing}");
}

#[test]
fn names_are_shown_as_python_showed_them() {
    assert_eq!(show(b"a\xffb\xc3\xa9"), "a\\udcffbé");
    assert_eq!(repr(b"a\xff"), "'a\\udcff'");
    assert_eq!(repr(b"it's"), "\"it's\"");
    assert_eq!(repr("é\u{200b}\n\x7f".as_bytes()), "'é\\u200b\\n\\x7f'");
    assert_eq!(refusal(check_pkg_names(&[b"Bad\xff".to_vec()])), "'Bad\\udcff': not a package name");
}

#[test]
fn name_lines_as_the_pattern_finds_them() {
    assert_eq!(name_line("Name            : yggdrasil-x\nVersion : 1\n"), "yggdrasil-x");
    assert_eq!(name_line("Other\nName:\n  heimdall-git rest"), "heimdall-git");
    assert_eq!(name_line(" Name : x\n"), "");
    assert_eq!(name_line("Name : \nName : y"), "Name", "\\s* goes over the line break");
    assert_eq!(name_line("Names : x\nName\u{2003}:\u{a0}z\u{2028}w"), "z");
}

#[test]
fn the_help_is_the_scripts() {
    assert!(DOC.starts_with("labyrinth-agent: the root actions the engineering agent"));
    assert!(DOC.ends_with("read as the account sudo ran this for\n"));
}

// ---- random command lines against audit.rs's allow-list

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Words a command line may have: every one the gate knows, near misses, and tricks.
const WORDS: &[&str] = &[
    "pkg-install",
    "pkg-add",
    "service",
    "logs",
    "maintenance",
    "limbo",
    "rollback",
    "firewall-status",
    "deploy",
    "-h",
    "--help",
    "--",
    "",
    "-",
    "on",
    "off",
    "status",
    "--for",
    "--reason",
    "--config",
    "--config=/tmp/x",
    "-config",
    "4h",
    "why",
    "up",
    "down",
    "doctor",
    "list",
    "dry-run",
    "12",
    "-1",
    "+5",
    "all",
    "²",
    "start",
    "stop",
    "restart",
    "reload",
    "is-active",
    "enable",
    "disable",
    "enable-now",
    "disable-now",
    "user-enable",
    "user-disable",
    "daemon-reload",
    "mask",
    "edit",
    "link",
    "heimdall",
    "heimdall.service",
    "heimdall-x.timer",
    "labyrinth-agent-window-close.timer",
    "labyrinth-agent",
    "labyrinth-x.service",
    "yggdrasil-tune",
    "yggdrasil-x@a.service",
    "limbo.service",
    "sarina-service",
    "ollama",
    "systemd-sysctl",
    "sshd",
    "--now",
    "--global",
    "-u",
    "--unit",
    "-n",
    "--lines",
    "-o",
    "--output",
    "--since",
    "--until",
    "-p",
    "--priority",
    "-t",
    "--identifier",
    "-g",
    "--grep",
    "-b",
    "--boot",
    "-k",
    "-q",
    "-f",
    "-r",
    "-x",
    "-a",
    "--all",
    "--no-hostname",
    "--case-sensitive",
    "--case-sensitive=no",
    "--dmesg",
    "--follow",
    "--vacuum-time=1s",
    "--vacuum-size=1",
    "--vacuum-files=1",
    "--rotate",
    "--flush",
    "--sync",
    "--setup-keys",
    "--relinquish-var",
    "--smart-relinquish-var",
    "--update-catalog",
    "--cursor-file=/etc/x",
    "--cursor-file",
    "-D",
    "--directory=/tmp",
    "--file=/x",
    "--root=/",
    "-M",
    "--machine=x",
    "--vac",
    "--rot",
    "-kr",
    "-nu",
    "-n5",
    "-n=5",
    "--lines=5",
    "-u=x",
    "_COMM=sudo",
    "_PID=1",
    "PRIORITY=3",
    "MESSAGE=x",
    "_SYSTEMD_UNIT=a/b",
    "0123456789abcdef0123456789abcdef",
    "0123456789ABCDEF0123456789ABCDEF",
    "gnu-efi",
    "htop",
    "sshfs",
    "remmina",
    "tailscale",
    "python-lsp",
    "openssh",
    "xorg-server",
    "--overwrite=*",
    "extra/htop",
    "htop\n",
    "/tmp/yggdrasil-a-1-1-x86_64.pkg.tar.zst",
    "/tmp/heimdall-git-1-1-x86_64.pkg.tar.xz",
    "/tmp/openssh-1-1-x86_64.pkg.tar.zst",
    "/tmp/look-1-1-x86_64.pkg.tar.zst",
    "/tmp/sudo-1-1-x86_64.pkg.tar.zst",
    "/tmp/none-1-1-x86_64.pkg.tar.zst",
    "/tmp/../tmp/yggdrasil-a-1-1-x86_64.pkg.tar.zst",
    "tmp/yggdrasil-a-1-1-x86_64.pkg.tar.zst",
    "/var/lib/limbo/build-agent/t.json",
    "/etc/limbo/validation/a.json",
    "/var/lib/limbo/build-agent/../../../../etc/shadow.json",
    "/etc/shadow",
    "/var/lib/limbo/build-agent/t.json\n",
    "/usr/bin/yggdrasil-settings",
    "/usr/bin/yggdrasil-spell",
    "/usr/bin/yggdrasil-gametime",
    "/usr/bin/minos",
    "/usr/bin/ygg-x",
    "/usr/bin/sudo",
    "/usr/bin/yggdrasil-",
    "/usr/bin/yggdrasil-X",
    "/usr/share/yggdrasil/a/b.png",
    "/usr/share/yggdrasil/gpu-unlock/supported.json",
    "/usr/share/yggdrasil/./gpu-unlock/s.json",
    "/usr/share/labyrinth/pkgbuilds/x/PKGBUILD",
    "/usr/share/labyrinth/grub-evodevo/install.sh",
    "/usr/share/icons/a/b.svg",
    "/usr/share/icons/.svg",
    "/usr/share/applications/x.desktop",
    "/usr/share/wayland-sessions/y.desktop",
    "/etc/yggdrasil/a.json",
    "/etc/yggdrasil/x.conf",
    "/etc/yggdrasil/../sudoers.d/x.conf",
    "/etc/sudoers.d/x",
    "/usr/share/yggdrasil/x/",
];

fn random_word(r: &mut Rng) -> B {
    match r.below(12) {
        // a word of the list
        0..=8 => WORDS[r.below(WORDS.len())].as_bytes().to_vec(),
        // a word of the list with a byte changed or added
        9 => {
            let mut w = WORDS[r.below(WORDS.len())].as_bytes().to_vec();
            let c = [b'\n', b'/', b'.', b'-', b'=', 0xff, 0xc3, b' ', b'@', b'A'][r.below(10)];
            if w.is_empty() || r.below(2) == 0 {
                w.push(c);
            } else {
                let i = r.below(w.len());
                w[i] = c;
            }
            w
        }
        // random bytes
        10 => (0..r.below(12)).map(|_| r.next() as u8).collect(),
        // a long word
        _ => vec![b'a'; 1 + r.below(5000)],
    }
}

/// Random command lines: whatever decide() lets through must be within audit.rs's allow-list.
#[test]
fn random_command_lines_stay_in_the_allow_list() {
    let n: usize = std::env::var("LABYRINTH_AGENT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(60_000);
    let mut r = Rng(0x9E37_79B9_7F4A_7C15 ^ std::env::var("LABYRINTH_AGENT_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1));
    let mut ran = 0;
    for _ in 0..n {
        let len = 1 + r.below(6);
        let mut words: Vec<B> = (0..len).map(|_| random_word(&mut r)).collect();
        // most lines start with a command
        if r.below(4) != 0 {
            words[0] = WORDS[r.below(9)].as_bytes().to_vec();
        }
        let mut m = standard_mock();
        if decide(&words[0], &words[1..], &mut m).is_ok() {
            ran += 1;
            audit::check_one(&words);
        }
    }
    assert!(ran > n / 50, "too few command lines got through ({ran}): the cases are not reaching the actions");
}

/// The allow-list does catch what the hole let through.
#[test]
fn the_allow_list_catches_the_old_holes() {
    let mut m = standard_mock();
    let plan = |a: &[&str]| Plan::Runs(vec![bs(a)]);
    for bad in [
        plan(&["journalctl", "--no-pager", "-n", "--vacuum-time=1s"]),
        plan(&["journalctl", "--no-pager", "--lines", "--rotate"]),
        plan(&["systemctl", "stop", "labyrinth-agent-window-close.timer"]),
        plan(&["systemctl", "disable", "--now", "heimdall.service"]),
        plan(&["limbo", "status", "/etc/shadow"]),
        plan(&["pacman", "-S", "--needed", "--noconfirm", "sshfs"]),
        plan(&["heimdalld", "maintenance", "on", "--config", "/tmp/x"]),
        Plan::Deploy { src: b("/etc/shadow"), dest: b("/usr/bin/yggdrasil-spell"), mode: 0o755 },
        Plan::Deploy { src: b("/x"), dest: b("/usr/share/yggdrasil/gpu-unlock/supported.json"), mode: 0o644 },
        Plan::Deploy { src: b("/x"), dest: b("/usr/bin/yggdrasil-x\n"), mode: 0o755 },
    ] {
        assert!(audit::audit(&bad, &mut m).is_err(), "{bad:?}");
    }
    assert!(audit::audit(&plan(&["journalctl", "--no-pager", "-n", "5", "-u", "--rotate"]), &mut m).is_ok(), "--rotate is -u's value");
}
