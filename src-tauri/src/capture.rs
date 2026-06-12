use std::path::{Path, PathBuf};

pub fn captures_dir(home: &Path) -> PathBuf {
    home.join("Pictures").join("DigitShot")
}

pub fn capture_filename(now: &chrono::DateTime<chrono::Local>) -> String {
    format!("DigitShot {}.png", now.format("%Y-%m-%d at %H.%M.%S"))
}

/// Interprets the result of a `screencapture -i` run.
/// Esc during capture exits 0 but writes no file -> None.
pub fn capture_result(exit_success: bool, file_exists: bool, path: PathBuf) -> Option<PathBuf> {
    if exit_success && file_exists {
        Some(path)
    } else {
        None
    }
}

/// Spawns the native interactive capture UI. Blocks until the user
/// finishes (drag/window-pick) or cancels with Esc.
pub fn run_interactive_capture(dir: &Path) -> std::io::Result<Option<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(capture_filename(&chrono::Local::now()));
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .arg("-i")
        .arg(&path)
        .status()?;
    Ok(capture_result(status.success(), path.exists(), path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn filename_is_sortable_and_finder_safe() {
        let t = chrono::Local.with_ymd_and_hms(2026, 6, 12, 9, 41, 7).unwrap();
        assert_eq!(capture_filename(&t), "DigitShot 2026-06-12 at 09.41.07.png");
    }

    #[test]
    fn captures_dir_is_under_pictures() {
        assert_eq!(
            captures_dir(Path::new("/Users/x")),
            PathBuf::from("/Users/x/Pictures/DigitShot")
        );
    }

    #[test]
    fn esc_during_capture_yields_none() {
        // screencapture exits 0 on Esc but writes nothing
        assert_eq!(capture_result(true, false, PathBuf::from("/tmp/a.png")), None);
        assert!(capture_result(true, true, PathBuf::from("/tmp/a.png")).is_some());
    }
}
