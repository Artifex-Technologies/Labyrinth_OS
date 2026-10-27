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

    def test_only_our_own_packages_are_installed(self):
        ours = self.pkg("yggdrasil-settings")
        self.assertEqual(self.m.check_pkg_files([ours], lambda p: "yggdrasil-settings"), ["yggdrasil-settings"])
        for name in ["sudo", "systemd", "evil-yggdrasil", "openssh"]:
            with self.assertRaises(self.m.Refused, msg=name):
                self.m.check_pkg_files([self.pkg(name)], lambda p, n=name: n)
        link = os.path.join(self.d, "link-1-1-x86_64.pkg.tar.zst")
        os.symlink(ours, link)
        with self.assertRaises(self.m.Refused):
            self.m.check_pkg_files([link], lambda p: "yggdrasil-settings")
        with self.assertRaises(self.m.Refused):
            self.m.check_pkg_files(["relative.pkg.tar.zst"], lambda p: "yggdrasil-x")

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
        for verb, mf in [("up", "/home/zlothra/evil.json"), ("up", "/var/lib/limbo/build-agent/../../../etc/x.json"), ("hold", ""), ("up", "")]:
            with self.assertRaises(self.m.Refused, msg=(verb, mf)):
                self.m.limbo_argv(verb, mf)

    def test_deploy_only_to_user_side_desktop_files(self):
        self.assertEqual(self.m.deploy_mode("/usr/bin/yggdrasil-settings"), 0o755)
        self.assertEqual(self.m.deploy_mode("/usr/share/yggdrasil/theme/x.png"), 0o644)
        self.assertEqual(self.m.deploy_mode("/etc/yggdrasil/tuning.json"), 0o644)
        for bad in ["/usr/lib/yggdrasil/tune", "/usr/local/bin/labyrinth-x", "/etc/sudoers.d/x", "/usr/bin/sudo",
                    "/etc/systemd/system/x.service", "/usr/share/yggdrasil/../../../etc/passwd", "/usr/share/polkit-1/actions/x.policy",
                    "/etc/yggdrasil/../sudoers"]:
            with self.assertRaises(self.m.Refused, msg=bad):
                self.m.deploy_mode(bad)


if __name__ == "__main__":
    unittest.main()
