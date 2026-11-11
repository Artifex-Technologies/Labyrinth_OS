"""Tests for ipconfig's layout. Run: python packages/tools/python/tests/test_ipconfig.py"""
import contextlib
import importlib.machinery
import importlib.util
import io
import os
import unittest

path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "ipconfig")
loader = importlib.machinery.SourceFileLoader("ipconfig", path)
spec = importlib.util.spec_from_loader("ipconfig", loader)
ipc = importlib.util.module_from_spec(spec)
loader.exec_module(ipc)


class Layout(unittest.TestCase):
    def test_a_second_value_lines_up_under_the_first(self):
        # fixed 2026-11-11: a second gateway or DNS server was printed two columns right of the first one's value
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            ipc.line("Default Gateway", "fe80::1")
            print(" " * ipc.CONTINUED + "192.0.2.1")
        first, second = out.getvalue().splitlines()
        self.assertEqual(first, "   Default Gateway. . . . . . . . . .  : fe80::1")
        self.assertEqual(first.index("fe80"), second.index("192"))

    def test_lease(self):
        self.assertEqual(ipc.lease(3725), "01:02:05 remaining")
        self.assertEqual(ipc.lease(90061), "1 days 01:01:01 remaining")
        self.assertIsNone(ipc.lease(4294967295))
        self.assertIsNone(ipc.lease(1.5))


if __name__ == "__main__":
    unittest.main()
