//! Python's regular expressions on text, for the regex crate: \w, \d and \s stand for Python's own classes (str.isalnum() or
//! "_", the Unicode decimal digits, str.isspace()), built from pycompat's tables, so a pattern matches exactly the text the
//! script's pattern matched. Leftmost-first matching with lazy and greedy repeats is the same in both for patterns without
//! back-references or look-arounds.

use crate::pycompat::unicode_tables::{DECIMAL_ZEROS, SPACE, WORD};

fn class(ranges: impl Iterator<Item = (u32, u32)>) -> String {
    let mut out = String::from("[");
    for (lo, hi) in ranges {
        if lo == hi {
            out.push_str(&format!("\\x{{{lo:x}}}"));
        } else {
            out.push_str(&format!("\\x{{{lo:x}}}-\\x{{{hi:x}}}"));
        }
    }
    out.push(']');
    out
}

/// A pattern written for Python's re, with \w \d \s (outside brackets) made Python's.
pub fn translate(pattern: &str) -> String {
    let word = class(WORD.iter().copied());
    let digit = class(DECIMAL_ZEROS.iter().map(|z| (*z, z + 9)));
    let space = class(SPACE.iter().copied());
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('w') => out.push_str(&word),
                Some('d') => out.push_str(&digit),
                Some('s') => out.push_str(&space),
                Some(n) => {
                    out.push('\\');
                    out.push(n);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn classes_as_python() {
        let w = regex::Regex::new(&format!("^{}+$", super::translate(r"\w"))).unwrap();
        assert!(w.is_match("abc_\u{e9}\u{661}"));
        assert!(!w.is_match("\u{301}"), "a combining mark is not \\w in Python");
        let d = regex::Regex::new(&format!("^{}+$", super::translate(r"\d"))).unwrap();
        assert!(d.is_match("0\u{661}\u{ff19}") && !d.is_match("\u{b2}"));
        let s = regex::Regex::new(&format!("^{}+$", super::translate(r"\s"))).unwrap();
        assert!(s.is_match(" \t\u{1c}\u{85}"));
    }
}
