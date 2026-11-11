//! ipconfig: Windows' `ipconfig`, laid out the way Windows prints it, from `ip -j` and NetworkManager (`nmcli`); /all, /release,
//! /renew and /flushdns (also written -all or --all). The Python script of the same name with the same options, output and exit
//! codes; its help text is src/doc/ipconfig.txt.
//!
//! Not 1:1: a tool's output that is not UTF-8 has its bad bytes replaced (the script stopped); JSON with NaN, Infinity or a lone
//! surrogate counts as not JSON.

use crate::appnet::truthy;
use crate::py::pyjson::py_str;
use crate::py::{self, Crash, Map, Value, R};
use std::time::Duration;

pub const DOC: &str = include_str!("doc/ipconfig.txt");

/// Where a further gateway or DNS server goes: under the first one's value.
const CONTINUED: usize = 41;

/// subprocess.run(cmd, capture_output=True, text=True, timeout=10): standard output when the command succeeded, else "".
fn run(cmd: &[&str]) -> String {
    let o = py::Run { inherit_stdin: true, timeout: Some(Duration::from_secs(10)), ..Default::default() };
    match py::run_opts(cmd, o) {
        Ok(out) if out.code == 0 => out.stdout,
        _ => String::new(),
    }
}

fn ip_json(args: &[&str]) -> Value {
    let mut cmd = vec!["ip", "-j"];
    cmd.extend_from_slice(args);
    let out = run(&cmd);
    py::pyjson::loads(if out.is_empty() { "[]" } else { &out }).unwrap_or(Value::Array(vec![]))
}

fn crash(what: &str) -> Crash {
    Crash(what.to_string())
}

/// NetworkManager's view of one device: GENERAL.CONNECTION, IP4.DNS ... with their values in order.
fn nm_device(dev: &str) -> Vec<(String, Vec<String>)> {
    let mut info: Vec<(String, Vec<String>)> = vec![];
    for line in py::splitlines(&run(&["nmcli", "-t", "-e", "no", "device", "show", dev])) {
        let (key, _, value) = py::partition(line, ":");
        let key = key.split('[').next().unwrap_or("");
        if !value.is_empty() {
            match info.iter_mut().find(|(k, _)| k == key) {
                Some((_, v)) => v.push(value.to_string()),
                None => info.push((key.to_string(), vec![value.to_string()])),
            }
        }
    }
    info
}

fn nm_get<'a>(nm: &'a [(String, Vec<String>)], key: &str) -> &'a [String] {
    nm.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_slice()).unwrap_or(&[])
}

/// The adapter's product name (e.g. "Intel Corporation Ethernet Connection I219-V"), else its driver, else "Virtual adapter".
fn model(dev: &str) -> String {
    let mut props: Vec<(String, String)> = vec![];
    for line in py::splitlines(&run(&["udevadm", "info", "-q", "property", "-p", &format!("/sys/class/net/{dev}")])) {
        let (k, _, v) = py::partition(line, "=");
        match props.iter_mut().find(|(x, _)| x == k) {
            Some(p) => p.1 = v.to_string(),
            None => props.push((k.to_string(), v.to_string())),
        }
    }
    let get = |k: &str| props.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str()).unwrap_or("");
    let name: Vec<&str> = [get("ID_VENDOR_FROM_DATABASE"), get("ID_MODEL_FROM_DATABASE")].into_iter().filter(|x| !x.is_empty()).collect();
    if !name.is_empty() {
        return name.join(" ");
    }
    let link = format!("/sys/class/net/{dev}/device/driver");
    let driver = py::basename(&py::realpath(&link)).to_string();
    if py::exists(&link) {
        driver
    } else {
        "Virtual adapter".into()
    }
}

fn is_text(v: Option<&Value>, s: &str) -> bool {
    matches!(v, Some(Value::String(x)) if x == s)
}

fn kind(link: &Map<String, Value>, dev: &str) -> &'static str {
    if py::isdir(format!("/sys/class/net/{dev}/wireless")) {
        return "Wireless LAN adapter";
    }
    let t = link.get("link_type");
    if is_text(t, "ether") {
        "Ethernet adapter"
    } else if !is_text(t, "loopback") {
        "Tunnel adapter"
    } else {
        ""
    }
}

fn resolv_conf() -> R<Vec<String>> {
    let text = match py::read_text("/etc/resolv.conf") {
        Ok(t) => t,
        Err(py::ReadError::Os(_)) => return Ok(vec![]),
        Err(e) => return Err(Crash(e.message("/etc/resolv.conf"))),
    };
    Ok(py::lines(&text)
        .into_iter()
        .filter(|l| l.starts_with("nameserver") && py::split(l).len() > 1)
        .map(|l| py::split(l)[1].to_string())
        .collect())
}

/// "1 days 02:03:04 remaining" for an int number of seconds below 2^32-1 (bool counts, as in Python), else None.
pub fn lease(seconds: Option<&Value>) -> Option<String> {
    let s: i64 = match seconds? {
        Value::Bool(b) => *b as i64,
        Value::Number(n) => n.as_i64()?,
        _ => return None,
    };
    if s >= 4294967295 {
        return None;
    }
    let (d, rem) = (s.div_euclid(86400), s.rem_euclid(86400));
    let (h, rem) = (rem / 3600, rem % 3600);
    let (m, s) = (rem / 60, rem % 60);
    Some(if d != 0 { format!("{d} days {h:02}:{m:02}:{s:02} remaining") } else { format!("{h:02}:{m:02}:{s:02} remaining") })
}

/// The prefix length of a netmask written as Python's ipaddress takes it after "0.0.0.0/": a number 0..32, a netmask or a
/// hostmask in dotted form; None where it raised NetmaskValueError.
pub fn prefix_of(mask: &str) -> Option<u32> {
    if mask.contains('/') {
        return None; // "Only one '/' permitted"
    }
    if !mask.is_empty() && mask.bytes().all(|b| b.is_ascii_digit()) {
        return crate::pycompat::int(mask).filter(|p| (0..=32).contains(p)).map(|p| p as u32);
    }
    let octets: Vec<&str> = mask.split('.').collect();
    if mask.is_empty() || octets.len() != 4 {
        return None;
    }
    let mut ip: u32 = 0;
    for o in octets {
        if o.is_empty() || !o.bytes().all(|b| b.is_ascii_digit()) || o.len() > 3 || (o != "0" && o.starts_with('0')) {
            return None;
        }
        let v: u32 = o.parse().ok()?;
        if v > 255 {
            return None;
        }
        ip = (ip << 8) | v;
    }
    let from_int = |ip: u32| {
        let zeros = if ip == 0 { 32 } else { ip.trailing_zeros() };
        let prefix = 32 - zeros;
        let ones = if zeros == 32 { 0 } else { ip >> zeros };
        let all = if prefix == 0 { 0 } else { u32::MAX >> (32 - prefix) };
        (ones == all).then_some(prefix)
    };
    from_int(ip).or_else(|| from_int(!ip))
}

/// str(IPv4Network("0.0.0.0/<prefixlen>").netmask)
fn netmask(prefix: u32) -> String {
    let m: u32 = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    format!("{}.{}.{}.{}", m >> 24, (m >> 16) & 255, (m >> 8) & 255, m & 255)
}

fn hostname() -> String {
    let mut buf = vec![0u8; 256];
    unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// The items `for x in v` gives (a list's items, an object's keys, a text's characters); None where it raised TypeError.
fn iter_of(v: &Value) -> Option<Vec<Value>> {
    match v {
        Value::Array(a) => Some(a.clone()),
        Value::Object(m) => Some(m.keys().map(|k| Value::String(k.clone())).collect()),
        Value::String(s) => Some(s.chars().map(|c| Value::String(c.to_string())).collect()),
        _ => None,
    }
}

struct Out(String);

impl Out {
    fn print(&mut self, s: &str) {
        self.0.push_str(s);
        self.0.push('\n');
    }

    /// "   IPv4 Address. . . . . . . . . . . : 192.0.2.5", as Windows aligns it.
    fn line(&mut self, label: &str, value: &str) {
        let width = 36;
        let dots: String = format!("{label}{}", ". ".repeat(width)).chars().take(width - 1).collect();
        let lab = crate::pycompat::rstrip(&dots);
        self.print(&format!("   {lab:<w$} : {value}", w = width - 1));
    }
}

fn show(full: bool, out: &mut Out) -> R<()> {
    let links = ip_json(&["addr"]);
    let r4 = ip_json(&["route", "show", "default"]);
    let r6 = ip_json(&["-6", "route", "show", "default"]);
    let routes = match (r4, r6) {
        (Value::Array(mut a), Value::Array(b)) => {
            a.extend(b);
            a
        }
        _ => return Err(crash("TypeError: can only concatenate list to list")),
    };
    let mut gateways: Vec<(String, Vec<Value>)> = vec![];
    for r in routes {
        let r = match r {
            Value::Object(m) => m,
            _ => return Err(crash("AttributeError: object has no attribute 'get'")),
        };
        if let Some(gw) = r.get("gateway").filter(|g| truthy(g)) {
            match r.get("dev") {
                Some(Value::String(dev)) => match gateways.iter_mut().find(|(d, _)| d == dev) {
                    Some((_, v)) => v.push(gw.clone()),
                    None => gateways.push((dev.clone(), vec![gw.clone()])),
                },
                Some(Value::Array(_)) | Some(Value::Object(_)) => return Err(crash("TypeError: unhashable type")),
                _ => {} // a key no adapter name equals
            }
        }
    }
    out.print("\nLabyrinth OS IP Configuration\n");
    if full {
        out.line("Host Name", &hostname());
        out.line("Primary Dns Suffix", "");
        out.line("Node Type", "Hybrid");
        let fwd = py::read_text("/proc/sys/net/ipv4/ip_forward").map_err(|e| Crash(e.message("/proc/sys/net/ipv4/ip_forward")))?;
        out.line("IP Routing Enabled", if py::strip(&fwd) == "1" { "Yes" } else { "No" });
        out.print("");
    }
    let links = iter_of(&links).ok_or_else(|| crash("TypeError: object is not iterable"))?;
    for link in links {
        let link = match link {
            Value::Object(m) => m,
            _ => return Err(crash("AttributeError: object has no attribute 'get'")),
        };
        if is_text(link.get("link_type"), "loopback") {
            continue;
        }
        let dev = match link.get("ifname") {
            None => return Err(crash("KeyError: 'ifname'")),
            Some(Value::String(s)) if s.contains('\0') => return Err(crash("ValueError: embedded null byte")),
            Some(Value::String(s)) => s.clone(),
            Some(_) => return Err(crash("TypeError: expected str, bytes or os.PathLike object")),
        };
        let nm = nm_device(&dev);
        let name = nm_get(&nm, "GENERAL.CONNECTION").first().cloned().unwrap_or_else(|| dev.clone());
        let shown = if name != dev { format!(" ({name})") } else { String::new() };
        out.print(&format!("{} {dev}{shown}:\n", kind(&link, &dev)));
        let addrs = link.get("addr_info").cloned().unwrap_or(Value::Array(vec![]));
        let up = (is_text(link.get("operstate"), "UP") || is_text(link.get("operstate"), "UNKNOWN")) && truthy(&addrs);
        if !up {
            out.line("Media State", "Media disconnected");
        }
        out.line("Connection-specific DNS Suffix", &nm_get(&nm, "IP4.DOMAIN").join(" "));
        if full {
            out.line("Description", &model(&dev));
            let mac = match link.get("address") {
                None => String::new(),
                Some(Value::String(s)) => s.to_uppercase().replace(':', "-"),
                Some(_) => return Err(crash("AttributeError: object has no attribute 'upper'")),
            };
            out.line("Physical Address", &mac);
            // any(a.get("dynamic") for a in addrs): stops at the first true one
            let mut dhcp = false;
            for a in iter_of(&addrs).ok_or_else(|| crash("TypeError: object is not iterable"))? {
                match a {
                    Value::Object(m) => {
                        if m.get("dynamic").map(truthy).unwrap_or(false) {
                            dhcp = true;
                            break;
                        }
                    }
                    _ => return Err(crash("AttributeError: object has no attribute 'get'")),
                }
            }
            out.line("DHCP Enabled", if dhcp { "Yes" } else { "No" });
            out.line("Autoconfiguration Enabled", "Yes");
        }
        if up {
            let items = iter_of(&addrs).ok_or_else(|| crash("TypeError: object is not iterable"))?;
            let family = |a: &Value| -> R<(Map<String, Value>, Value)> {
                match a {
                    Value::Object(m) => match m.get("family") {
                        Some(f) => Ok((m.clone(), f.clone())),
                        None => Err(crash("KeyError: 'family'")),
                    },
                    _ => Err(crash("TypeError: indices must be integers")),
                }
            };
            let local = |a: &Map<String, Value>| a.get("local").cloned().ok_or_else(|| crash("KeyError: 'local'"));
            for a in &items {
                let (a, f) = family(a)?;
                if f == "inet6" {
                    let link_scope = is_text(a.get("scope"), "link");
                    let label = if link_scope { "Link-local IPv6 Address" } else { "IPv6 Address" };
                    let l = py_str(&local(&a)?);
                    out.line(label, &if link_scope { format!("{l}%{dev}") } else { l });
                }
            }
            for a in &items {
                let (a, f) = family(a)?;
                if f == "inet" {
                    let l = match local(&a)? {
                        Value::String(s) => s,
                        _ => return Err(crash("TypeError: unsupported operand type(s) for +")),
                    };
                    out.line("IPv4 Address", &format!("{l}{}", if full { "(Preferred)" } else { "" }));
                    let p = a.get("prefixlen").ok_or_else(|| crash("KeyError: 'prefixlen'"))?;
                    let prefix = prefix_of(&py_str(p)).ok_or_else(|| crash("NetmaskValueError: not a valid netmask"))?;
                    out.line("Subnet Mask", &netmask(prefix));
                    if full && a.get("dynamic").map(truthy).unwrap_or(false) {
                        if let Some(left) = lease(a.get("valid_life_time")) {
                            out.line("Lease Expires", &left);
                        }
                    }
                }
            }
            let gws: &[Value] = gateways.iter().find(|(d, _)| *d == dev).map(|(_, v)| v.as_slice()).unwrap_or(&[]);
            out.line("Default Gateway", &gws.first().map(py_str).unwrap_or_default());
            for g in gws.iter().skip(1) {
                match g {
                    Value::String(s) => out.print(&format!("{}{s}", " ".repeat(CONTINUED))),
                    _ => return Err(crash("TypeError: can only concatenate str to str")),
                }
            }
            if full {
                let mut dns: Vec<String> = nm_get(&nm, "IP4.DNS").to_vec();
                dns.extend(nm_get(&nm, "IP6.DNS").iter().cloned());
                if dns.is_empty() {
                    dns = resolv_conf()?;
                }
                if let Some(first) = dns.first() {
                    out.line("DNS Servers", first);
                    for d in &dns[1..] {
                        out.print(&format!("{}{d}", " ".repeat(CONTINUED)));
                    }
                }
            }
        }
        out.print("");
    }
    Ok(())
}

fn nm_devices() -> Vec<String> {
    let out = run(&["nmcli", "-t", "-f", "DEVICE,TYPE,STATE", "device"]);
    py::splitlines(&out)
        .into_iter()
        .filter(|l| {
            let f: Vec<&str> = l.split(':').collect();
            f.len() > 1 && (f[1] == "ethernet" || f[1] == "wifi") && !l.contains("unavailable")
        })
        .map(|l| l.split(':').next().unwrap_or("").to_string())
        .collect()
}

fn run_main(argv: &[String], out: &mut Out) -> R<i32> {
    let opt = argv.get(1).map(|a| a.trim_start_matches(['/', '-']).to_lowercase());
    match opt.as_deref() {
        None => show(false, out)?,
        Some("all") => show(true, out)?,
        Some(o @ ("release" | "renew")) => {
            let verb = if o == "release" { "disconnect" } else { "connect" };
            for d in nm_devices() {
                py::call(&["nmcli", "device", verb, &d])?;
            }
        }
        Some("flushdns") => {
            let ok = py::run(&["resolvectl", "flush-caches"])?.code == 0;
            out.print(if ok {
                "Successfully flushed the DNS Resolver Cache."
            } else {
                "There is no DNS cache running on this computer (nothing to flush)."
            });
        }
        Some("?" | "h" | "help") => out.print(DOC),
        Some(_) => {
            out.print(&format!("Error: unrecognized or incomplete command line.\n{DOC}"));
            return Ok(1);
        }
    }
    Ok(0)
}

pub fn main(argv: &[String]) -> i32 {
    let mut out = Out(String::new());
    let r = run_main(argv, &mut out);
    // what was printed before a stop is shown, as Python's stdout was flushed at the end
    use std::io::Write;
    let _ = std::io::stdout().write_all(out.0.as_bytes());
    let _ = std::io::stdout().flush();
    match r {
        Ok(rc) => rc,
        Err(c) => py::crashed("ipconfig", &c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn netmasks_as_ipaddress_reads_them() {
        for (m, p) in [
            ("24", Some(24)),
            ("0", Some(0)),
            ("32", Some(32)),
            ("33", None),
            ("255.255.255.0", Some(24)),
            ("0.0.0.255", Some(24)),
            ("0.0.0.0", Some(0)),
            ("255.255.255.255", Some(32)),
            ("255.0.255.0", None),
            ("True", None),
            ("24.0", None),
            ("", None),
            ("1/2", None),
            ("255.255.255.00", None),
            ("٢٤", None),
            ("024", Some(24)),
        ] {
            assert_eq!(prefix_of(m), p, "{m}");
        }
        assert_eq!(netmask(24), "255.255.255.0");
        assert_eq!(netmask(0), "0.0.0.0");
        assert_eq!(netmask(32), "255.255.255.255");
    }

    #[test]
    fn leases() {
        assert_eq!(lease(Some(&json!(3725))).as_deref(), Some("01:02:05 remaining"));
        assert_eq!(lease(Some(&json!(90061))).as_deref(), Some("1 days 01:01:01 remaining"));
        assert_eq!(lease(Some(&json!(4294967295u64))), None);
        assert_eq!(lease(Some(&json!(-5))).as_deref(), Some("-1 days 23:59:55 remaining"));
        assert_eq!(lease(Some(&json!(true))).as_deref(), Some("00:00:01 remaining"));
        assert_eq!(lease(Some(&json!(1.5))), None);
        assert_eq!(lease(None), None);
    }

    /// Fixed 2026-11-11: a second gateway or DNS server was printed two columns right of the first one's value.
    #[test]
    fn a_second_value_lines_up_under_the_first() {
        let mut o = Out(String::new());
        o.line("Default Gateway", "fe80::1");
        o.print(&format!("{}{}", " ".repeat(CONTINUED), "192.0.2.1"));
        let lines: Vec<&str> = o.0.lines().collect();
        assert_eq!(lines[0].find("fe80"), lines[1].find("192"));
        assert_eq!(lines[0], "   Default Gateway. . . . . . . . . .  : fe80::1");
    }
}
