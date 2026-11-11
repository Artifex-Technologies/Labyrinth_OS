"""labyrinth-agent's checks without root: which package files, package names, units, journal options, Limbo manifests and
deploy targets the agent may use. Nothing is installed or changed."""
import importlib.machinery
import importlib.util
import os
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.join(HERE, "..", "profile", "airootfs", "usr", "local", "sbin", "labyrinth-agent")


def load():
    loader = importlib.machinery.SourceFileLoader("labyrinth_agent", TOOL)
    spec = importlib.util.spec_from_loader("labyrinth_agent", loader)
    m = importlib.util.module_from_spec(spec)
    loader.exec_module(m)
    return m


class Agent(unittest.TestCase):
    def setUp(self):
        self.m = load()
        self.d = tempfile.mkdtemp()

    def pkg(self, name):
        p = os.path.join(self.d, name + "-1-1-x86_64.pkg.tar.zst")
        open(p, "w").close()
        return p

    def copier(self):
        """pkg-install's copies as Stage.copy makes them (read as the caller: here, this account), in a temporary folder."""
        d = tempfile.mkdtemp()

        def copy(p, i):
            c = os.path.join(d, "%d-%s" % (i, os.path.basename(p)))
            with open(c, "wb") as f:
                f.write(self.m.read_as(p, None))
            return c
        return copy

    def test_only_our_own_packages_are_installed(self):
        ours = self.pkg("yggdrasil-settings")
        names, copies = self.m.check_pkg_files([ours], lambda p: "yggdrasil-settings", self.copier())
        self.assertEqual(names, ["yggdrasil-settings"])
        self.assertNotEqual(copies, [ours])
        for name in ["sudo", "systemd", "evil-yggdrasil", "openssh"]:
            with self.assertRaises(self.m.Refused, msg=name):
                self.m.check_pkg_files([self.pkg(name)], lambda p, n=name: n, self.copier())
        link = os.path.join(self.d, "link-1-1-x86_64.pkg.tar.zst")
        os.symlink(ours, link)
        with self.assertRaises(self.m.Refused):
            self.m.check_pkg_files([link], lambda p: "yggdrasil-settings", self.copier())
        with self.assertRaises(self.m.Refused):
            self.m.check_pkg_files(["relative.pkg.tar.zst"], lambda p: "yggdrasil-x", self.copier())

    def test_repository_packages_but_never_remote_access_tools(self):
        self.assertEqual(self.m.check_pkg_names(["gnu-efi", "python-pillow"]), ["gnu-efi", "python-pillow"])
        for bad in ["tailscale", "ngrok", "cloudflared", "anydesk-bin", "openssh", "x11vnc", "samba", "--overwrite=*", "a b"]:
            with self.assertRaises(self.m.Refused, msg=bad):
                self.m.check_pkg_names([bad])

    def test_services_of_the_ecosystem_only(self):
        self.assertEqual(self.m.service_argv("restart", ["heimdall.service"]), ["systemctl", "restart", "heimdall.service"])
        self.assertEqual(self.m.service_argv("enable-now", ["yggdrasil-tune"]), ["systemctl", "enable", "--now", "yggdrasil-tune"])
        self.assertEqual(self.m.service_argv("user-enable", ["labyrinth-voiced.service"]), ["systemctl", "--global", "enable", "labyrinth-voiced.service"])
        self.assertEqual(self.m.service_argv("daemon-reload", []), ["systemctl", "daemon-reload"])
        for action, units in [("restart", ["sshd"]), ("edit", ["heimdall"]), ("link", ["/tmp/x.service"]), ("restart", ["heimdall;id"]),
                              ("restart", []), ("mask", ["ufw"]), ("stop", ["ufw.service"])]:
            with self.assertRaises(self.m.Refused, msg=(action, units)):
                self.m.service_argv(action, units)

    def test_the_journal_is_only_read(self):
        self.assertEqual(self.m.logs_argv(["-u", "heimdall", "-n", "50", "--since", "-1h"]),
                         ["journalctl", "--no-pager", "-u", "heimdall", "-n", "50", "--since", "-1h"])
        self.assertEqual(self.m.logs_argv(["-b", "-1", "-k"]), ["journalctl", "--no-pager", "-b", "-1", "-k"])
        self.assertEqual(self.m.logs_argv(["_COMM=sudo", "--output=cat"]), ["journalctl", "--no-pager", "_COMM=sudo", "--output=cat"])
        for bad in [["--vacuum-time=1s"], ["--rotate"], ["--flush"], ["--setup-keys"], ["-D", "/tmp/x"], ["--file", "/x"], ["-u"]]:
            with self.assertRaises(self.m.Refused, msg=bad):
                self.m.logs_argv(bad)

    def test_limbo_manifests_from_the_agents_folders_only(self):
        self.assertEqual(self.m.limbo_argv("up", "/var/lib/limbo/build-agent/test.json"), ["limbo", "up", "/var/lib/limbo/build-agent/test.json"])
        self.assertEqual(self.m.limbo_argv("doctor", ""), ["limbo", "doctor"])
        for verb, mf in [("up", "/home/alex/evil.json"), ("up", "/var/lib/limbo/build-agent/../../../etc/x.json"), ("hold", ""), ("up", "")]:
            with self.assertRaises(self.m.Refused, msg=(verb, mf)):
                self.m.limbo_argv(verb, mf)

    def test_deploy_only_to_user_side_desktop_files(self):
        self.assertEqual(self.m.deploy_mode("/usr/bin/yggdrasil-settings"), 0o755)
        self.assertEqual(self.m.deploy_mode("/usr/share/yggdrasil/theme/x.png"), 0o644)
        self.assertEqual(self.m.deploy_mode("/etc/yggdrasil/greeter.conf"), 0o644)
        for bad in ["/usr/lib/yggdrasil/tune", "/usr/local/bin/labyrinth-x", "/etc/sudoers.d/x", "/usr/bin/sudo",
                    "/etc/systemd/system/x.service", "/usr/share/yggdrasil/../../../etc/passwd", "/usr/share/polkit-1/actions/x.policy",
                    "/etc/yggdrasil/../sudoers"]:
            with self.assertRaises(self.m.Refused, msg=bad):
                self.m.deploy_mode(bad)


class HolesClosed(unittest.TestCase):
    """The holes found while porting labyrinth-agent to Rust (2026-11-11), closed in the script and the port alike (the same
    tests are in packages/tools/src/agent/tests.rs)."""

    def setUp(self):
        self.m = load()
        self.d = tempfile.mkdtemp()

    def refused(self, f, *a):
        with self.assertRaises(self.m.Refused) as cm:
            f(*a)
        return str(cm.exception)

    def test_a_count_after_n_or_nothing(self):
        # journalctl's -n takes the next word only when it is a count: `logs -n --vacuum-time=1s` vacuumed the journal as root
        self.assertEqual(self.m.logs_argv(["-n", "5"]), ["journalctl", "--no-pager", "-n", "5"])
        self.assertEqual(self.m.logs_argv(["--lines", "all", "-n", "+3"]), ["journalctl", "--no-pager", "--lines", "all", "-n", "+3"])
        self.assertEqual(self.m.logs_argv(["-n", "-k"]), ["journalctl", "--no-pager", "-n", "-k"])
        for bad in ["--vacuum-time=1s", "--rotate", "--flush", "--setup-keys", "--cursor-file=/etc/x", "--relinquish-var", "-D"]:
            for n in ["-n", "--lines"]:
                self.assertIn("not a read-only journal option", self.refused(self.m.logs_argv, [n, bad]), (n, bad))

    def test_a_newline_at_the_end_is_not_a_name(self):
        self.assertFalse(self.m.OURS.fullmatch("yggdrasil-x\n"))
        for f, a in [(self.m.check_pkg_names, (["htop\n"],)), (self.m.service_argv, ("restart", ["heimdall\n"])),
                     (self.m.deploy_mode, ("/usr/bin/yggdrasil-x\n",)), (self.m.deploy_mode, ("/etc/yggdrasil/x.json\n",)),
                     (self.m.logs_argv, (["_COMM=x\n"],))]:
            self.refused(f, *a)

    def test_the_window_and_the_watcher(self):
        for action in ["stop", "restart", "start", "disable"]:
            self.assertIn("the owner's", self.refused(self.m.service_argv, action, ["labyrinth-agent-window-close.timer"]))
        for action in ["stop", "disable", "disable-now", "user-disable"]:
            self.assertIn("Heimdall watches the agent", self.refused(self.m.service_argv, action, ["heimdall.service"]))
            self.m.service_argv(action, ["limbo.service"])
        for action in ["start", "restart", "reload", "status", "enable", "enable-now"]:
            self.m.service_argv(action, ["heimdall.service"])

    def test_remote_access_also_not_as_a_dependency(self):
        why = self.refused(self.m.check_transaction, ["sshfs"], b"sshfs\nfuse3\nopenssh\n")
        self.assertTrue(why.startswith("openssh, which sshfs would install"), why)
        self.m.check_transaction(["htop"], b"htop\n  ncurses \n\n")
        for n in ["headscale", "netbird", "netbird-bin", "frpc", "frps", "bore", "bore-cli", "tor", "cloudflared-bin", "gnome-remote-desktop",
                  "krfb", "ttyd", "python-lsp-server"]:
            self.assertTrue(self.m.NEVER_ADD.fullmatch(n), n)

    def test_limbo_reads_a_manifest_only_from_the_agents_folders(self):
        for verb in ["doctor", "status"]:
            self.assertIn("a manifest only from under", self.refused(self.m.limbo_argv, verb, "/etc/shadow"))
            self.assertEqual(self.m.limbo_argv(verb, "/etc/limbo/validation/a.json"), ["limbo", verb, "/etc/limbo/validation/a.json"])

    def test_deploy_paths_are_clean_and_not_where_root_looks(self):
        for bad in ["/usr/share/yggdrasil/./x", "/usr/share/yggdrasil/x/", "/usr/share/yggdrasil//x"]:
            self.assertTrue(self.refused(self.m.deploy_mode, bad).endswith("not a clean path"), bad)
        for bad in ["/usr/share/yggdrasil/gpu-unlock/supported.json", "/usr/share/labyrinth/pkgbuilds/x/PKGBUILD",
                    "/usr/share/labyrinth/grub-evodevo/install.sh"]:
            self.assertIn("root uses what is there", self.refused(self.m.deploy_mode, bad), bad)

    def test_programs_root_runs(self):
        units = os.path.join(self.d, "units")
        os.makedirs(os.path.join(units, "x.service.d"))
        with open(os.path.join(units, "yggdrasil-spell-global.service"), "w") as f:
            f.write("[Service]\nExecStart=/usr/bin/yggdrasil-spell global merge\n")
        with open(os.path.join(units, "x.service.d", "o.conf"), "w") as f:
            f.write("ExecStart=yggdrasil-dropin\n")
        os.symlink("/dev/null", os.path.join(units, "masked.service"))
        self.assertEqual(self.m.root_runs("yggdrasil-spell", (units,)), os.path.join(units, "yggdrasil-spell-global.service"))
        self.assertEqual(self.m.root_runs("yggdrasil-dropin", (units,)), os.path.join(units, "x.service.d", "o.conf"))
        self.assertIsNone(self.m.root_runs("yggdrasil-spellcheck", (units,)))
        self.assertIsNone(self.m.root_runs("yggdrasil-spell", ("/nonexistent",)))

    def test_programs_get_a_clean_environment(self):
        env = self.m.child_env({"PATH": "/home/agent/bin", "LD_PRELOAD": "/x.so", "SYSTEMD_PAGER": "sh", "LIMBO_GO_BINARY": "/x",
                                "HOME": "/home/agent", "SUDO_USER": "claude-agent", "TERM": "xterm", "USER": "root"})
        self.assertEqual(env, {"PATH": "/usr/bin", "HOME": "/root", "USER": "root", "SUDO_USER": "claude-agent", "TERM": "xterm"})
        self.assertNotIn("TERM", self.m.child_env({"TERM": "../../tmp/x"}))

    def test_long_journal_lines_are_cut(self):
        self.assertEqual(self.m.log_bytes("short"), b"short")
        cut = self.m.log_bytes("x" * (self.m.LOG_MAX + 10))
        self.assertTrue(cut.endswith(b" [10 bytes more]") and len(cut) == self.m.LOG_MAX + 16)

    def test_rollback_counts_in_ascii_digits(self):
        self.assertTrue(self.m.DIGITS.fullmatch("12"))
        for n in ["\u00b2", "\u0661\u0662", "1a", ""]:
            self.assertFalse(self.m.DIGITS.fullmatch(n), n)

    def test_the_caller_from_sudo(self):
        self.assertIsNone(self.m.caller({}))
        self.assertIsNone(self.m.caller({"SUDO_UID": "0", "SUDO_GID": "0"}))
        for env in [{"SUDO_UID": "", "SUDO_GID": "1"}, {"SUDO_UID": "1"}, {"SUDO_UID": "x", "SUDO_GID": "1"},
                    {"SUDO_UID": "4294967295", "SUDO_GID": "1"}]:
            self.refused(self.m.caller, env)
        self.assertEqual(self.m.caller({"SUDO_UID": "3999999", "SUDO_GID": "3999998"}), (3999999, 3999998, [3999998], "3999999"))

    def test_the_source_is_read_by_a_child(self):
        f = os.path.join(self.d, "f")
        with open(f, "wb") as out:
            out.write(b"data\xff")
        os.symlink(f, os.path.join(self.d, "l"))
        self.assertEqual(self.m.read_as(f, None), b"data\xff")
        self.assertTrue(self.refused(self.m.read_as, os.path.join(self.d, "l"), None).endswith("not a regular file"))
        self.assertTrue(self.refused(self.m.read_as, self.d, None).endswith("not a regular file"))
        self.assertTrue(self.refused(self.m.read_as, os.path.join(self.d, "none"), None).endswith("cannot read it (No such file or directory)"))

    def test_pkg_install_checks_and_installs_copies(self):
        """The name is read from the copy and the copy is what is installed: a file changed after it was copied is not."""
        p = os.path.join(self.d, "yggdrasil-x-1-1-x86_64.pkg.tar.zst")
        with open(p, "w") as f:
            f.write("Name : yggdrasil-x\n")
        copy = Agent.copier(self)

        def name_of(c):
            with open(p, "w") as f:  # the caller changes the file once it has been copied
                f.write("Name : openssh\n")
            return open(c).read().split(":")[1].strip()
        names, copies = self.m.check_pkg_files([p], name_of, copy)
        self.assertEqual(names, ["yggdrasil-x"])
        self.assertEqual(open(copies[0]).read(), "Name : yggdrasil-x\n")
        self.assertEqual(self.m.STAGE, "/var/cache/labyrinth-agent")
        if os.geteuid() != 0:
            with self.assertRaises((OSError, self.m.Refused)):  # a folder only root may enter: not here
                self.m.stage_dir()

    def test_settings_guarded_by_polkit_are_not_deployed(self):
        for g in ["case-fans.json", "gpu-fan.json", "power-limits.json", "tuning.json", "protected-accounts", "protected-packages"]:
            self.assertIn("only with the owner's password (polkit)", self.refused(self.m.deploy_mode, "/etc/yggdrasil/" + g), g)
        self.assertEqual(self.m.deploy_mode("/etc/yggdrasil/tuning2.json"), 0o644)

    def test_limbo_manifests_are_roots_alone(self):
        if os.path.exists("/etc/passwd") and not os.path.islink("/etc"):
            self.assertIsNone(self.m.root_only("/etc/passwd"))
        f = os.path.join(self.d, "m.json")
        open(f, "w").close()
        why = self.m.root_only(f)
        self.assertTrue("is not root's alone" in why or "is a link" in why, why)
        self.assertTrue(self.m.root_only("/nonexistent-x/m.json").endswith("No such file or directory"))
        if os.path.islink("/bin"):
            self.assertEqual(self.m.root_only("/bin/sh"), "/bin is a link")

    def test_the_help_is_the_rust_ports(self):
        doc = os.path.join(HERE, "..", "packages", "tools", "src", "doc", "labyrinth-agent.txt")
        if os.path.exists(doc):
            self.assertEqual(open(doc).read(), self.m.__doc__)


if __name__ == "__main__":
    unittest.main()
