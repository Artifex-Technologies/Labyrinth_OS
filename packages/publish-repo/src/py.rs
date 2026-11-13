//! The few Python behaviours the publish depended on, so the port reads the same text the same way: what counts as white
//! space and as a line break, text from a program (its line endings), pathlib's tidying of a folder name, and the
//! published.json that json.dumps(..., indent=1) wrote (the next publish compares against it).

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

/// str.isspace() for one character: Unicode's White_Space and the four separators \x1c to \x1f.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// str.strip()
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// str.split() without arguments
pub fn split_ws(s: &str) -> Vec<&str> {
    s.split(is_space).filter(|p| !p.is_empty()).collect()
}

/// str.splitlines(): \n, \r, \r\n, \v, \f, \x1c, \x1d, \x1e, \x85, U+2028 and U+2029 end a line; no empty last line.
pub fn splitlines(s: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            lines.push(&s[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(_, '\n')) = chars.peek() {
                    chars.next();
                    end += 1;
                }
            }
            start = end;
        }
    }
    if start < s.len() {
        lines.push(&s[start..]);
    }
    lines
}

/// What subprocess's text mode and Path.read_text() do to line endings: \r\n and \r become \n.
pub fn universal_newlines(s: &str) -> String {
    if s.contains('\r') {
        s.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        s.to_string()
    }
}

/// pathlib's tidying of a path given as text: repeated slashes and "." parts go, so does a slash at the end; two slashes at
/// the start stay (POSIX leaves their meaning open). Nothing left is ".", given here as an empty path, which joins the way
/// Path(".") / "x" does ("x").
pub fn pathlib(s: &OsStr) -> PathBuf {
    let b = s.as_bytes();
    let root: &[u8] = if b.starts_with(b"//") && !b.starts_with(b"///") {
        b"//"
    } else if b.starts_with(b"/") {
        b"/"
    } else {
        b""
    };
    let parts: Vec<&[u8]> = b.split(|&c| c == b'/').filter(|p| !p.is_empty() && *p != b".").collect();
    let mut out = root.to_vec();
    out.extend_from_slice(&parts.join(&b'/'));
    PathBuf::from(OsStr::from_bytes(&out))
}

/// json.dumps of one string with ensure_ascii: anything outside space..~ as \uXXXX (lower-case hex).
pub fn json_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\x08' => o.push_str("\\b"),
            '\x0c' => o.push_str("\\f"),
            ' '..='~' => o.push(c),
            _ => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    o.push_str(&format!("\\u{u:04x}"));
                }
            }
        }
    }
    o.push('"');
    o
}

/// published.json as json.dumps({"heads": heads, "files": files}, indent=1) + "\n" wrote it.
pub fn state_json(heads: &[(String, String)], files: &[String]) -> String {
    let mut o = String::from("{\n \"heads\": ");
    if heads.is_empty() {
        o.push_str("{}");
    } else {
        o.push_str("{\n");
        let items: Vec<String> = heads.iter().map(|(k, v)| format!("  {}: {}", json_string(k), json_string(v))).collect();
        o.push_str(&items.join(",\n"));
        o.push_str("\n }");
    }
    o.push_str(",\n \"files\": ");
    if files.is_empty() {
        o.push_str("[]");
    } else {
        o.push_str("[\n");
        let items: Vec<String> = files.iter().map(|f| format!("  {}", json_string(f))).collect();
        o.push_str(&items.join(",\n"));
        o.push_str("\n ]");
    }
    o.push_str("\n}\n");
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_spaces_as_python() {
        assert_eq!(splitlines("a\r\nb\rc\nd\x0be\u{2028}f\n"), vec!["a", "b", "c", "d", "e", "f"]);
        assert_eq!(splitlines("a\n\nb"), vec!["a", "", "b"]);
        assert_eq!(splitlines(""), Vec::<&str>::new());
        assert_eq!(split_ws(" a\x1fb\u{a0}c  "), vec!["a", "b", "c"]);
        assert_eq!(strip("\x1c x \n"), "x");
        assert_eq!(universal_newlines("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn paths_tidied_as_pathlib() {
        let p = |s: &str| pathlib(OsStr::new(s));
        assert_eq!(p("/var//lib/./x/"), PathBuf::from("/var/lib/x"));
        assert_eq!(p("//a//b"), PathBuf::from("//a/b"));
        assert_eq!(p("///a"), PathBuf::from("/a"));
        assert_eq!(p("."), PathBuf::new());
        assert_eq!(p("").join("stage"), PathBuf::from("stage"));
        assert_eq!(p("./a/../b"), PathBuf::from("a/../b"));
    }

    #[test]
    fn state_as_json_dumps_wrote_it() {
        let heads = vec![("Labyrinth_OS".to_string(), "abc".to_string()), ("Sarina".to_string(), "d\u{e9}\u{1f600}\x7f".to_string())];
        assert_eq!(
            state_json(&heads, &["a.db".to_string(), "b".to_string()]),
            "{\n \"heads\": {\n  \"Labyrinth_OS\": \"abc\",\n  \"Sarina\": \"d\\u00e9\\ud83d\\ude00\\u007f\"\n },\n \"files\": [\n  \"a.db\",\n  \"b\"\n ]\n}\n"
        );
        assert_eq!(state_json(&heads[..1], &[]), "{\n \"heads\": {\n  \"Labyrinth_OS\": \"abc\"\n },\n \"files\": []\n}\n");
    }
}
