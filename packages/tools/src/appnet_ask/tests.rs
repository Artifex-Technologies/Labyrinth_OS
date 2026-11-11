//! tests/test_labyrinth_appnet_ask.py, carried over: ParseTests, EngineTests and ServiceTests (RulesTests are in appnet's
//! tests, AgentTests in ask_agent's). The state files are named by the environment, which is the process's: these tests hold
//! env_lock.
use super::*;
use crate::testutil::{env_lock, TempDir};
use std::sync::{Arc, Mutex};

const LOG: &str = "labask:pdfcraft IN= OUT=wlan0 SRC=192.168.1.5 DST=93.184.216.34 LEN=60 TOS=0x00 PREC=0x00 TTL=64 ID=1 DF PROTO=TCP SPT=40000 DPT=443 WINDOW=64240 RES=0x00 SYN URGP=0 UID=1000 GID=1000";
const LOG6: &str = "labask:pdfcraft IN= OUT=wlan0 SRC=fe80::1 DST=2606:4700::1111 LEN=80 TC=0 HOPLIMIT=64 FLOWLBL=0 PROTO=UDP SPT=5000 DPT=53 LEN=40 UID=1000 GID=1000";

struct Fixture {
    _d: TempDir,
    _g: std::sync::MutexGuard<'static, ()>,
    nft: Arc<Mutex<Vec<Vec<String>>>>,
    applied: Arc<Mutex<usize>>,
    clock: Arc<Mutex<f64>>,
    eng: Engine,
}

impl Fixture {
    fn new() -> Fixture {
        let g = env_lock();
        let d = TempDir::new();
        std::env::set_var("LABYRINTH_APPNET_STATE", d.p("rules.json"));
        std::env::set_var("LABYRINTH_ASK_STATE", d.p("ask.json"));
        std::env::set_var("LABYRINTH_CGROUP_ROOT", d.p("cgroup"));
        let rules: appnet::Rules = vec![("1000".into(), vec![("pdfcraft".into(), "ask".into()), ("other".into(), "block".into())])];
        appnet::save_state(&rules, None).unwrap();
        let nft = Arc::new(Mutex::new(vec![]));
        let applied = Arc::new(Mutex::new(0));
        let clock = Arc::new(Mutex::new(100.0));
        let (n, a, c) = (nft.clone(), applied.clone(), clock.clone());
        let eng = Engine::new(
            Box::new(move |args: &[String]| {
                n.lock().unwrap().push(args.to_vec());
                Ok(())
            }),
            Box::new(move |_r: &appnet::Rules| {
                *a.lock().unwrap() += 1;
                Ok(())
            }),
            Box::new(move || *c.lock().unwrap()),
            60.0,
        );
        Fixture { _d: d, _g: g, nft, applied, clock, eng }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.nft.lock().unwrap().clone()
    }
}

fn ev(ip: &str, app: &str) -> Event {
    Event { uid: "1000".into(), app: app.into(), ip: ip.into(), port: 443, proto: "TCP".into() }
}

fn v(x: serde_json::Value) -> Value {
    x
}

fn allow_of(app: &str) -> Vec<(String, String)> {
    let ask = appnet::load_ask(None).unwrap();
    entry_of(&ask, "1000", app).map(|e| e.allow.clone()).unwrap_or_default()
}

// ---- ParseTests

#[test]
fn log_lines() {
    let re = line_regex();
    assert_eq!(parse_log(&re, LOG), Some(Event { uid: "1000".into(), app: "pdfcraft".into(), ip: "93.184.216.34".into(), port: 443, proto: "TCP".into() }));
    let v6 = parse_log(&re, LOG6).unwrap();
    assert_eq!((v6.ip.as_str(), v6.port, v6.proto.as_str()), ("2606:4700::1111", 53, "UDP"));
    assert_eq!(parse_log(&re, "IN=eth0 OUT= SRC=1.1.1.1 DST=2.2.2.2 PROTO=TCP"), None);
    assert_eq!(parse_log(&re, "something else"), None);
}

// ---- EngineTests

#[test]
fn one_question_per_app_and_destination() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().unwrap();
    assert_eq!((q.app.as_str(), q.ip.as_str()), ("pdfcraft", "93.184.216.34"));
    assert!(f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().is_none()); // same destination, still pending
    assert!(f.eng.asked(&ev("10.0.0.1", "pdfcraft")).unwrap().is_some()); // another destination
    assert!(f.eng.asked(&ev("93.184.216.34", "other")).unwrap().is_none()); // not in ask mode: never asked
}

#[test]
fn allow_remembers_and_adds_to_the_live_set() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().unwrap();
    let (ok, _) = f.eng.decide("1000", &Value::from(q.id), &v(serde_json::json!("allow")), &v(serde_json::json!("example.com"))).unwrap();
    assert!(ok);
    let c = f.calls();
    assert_eq!(c[0][..4], ["add", "element", "inet", "labyrinth_apps"]);
    assert_eq!(c[0][4], appnet::set_name(1000, "pdfcraft", 4));
    assert_eq!(c[0][c[0].len() - 2], "93.184.216.34");
    assert_eq!(allow_of("pdfcraft"), vec![("93.184.216.34".to_string(), "example.com".to_string())]);
    assert!(f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().is_none()); // decided: not asked again
}

#[test]
fn ipv6_goes_to_the_v6_set() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("2606:4700::1111", "pdfcraft")).unwrap().unwrap();
    f.eng.decide("1000", &Value::from(q.id), &v(serde_json::json!("allow")), &Value::from("")).unwrap();
    assert!(f.calls()[0][4].ends_with("_6"));
}

#[test]
fn deny_is_remembered_and_never_asked_again() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().unwrap();
    f.eng.decide("1000", &Value::from(q.id), &v(serde_json::json!("deny")), &Value::from("")).unwrap();
    assert!(f.calls().is_empty());
    let ask = appnet::load_ask(None).unwrap();
    assert!(entry_of(&ask, "1000", "pdfcraft").unwrap().deny.iter().any(|(ip, _)| ip == "93.184.216.34"));
    assert!(f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().is_none());
}

#[test]
fn closing_the_notification_asks_again_after_the_cooldown() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().unwrap();
    f.eng.decide("1000", &Value::from(q.id), &v(serde_json::json!("ignore")), &Value::from("")).unwrap();
    assert!(f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().is_none());
    *f.clock.lock().unwrap() += 61.0;
    assert!(f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().is_some());
}

#[test]
fn only_the_owner_answers_and_only_real_questions() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().unwrap();
    let allow = v(serde_json::json!("allow"));
    assert!(!f.eng.decide("1001", &Value::from(q.id), &allow, &Value::from("")).unwrap().0); // someone else
    assert!(!f.eng.decide("1000", &Value::from(999), &allow, &Value::from("")).unwrap().0); // no such question
    assert!(f.calls().is_empty());
    assert!(!f.eng.decide("1000", &Value::from(q.id), &v(serde_json::json!("bogus")), &Value::from("")).unwrap().0); // (the question is consumed)
}

#[test]
fn allow_all_puts_the_app_back_to_any_connection() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().unwrap();
    assert!(f.eng.decide("1000", &Value::from(q.id), &v(serde_json::json!("allow-all")), &Value::from("")).unwrap().0);
    assert_eq!(appnet::load_state(None).unwrap(), vec![("1000".to_string(), vec![("other".to_string(), "block".to_string())])]);
    assert_eq!(*f.applied.lock().unwrap(), 1);
}

#[test]
fn forget_asks_again() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().unwrap();
    f.eng.decide("1000", &Value::from(q.id), &v(serde_json::json!("allow")), &Value::from("")).unwrap();
    assert!(f.eng.forget("1000", "pdfcraft", "93.184.216.34").unwrap());
    assert_eq!(f.calls().last().unwrap()[0], "delete");
    assert!(f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().is_some());
    assert!(!f.eng.forget("1000", "pdfcraft", "not an ip").unwrap());
}

// ---- beyond the Python tests

#[test]
fn ids_as_a_python_dict_finds_them_and_names_cleaned() {
    let mut f = Fixture::new();
    let q = f.eng.asked(&ev("93.184.216.34", "pdfcraft")).unwrap().unwrap();
    assert_eq!(q.id, 1);
    // 1.0 and true are the key 1 in Python; the name keeps only A-Z a-z 0-9 . _ -
    assert!(f.eng.decide("1000", &v(serde_json::json!(1.0)), &v(serde_json::json!("allow")), &Value::from("ex ample.com/<x>")).unwrap().0);
    assert_eq!(allow_of("pdfcraft"), vec![("93.184.216.34".to_string(), "example.comx".to_string())]);
    let q = f.eng.asked(&ev("10.0.0.2", "pdfcraft")).unwrap().unwrap();
    assert!(matches!(f.eng.decide("1000", &v(serde_json::json!([q.id])), &Value::from("allow"), &Value::from("")), Err(Stop::Other(_))));
    assert!(matches!(f.eng.decide("1000", &Value::from(q.id), &Value::from("allow"), &Value::from(5)), Err(Stop::Other(_))));
    assert!(!f.eng.decide("1000", &v(serde_json::json!(true)), &Value::from("deny"), &Value::Null).unwrap().0, "true is 1, which is answered");
    assert!(f.eng.decide("1000", &Value::from(q.id), &Value::from("deny"), &Value::Null).unwrap().0);
}

#[test]
fn floats_as_python_reads_them() {
    for (t, want) in [("60", 60.0), (" 1.5\n", 1.5), ("1_000", 1000.0), ("1e2", 100.0), (".5", 0.5), ("5.", 5.0)] {
        assert_eq!(py_float(t), Some(want), "{t:?}");
    }
    assert!(py_float("inf").unwrap().is_infinite());
    for bad in ["", "x", "_1", "1__0", "0x10", "1e"] {
        assert_eq!(py_float(bad), None, "{bad:?}");
    }
}

// ---- ServiceTests: the real socket service, fed a fake kernel log, answered by the real agent code with a fake notify-send

#[test]
fn question_notification_answer_and_set_update() {
    let f = Fixture::new();
    let d = TempDir::new();
    let me = py::getuid().to_string();
    let rules: appnet::Rules = vec![(me.clone(), vec![("pdfcraft".into(), "ask".into())])];
    appnet::save_state(&rules, None).unwrap();
    let sock = d.p("s.sock");
    let nft = f.nft.clone();
    let eng = Engine::new(
        Box::new(move |args: &[String]| {
            nft.lock().unwrap().push(args.to_vec());
            Ok(())
        }),
        Box::new(|_r: &appnet::Rules| Ok(())),
        Box::new(|| 0.0),
        60.0,
    );
    let shared = Arc::new(Mutex::new(Shared { engine: eng, subs: HashMap::new() }));
    let listener = UnixListener::bind(&sock).unwrap();
    let sh = shared.clone();
    std::thread::spawn(move || {
        let mut n = 0;
        for s in listener.incoming().flatten() {
            n += 1;
            let sh = sh.clone();
            std::thread::spawn(move || handle(sh, s, n));
        }
    });
    let notify = d.p("notify");
    let seen = d.p("seen");
    std::fs::write(&notify, format!("#!/bin/sh\nprintf \"%s\\n\" \"$*\" > {seen}\necho allow\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&notify, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut c = UnixStream::connect(&sock).unwrap();
    c.write_all(b"{\"op\":\"subscribe\"}\n").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let line = LOG.replace("UID=1000", &format!("UID={me}"));
    {
        let mut s = shared.lock().unwrap();
        let q = s.engine.asked(&parse_log(&line_regex(), &line).unwrap()).unwrap().unwrap();
        push(&mut s, &q);
    }
    let mut r = BufReader::new(c.try_clone().unwrap());
    let _ = c.set_read_timeout(Some(Duration::from_secs(3)));
    let mut got = String::new();
    r.read_line(&mut got).unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!((got["type"].as_str(), got["app"].as_str(), got["ip"].as_str()), (Some("ask"), Some("pdfcraft"), Some("93.184.216.34")));
    // the agent's handling, with the name it would have looked up
    let q = crate::ask_agent::Q::from(got.as_object().unwrap()).unwrap();
    let key = crate::ask_agent::ask_user(&q, "example.com", &notify, "300");
    crate::ask_agent::answer(&sock, &q, "example.com", &key);
    let shown = std::fs::read_to_string(&seen).unwrap();
    assert!(shown.contains("Pdfcraft wants to connect to example.com"));
    assert!(shown.contains("-A allow=Allow"));
    let ask = appnet::load_ask(None).unwrap();
    assert_eq!(entry_of(&ask, &me, "pdfcraft").unwrap().allow, vec![("93.184.216.34".to_string(), "example.com".to_string())]);
    let uid_n: i128 = me.parse().unwrap();
    assert_eq!(f.calls()[0][4], appnet::set_name(uid_n, "pdfcraft", 4));
    c.write_all(b"{\"op\":\"list\"}\n").unwrap();
    let mut lst = String::new();
    r.read_line(&mut lst).unwrap();
    let lst: serde_json::Value = serde_json::from_str(&lst).unwrap();
    assert_eq!(lst["apps"]["pdfcraft"]["allow"], serde_json::json!({"93.184.216.34": "example.com"}));
}
