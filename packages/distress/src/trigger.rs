//! "Typed N times within M seconds": a typed distress password only counts once enough of them fall inside the window.

/// Records a hit at `now` and says whether the rule is met. `hits` keeps only those still inside the window; once met it is cleared.
pub fn register(hits: &mut Vec<i64>, now: i64, count: u32, window_secs: u32) -> bool {
    hits.retain(|t| now - *t <= window_secs as i64 && *t <= now);
    hits.push(now);
    if hits.len() as u32 >= count.max(1) {
        hits.clear();
        true
    } else {
        false
    }
}

pub fn load() -> Vec<i64> {
    std::fs::read_to_string(crate::config::state_dir().join("hits.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn store(hits: &[i64]) {
    let d = crate::config::state_dir();
    if std::fs::create_dir_all(&d).is_ok() {
        let _ = std::fs::write(d.join("hits.json"), serde_json::to_string(hits).unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use super::register;

    #[test]
    fn single_entry_fires_at_once() {
        let mut h = vec![];
        assert!(register(&mut h, 100, 1, 60));
    }

    #[test]
    fn three_in_a_minute() {
        let mut h = vec![];
        assert!(!register(&mut h, 100, 3, 60));
        assert!(!register(&mut h, 130, 3, 60));
        assert!(register(&mut h, 150, 3, 60));
        assert!(h.is_empty());
    }

    #[test]
    fn old_hits_expire() {
        let mut h = vec![];
        assert!(!register(&mut h, 100, 2, 60));
        assert!(!register(&mut h, 200, 2, 60));
        assert!(register(&mut h, 230, 2, 60));
    }
}
