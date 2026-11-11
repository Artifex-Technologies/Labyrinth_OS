//! tests/test_labyrinth_launch.py, carried over (the environment is the process's: the tests that set it hold env_lock).
use super::*;
use crate::testutil::{env_lock, TempDir};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn exec_field_codes() {
    assert_eq!(expand_exec("minos %U", &s(&["/a b", "/c"]), "", "", ""), s(&["minos", "/a b", "/c"]));
    assert_eq!(expand_exec("viewer %f", &s(&["/x", "/y"]), "", "", ""), s(&["viewer", "/x"]));
    assert_eq!(expand_exec("app %i %c --name=%c 100%%", &[], "My App", "ic", ""), s(&["app", "--icon", "ic", "My App", "--name=My App", "100%"]));
    assert_eq!(expand_exec("sh -c \"echo hi %u\"", &[], "", "", ""), s(&["sh", "-c", "echo hi "]));
    assert_eq!(expand_exec("app %F", &[], "", "", ""), s(&["app"]));
}

#[test]
fn slice_names_have_no_dash_and_are_stable() {
    assert_eq!(slice_name("com.mitchellh.ghostty"), "lapp_com_mitchellh_ghostty.slice");
    assert!(!slice_name("spotify-launcher").contains('-'));
    assert_eq!(slice_name("a-b"), slice_name("a-b"));
}

#[test]
fn policy_defaults_and_usage_counting() {
    let _g = env_lock();
    let d = TempDir::new();
    std::env::set_var("XDG_CONFIG_HOME", d.p("c"));
    std::env::set_var("XDG_DATA_HOME", d.p("d"));
    assert!(load_policy("x").is_empty());
    std::fs::create_dir_all(d.p("c/labyrinth/apps")).unwrap();
    std::fs::write(policy_path("x"), r#"{"sandbox": true, "files": "folders"}"#).unwrap();
    assert_eq!(load_policy("x")["files"], "folders");
    count_launch("x", Some(100)).unwrap();
    count_launch("x", Some(200)).unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(d.p("d/labyrinth/app-usage.json")).unwrap()).unwrap();
    assert_eq!(v, serde_json::json!({"x": {"launches": 2, "last": 200}}));
}

#[test]
fn build_command_wraps_in_a_scope_and_optionally_a_sandbox() {
    let _g = env_lock();
    let d = TempDir::new();
    std::env::set_var("XDG_CONFIG_HOME", d.p("c"));
    std::env::set_var("XDG_DATA_HOME", d.p("d"));
    std::env::set_var("XDG_DATA_DIRS", d.p("share"));
    std::fs::create_dir_all(d.p("share/applications")).unwrap();
    std::fs::write(d.p("share/applications/demo.desktop"), "[Desktop Entry]\nType=Application\nName=Demo\nExec=demo --open %U\nIcon=demo\nPath=/tmp\n").unwrap();
    let (cmd, cwd) = build_command("demo", &s(&["/file"])).unwrap().unwrap();
    assert_eq!(cmd[..4], s(&["systemd-run", "--user", "--scope", "--quiet"])[..]);
    assert!(cmd.contains(&"--slice=lapp_demo.slice".to_string()));
    assert_eq!(cmd[cmd.len() - 3..], s(&["demo", "--open", "/file"])[..]);
    assert_eq!(cwd.as_deref(), Some("/tmp"));
    assert!(build_command("nothing-here", &[]).unwrap().is_none());
    std::fs::create_dir_all(d.p("c/labyrinth/apps")).unwrap();
    std::fs::write(policy_path("demo"), r#"{"sandbox": true, "files": "folders", "see_others": false}"#).unwrap();
    let (cmd, _) = build_command("demo", &[]).unwrap().unwrap();
    if py::which("bwrap").is_some() {
        assert!(cmd.contains(&"bwrap".to_string()));
        assert!(cmd.contains(&"--unshare-pid".to_string()));
        assert!(cmd.contains(&"--tmpfs".to_string()));
        let i = cmd.iter().position(|c| c == "--").unwrap();
        assert_eq!(cmd[i + 1..], s(&["demo", "--open"])[..]); // after the sandbox's own arguments: the app's command
    }
    std::env::remove_var("XDG_DATA_DIRS");
}

#[test]
fn network_rule_is_read_for_this_user_only() {
    let d = TempDir::new();
    let p = d.p("r.json");
    let me = py::getuid().to_string();
    let other = (py::getuid() + 1).to_string();
    std::fs::write(&p, format!(r#"{{"rules": {{"{me}": {{"a": "block", "b": "bogus", "c": "wifi"}}, "{other}": {{"d": "block"}}}}}}"#)).unwrap();
    assert_eq!(network_rule("a", Some(&p)).as_deref(), Some("block"));
    assert_eq!(network_rule("c", Some(&p)).as_deref(), Some("wifi"));
    assert_eq!(network_rule("b", Some(&p)), None);
    assert_eq!(network_rule("d", Some(&p)), None);
    assert_eq!(network_rule("a", Some(&d.p("none.json"))), None);
}

// ---- beyond the Python tests

#[test]
fn an_exec_line_shlex_cannot_read_is_split_on_whitespace() {
    assert_eq!(expand_exec("app 'unclosed %f", &s(&["/x"]), "", "", ""), s(&["app", "'unclosed", "/x"]));
    assert_eq!(expand_exec("app %k %d%D%n%N%v%m x%%y", &[], "", "", "/p.desktop"), s(&["app", "/p.desktop", "x%y"]));
}

#[test]
fn desktop_entries_first_value_wins_and_localised_names_are_skipped() {
    let d = TempDir::new();
    let p = d.p("a.desktop");
    std::fs::write(&p, "Name=outside\n[Desktop Entry]\n Name = One \nName[de]=Eins\nName=Two\n# Exec=no\nExec=a=b\n[Other]\nIcon=x\n").unwrap();
    assert_eq!(parse_desktop(&p).unwrap(), vec![("Name".to_string(), "One".to_string()), ("Exec".to_string(), "a=b".to_string())]);
}
