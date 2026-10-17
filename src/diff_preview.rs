use crate::diff_model::{self, DiffRow, RenderedDiff, RowKind};
use git2::{Diff, DiffFile, DiffFormat, FileMode, Repository};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PREVIEW_LIMIT: usize = 512 * 1024;
const INPUT_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub struct DiffRenderer {
    program: Option<PathBuf>,
}
impl DiffRenderer {
    #[cfg(test)]
    pub fn with_program(program: PathBuf) -> Self {
        Self {
            program: Some(program),
        }
    }

    #[cfg(test)]
    pub fn is_available(&self) -> bool {
        self.program.is_some()
    }

    // Detection is per preview, so installation/removal takes effect without restarting.
    pub fn detect() -> Self {
        let name = if cfg!(windows) { "difft.exe" } else { "difft" };
        let program = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
            .map(|dir| dir.join(name))
            .find(|path| {
                path.is_file() && {
                    let mut command = Command::new(path);
                    command.arg("--version");
                    capture(&mut command, Instant::now() + Duration::from_secs(2), 4096)
                        .is_ok_and(|text| text.starts_with("Difftastic "))
                }
            });
        Self { program }
    }

    pub fn render(
        &self,
        repo: &Repository,
        diff: Diff<'_>,
        workdir: bool,
    ) -> Result<RenderedDiff, git2::Error> {
        if diff.deltas().len() == 0 {
            return Ok(RenderedDiff {
                text: "No changes.\n".into(),
                rows: vec![],
            });
        }
        if let Some(program) = &self.program {
            if let Ok(mut document) = self.render_difft(program, repo, &diff, workdir) {
                document.text.insert_str(0, "Diff renderer: difft\n");
                return Ok(document);
            }
            let text = format!(
                "Diff renderer: Git (difft could not render this preview)\n{}",
                unified(diff)?
            );
            let rows = diff_model::from_unified(
                text.lines().skip(1).collect::<Vec<_>>().join("\n").as_str(),
            );
            return Ok(RenderedDiff { text, rows });
        }
        let text = unified(diff)?;
        let rows = diff_model::from_unified(&text);
        Ok(RenderedDiff { text, rows })
    }

    fn render_difft(
        &self,
        program: &PathBuf,
        repo: &Repository,
        diff: &Diff<'_>,
        workdir: bool,
    ) -> std::io::Result<RenderedDiff> {
        let temp = tempfile::tempdir()?;
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut document = RenderedDiff::default();
        for delta in diff.deltas() {
            if document.text.len() >= PREVIEW_LIMIT || document.rows.len() >= diff_model::ROW_LIMIT
            {
                document
                    .rows
                    .push(DiffRow::label(RowKind::Note, "Preview truncated."));
                break;
            }
            let old = delta.old_file();
            let new = delta.new_file();
            if old.mode() == FileMode::Commit || new.mode() == FileMode::Commit {
                return Err(std::io::Error::other("Submodule changes use Git preview"));
            }
            let old_data = content(repo, &old, false)?;
            let new_data = content(repo, &new, workdir)?;
            let old_path = temp.path().join("old");
            let new_path = temp.path().join("new");
            std::fs::write(&old_path, &old_data)?;
            std::fs::write(&new_path, &new_data)?;
            let display = new
                .path()
                .or_else(|| old.path())
                .ok_or_else(|| std::io::Error::other("Missing diff path"))?;
            let mut command = Command::new(program);
            // Git's external-diff protocol carries the original name for language detection
            // without allowing repository paths to become command options or shell code.
            command
                .args(["--color=never", "--display=json", "--width=120"])
                .arg("--")
                .arg(display)
                .arg(&old_path)
                .arg(old.id().to_string())
                .arg(format!("{:06o}", i32::from(old.mode())))
                .arg(&new_path)
                .arg(new.id().to_string())
                .arg(format!("{:06o}", i32::from(new.mode())));
            // Environment overrides must not inject ANSI output, check-only mode, or exit-code behavior.
            for (key, _) in
                std::env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("DFT_"))
            {
                command.env_remove(key);
            }
            command.env("DFT_UNSTABLE", "yes");
            let json = capture(&mut command, deadline, 4 * 1024 * 1024)?;
            let mut rows =
                diff_model::from_difft(&json, &display.to_string_lossy(), &old_data, &new_data)
                    .map_err(std::io::Error::other)?;
            if rows.is_empty() && old.mode() != new.mode() {
                rows.push(DiffRow::label(RowKind::File, display.to_string_lossy()));
                rows.push(DiffRow::label(
                    RowKind::Note,
                    format!(
                        "File mode changed: {:06o} → {:06o}",
                        i32::from(old.mode()),
                        i32::from(new.mode())
                    ),
                ));
            }
            for row in &rows {
                let prefix = match row.kind {
                    RowKind::Added => "+",
                    RowKind::Removed => "-",
                    _ => "",
                };
                document.text.push_str(&format!("{prefix}{}\n", row.text));
            }
            document.rows.extend(rows);
        }
        document.limit();
        Ok(document)
    }
}

fn content(repo: &Repository, file: &DiffFile<'_>, workdir: bool) -> std::io::Result<Vec<u8>> {
    if file.mode() == FileMode::Unreadable {
        return Ok(Vec::new());
    }
    if workdir {
        let path = repo
            .workdir()
            .zip(file.path())
            .map(|(root, path)| root.join(path))
            .ok_or_else(|| std::io::Error::other("Missing working-tree path"))?;
        if file.mode() == FileMode::Link {
            return Ok(std::fs::read_link(path)?
                .as_os_str()
                .as_encoded_bytes()
                .to_vec());
        }
        if std::fs::metadata(&path)?.len() > INPUT_LIMIT as u64 {
            return Err(std::io::Error::other(
                "File exceeds structural preview limit",
            ));
        }
        let mut data = Vec::new();
        std::fs::File::open(path)?
            .take(INPUT_LIMIT as u64 + 1)
            .read_to_end(&mut data)?;
        if data.len() > INPUT_LIMIT {
            return Err(std::io::Error::other(
                "File exceeds structural preview limit",
            ));
        }
        return Ok(data);
    }
    if file.id().is_zero() {
        return Ok(Vec::new());
    }
    let blob = repo.find_blob(file.id()).map_err(std::io::Error::other)?;
    if blob.size() > INPUT_LIMIT {
        return Err(std::io::Error::other(
            "Blob exceeds structural preview limit",
        ));
    }
    Ok(blob.content().to_vec())
}

// Spool output to private temporary files so a child cannot fill an unbounded output buffer.
// All polling and filesystem work runs on the existing feature worker executor.
fn capture(command: &mut Command, deadline: Instant, limit: usize) -> std::io::Result<String> {
    let output = tempfile::tempfile()?;
    command
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn()?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match result {
                    Err(error) => error,
                    _ => {
                        std::io::Error::new(std::io::ErrorKind::TimedOut, "Diff renderer timed out")
                    }
                });
            }
        }
    };
    if !status.success() {
        return Err(std::io::Error::other("Diff renderer failed"));
    }
    use std::io::{Seek, SeekFrom};
    let mut output = output;
    output.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    output.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[Preview truncated at 512 KiB]\n");
    }
    Ok(text)
}

fn unified(diff: git2::Diff<'_>) -> Result<String, git2::Error> {
    if diff.deltas().len() == 0 {
        return Ok("No changes.\n".into());
    }
    let mut text = Vec::new();
    let mut truncated = false;
    diff.print(DiffFormat::Patch, |_, _, line| {
        if truncated || text.len() + line.content().len() + 1 > 512 * 1024 {
            truncated = true;
            return true;
        }
        if matches!(line.origin(), '+' | '-' | ' ') {
            text.push(line.origin() as u8);
        }
        text.extend_from_slice(line.content());
        true
    })?;
    let mut text = String::from_utf8_lossy(&text).into_owned();
    if truncated {
        text.push_str("\n[Preview truncated at 512 KiB]\n");
    }
    if text.is_empty() {
        text.push_str("No textual differences (file may be binary).\n");
    }
    Ok(text)
}
