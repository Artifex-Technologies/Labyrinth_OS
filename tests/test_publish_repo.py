"""Tests for scripts/publish-repo.py's file names. Run: python tests/test_publish_repo.py"""
import importlib.machinery
import importlib.util
import os
import unittest
from pathlib import Path

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
_loader = importlib.machinery.SourceFileLoader("publish_repo", os.path.join(ROOT, "scripts", "publish-repo.py"))
_spec = importlib.util.spec_from_loader("publish_repo", _loader)
pub = importlib.util.module_from_spec(_spec)
_loader.exec_module(pub)


class OnlineNames(unittest.TestCase):
    def test_an_epoch_gets_a_name_github_keeps(self):
        f = Path("/x/yggdrasil-2048-git-1:0.1.0.r12.gabc1234-1-x86_64.pkg.tar.zst")
        name = pub.online_name(f)
        self.assertEqual(name, "yggdrasil-2048-git-1.0.1.0.r12.gabc1234-1-x86_64.pkg.tar.zst")
        self.assertTrue(pub.SAFE_NAME.match(name))
        self.assertFalse(pub.SAFE_NAME.match(f.name), "the colon is what GitHub would rename")

    def test_other_names_stay(self):
        f = Path("/x/saga-md-0.1.0.r467-1-x86_64.pkg.tar.zst")
        self.assertEqual(pub.online_name(f), f.name)


if __name__ == "__main__":
    unittest.main()
