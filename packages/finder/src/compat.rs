//! Reading data saved before the app was renamed from "Labyrinth Files" to "Finder".
//!
//! Settings, Proton Drive links and verification reports used to live in a `labyrinth-files`
//! folder under the config directory. Writes go to the new `finder` folder; reads fall back to
//! the old one when the new file does not exist, so nothing the user set up is lost.

use std::path::{Path, PathBuf};

const OLD: &str = "labyrinth-files";
const NEW: &str = "finder";

/// The path the same file had under the old folder name, if `p` is inside the new folder.
pub fn legacy_path(p: &Path) -> Option<PathBuf> {
    let parent = p.parent()?;
    if parent.file_name()? != NEW {
        return None;
    }
    Some(parent.with_file_name(OLD).join(p.file_name()?))
}

/// Read `p`, or its legacy twin when `p` is missing.
pub fn read_to_string(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok().or_else(|| legacy_path(p).and_then(|l| std::fs::read_to_string(l).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_new_folder_to_old() {
        assert_eq!(legacy_path(Path::new("/c/finder/state.json")), Some(PathBuf::from("/c/labyrinth-files/state.json")));
        assert_eq!(legacy_path(Path::new("/c/other/state.json")), None);
        assert_eq!(legacy_path(Path::new("state.json")), None);
    }

    #[test]
    fn falls_back_only_when_new_is_missing() {
        let d = std::env::temp_dir().join(format!("compat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("labyrinth-files")).unwrap();
        std::fs::write(d.join("labyrinth-files/a.json"), "old").unwrap();
        let new = d.join("finder/a.json");
        assert_eq!(read_to_string(&new).as_deref(), Some("old"));
        std::fs::create_dir_all(d.join("finder")).unwrap();
        std::fs::write(&new, "new").unwrap();
        assert_eq!(read_to_string(&new).as_deref(), Some("new"));
        assert!(read_to_string(&d.join("finder/missing.json")).is_none());
    }
}
