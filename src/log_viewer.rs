use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 256 * 1024;
const MAX_LINES: usize = 2000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub path: PathBuf,
    pub text: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Loading,
    Ready(Snapshot),
    Failed(String),
}

impl State {
    pub fn display_text(&self) -> &str {
        match self {
            Self::Loading => "Loading application logs…",
            Self::Failed(error) => error,
            Self::Ready(snapshot) if snapshot.text.is_empty() => "No logs recorded today.",
            Self::Ready(snapshot) => &snapshot.text,
        }
    }
}

pub struct Viewer {
    pub state: State,
    pub following: bool,
    pub pending: Option<Snapshot>,
    pub refresh_error: Option<String>,
}

impl Viewer {
    pub fn new() -> Self {
        Self {
            state: State::Loading,
            following: true,
            pending: None,
            refresh_error: None,
        }
    }

    pub fn accept(&mut self, result: Result<Snapshot, String>, manual: bool) {
        match result {
            Ok(snapshot) => {
                self.refresh_error = None;
                if self.following || !matches!(self.state, State::Ready(_)) {
                    self.state = State::Ready(snapshot);
                    self.pending = None;
                } else {
                    self.pending = match &self.state {
                        State::Ready(current) if *current == snapshot => None,
                        _ => Some(snapshot),
                    };
                }
            }
            Err(error) => {
                if manual || !matches!(self.state, State::Ready(_)) {
                    self.state = State::Failed(error);
                    self.pending = None;
                    self.refresh_error = None;
                } else {
                    // Keep readable content and the user's position on a polling error.
                    self.refresh_error = Some(error);
                }
            }
        }
    }

    pub fn resume(&mut self) {
        self.following = true;
        if let Some(snapshot) = self.pending.take() {
            self.state = State::Ready(snapshot);
        }
    }
}

pub struct LogViewerHost;

impl LogViewerHost {
    pub fn load() -> Result<Snapshot, String> {
        #[cfg(feature = "test-rpc")]
        crate::test_rpc::worker_gate::wait("GIT_MASTER_TEST_LOGS_GATE");
        let path = crate::operation_log::log_path();
        read_tail(&path).map_err(|error| format!("Cannot read {}: {error}", path.display()))
    }
}

fn read_tail(path: &Path) -> std::io::Result<Snapshot> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Snapshot {
                path: path.to_owned(),
                text: String::new(),
                truncated: false,
            });
        }
        Err(error) => return Err(error),
    };
    let start = file.metadata()?.len().saturating_sub(MAX_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES).read_to_end(&mut bytes)?;
    // Skip the partial first entry when the byte limit lands inside a line.
    let bytes = if start > 0 {
        bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.as_slice(), |end| &bytes[end + 1..])
    } else {
        &bytes
    };
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.lines().rev().take(MAX_LINES + 1).collect::<Vec<_>>();
    let truncated = start > 0 || lines.len() > MAX_LINES;
    lines.truncate(MAX_LINES);
    lines.reverse();
    Ok(Snapshot {
        path: path.to_owned(),
        text: lines.join("\n"),
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(text: &str) -> Snapshot {
        Snapshot {
            path: PathBuf::from("application.log"),
            text: text.into(),
            truncated: true,
        }
    }

    #[test]
    fn paused_reader_keeps_visible_content_until_resuming_even_when_tail_rolls() {
        let mut viewer = Viewer::new();
        viewer.accept(Ok(snapshot("old visible entry")), false);
        viewer.following = false;
        viewer.accept(Ok(snapshot("new tail without old entry")), false);
        assert_eq!(viewer.state.display_text(), "old visible entry");
        assert_eq!(
            viewer.pending.as_ref().unwrap().text,
            "new tail without old entry"
        );
        viewer.resume();
        assert_eq!(viewer.state.display_text(), "new tail without old entry");
        assert!(viewer.following && viewer.pending.is_none());
    }

    #[test]
    fn automatic_failure_keeps_last_good_content_and_recovers() {
        let mut viewer = Viewer::new();
        viewer.accept(Ok(snapshot("last good entry")), false);
        viewer.accept(Err("temporarily unavailable".into()), false);
        assert_eq!(viewer.state.display_text(), "last good entry");
        assert_eq!(
            viewer.refresh_error.as_deref(),
            Some("temporarily unavailable")
        );
        viewer.accept(Ok(snapshot("recovered entry")), false);
        assert_eq!(viewer.state.display_text(), "recovered entry");
        assert!(viewer.refresh_error.is_none());
        viewer.accept(Err("manual failure".into()), true);
        assert!(matches!(viewer.state, State::Failed(_)));
        assert!(viewer.refresh_error.is_none());
    }

    #[test]
    fn tail_bounds_large_logs_and_keeps_latest_entry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("application.log");
        let data = (0..6000)
            .map(|i| format!("entry {i}: {}\n", "中".repeat(25)))
            .collect::<String>();
        std::fs::write(&path, data).unwrap();
        let tail = read_tail(&path).unwrap();
        assert!(tail.truncated);
        assert!(tail.text.len() <= MAX_BYTES as usize);
        assert_eq!(tail.text.lines().count(), MAX_LINES);
        assert!(tail.text.lines().last().unwrap().starts_with("entry 5999:"));
        assert!(!tail.text.contains('\u{fffd}'));
    }

    #[test]
    fn missing_is_empty_but_invalid_path_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            read_tail(&directory.path().join("missing.log"))
                .unwrap()
                .text
                .is_empty()
        );
        assert!(read_tail(directory.path()).is_err());
    }
}
