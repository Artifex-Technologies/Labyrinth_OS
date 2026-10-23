"""Checks on Labyrinth's catalog for Agora (the app store), which Agora reads through the packages integration.
Run: python tests/test_agora_catalog.py"""
import json
import os
import re
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHARE = os.path.join(ROOT, "profile", "airootfs", "usr", "share")
CATALOG = json.load(open(os.path.join(SHARE, "labyrinth", "agora", "catalog.json")))
COMPONENTS = json.load(open(os.path.join(SHARE, "labyrinth", "components.json")))
APPS = CATALOG["apps"]


def listed_in(edition):
    return [a for a in APPS if not a.get("editions") or edition in a["editions"]]


class Catalog(unittest.TestCase):
    def test_every_app_has_a_category_and_featured_apps_exist(self):
        self.assertGreater(len(APPS), 50)
        for a in APPS:
            self.assertTrue(a.get("id") and a.get("name") and a.get("category") and a.get("subcategory"), a.get("id"))
        ids = {a["id"] for a in APPS}
        for f in CATALOG["featured"]:
            self.assertIn(f["id"], ids)

    def test_package_names_are_plain(self):
        plain = re.compile(r"^[A-Za-z0-9._+@-]+$")
        for a in APPS:
            src = a["source"]
            for name in src.get("packages", []) + [src.get("app", "x"), src.get("id", "x")]:
                self.assertRegex(name, plain, a["id"])

    def test_components_in_the_catalog_exist(self):
        known = {c["id"] for c in COMPONENTS["components"]}
        for a in APPS:
            if a["source"]["type"] == "component":
                self.assertIn(a["source"]["id"], known, a["id"])

    def test_chymaera_only_apps_are_not_listed_in_labyrinth(self):
        lab, chy = listed_in("labyrinth"), listed_in("chymaera")
        self.assertFalse(any(a["id"] in ("ghidra", "wireshark") for a in lab))
        self.assertTrue(any(a["id"] == "ghidra" for a in chy) and any(a["id"] == "krita" for a in chy))
        for a in lab:
            text = (a.get("description", "") + " " + a.get("subtitle", "")).lower()
            self.assertNotIn("pentest", text, "nothing in the Labyrinth listing talks about penetration testing")

    def test_the_integration_points_at_this_catalog(self):
        conf = open(os.path.join(SHARE, "yggdrasil", "integrations", "labyrinth-packages.conf")).read()
        self.assertIn("catalog = /usr/share/labyrinth/agora/catalog.json", conf)
        self.assertIn("kind = packages", conf)


if __name__ == "__main__":
    unittest.main()
