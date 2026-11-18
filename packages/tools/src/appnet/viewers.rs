//! Viewer apps start on "ask" (the privacy pack, docs/PRIVACY_PACK.md): PDF viewers, office suites, image viewers and media
//! players open files that came from anywhere, and such a file can make the app contact a server the moment it is opened (a
//! remote template, a picture or font on the web, a playlist entry, album art and track names looked up online). So the first
//! time one of these apps tries to go online, the person is asked (labyrinth-appnet-ask), and one answer ("Allow all") puts it
//! back on any connection.
//!
//! The list is compiled in (VIEWERS: desktop ids, as labyrinth-launch and Settings > Apps name apps). `labyrinth-appnet apply`
//! (root, at boot and at every login) gives each listed app the rule "ask" for every person logged in (uid 1000 to 60000 with
//! a user manager running) who has no rule for it yet, once: the state file remembers which apps it did that for, so a
//! person's own choice later (Allow in Settings > Apps) is never overridden. Apps that are not installed get the rule too;
//! it does nothing until the app is installed and started.
//!
//! The switch (Settings > Security > Privacy, on by default): `labyrinth-appnet viewers off` (root, through pkexec) takes back
//! the rules it added that are still "ask" and adds no more for that person; `viewers on` adds them again (also for apps the
//! person had set to Allow before). `viewers status --json` (no root): {"on":bool,"apps":[the list],"asking":[apps on "ask"
//! because of it]}.
//!
//! State: /var/lib/labyrinth/app-network-viewers.json (readable by all; it holds app ids only), {UID: {"on": bool,
//! "seeded": [ids it set to ask], "had_rule": [ids that already had a rule]}}. LABYRINTH_APPNET_VIEWERS names another file;
//! set to "" it switches the defaults off (the parity harness's worlds, which predate them).

use super::{load_state, save_json, valid_app, Rules};
use crate::py::{self, Map, Value, R};

/// The apps that start on "ask": desktop ids (package and Flathub ids where they differ).
pub const VIEWERS: &[&str] = &[
    // on the ISO: Yggdrasil's media player; darktable (its map view downloads map tiles around where photos were taken)
    "yggdrasil-player",
    "org.darktable.darktable",
    "org.darktable.Darktable",
    // PDF viewers
    "org.gnome.Evince",
    "org.gnome.Papers",
    "org.kde.okular",
    "org.pwmt.zathura",
    "mupdf",
    "qpdfview",
    "atril",
    "xreader",
    "sioyek",
    "ai.storyteller.pdfcraft",
    // office suites (LibreOffice's apps one by one, as its package names them; the Flathub ones likewise)
    "libreoffice-startcenter",
    "libreoffice-writer",
    "libreoffice-calc",
    "libreoffice-impress",
    "libreoffice-draw",
    "libreoffice-math",
    "libreoffice-base",
    "org.libreoffice.LibreOffice",
    "org.libreoffice.LibreOffice.writer",
    "org.libreoffice.LibreOffice.calc",
    "org.libreoffice.LibreOffice.impress",
    "org.libreoffice.LibreOffice.draw",
    "org.libreoffice.LibreOffice.math",
    "org.libreoffice.LibreOffice.base",
    "onlyoffice-desktopeditors",
    "org.onlyoffice.desktopeditors",
    "wps-office-wps",
    "wps-office-et",
    "wps-office-wpp",
    "wps-office-pdf",
    "org.kde.calligrawords",
    "org.kde.calligrasheets",
    "org.kde.calligrastage",
    "abiword",
    "com.abisource.AbiWord",
    "org.gnumeric.gnumeric",
    "ai.storyteller.wordcraft",
    "ai.storyteller.gridcraft",
    "ai.storyteller.deckcraft",
    // image viewers
    "org.gnome.Loupe",
    "org.gnome.eog",
    "org.kde.gwenview",
    "org.nomacs.ImageLounge",
    "feh",
    "imv",
    "imv-dir",
    "org.xfce.ristretto",
    "org.gnome.gThumb",
    "qimgv",
    "swayimg",
    "nsxiv",
    "sxiv",
    "eom",
    "lximage-qt",
    // media players (music players look up album art and track names online)
    "vlc",
    "org.videolan.VLC",
    "mpv",
    "io.mpv.Mpv",
    "io.github.celluloid_player.Celluloid",
    "com.github.rafostar.Clapper",
    "org.gnome.Totem",
    "org.gnome.Showtime",
    "org.kde.haruna",
    "org.kde.dragonplayer",
    "smplayer",
    "info.smplayer.SMPlayer",
    "org.xfce.Parole",
    "org.kde.elisa",
    "org.gnome.Rhythmbox3",
    "org.gnome.Lollypop",
    "audacious",
    "org.strawberrymusicplayer.strawberry",
    "org.clementine_player.Clementine",
    "io.github.quodlibet.QuodLibet",
    "deadbeef",
];

/// The state file, or None when the defaults are switched off for this system (LABYRINTH_APPNET_VIEWERS="").
pub fn path() -> Option<String> {
    let p = py::env_get("LABYRINTH_APPNET_VIEWERS", "/var/lib/labyrinth/app-network-viewers.json");
    (!p.is_empty()).then_some(p)
}

/// One person's record.
#[derive(Debug, Clone, PartialEq)]
pub struct UserState {
    pub on: bool,
    pub seeded: Vec<String>,
    pub had_rule: Vec<String>,
}

impl Default for UserState {
    fn default() -> Self {
        UserState { on: true, seeded: vec![], had_rule: vec![] }
    }
}

/// {uid: record}, in the file's order.
pub type State = Vec<(String, UserState)>;

fn ids(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().filter(|s| valid_app(s)).map(String::from)).collect(),
        _ => vec![],
    }
}

/// The state file; a missing or broken one is no records (everyone on), bad entries are left out.
pub fn load(path: &str) -> State {
    let Ok(Value::Object(m)) = py::load_json(path) else { return vec![] };
    m.iter()
        .filter(|(u, _)| !u.is_empty() && u.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|(u, v)| match v {
            Value::Object(e) => Some((
                u.clone(),
                UserState {
                    on: e.get("on").and_then(Value::as_bool).unwrap_or(true),
                    seeded: ids(e.get("seeded")),
                    had_rule: ids(e.get("had_rule")),
                },
            )),
            _ => None,
        })
        .collect()
}

pub fn save(state: &State, path: &str) -> R<()> {
    let mut top = Map::new();
    for (u, s) in state {
        let mut e = Map::new();
        e.insert("on".into(), Value::Bool(s.on));
        e.insert("seeded".into(), Value::Array(s.seeded.iter().map(|x| Value::from(x.as_str())).collect()));
        e.insert("had_rule".into(), Value::Array(s.had_rule.iter().map(|x| Value::from(x.as_str())).collect()));
        top.insert(u.clone(), Value::Object(e));
    }
    save_json(path, &Value::Object(top), 0o644)
}

fn record<'a>(state: &'a mut State, uid: &str) -> &'a mut UserState {
    let i = match state.iter().position(|(u, _)| u == uid) {
        Some(i) => i,
        None => {
            state.push((uid.to_string(), UserState::default()));
            state.len() - 1
        }
    };
    &mut state[i].1
}

fn rules_of<'a>(rules: &'a mut Rules, uid: &str) -> &'a mut Vec<(String, String)> {
    let i = match rules.iter().position(|(u, _)| u == uid) {
        Some(i) => i,
        None => {
            rules.push((uid.to_string(), vec![]));
            rules.len() - 1
        }
    };
    &mut rules[i].1
}

fn drop_empty(rules: &mut Rules) {
    rules.retain(|(_, apps)| !apps.is_empty());
}

/// Gives the person the viewer rules they have not had yet (when their switch is on). True when the rules or the record
/// changed (both files are then written).
pub fn seed(rules: &mut Rules, state: &mut State, uid: &str) -> bool {
    let st = record(state, uid);
    if !st.on {
        return false;
    }
    let before = rules.iter().any(|(u, _)| u == uid);
    let mine = rules_of(rules, uid);
    let mut changed = false;
    for app in VIEWERS {
        let app = app.to_string();
        if st.seeded.contains(&app) || st.had_rule.contains(&app) {
            continue;
        }
        if mine.iter().any(|(a, _)| *a == app) {
            st.had_rule.push(app);
        } else {
            mine.push((app.clone(), "ask".into()));
            st.seeded.push(app);
        }
        changed = true;
    }
    if !before {
        drop_empty(rules);
    }
    changed
}

/// Switched off: the rules it added that are still "ask" go, and no more are added.
pub fn turn_off(rules: &mut Rules, state: &mut State, uid: &str) {
    let st = record(state, uid);
    let seeded = std::mem::take(&mut st.seeded);
    st.had_rule.clear();
    st.on = false;
    if let Some((_, mine)) = rules.iter_mut().find(|(u, _)| u == uid) {
        mine.retain(|(a, m)| !(m == "ask" && seeded.contains(a)));
    }
    drop_empty(rules);
}

/// Switched on: every listed app without a rule gets "ask" again.
pub fn turn_on(rules: &mut Rules, state: &mut State, uid: &str) {
    let st = record(state, uid);
    *st = UserState::default();
    seed(rules, state, uid);
}

/// The apps on "ask" because of the defaults (added by them and not changed since).
pub fn asking(rules: &Rules, state: &State, uid: &str) -> Vec<String> {
    let Some((_, st)) = state.iter().find(|(u, _)| u == uid) else { return vec![] };
    let mine = rules.iter().find(|(u, _)| u == uid).map(|(_, a)| a.as_slice()).unwrap_or(&[]);
    st.seeded.iter().filter(|a| mine.iter().any(|(x, m)| x == *a && m == "ask")).cloned().collect()
}

/// Whether the person's switch is on (no record: on).
pub fn is_on(state: &State, uid: &str) -> bool {
    state.iter().find(|(u, _)| u == uid).is_none_or(|(_, s)| s.on)
}

/// The people logged in now: uid 1000 to 60000 whose user manager has a cgroup under `root`, in number order.
pub fn logged_in(root: &str) -> Vec<String> {
    let base = py::join(root, "user.slice");
    let mut out: Vec<u32> = std::fs::read_dir(&base)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let n = e.file_name().into_string().ok()?;
                    let uid: u32 = n.strip_prefix("user-")?.strip_suffix(".slice")?.parse().ok()?;
                    let manager = py::join_all(&[&base, &n, &format!("user@{uid}.service")]);
                    ((1000..=60000).contains(&uid) && py::isdir(&manager)).then_some(uid)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_unstable();
    out.into_iter().map(|u| u.to_string()).collect()
}

/// `apply`'s first step (root only, when the defaults are on for this system): the viewer rules for everyone logged in.
/// Returns the rules to load. A file that cannot be written is said on standard error; the rules are loaded anyway.
pub fn seed_logged_in(cgroup_root: &str) -> R<Option<Rules>> {
    let Some(vp) = path() else { return Ok(None) };
    if py::geteuid() != 0 {
        return Ok(None);
    }
    let mut rules = load_state(None)?;
    let mut state = load(&vp);
    let mut changed = false;
    for uid in logged_in(cgroup_root) {
        changed |= seed(&mut rules, &mut state, &uid);
    }
    if changed {
        if let Err(e) = super::save_state(&rules, None).and_then(|_| save(&state, &vp)) {
            eprintln!("labyrinth-appnet: the viewer apps' rules could not be saved: {}", e.0);
        }
    }
    Ok(Some(rules))
}

/// `viewers status [--json]`, for the person running it.
pub fn status(json: bool) -> R<i32> {
    let uid = py::getuid().to_string();
    let rules = load_state(None)?;
    let (on, asking) = match path() {
        Some(vp) => {
            let st = load(&vp);
            (is_on(&st, &uid), asking(&rules, &st, &uid))
        }
        None => (false, vec![]),
    };
    if json {
        let mut m = Map::new();
        m.insert("on".into(), Value::Bool(on));
        m.insert("apps".into(), Value::Array(VIEWERS.iter().map(|a| Value::from(*a)).collect()));
        m.insert("asking".into(), Value::Array(asking.iter().map(|a| Value::from(a.as_str())).collect()));
        println!("{}", serde_json::to_string(&Value::Object(m)).unwrap_or_default());
    } else {
        println!("viewer apps ask before going online: {}", if on { "on" } else { "off" });
        for a in &asking {
            println!("  {a}");
        }
    }
    Ok(0)
}

/// `viewers on|off` (root, through pkexec: the person is PKEXEC_UID or SUDO_UID), then the rules are loaded.
pub fn switch(on: bool) -> R<i32> {
    if py::geteuid() != 0 {
        eprintln!("labyrinth-appnet: viewers on|off needs root (run it with pkexec)");
        return Ok(1);
    }
    let Some(vp) = path() else {
        eprintln!("labyrinth-appnet: the viewer apps' defaults are switched off on this system");
        return Ok(1);
    };
    let uid = match py::env("PKEXEC_UID").filter(|u| !u.is_empty()).or_else(|| py::env("SUDO_UID")) {
        Some(u) if crate::pycompat::isdigit(&u) => u,
        _ => {
            eprintln!("labyrinth-appnet: cannot tell whose apps these are (no PKEXEC_UID)");
            return Ok(1);
        }
    };
    let mut rules = load_state(None)?;
    let mut state = load(&vp);
    if on {
        turn_on(&mut rules, &mut state, &uid);
    } else {
        turn_off(&mut rules, &mut state, &uid);
    }
    super::save_state(&rules, None)?;
    save(&state, &vp)?;
    super::apply(Some(&rules))
}
