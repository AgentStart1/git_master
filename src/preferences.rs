use crate::ui::theme::{self, ThemeMode};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
static WRITER: Mutex<()> = Mutex::new(());

pub fn path() -> PathBuf {
    #[cfg(feature = "test-rpc")]
    if let Some(dir) = std::env::var_os("GIT_MASTER_TEST_CONFIG_DIR") {
        return PathBuf::from(dir).join("appearance");
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("GitMaster").join("appearance")
}
pub fn load(path: &Path) -> ThemeMode {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| ThemeMode::parse(&s))
        .unwrap_or(ThemeMode::Dark)
}
fn save(path: &Path, mode: ThemeMode) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("Missing settings directory"))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    writeln!(file, "{}", mode.name())?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn save_current() -> std::io::Result<()> {
    let _guard = WRITER.lock().unwrap_or_else(|e| e.into_inner());
    // Read after serialization: rapid toggles cannot persist an older mode last.
    save(&path(), theme::mode())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_round_trips_and_replaces_previous_mode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings/appearance");
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            save(&path, mode).unwrap();
            assert_eq!(load(&path), mode);
        }
    }
    #[test]
    fn missing_or_invalid_preferences_default_to_dark() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("appearance");
        assert_eq!(load(&path), ThemeMode::Dark);
        std::fs::write(&path, "invalid").unwrap();
        assert_eq!(load(&path), ThemeMode::Dark);
    }
}

pub fn recent_workspaces() -> Vec<PathBuf> {
    std::fs::read(path().with_file_name("workspaces.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}
pub fn remember_workspace(workspace: &Path) -> std::io::Result<Vec<PathBuf>> {
    let _guard = WRITER.lock().unwrap_or_else(|e| e.into_inner());
    let mut paths = recent_workspaces();
    paths.retain(|p| p != workspace);
    paths.insert(0, workspace.to_owned());
    paths.truncate(8);
    let target = path().with_file_name("workspaces.json");
    let parent = target
        .parent()
        .ok_or_else(|| std::io::Error::other("Missing settings directory"))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(&mut file, &paths)?;
    file.persist(target).map_err(|e| e.error)?;
    Ok(paths)
}
