//! tests/test_labyrinth_appnet.py and the RulesTests of tests/test_labyrinth_appnet_ask.py, carried over.
use super::*;
use crate::testutil::TempDir;

fn rules(v: &[(&str, &[(&str, &str)])]) -> Rules {
    v.iter().map(|(u, apps)| (u.to_string(), apps.iter().map(|(a, m)| (a.to_string(), m.to_string())).collect())).collect()
}

#[test]
fn launcher_and_helper_agree_on_the_slice() {
    for app in ["firefox", "com.mitchellh.ghostty", "spotify-launcher", "a b", "x_y"] {
        assert_eq!(slice_name(app), crate::launch::slice_name(app));
    }
}

#[test]
fn cgroup_path_and_level() {
    assert_eq!(cgroup_path(1000, "firefox"), "user.slice/user-1000.slice/user@1000.service/lapp_firefox.slice");
    assert_eq!(rule_text(1000, "firefox", "block").unwrap()[1].matches("level 4 \"").count(), 1);
}

#[test]
fn modes() {
    assert!(rule_text(1000, "a", "allow").is_none());
    let block = rule_text(1000, "a", "block").unwrap();
    // DNS to every resolver (the local one listens on 127.0.0.1 and ::1), then the loopback relays, then anything off loopback
    assert!(block[0].contains("meta l4proto { tcp, udp } th dport 53 drop"));
    assert!(!block[0].contains("daddr"));
    assert!(block[1].contains("oifname \"lo\" tcp dport { 8899, 8930, 11434 } drop"));
    assert!(block[2].contains("oifname != \"lo\" drop"));
    let eth = &rule_text(1000, "a", "ethernet").unwrap()[0];
    assert!(eth.contains("oifname != \"lo\"") && eth.contains("oifname != \"en*\"") && eth.contains("oifname != \"eth*\"") && !eth.contains("wl*"));
    let wifi = &rule_text(1000, "a", "wifi").unwrap()[0];
    assert!(wifi.contains("oifname != \"wl*\"") && !wifi.contains("en*"));
}

#[test]
fn ruleset_skips_apps_whose_slice_is_not_running_and_orders_users() {
    let r = rules(&[("1000", &[("b", "block"), ("a", "wifi")]), ("1001", &[("c", "block")])]);
    let up = [cgroup_path(1000, "a"), cgroup_path(1001, "c")];
    let text = ruleset(&r, &|p| up.iter().any(|x| x == p), &vec![]).unwrap();
    assert!(text.contains("lapp_a.slice"));
    assert!(text.contains("lapp_c.slice"));
    assert!(!text.contains("lapp_b.slice"));
    assert!(text.starts_with("table inet labyrinth_apps\ndelete table inet labyrinth_apps\n"));
}

#[test]
fn state_is_validated() {
    let d = TempDir::new();
    let p = d.p("s.json");
    std::fs::write(&p, r#"{"rules": {"1000": {"ok": "block", "../x": "block", "bad": "explode", "free": "allow"}, "x": 5}}"#).unwrap();
    assert_eq!(load_state(Some(&p)).unwrap(), rules(&[("1000", &[("ok", "block")])]));
    save_state(&rules(&[("1000", &[("a", "wifi")])]), Some(&p)).unwrap();
    assert_eq!(load_state(Some(&p)).unwrap(), rules(&[("1000", &[("a", "wifi")])]));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o644);
    assert_eq!(load_state(Some(&d.p("missing.json"))).unwrap(), vec![]);
}

#[test]
fn app_ids_cannot_inject_into_the_ruleset() {
    let long = "x".repeat(200);
    for bad in ["a\"; drop", "a b", "", ".hidden", long.as_str(), "a/b"] {
        assert!(!valid_app(bad), "{bad}");
    }
    assert!(valid_app("com.mitchellh.ghostty") && valid_app("spotify-launcher"));
}

// ---- test_labyrinth_appnet_ask.py: RulesTests

#[test]
fn ask_rules_pass_loopback_and_allowed_then_log_and_drop() {
    let r = rule_text(1000, "pdfcraft", "ask").unwrap();
    assert_eq!(r.len(), 6);
    assert!(r[0].contains("oifname \"lo\" tcp dport { 8899, 8930, 11434 } drop")); // no relay through Ollama, Sarina, Heimdall
    assert!(r[1].contains("oifname \"lo\" accept"));
    assert!(r[2].contains(&format!("@{}", set_name(1000, "pdfcraft", 4))));
    assert!(r[3].contains(&format!("@{}", set_name(1000, "pdfcraft", 6))));
    assert!(r[4].contains("log prefix \"labask:pdfcraft \" flags skuid"));
    assert!(r[5].contains(" drop "));
}

#[test]
fn ruleset_declares_the_sets_with_what_you_allowed() {
    let r = rules(&[("1000", &[("pdfcraft", "ask"), ("x", "block")])]);
    let asked: Ask = vec![(
        "1000".into(),
        vec![("pdfcraft".into(), AskEntry { allow: vec![("1.2.3.4".into(), "a".into()), ("2606:4700::1111".into(), "".into())], deny: vec![] })],
    )];
    let text = ruleset(&r, &|_| true, &asked).unwrap();
    assert!(text.contains("type ipv4_addr; flags timeout; elements = { 1.2.3.4 }"));
    assert!(text.contains("type ipv6_addr; flags timeout; elements = { 2606:4700::1111 }"));
    assert!(text.find("set ask_1000_pdfcraft").unwrap() < text.find("chain output").unwrap());
    assert!(!text.contains("ask_1000_x"));
}

#[test]
fn set_names_differ_for_apps_that_clean_to_the_same_text() {
    assert_ne!(set_name(1, "a-b", 4), set_name(1, "a_b", 4));
}

#[test]
fn state_file_is_private_and_bad_entries_are_dropped() {
    let d = TempDir::new();
    let p = d.p("a.json");
    let one: Ask = vec![("1000".into(), vec![("app".into(), AskEntry { allow: vec![("1.2.3.4".into(), "n".into())], deny: vec![] })])];
    save_ask(&one, Some(&p)).unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
    std::fs::write(&p, r#"{"1000": {"app": {"allow": {"not an ip": "x", "5.6.7.8": "y"}}, "../bad": {"allow": {}}}}"#).unwrap();
    let want: Ask = vec![("1000".into(), vec![("app".into(), AskEntry { allow: vec![("5.6.7.8".into(), "y".into())], deny: vec![] })])];
    assert_eq!(load_ask(Some(&p)).unwrap(), want);
}

// ---- beyond the Python tests

#[test]
fn addresses_as_pythons_ipaddress_reads_them() {
    for ok in ["1.2.3.4", "0.0.0.0", "255.255.255.255", "::", "::1", "1::", "2606:4700::1111", "fe80::1%eth0", "::ffff:1.2.3.4", "1:2:3:4:5:6:7:8",
        "1:2:3:4:5:6:1.2.3.4", "1::8", "1:2:3:4:5:6:7::"] {
        assert!(valid_ip(ok), "{ok}");
    }
    for bad in ["", "1.2.3", "1.2.3.4.5", "01.2.3.4", "256.1.1.1", "1.2.3.4/8", "1.2.3.\u{661}", " 1.2.3.4", ":::", "1:2", "1:2:3:4:5:6:7:8:9", "1::2::3",
        ":1::2", "1::2:", "12345::", "g::", "fe80::1%", "fe80::1%a%b", "1:2:3:4:5:6:7:1.2.3.4", "::1.2.3", "1:2:3:4:5:6:7:8::"] {
        assert!(!valid_ip(bad), "{bad}");
    }
}

#[test]
fn crc32_as_zlib() {
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"pdfcraft"), 0x5a58_77d1);
    assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414f_a339);
}
