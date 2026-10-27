"""Tests for the "ask" network mode: rules, the question engine, the socket service and the notification agent.
Run: python tests/test_labyrinth_appnet_ask.py"""
import asyncio
import json
import os
import stat
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_labyrinth_apps import ROOT, load_module  # noqa: E402

BIN = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin")
appnet = load_module("labyrinth_appnet_for_ask", os.path.join(BIN, "labyrinth-appnet"))
ask = load_module("labyrinth_appnet_ask", os.path.join(BIN, "labyrinth-appnet-ask"))
agent = load_module("labyrinth_appnet_ask_agent", os.path.join(BIN, "labyrinth-appnet-ask-agent"))

LOG = ("labask:pdfcraft IN= OUT=wlan0 SRC=192.168.1.5 DST=93.184.216.34 LEN=60 TOS=0x00 PREC=0x00 TTL=64 ID=1 DF "
       "PROTO=TCP SPT=40000 DPT=443 WINDOW=64240 RES=0x00 SYN URGP=0 UID=1000 GID=1000")
LOG6 = ("labask:pdfcraft IN= OUT=wlan0 SRC=fe80::1 DST=2606:4700::1111 LEN=80 TC=0 HOPLIMIT=64 FLOWLBL=0 "
        "PROTO=UDP SPT=5000 DPT=53 LEN=40 UID=1000 GID=1000")


class Fixture(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = self.tmp.name
        appnet.STATE = os.path.join(t, "rules.json")
        appnet.ASK_STATE = os.path.join(t, "ask.json")
        appnet.save_state({"1000": {"pdfcraft": "ask", "other": "block"}})
        self.nft_calls = []
        self.clock = [100.0]
        self.eng = ask.Engine(appnet, self.nft_calls.append, now=lambda: self.clock[0])

    def tearDown(self):
        self.tmp.cleanup()


class RulesTests(unittest.TestCase):
    def test_ask_rules_pass_loopback_and_allowed_then_log_and_drop(self):
        r = appnet.rule_text(1000, "pdfcraft", "ask")
        self.assertEqual(len(r), 6)
        self.assertIn('oifname "lo" tcp dport { 8899, 8930, 11434 } drop', r[0])  # no relay through Ollama, Sarina, Heimdall
        self.assertIn('oifname "lo" accept', r[1])
        self.assertIn("@" + appnet.set_name(1000, "pdfcraft", 4), r[2])
        self.assertIn("@" + appnet.set_name(1000, "pdfcraft", 6), r[3])
        self.assertIn('log prefix "labask:pdfcraft " flags skuid', r[4])
        self.assertIn(" drop ", r[5])

    def test_ruleset_declares_the_sets_with_what_you_allowed(self):
        rules = {"1000": {"pdfcraft": "ask", "x": "block"}}
        asked = {"1000": {"pdfcraft": {"allow": {"1.2.3.4": "a", "2606:4700::1111": ""}, "deny": {}}}}
        text = appnet.ruleset(rules, exists=lambda p: True, ask=asked)
        self.assertIn("type ipv4_addr; flags timeout; elements = { 1.2.3.4 }", text)
        self.assertIn("type ipv6_addr; flags timeout; elements = { 2606:4700::1111 }", text)
        self.assertLess(text.index("set ask_1000_pdfcraft"), text.index("chain output"))
        self.assertNotIn("ask_1000_x", text)

    def test_set_names_differ_for_apps_that_clean_to_the_same_text(self):
        self.assertNotEqual(appnet.set_name(1, "a-b", 4), appnet.set_name(1, "a_b", 4))

    def test_state_file_is_private_and_bad_entries_are_dropped(self):
        with tempfile.TemporaryDirectory() as t:
            p = os.path.join(t, "a.json")
            appnet.save_ask({"1000": {"app": {"allow": {"1.2.3.4": "n"}, "deny": {}}}}, p)
            self.assertEqual(stat.S_IMODE(os.stat(p).st_mode), 0o600)
            open(p, "w").write(json.dumps({"1000": {"app": {"allow": {"not an ip": "x", "5.6.7.8": "y"}}, "../bad": {"allow": {}}}}))
            self.assertEqual(appnet.load_ask(p), {"1000": {"app": {"allow": {"5.6.7.8": "y"}, "deny": {}}}})


class ParseTests(unittest.TestCase):
    def test_log_lines(self):
        self.assertEqual(ask.parse_log(LOG), {"uid": "1000", "app": "pdfcraft", "ip": "93.184.216.34", "port": 443, "proto": "TCP"})
        v6 = ask.parse_log(LOG6)
        self.assertEqual((v6["ip"], v6["port"], v6["proto"]), ("2606:4700::1111", 53, "UDP"))
        self.assertIsNone(ask.parse_log("IN=eth0 OUT= SRC=1.1.1.1 DST=2.2.2.2 PROTO=TCP"))
        self.assertIsNone(ask.parse_log("something else"))


class EngineTests(Fixture):
    def ev(self, ip="93.184.216.34", app="pdfcraft"):
        return {"uid": "1000", "app": app, "ip": ip, "port": 443, "proto": "TCP"}

    def test_one_question_per_app_and_destination(self):
        q = self.eng.asked(self.ev())
        self.assertEqual((q["app"], q["ip"]), ("pdfcraft", "93.184.216.34"))
        self.assertIsNone(self.eng.asked(self.ev()))                      # same destination, still pending
        self.assertIsNotNone(self.eng.asked(self.ev("10.0.0.1")))         # another destination
        self.assertIsNone(self.eng.asked(self.ev(app="other")))           # not in ask mode: never asked

    def test_allow_remembers_and_adds_to_the_live_set(self):
        q = self.eng.asked(self.ev())
        ok, _ = self.eng.decide("1000", q["id"], "allow", "example.com")
        self.assertTrue(ok)
        self.assertEqual(self.nft_calls[0][:4], ["add", "element", "inet", "labyrinth_apps"])
        self.assertEqual(self.nft_calls[0][4], appnet.set_name(1000, "pdfcraft", 4))
        self.assertEqual(self.nft_calls[0][-2], "93.184.216.34")
        self.assertEqual(appnet.load_ask()["1000"]["pdfcraft"]["allow"], {"93.184.216.34": "example.com"})
        self.assertIsNone(self.eng.asked(self.ev()))                      # decided: not asked again

    def test_ipv6_goes_to_the_v6_set(self):
        q = self.eng.asked(self.ev("2606:4700::1111"))
        self.eng.decide("1000", q["id"], "allow")
        self.assertTrue(self.nft_calls[0][4].endswith("_6"))

    def test_deny_is_remembered_and_never_asked_again(self):
        q = self.eng.asked(self.ev())
        self.eng.decide("1000", q["id"], "deny")
        self.assertEqual(self.nft_calls, [])
        self.assertIn("93.184.216.34", appnet.load_ask()["1000"]["pdfcraft"]["deny"])
        self.assertIsNone(self.eng.asked(self.ev()))

    def test_closing_the_notification_asks_again_after_the_cooldown(self):
        q = self.eng.asked(self.ev())
        self.eng.decide("1000", q["id"], "ignore")
        self.assertIsNone(self.eng.asked(self.ev()))
        self.clock[0] += ask.COOLDOWN + 1
        self.assertIsNotNone(self.eng.asked(self.ev()))

    def test_only_the_owner_answers_and_only_real_questions(self):
        q = self.eng.asked(self.ev())
        self.assertFalse(self.eng.decide("1001", q["id"], "allow")[0])    # someone else
        self.assertFalse(self.eng.decide("1000", 999, "allow")[0])        # no such question
        self.assertEqual(self.nft_calls, [])
        self.assertFalse(self.eng.decide("1000", q["id"], "bogus")[0])        # (the question is consumed)

    def test_allow_all_puts_the_app_back_to_any_connection(self):
        applied = []
        appnet.apply = lambda rules=None: applied.append(rules) or 0
        q = self.eng.asked(self.ev())
        self.assertTrue(self.eng.decide("1000", q["id"], "allow-all")[0])
        self.assertEqual(appnet.load_state()["1000"], {"other": "block"})
        self.assertEqual(len(applied), 1)

    def test_forget_asks_again(self):
        q = self.eng.asked(self.ev())
        self.eng.decide("1000", q["id"], "allow")
        self.assertTrue(self.eng.forget("1000", "pdfcraft", "93.184.216.34"))
        self.assertEqual(self.nft_calls[-1][0], "delete")
        self.assertIsNotNone(self.eng.asked(self.ev()))
        self.assertFalse(self.eng.forget("1000", "pdfcraft", "not an ip"))


class AgentTests(unittest.TestCase):
    def test_titles_and_message(self):
        self.assertEqual(agent.app_title("com.mitchellh.ghostty"), "Ghostty")
        self.assertEqual(agent.app_title("spotify-launcher"), "Spotify Launcher")
        q = {"app": "pdfcraft", "ip": "1.2.3.4", "port": 443, "proto": "TCP"}
        self.assertEqual(agent.message(q, "example.com"), ("Pdfcraft wants to connect to example.com", "1.2.3.4 port 443 (TCP)"))
        self.assertEqual(agent.message(q, ""), ("Pdfcraft wants to connect to 1.2.3.4", "TCP port 443"))


class ServiceTests(unittest.IsolatedAsyncioTestCase):
    """The real socket service, fed a fake kernel log, answered by the real agent code with a fake notify-send."""

    async def test_question_notification_answer_and_set_update(self):
        with tempfile.TemporaryDirectory() as t:
            appnet.STATE, appnet.ASK_STATE = os.path.join(t, "r.json"), os.path.join(t, "a.json")
            appnet.save_state({"1000": {"pdfcraft": "ask"}})
            sock = os.path.join(t, "s.sock")
            nft = []
            eng = ask.Engine(appnet, nft.append)
            srv = ask.Server(eng)
            server = await asyncio.start_unix_server(srv.handle, sock)
            notify = os.path.join(t, "notify")
            seen = os.path.join(t, "seen")
            with open(notify, "w") as f:
                f.write('#!/bin/sh\nprintf "%s\\n" "$*" > ' + seen + '\necho allow\n')
            os.chmod(notify, 0o755)
            agent.SOCKET, agent.NOTIFY = sock, notify
            agent.reverse_name = lambda ip, wait=1.5: "example.com"
            loop = asyncio.get_running_loop()
            r, w = await asyncio.open_unix_connection(sock)
            w.write(b'{"op":"subscribe"}\n')
            await w.drain()
            await asyncio.sleep(0.1)
            q = eng.asked(ask.parse_log(LOG))
            await srv.push(q)
            got = json.loads(await asyncio.wait_for(r.readline(), 3))
            self.assertEqual((got["type"], got["app"], got["ip"]), ("ask", "pdfcraft", "93.184.216.34"))
            await loop.run_in_executor(None, agent.handle, got)      # notification -> "allow" -> decide over the socket
            self.assertIn("Pdfcraft wants to connect to example.com", open(seen).read())
            self.assertIn("-A allow=Allow", open(seen).read())
            self.assertEqual(appnet.load_ask()["1000"]["pdfcraft"]["allow"], {"93.184.216.34": "example.com"})
            self.assertEqual(nft[0][4], appnet.set_name(1000, "pdfcraft", 4))
            w.write(b'{"op":"list"}\n')
            await w.drain()
            lst = json.loads(await asyncio.wait_for(r.readline(), 3))
            self.assertEqual(lst["apps"]["pdfcraft"]["allow"], {"93.184.216.34": "example.com"})
            w.close()
            server.close()
            await server.wait_closed()


if __name__ == "__main__":
    unittest.main()
