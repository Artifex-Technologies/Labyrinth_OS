"""Versions (docs/VERSIONING.md): the helper every recipe uses (scripts/package-version.sh), its copies, the recipes that use it,
the system version in os-release, the os-release update in labyrinth-tools and scripts/release.sh.
Run: python tests/test_package_version.py"""
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
YGG = Path(os.environ.get("YGGDRASIL_DIR", ROOT.parent / "Yggdrasil"))
HELPER = ROOT / "scripts" / "package-version.sh"
STORE = ROOT / "profile" / "airootfs" / "usr" / "share" / "labyrinth" / "pkgbuilds"
# data whose date is its version (the keys it holds, the dictionaries' snapshot)
DATED = {"labyrinth-keyring", "yggdrasil-spell-packs"}
ENV = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid", GIT_COMMITTER_NAME="t",
           GIT_COMMITTER_EMAIL="t@example.invalid", GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")


def block(text):
    m = re.search(r"^# --- package version \(scripts/package-version.sh\) ---$.*?^# --- end of package version ---$", text, re.M | re.S)
    return m.group(0) if m else None


def recipes():
    dirs = [ROOT / "packages" / "pkgbuilds", STORE] + ([YGG / "packages" / "pkgbuilds"] if YGG.is_dir() else [])
    return [p for d in dirs for p in sorted(d.glob("*/PKGBUILD"))]


def sh(cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd, env=ENV, check=True, capture_output=True, text=True).stdout.strip()


class Recipes(unittest.TestCase):
    def test_the_copies_of_the_helper_are_the_same(self):
        canon = block(HELPER.read_text())
        self.assertTrue(canon)
        copies = sorted(STORE.glob("*/PKGBUILD")) + ([YGG / "scripts" / "package-version.sh"] if YGG.is_dir() else [])
        for p in copies:
            self.assertEqual(block(p.read_text()), canon, f"{p}: copy the lines between the markers from {HELPER}")

    def test_every_recipe_works_out_its_version(self):
        for p in recipes():
            t = p.read_text()
            name = p.parent.name
            if name in DATED:
                self.assertNotIn("_version ", t, name)
                continue
            self.assertRegex(t, r"(?m)^_version ", f"{name}: no _version line (docs/VERSIONING.md)")
            self.assertNotRegex(t, r"\.r%s|rev-list --count HEAD\)", f"{name}: the old commit-count version")
            if re.search(r"(?m)^pkgver\(\)", t):
                # a recipe that builds a clone: once the clone is there, for the parent (prepare) and makepkg's pkgver step
                for fn in ("pkgver", "prepare"):
                    body = re.search(rf"(?ms)^{fn}\(\) \{{\n(.*?)^\}}", t)
                    self.assertTrue(body and "_version " in body.group(1), f"{name}: {fn}() does not call _version")
                self.assertIn('_version "$(_srcdir)/', t, f"{name}: the clone for the packaging step")

    def test_the_version_line_names_every_folder_a_recipe_builds_from(self):
        for p in recipes():
            t = p.read_text()
            line = re.search(r'(?m)^_version "\$startdir".*$', t)
            if not line:
                continue
            for var, _ in re.findall(r'(?m)^(_[a-z_]+)="\$startdir/(\.\./\.\./[^"]+)"$', t):
                self.assertIn(f'"${var}"', line.group(0), f"{p.parent.name}: {var} is not on its _version line")

    def test_the_static_lines_start_at_one(self):
        for p in recipes():
            if p.parent.name in DATED:
                continue
            t = p.read_text()
            self.assertRegex(t, r"(?m)^pkgver=1\.0\.0$", p.parent.name)
            self.assertRegex(t, r"(?m)^pkgrel=1$", p.parent.name)


class Helper(unittest.TestCase):
    """_version against two small repositories: A (the source, with a release) and B (a recipe in another repository)."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        a, b = self.tmp / "A", self.tmp / "B"
        for d in ("packages/x", "packages/y", "packages/z", "pkgbuilds/px"):
            (a / d).mkdir(parents=True)
        (b / "r").mkdir(parents=True)
        sh(["git", "init", "-q", str(a)]); sh(["git", "init", "-q", str(b)])
        (a / "packages/x/Cargo.toml").write_text('[dependencies]\nyy = { path = "../y" }\n# zz = { path = "../z" }\n')
        for f in ("packages/y/f", "packages/z/f", "pkgbuilds/px/PKGBUILD"):
            (a / f).write_text("a\n")
        self.commit(a, "before the first release")
        (a / "VERSION").write_text("1.0.0\n")
        self.commit(a, "release")
        for f in ("packages/y/f", "packages/z/f", "pkgbuilds/px/PKGBUILD"):
            (a / f).write_text("b\n")
            self.commit(a, f)
        (b / "r" / "PKGBUILD").write_text("a\n")
        self.commit(b, "recipe")

    def commit(self, repo, msg):
        sh(["git", "-C", str(repo), "add", "-A"])
        sh(["git", "-C", str(repo), "commit", "-qm", msg])

    def version(self, startdir, *args):
        q = " ".join(f"'{a}'" for a in args)
        return sh(["bash", "-c", f". '{HELPER}'; pkgver=keep; pkgrel=0; startdir='{startdir}'; _version {q}; echo \"$pkgver-$pkgrel\""],
                  cwd=self.tmp)

    def test_folders_their_crates_and_the_recipe(self):
        own = self.tmp / "A/pkgbuilds/px"
        # y (a path dependency of x; z is only in a comment) and the recipe: 1 + 2
        self.assertEqual(self.version(own, "A", str(self.tmp / "A/packages/x")), "1.0.0-3")
        self.assertEqual(self.version(own, "A", "packages/x"), "1.0.0-3", "paths from the repository's top")
        self.assertEqual(self.version(own, "A"), "1.0.0-4", "no folder: the whole repository")

    def test_a_recipe_in_another_repository_counts_by_date(self):
        self.assertEqual(self.version(self.tmp / "B/r", "A"), "1.0.0-5")
        self.assertEqual(self.version(self.tmp / "B/r", "A", "packages/z"), "1.0.0-3")

    def test_an_apps_own_recipe_built_from_a_clone_is_not_counted_twice(self):
        sh(["git", "clone", "-q", str(self.tmp / "A"), str(self.tmp / "C")])
        self.assertEqual(self.version(self.tmp / "A/pkgbuilds/px", "C"), "1.0.0-4")

    def test_a_release_starts_again_at_one(self):
        (self.tmp / "A/VERSION").write_text("1.1.0\n")
        self.commit(self.tmp / "A", "release 1.1.0")
        self.assertEqual(self.version(self.tmp / "A/pkgbuilds/px", "A", "packages/x"), "1.1.0-1")

    def test_no_clone_yet_leaves_the_recipe_alone(self):
        self.assertEqual(self.version(self.tmp, str(self.tmp / "missing")), "keep-0")

    def test_srcdir_before_makepkg_sets_it(self):
        r = self.tmp / "A/pkgbuilds/px"
        out = sh(["bash", "-c", f". '{HELPER}'; pkgname=p; startdir={r}; BUILDDIR={r}; _srcdir; BUILDDIR=/b; _srcdir; srcdir=/s; _srcdir"])
        self.assertEqual(out.split("\n"), [f"{r}/src", "/b/p/src", "/s"])


class SystemVersion(unittest.TestCase):
    def test_os_release_names_the_system_version(self):
        v = (ROOT / "VERSION").read_text().strip()
        self.assertRegex(v, r"^\d+\.\d+\.\d+$")
        osr = (ROOT / "profile/airootfs/etc/os-release").read_text()
        self.assertIn(f"\nVERSION_ID={v}\n", osr)
        self.assertIn(f'\nVERSION="{v[:-2] if v.endswith(".0") else v}"\n', osr)
        self.assertIn("\nID=labyrinth\n", osr)
        if YGG.is_dir():
            self.assertEqual((YGG / "VERSION").read_text().strip(), v, "Yggdrasil's VERSION is the system's")

    def run_install(self, text, version):
        d = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d)
        (d / "os-release").write_text(text)
        script = (ROOT / "packages/pkgbuilds/labyrinth-tools/labyrinth-tools.install").read_text()
        script = script.replace("/etc/os-release", str(d / "os-release")).replace("/etc/.os-release", str(d / ".os-release"))
        (d / "x.install").write_text(script)
        subprocess.run(["sh", "-c", f". '{d}/x.install'; post_upgrade {version} 0.1.0-2"], check=True)
        return (d / "os-release").read_text(), sorted(p.name for p in d.iterdir())

    def test_labyrinth_tools_writes_it_on_an_update(self):
        before = 'NAME="Labyrinth OS"\nPRETTY_NAME="Labyrinth OS"\nID=labyrinth\nBUILD_ID=rolling\nIMAGE_VERSION=2026.10.13\n'
        after, files = self.run_install(before, "1.0.0-3")
        self.assertEqual(after, 'NAME="Labyrinth OS"\nPRETTY_NAME="Labyrinth OS"\nVERSION="1.0"\nVERSION_ID=1.0.0\nID=labyrinth\n'
                                'BUILD_ID=rolling\nIMAGE_VERSION=2026.10.13\n')
        self.assertEqual(files, ["os-release", "x.install"], "no temporary file left")
        again, _ = self.run_install(after, "1:1.1.1-1")
        self.assertIn('VERSION="1.1.1"\nVERSION_ID=1.1.1\n', again)
        self.assertEqual(again.count("VERSION="), 2, "replaced, not added (IMAGE_VERSION stays)")

    def test_another_system_keeps_its_own(self):
        other = 'NAME="LykanOS"\nID=lykanos\nVERSION="2"\n'
        self.assertEqual(self.run_install(other, "1.0.0-1")[0], other)


class Release(unittest.TestCase):
    """scripts/release.sh in a copy of the layout: Labyrinth_OS (its script and os-release), Yggdrasil and Heimdall."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        lab, ygg, hei = self.tmp / "Labyrinth_OS", self.tmp / "Yggdrasil", self.tmp / "Heimdall"
        (lab / "scripts").mkdir(parents=True)
        shutil.copy2(ROOT / "scripts/release.sh", lab / "scripts/release.sh")
        (lab / "profile/airootfs/etc").mkdir(parents=True)
        (lab / "profile/airootfs/etc/os-release").write_text('NAME="Labyrinth OS"\nVERSION="1.0"\nVERSION_ID=1.0.0\nID=labyrinth\n')
        for r in (lab, ygg, hei):
            r.mkdir(exist_ok=True)
            (r / "VERSION").write_text("1.0.0\n")
            sh(["git", "init", "-q", "-b", "main", str(r)])
            sh(["git", "-C", str(r), "add", "-A"])
            sh(["git", "-C", str(r), "commit", "-qm", "1.0.0"])
        self.lab, self.ygg, self.hei = lab, ygg, hei

    def release(self, *args, ok=True):
        p = subprocess.run([str(self.lab / "scripts/release.sh"), *args], env=dict(ENV, YGGDRASIL_DIR=str(self.ygg)),
                           capture_output=True, text=True)
        self.assertEqual(p.returncode == 0, ok, p.stdout + p.stderr)
        return p

    def test_an_app_release(self):
        self.release("heimdall", "minor")
        self.assertEqual((self.hei / "VERSION").read_text(), "1.1.0\n")
        self.assertEqual(sh(["git", "-C", str(self.hei), "describe", "--exact-match"]), "v1.1.0")
        self.assertEqual(sh(["git", "-C", str(self.hei), "status", "--porcelain"]), "")
        self.assertEqual((self.lab / "VERSION").read_text(), "1.0.0\n", "the system is not touched")

    def test_a_system_release_is_both_repositories_and_os_release(self):
        self.release("system", "patch")
        for r in (self.lab, self.ygg):
            self.assertEqual((r / "VERSION").read_text(), "1.0.1\n")
            self.assertEqual(sh(["git", "-C", str(r), "describe", "--exact-match"]), "v1.0.1")
        osr = (self.lab / "profile/airootfs/etc/os-release").read_text()
        self.assertIn('VERSION="1.0.1"\nVERSION_ID=1.0.1\n', osr)
        self.release("system", "1.1.0")
        self.assertIn('VERSION="1.1"\nVERSION_ID=1.1.0\n', (self.lab / "profile/airootfs/etc/os-release").read_text())

    def test_sarina_release_writes_its_go_copy(self):
        sar = self.tmp / "Sarina"
        (sar / "go/internal/version").mkdir(parents=True)
        (sar / "VERSION").write_text("1.0.0\n")
        go = sar / "go/internal/version/version.go"
        go.write_text('package version\n\n// the VERSION file\nconst Version = "1.0.0"\n')
        helpers = sar / "python/sarina_helpers"
        (helpers / "sarina_helpers").mkdir(parents=True)
        (helpers / "pyproject.toml").write_text('[project]\nname = "sarina-helpers"\nversion = "1.0.0"\n')
        (helpers / "sarina_helpers/__init__.py").write_text("__version__ = '1.0.0'\n")
        sh(["git", "init", "-q", "-b", "main", str(sar)])
        sh(["git", "-C", str(sar), "add", "-A"])
        sh(["git", "-C", str(sar), "commit", "-qm", "1.0.0"])
        go.write_text(go.read_text() + "// not committed\n")
        self.release("sarina", "patch", ok=False)
        self.assertEqual((sar / "VERSION").read_text(), "1.0.0\n", "nothing half done")
        sh(["git", "-C", str(sar), "checkout", "--", "go"])
        self.release("sarina", "minor")
        self.assertEqual((sar / "VERSION").read_text(), "1.1.0\n")
        self.assertEqual(go.read_text(), 'package version\n\n// the VERSION file\nconst Version = "1.1.0"\n')
        self.assertIn('version = "1.1.0"', (helpers / "pyproject.toml").read_text())
        self.assertEqual((helpers / "sarina_helpers/__init__.py").read_text(), "__version__ = '1.1.0'\n")
        self.assertEqual(sh(["git", "-C", str(sar), "describe", "--exact-match"]), "v1.1.0")
        self.assertEqual(sh(["git", "-C", str(sar), "status", "--porcelain"]), "", "both in the release commit")

    def test_refuses_what_is_not_newer_or_not_a_version(self):
        for bad in ("1.0.0", "0.9.0", "1.1", "v1.1.0"):
            self.release("heimdall", bad, ok=False)
        self.release("nobody", "patch", ok=False)
        self.assertEqual((self.hei / "VERSION").read_text(), "1.0.0\n")
        (self.ygg / "VERSION").write_text("1.0.5\n")
        sh(["git", "-C", str(self.ygg), "commit", "-qam", "out of step"])
        self.release("system", "patch", ok=False)
        self.assertEqual((self.lab / "VERSION").read_text(), "1.0.0\n", "nothing half done")


if __name__ == "__main__":
    unittest.main()
