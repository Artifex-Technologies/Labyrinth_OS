"""Vivaldi's privacy policies on Labyrinth's image (docs/PHONE_HOME.md): valid JSON, only the policies meant, of the right types.
Run: python tests/test_vivaldi_policies.py"""
import json
import os
import unittest

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "etc", "vivaldi", "policies")

# what each file may hold: locked (managed) only what costs nothing but data sent away; defaults (recommended) what the person
# may want back (prefetching speed, WebRTC on a local network)
MANAGED = {"MetricsReportingEnabled": False, "UrlKeyedAnonymizedDataCollectionEnabled": False, "SpellCheckServiceEnabled": False,
           "AlternateErrorPagesEnabled": False, "PrivacySandboxPromptEnabled": False, "PrivacySandboxAdTopicsEnabled": False,
           "PrivacySandboxSiteEnabledAdsEnabled": False, "PrivacySandboxAdMeasurementEnabled": False}
RECOMMENDED = {"NetworkPredictionOptions": 2, "WebRtcIPHandling": "default_public_interface_only"}


class Policies(unittest.TestCase):
    def load(self, kind):
        with open(os.path.join(ROOT, kind, "labyrinth-privacy.json")) as f:
            return json.load(f)

    def test_locked_settings_only_send_less(self):
        self.assertEqual(self.load("managed"), MANAGED)

    def test_defaults_the_person_can_change(self):
        self.assertEqual(self.load("recommended"), RECOMMENDED)

    def test_nothing_else_is_shipped(self):
        for kind in ("managed", "recommended"):
            self.assertEqual(os.listdir(os.path.join(ROOT, kind)), ["labyrinth-privacy.json"])


if __name__ == "__main__":
    unittest.main()
