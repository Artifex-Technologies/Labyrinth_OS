//! labyrinth-appnet: network rules for single apps (Settings > Apps): one nftables table, `inet labyrinth_apps`, with a rule for
//! each limited app matching its systemd slice. The Python script of the same name with the same commands, files, ruleset text,
//! output and exit codes; its help text is src/doc/labyrinth-appnet.txt.
//!
//!   set APP MODE   change one app (root, through pkexec: the person who asked is PKEXEC_UID or SUDO_UID)
//!   apply          load every rule (root: labyrinth-appnet.service)
//!   get            the rules as JSON
//!   prepare        as the user at login: start the slices of apps with a rule, then ask for `apply`
//!   ruleset        print the nftables ruleset `apply` would load
//!
//! /etc/labyrinth/app-network.json: {"rules": {"1000": {"firefox": "block"}}}; /var/lib/labyrinth/app-ask.json holds what was
//! allowed and denied for apps in ask mode (labyrinth-appnet-ask).

use crate::py::{self, Crash, Map, Value, R};

pub const DOC: &str = include_str!("doc/labyrinth-appnet.txt");
pub const TABLE: &str = "labyrinth_apps";
pub const MODES: &[&str] = &["allow", "block", "ethernet", "wifi", "ask"];
const WIRED: &[&str] = &["en*", "eth*"];
const WIRELESS: &[&str] = &["wl*"];
const RELAY_PORTS: &[u32] = &[8899, 8930, 11434]; // Sarina, Heimdall, Ollama on loopback

/// {uid: {app: mode}} in the file's order.
pub type Rules = Vec<(String, Vec<(String, String)>)>;
/// {uid: {app: {"allow": {ip: name}, "deny": {ip: name}}}}
pub type Ask = Vec<(String, Vec<(String, AskEntry)>)>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AskEntry {
    pub allow: Vec<(String, String)>,
    pub deny: Vec<(String, String)>,
}

pub fn state_path() -> String {
    py::env_get("LABYRINTH_APPNET_STATE", "/etc/labyrinth/app-network.json")
}

pub fn ask_path() -> String {
    py::env_get("LABYRINTH_ASK_STATE", "/var/lib/labyrinth/app-ask.json")
}

fn cgroup_root() -> String {
    py::env_get("LABYRINTH_CGROUP_ROOT", "/sys/fs/cgroup")
}

/// re.sub(r"[^A-Za-z0-9_]", "_", s)
fn clean(s: &str, extra: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || extra.contains(c) { c } else { '_' }).collect()
}

/// The slice an app runs in (labyrinth-launch builds the same name): no dash, so it sits right under the user manager.
pub fn slice_name(app_id: &str) -> String {
    format!("lapp_{}.slice", clean(app_id, ""))
}

pub fn cgroup_path(uid: i128, app_id: &str) -> String {
    format!("user.slice/user-{uid}.slice/user@{uid}.service/{}", slice_name(app_id))
}

/// An app id that can go into the ruleset: 1 to 120 of A-Z a-z 0-9 . _ + -, not starting with a dot.
pub fn valid_app(app_id: &str) -> bool {
    let n = app_id.chars().count();
    (1..=120).contains(&n) && app_id.chars().all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c)) && !app_id.starts_with('.')
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// Python's truth of a JSON value.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
    }
}

/// The rules file, checked: modes other than allow, app ids that are valid. A missing or broken file is no rules; a "rules"
/// that is not an object stopped the script (a Crash).
pub fn load_state(path: Option<&str>) -> R<Rules> {
    let p = path.map(String::from).unwrap_or_else(state_path);
    let d = match py::load_json(&p) {
        Ok(d) => d,
        Err(_) => return Ok(vec![]),
    };
    let rules = match &d {
        Value::Object(m) => match m.get("rules") {
            None => return Ok(vec![]),
            Some(Value::Object(r)) => r.clone(),
            Some(other) => return Err(Crash(format!("AttributeError: '{}' object has no attribute 'items'", type_name(other)))),
        },
        _ => return Ok(vec![]),
    };
    let mut out: Rules = vec![];
    for (u, apps) in rules {
        if let Value::Object(apps) = apps {
            let kept: Vec<(String, String)> = apps
                .iter()
                .filter_map(|(a, m)| match m {
                    Value::String(m) if MODES.contains(&m.as_str()) && m != "allow" && valid_app(a) => Some((a.clone(), m.clone())),
                    _ => None,
                })
                .collect();
            out.push((u, kept));
        }
    }
    Ok(out)
}

fn rules_value(rules: &Rules) -> Value {
    let mut m = Map::new();
    for (u, apps) in rules {
        let mut a = Map::new();
        for (app, mode) in apps {
            a.insert(app.clone(), Value::String(mode.clone()));
        }
        m.insert(u.clone(), Value::Object(a));
    }
    Value::Object(m)
}

/// json.dump(..., sort_keys=True): every object's keys in order.
pub fn sorted_keys(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                out.insert(k.clone(), sorted_keys(&m[k]));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted_keys).collect()),
        other => other.clone(),
    }
}

/// Write a file through a temporary one beside it, with this mode, then rename it into place.
fn save_json(path: &str, v: &Value, mode: u32) -> R<()> {
    py::makedirs(&py::dirname(path))?;
    let tmp = format!("{path}.tmp");
    py::write_file(&tmp, &py::pyjson::dumps_indent(&sorted_keys(v), 1))?;
    py::chmod(&tmp, mode)?;
    py::replace(&tmp, path)
}

pub fn save_state(rules: &Rules, path: Option<&str>) -> R<()> {
    let p = path.map(String::from).unwrap_or_else(state_path);
    let mut top = Map::new();
    top.insert("rules".into(), rules_value(rules));
    save_json(&p, &Value::Object(top), 0o644)
}

/// zlib.crc32
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// The nftables set of the destinations allowed for an app in ask mode (family 4 or 6).
pub fn set_name(uid: i128, app_id: &str, family: u32) -> String {
    let c: String = clean(app_id, "").chars().take(60).collect();
    format!("ask_{uid}_{c}_{:08x}_{family}", crc32(app_id.as_bytes()))
}

/// What you decided for apps in ask mode. A missing or broken file is nothing; bad apps and addresses are left out.
pub fn load_ask(path: Option<&str>) -> R<Ask> {
    let p = path.map(String::from).unwrap_or_else(ask_path);
    let d = match py::load_json(&p) {
        Ok(Value::Object(d)) => d,
        _ => return Ok(vec![]),
    };
    let mut out: Ask = vec![];
    for (uid, apps) in d {
        let apps = match apps {
            Value::Object(a) => a,
            _ => continue,
        };
        for (app, v) in apps {
            let v = match v {
                Value::Object(v) if valid_app(&app) => v,
                _ => continue,
            };
            let mut entry = AskEntry::default();
            for k in ["allow", "deny"] {
                let ips = match v.get(k) {
                    Some(x) if truthy(x) => match x {
                        Value::Object(m) => m.clone(),
                        other => return Err(Crash(format!("AttributeError: '{}' object has no attribute 'items'", type_name(other)))),
                    },
                    _ => Map::new(),
                };
                let kept: Vec<(String, String)> = ips.iter().filter(|(ip, _)| valid_ip(ip)).map(|(ip, n)| (ip.clone(), py::pyjson::py_str(n))).collect();
                if k == "allow" {
                    entry.allow = kept;
                } else {
                    entry.deny = kept;
                }
            }
            match out.iter_mut().find(|(u, _)| *u == uid) {
                Some((_, a)) => match a.iter_mut().find(|(x, _)| *x == app) {
                    Some(e) => e.1 = entry,
                    None => a.push((app, entry)),
                },
                None => out.push((uid.clone(), vec![(app, entry)])),
            }
        }
    }
    Ok(out)
}

pub fn ask_value(ask: &Ask) -> Value {
    let pairs = |v: &Vec<(String, String)>| {
        let mut m = Map::new();
        for (k, x) in v {
            m.insert(k.clone(), Value::String(x.clone()));
        }
        Value::Object(m)
    };
    let mut top = Map::new();
    for (u, apps) in ask {
        let mut a = Map::new();
        for (app, e) in apps {
            let mut m = Map::new();
            m.insert("allow".into(), pairs(&e.allow));
            m.insert("deny".into(), pairs(&e.deny));
            a.insert(app.clone(), Value::Object(m));
        }
        top.insert(u.clone(), Value::Object(a));
    }
    Value::Object(top)
}

/// Written private (0o600): it is read through labyrinth-appnet-ask, which checks who asks.
pub fn save_ask(ask: &Ask, path: Option<&str>) -> R<()> {
    let p = path.map(String::from).unwrap_or_else(ask_path);
    save_json(&p, &ask_value(ask), 0o600)
}

fn parse_octet(s: &str) -> bool {
    !s.is_empty() && s.len() <= 3 && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0')) && s.parse::<u32>().map(|x| x <= 255).unwrap_or(false)
}

fn ipv4_ok(s: &str) -> bool {
    if s.is_empty() || s.contains('/') {
        return false;
    }
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4 && parts.iter().all(|o| parse_octet(o))
}

fn ipv6_ok(s: &str) -> bool {
    if s.contains('/') {
        return false;
    }
    let (addr, sep, scope) = py::partition(s, "%");
    if sep && (scope.is_empty() || scope.contains('%')) {
        return false;
    }
    if addr.is_empty() || addr.chars().count() > 45 {
        return false;
    }
    let mut parts: Vec<String> = addr.splitn(10, ':').map(String::from).collect();
    if parts.len() < 3 {
        return false;
    }
    if parts.last().unwrap().contains('.') {
        let v4 = parts.pop().unwrap();
        if !ipv4_ok(&v4) {
            return false;
        }
        parts.push("0".into());
        parts.push("0".into());
    }
    if parts.len() > 9 {
        return false;
    }
    let mut skip = None;
    for i in 1..parts.len() - 1 {
        if parts[i].is_empty() {
            if skip.is_some() {
                return false;
            }
            skip = Some(i);
        }
    }
    let (hi, lo) = match skip {
        Some(k) => {
            let mut hi = k;
            let mut lo = parts.len() - k - 1;
            if parts[0].is_empty() {
                hi -= 1;
                if hi != 0 {
                    return false;
                }
            }
            if parts.last().unwrap().is_empty() {
                lo -= 1;
                if lo != 0 {
                    return false;
                }
            }
            if 8 < hi + lo + 1 {
                return false;
            }
            (hi, lo)
        }
        None => {
            if parts.len() != 8 || parts[0].is_empty() || parts[7].is_empty() {
                return false;
            }
            (8, 0)
        }
    };
    let hextet = |h: &str| !h.is_empty() && h.len() <= 4 && h.bytes().all(|b| b.is_ascii_hexdigit());
    parts[..hi].iter().all(|h| hextet(h)) && parts[parts.len() - lo..].iter().all(|h| hextet(h))
}

/// ipaddress.ip_address(text) does not raise: an IPv4 address (four decimal octets, no leading zeros) or an IPv6 one (with
/// its ::, an IPv4 tail, a %scope), exactly as Python's ipaddress reads them.
pub fn valid_ip(text: &str) -> bool {
    ipv4_ok(text) || ipv6_ok(text)
}

fn note_of(app_id: &str, what: &str) -> String {
    format!("comment \"{}: {what}\"", clean(app_id, ".+-"))
}

fn relay_rule(m: &str, note: &str) -> String {
    let ports: Vec<String> = RELAY_PORTS.iter().map(|p| p.to_string()).collect();
    format!("{m} oifname \"lo\" tcp dport {{ {} }} drop {note}", ports.join(", "))
}

fn matcher(uid: i128, app_id: &str) -> String {
    let path = cgroup_path(uid, app_id);
    format!("socket cgroupv2 level {} \"{path}\"", path.matches('/').count() + 1)
}

/// Rules for ask mode: loopback and allowed destinations pass; the rest is logged (rate limited, with the user id) and dropped.
pub fn ask_rules(uid: i128, app_id: &str) -> Vec<String> {
    let m = matcher(uid, app_id);
    let note = note_of(app_id, "ask");
    vec![
        relay_rule(&m, &note),
        format!("{m} oifname \"lo\" accept {note}"),
        format!("{m} ip daddr @{} accept {note}", set_name(uid, app_id, 4)),
        format!("{m} ip6 daddr @{} accept {note}", set_name(uid, app_id, 6)),
        format!("{m} limit rate 4/second burst 8 packets log prefix \"labask:{app_id} \" flags skuid {note}"),
        format!("{m} drop {note}"),
    ]
}

/// The nftables rules of one app and mode (None for allow).
pub fn rule_text(uid: i128, app_id: &str, mode: &str) -> Option<Vec<String>> {
    let m = matcher(uid, app_id);
    let note = note_of(app_id, mode);
    match mode {
        // no DNS to any resolver, no loopback relays, then nothing but loopback
        "block" => Some(vec![format!("{m} meta l4proto {{ tcp, udp }} th dport 53 drop {note}"), relay_rule(&m, &note), format!("{m} oifname != \"lo\" drop {note}")]),
        "ask" => Some(ask_rules(uid, app_id)),
        "ethernet" | "wifi" => {
            let names = if mode == "ethernet" { WIRED } else { WIRELESS };
            let allowed: Vec<String> = std::iter::once("lo").chain(names.iter().copied()).map(|n| format!("oifname != \"{n}\"")).collect();
            Some(vec![format!("{m} {} drop {note}", allowed.join(" "))])
        }
        _ => None,
    }
}

/// int(uid) of a rules key, as Python reads it.
pub fn uid_int(uid: &str) -> R<i128> {
    crate::pycompat::int(uid).ok_or_else(|| Crash(format!("ValueError: invalid literal for int() with base 10: {}", py::repr(uid))))
}

/// The whole nftables script. `exists(path)` says whether a slice's cgroup is there; `ask` holds what was allowed for apps in
/// ask mode.
pub fn ruleset(rules: &Rules, exists: &dyn Fn(&str) -> bool, ask: &Ask) -> R<String> {
    let mut order: Vec<(i128, usize)> = vec![];
    for (i, (u, _)) in rules.iter().enumerate() {
        order.push((uid_int(u)?, i));
    }
    order.sort_by_key(|(n, _)| *n); // stable: equal numbers keep the file's order
    let (mut sets, mut body) = (vec![], vec![]);
    for (n, i) in order {
        let (uid, apps) = &rules[i];
        let mut names: Vec<&(String, String)> = apps.iter().collect();
        names.sort_by(|a, b| a.0.cmp(&b.0));
        for (app_id, mode) in names {
            if !exists(&cgroup_path(n, app_id)) {
                continue;
            }
            if mode == "ask" {
                let empty = vec![];
                let allowed = ask.iter().find(|(u, _)| u == uid).and_then(|(_, a)| a.iter().find(|(x, _)| x == app_id)).map(|(_, e)| &e.allow).unwrap_or(&empty);
                for (fam, kind) in [(4, "ipv4_addr"), (6, "ipv6_addr")] {
                    let mut ips: Vec<&str> = allowed.iter().map(|(ip, _)| ip.as_str()).filter(|ip| ip.contains(':') == (fam == 6)).collect();
                    ips.sort();
                    let elements = if ips.is_empty() { String::new() } else { format!(" elements = {{ {} }};", ips.join(", ")) };
                    sets.push(format!("  set {} {{ type {kind}; flags timeout;{elements} }}", set_name(n, app_id, fam)));
                }
            }
            for r in rule_text(n, app_id, mode).unwrap_or_default() {
                body.push(format!("    {r}"));
            }
        }
    }
    let mut lines = vec![format!("table inet {TABLE}"), format!("delete table inet {TABLE}"), format!("table inet {TABLE} {{")];
    lines.extend(sets);
    lines.push("  chain output {".into());
    lines.push("    type filter hook output priority -5; policy accept;".into());
    lines.extend(body);
    lines.extend(["  }".to_string(), "}".to_string(), String::new()]);
    Ok(lines.join("\n"))
}

/// The ruleset of the rules file and the ask file, for the slices that are running.
pub fn ruleset_now(rules: &Rules) -> R<String> {
    let root = cgroup_root();
    let ask = load_ask(None)?;
    ruleset(rules, &|p| py::isdir(py::join(&root, p)), &ask)
}

/// Load the rules (the file's when none are given). When nft refuses them nothing is limited: said on standard error and in
/// the journal (tag labyrinth-appnet), where Heimdall reads it.
pub fn apply(rules: Option<&Rules>) -> R<i32> {
    let loaded;
    let rules = match rules {
        Some(r) => r,
        None => {
            loaded = load_state(None)?;
            &loaded
        }
    };
    let script = ruleset_now(rules)?;
    let p = py::run_opts(&["nft", "-f", "-"], py::Run { input: Some(&script), ..Default::default() }).map_err(|e| Crash(e.message().to_string()))?;
    if p.code != 0 {
        let s = py::strip(&p.stderr);
        let msg = if s.is_empty() { "nft failed" } else { s };
        eprintln!("{msg}");
        let first = py::splitlines(msg).first().copied().unwrap_or("").to_string();
        py::call(&["logger", "-t", "labyrinth-appnet", "-p", "auth.warning", "--", &format!("per-app network rules did not load, apps are NOT limited: {first}")])?;
    }
    Ok(p.code)
}

fn cmd_set(app_id: &str, mode: &str) -> R<i32> {
    if py::geteuid() != 0 {
        eprintln!("labyrinth-appnet: set needs root (run it with pkexec)");
        return Ok(1);
    }
    if !valid_app(app_id) || !MODES.contains(&mode) {
        eprintln!("labyrinth-appnet: bad app id or mode");
        return Ok(2);
    }
    let uid = match py::env("PKEXEC_UID") {
        Some(u) if !u.is_empty() => Some(u),
        _ => py::env("SUDO_UID"),
    };
    let uid = match uid {
        Some(u) if crate::pycompat::isdigit(&u) => u,
        _ => {
            eprintln!("labyrinth-appnet: cannot tell whose app this is (no PKEXEC_UID)");
            return Ok(1);
        }
    };
    let mut rules = load_state(None)?;
    let idx = match rules.iter().position(|(u, _)| *u == uid) {
        Some(i) => i,
        None => {
            rules.push((uid.clone(), vec![]));
            rules.len() - 1
        }
    };
    {
        let mine = &mut rules[idx].1;
        if mode == "allow" {
            mine.retain(|(a, _)| a != app_id);
        } else {
            match mine.iter_mut().find(|(a, _)| a == app_id) {
                Some(e) => e.1 = mode.to_string(),
                None => mine.push((app_id.to_string(), mode.to_string())),
            }
        }
    }
    if rules[idx].1.is_empty() {
        rules.remove(idx);
    }
    save_state(&rules, None)?;
    apply(Some(&rules))
}

fn cmd_prepare() -> R<i32> {
    let uid = py::getuid().to_string();
    let rules = load_state(None)?;
    if let Some((_, apps)) = rules.iter().find(|(u, _)| *u == uid) {
        for (app_id, _) in apps {
            py::run(&["systemctl", "--user", "start", &slice_name(app_id)])?;
        }
    }
    py::run(&["systemctl", "start", "labyrinth-appnet.service"])?;
    Ok(0)
}

fn run(argv: &[String]) -> R<i32> {
    let c = argv[1].as_str();
    match c {
        "set" if argv.len() == 4 => cmd_set(&argv[2], &argv[3]),
        "apply" => apply(None),
        "get" => {
            let mut top = Map::new();
            top.insert("rules".into(), rules_value(&load_state(None)?));
            println!("{}", py::pyjson::dumps(&Value::Object(top)));
            Ok(0)
        }
        "prepare" => cmd_prepare(),
        "ruleset" => {
            println!("{}", ruleset_now(&load_state(None)?)?);
            Ok(0)
        }
        _ => {
            eprintln!("labyrinth-appnet: unknown command (see --help)");
            Ok(2)
        }
    }
}

/// The program: argv[0] is its name.
pub fn main(argv: &[String]) -> i32 {
    if argv.len() < 2 || argv[1] == "-h" || argv[1] == "--help" {
        println!("{DOC}");
        return if argv.len() >= 2 { 0 } else { 2 };
    }
    match run(argv) {
        Ok(rc) => rc,
        Err(e) => py::crashed("labyrinth-appnet", &e),
    }
}

#[cfg(test)]
mod tests;
