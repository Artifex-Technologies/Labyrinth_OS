//! steam_playall: turns on "Enable Steam Play for all other titles" in Steam's config.vdf (a CompatToolMapping entry "0"), never
//! changing a choice already made; labyrinth-gaming setup runs it while Steam is closed. The Python script steam_playall.py with
//! the same arguments (CONFIG_VDF [TOOL_NAME]), output, backup (CONFIG_VDF.labyrinth-EPOCH.bak) and exit codes; its help text is
//! src/doc/steam_playall.txt. Installed as steam_playall (no .py).
//!
//! Not 1:1: a path that is not UTF-8 has its bad bytes replaced (the script opened it as it was).

use crate::py::{self, Crash, R};

pub const DOC: &str = include_str!("doc/steam_playall.txt");
pub const DEFAULT_TOOL: &str = "proton_experimental";

/// What apply() did: the text was left as it was ("already"), an entry was added ("added"), or the file does not look as
/// expected ("refused:<why>").
/// Where the script's pattern `^([ \t]*)"CompatToolMapping"[ \t]*\r?\n[ \t]*\{[ \t]*\r?\n` (multi-line) first matches: the
/// indent, and where the match ends (written out, so the program carries no regular-expression engine).
fn find_block(text: &str) -> Option<(&str, usize)> {
    let b = text.as_bytes();
    let blanks = |mut i: usize| {
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        i
    };
    let newline = |i: usize| {
        let i = if b.get(i) == Some(&b'\r') { i + 1 } else { i };
        (b.get(i) == Some(&b'\n')).then_some(i + 1)
    };
    let mut start = 0;
    loop {
        let i = blanks(start);
        if b[i..].starts_with(b"\"CompatToolMapping\"") {
            let open = newline(blanks(i + 19)).map(blanks).filter(|&j| b.get(j) == Some(&b'{'));
            if let Some(end) = open.and_then(|j| newline(blanks(j + 1))) {
                return Some((&text[start..i], end));
            }
        }
        start += text[start..].find('\n')? + 1;
    }
}

pub fn apply(text: &str, tool: &str) -> (String, String) {
    let (indent, end) = match find_block(text) {
        Some(m) => m,
        None => return (text.to_string(), "refused:no CompatToolMapping block (run Steam once, then retry)".into()),
    };
    // the block's own closing brace
    let bytes = text.as_bytes();
    let (mut depth, mut i) = (1i64, end);
    while i < bytes.len() && depth != 0 {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    if depth != 0 {
        return (text.to_string(), "refused:unbalanced braces".into());
    }
    // a key "0" at the block's own level (not nested deeper)
    let block = &text[end..i - 1];
    let mut d = 0i64;
    for line in py::splitlines(block) {
        let s = py::strip(line);
        if s == "{" {
            d += 1;
        } else if s == "}" {
            d -= 1;
        } else if d == 0 && s == "\"0\"" {
            return (text.to_string(), "already".into());
        }
    }
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let t = format!("{indent}\t");
    let entry =
        format!("{t}\"0\"{nl}{t}{{{nl}{t}\t\"name\"\t\t\"{tool}\"{nl}{t}\t\"config\"\t\t\"\"{nl}{t}\t\"priority\"\t\t\"75\"{nl}{t}}}{nl}");
    (format!("{}{entry}{}", &text[..end], &text[end..]), "added".into())
}

fn run(argv: &[String]) -> R<i32> {
    let path = &argv[1];
    let tool = argv.get(2).map(String::as_str).unwrap_or(DEFAULT_TOOL);
    // read as it is (newline=""): a file with \r\n keeps them
    let bytes = std::fs::read(path).map_err(|e| py::os_err(e, path))?;
    let text = py::utf8(bytes).map_err(Crash)?;
    let (new, status) = apply(&text, tool);
    if status.starts_with("refused") {
        println!("{status}");
        return Ok(1);
    }
    if status == "added" {
        py::copy2(path, &format!("{path}.labyrinth-{}.bak", py::now() as i64))?;
        py::write_file(path, &new)?;
    }
    println!("{status}");
    Ok(0)
}

pub fn main(argv: &[String]) -> i32 {
    if argv.len() != 2 && argv.len() != 3 {
        println!("{DOC}");
        return 2;
    }
    match run(argv) {
        Ok(rc) => rc,
        Err(c) => py::crashed("steam_playall", &c),
    }
}

#[cfg(test)]
mod tests {
    //! tests/test_steam_playall.py, carried over.
    use super::*;

    fn base(inner: &str) -> String {
        format!(
            "\"InstallConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"Valve\"\n\t\t{{\n\t\t\t\"Steam\"\n\t\t\t{{\n\t\t\t\t\"CompatToolMapping\"\n\t\t\t\t{{\n{inner}\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n"
        )
    }
    const GAME: &str = "\t\t\t\t\t\"620\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"name\"\t\t\"GE-Proton9-1\"\n\t\t\t\t\t}\n";
    const ZERO: &str = "\t\t\t\t\t\"0\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"name\"\t\t\"proton_9\"\n\t\t\t\t\t}\n";

    #[test]
    fn adds_to_empty_block() {
        let (new, st) = apply(&base(""), DEFAULT_TOOL);
        assert_eq!(st, "added");
        assert!(new.contains("\"name\"\t\t\"proton_experimental\""));
        assert_eq!(new.matches('{').count(), new.matches('}').count());
    }

    #[test]
    fn adds_beside_per_game_entry_and_keeps_it() {
        let (new, st) = apply(&base(GAME), DEFAULT_TOOL);
        assert_eq!(st, "added");
        assert!(new.contains("\"GE-Proton9-1\""));
        assert!(new.contains("\"proton_experimental\""));
    }

    #[test]
    fn existing_default_is_never_changed() {
        let text = base(ZERO);
        let (new, st) = apply(&text, DEFAULT_TOOL);
        assert_eq!(st, "already");
        assert_eq!(new, text);
    }

    #[test]
    fn nested_zero_does_not_count_as_default() {
        let nested = "\t\t\t\t\t\"620\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"0\"\n\t\t\t\t\t\t{\n\t\t\t\t\t\t}\n\t\t\t\t\t}\n";
        assert_eq!(apply(&base(nested), DEFAULT_TOOL).1, "added");
    }

    #[test]
    fn idempotent() {
        let (once, _) = apply(&base(GAME), DEFAULT_TOOL);
        let (twice, st) = apply(&once, DEFAULT_TOOL);
        assert_eq!(st, "already");
        assert_eq!(once, twice);
    }

    #[test]
    fn refuses_without_block() {
        let text = "\"InstallConfigStore\"\n{\n}\n";
        let (new, st) = apply(text, DEFAULT_TOOL);
        assert!(st.starts_with("refused"));
        assert_eq!(new, text);
    }

    #[test]
    fn preserves_crlf() {
        let (new, _) = apply(&base("").replace('\n', "\r\n"), DEFAULT_TOOL);
        assert!(!new.contains("\r\n\n"));
        assert!(new.bytes().enumerate().all(|(i, b)| b != b'\n' || (i > 0 && new.as_bytes()[i - 1] == b'\r')));
    }

    /// The bug fixed on 2026-11-11: the script read the file with universal newlines, so a config.vdf with \r\n was written back
    /// with \n everywhere. The program keeps the file's line ends.
    #[test]
    fn the_program_keeps_a_files_crlf() {
        let d = crate::testutil::TempDir::new();
        let p = d.p("config.vdf");
        std::fs::write(&p, base("").replace('\n', "\r\n")).unwrap();
        assert_eq!(main(&["steam_playall".into(), p.clone()]), 0);
        let after = std::fs::read_to_string(&p).unwrap();
        assert!(after.contains("\"proton_experimental\"\r\n"));
        assert_eq!(after.matches('\n').count(), after.matches("\r\n").count());
    }
}
