use crate::diff_model::{DiffRow, RenderedDiff, RowKind};
use crate::diff_preview::DiffRenderer;
use git2::{DiffOptions, Oid, Repository, StatusOptions};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Change {
    pub path: PathBuf,
    pub status: String,
    pub staged: bool,
    pub unstaged: bool,
}
#[derive(Clone, Debug)]
pub struct Stash {
    pub index: usize,
    pub oid: Oid,
    pub message: String,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub changes: Vec<Change>,
    pub stashes: Vec<Stash>,
    pub diff: String,
    pub diff_rows: Vec<DiffRow>,
}
#[derive(Clone)]
pub enum Action {
    Refresh,
    Diff(PathBuf),
    Stage(PathBuf),
    Unstage(PathBuf),
    SaveStash,
    Apply(Oid),
    Pop(Oid),
    Drop(Oid),
    ShowStash(Oid),
}
impl Action {
    pub fn progress_message(&self) -> String {
        match self {
            Self::Refresh => "Reading repository changes and stashes…".into(),
            Self::Diff(path) => format!("Loading diff for {}…", path.display()),
            Self::Stage(path) => format!("Staging {}…", path.display()),
            Self::Unstage(path) => format!("Unstaging {}…", path.display()),
            Self::SaveStash => "Saving stash…".into(),
            Self::Apply(_) => "Applying stash…".into(),
            Self::Pop(_) => "Popping stash…".into(),
            Self::Drop(_) => "Deleting stash…".into(),
            Self::ShowStash(_) => "Loading stash changes…".into(),
        }
    }

    pub fn mutates(&self) -> bool {
        !matches!(self, Self::Refresh | Self::Diff(_) | Self::ShowStash(_))
    }
}

// Runs entirely on a worker; snapshots contain no UI or repository handles.
pub struct WorktreeHost {
    pub path: PathBuf,
    pub action: Action,
}
impl WorktreeHost {
    pub fn run(self) -> (Result<(), String>, Result<Snapshot, String>) {
        let renderer = if matches!(self.action, Action::Diff(_) | Action::ShowStash(_)) {
            DiffRenderer::detect()
        } else {
            DiffRenderer::default()
        };
        self.run_with_renderer(&renderer)
    }

    fn run_with_renderer(
        self,
        renderer: &DiffRenderer,
    ) -> (Result<(), String>, Result<Snapshot, String>) {
        let operation = match self.action {
            Action::Stage(_) => "stage",
            Action::Unstage(_) => "unstage",
            Action::SaveStash => "stash_save",
            Action::Apply(_) => "stash_apply",
            Action::Pop(_) => "stash_pop",
            Action::Drop(_) => "stash_drop",
            _ => "read",
        };
        let started = std::time::Instant::now();
        if self.action.mutates() {
            let _ = crate::operation_log::append(&format!("worktree operation={operation} START"));
        }
        #[cfg(feature = "test-rpc")]
        if self.action.mutates() {
            crate::test_rpc::worker_gate::wait("GIT_MASTER_TEST_WORKTREE_GATE");
        }
        let result = execute_document(&self.path, &self.action, renderer);
        if self.action.mutates() {
            let _ = crate::operation_log::append(&format!(
                "worktree operation={operation} END success={} elapsed_ms={}",
                result.is_ok(),
                started.elapsed().as_millis()
            ));
        }
        let snapshot = load(&self.path).map(|mut snapshot| {
            if let Ok(text) = &result {
                snapshot.diff = text.text.clone();
                snapshot.diff_rows = text.rows.clone();
            }
            snapshot
        });
        (result.map(|_| ()), snapshot)
    }
}
#[cfg(test)]
fn execute(path: &Path, action: &Action) -> Result<String, String> {
    execute_with_renderer(path, action, &DiffRenderer::default())
}
#[cfg(test)]
fn execute_with_renderer(
    path: &Path,
    action: &Action,
    renderer: &DiffRenderer,
) -> Result<String, String> {
    execute_document(path, action, renderer).map(|document| document.text)
}
fn execute_document(
    path: &Path,
    action: &Action,
    renderer: &DiffRenderer,
) -> Result<RenderedDiff, String> {
    let mut document = RenderedDiff::default();
    let mut repo = Repository::open(path).map_err(|e| e.to_string())?;
    let result = (|| -> Result<String, git2::Error> {
        match action {
            Action::Refresh => {}
            Action::Diff(path) => {
                let tree = head_tree(&repo)?;
                let mut opts = DiffOptions::new();
                opts.pathspec(path)
                    .disable_pathspec_match(true)
                    .include_untracked(true)
                    .recurse_untracked_dirs(true)
                    .show_untracked_content(true);
                let staged = repo.diff_tree_to_index(tree.as_ref(), None, Some(&mut opts))?;
                let work = repo.diff_index_to_workdir(None, Some(&mut opts))?;
                let staged = renderer.render(&repo, staged, false)?;
                let work = renderer.render(&repo, work, true)?;
                let text = format!("Staged\n{}\nWorking tree\n{}", staged.text, work.text);
                document.section("Staged", staged);
                document.section("Working tree", work);
                return Ok(text);
            }
            Action::Stage(path) => {
                let mut index = repo.index()?;
                let workdir = repo.workdir().ok_or_else(|| {
                    git2::Error::from_str("Bare repositories have no working tree")
                })?;
                match std::fs::symlink_metadata(workdir.join(path)) {
                    Ok(_) => index.add_path(path)?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        index.remove_path(path)?
                    }
                    Err(error) => return Err(git2::Error::from_str(&error.to_string())),
                }
                index.write()?;
            }
            Action::Unstage(path) => {
                let tree = head_tree(&repo)?;
                let mut options = StatusOptions::new();
                options.renames_head_to_index(true);
                let renamed_from = repo.statuses(Some(&mut options))?.iter().find_map(|entry| {
                    let delta = entry.head_to_index()?;
                    (entry.status().contains(git2::Status::INDEX_RENAMED)
                        && delta.new_file().path() == Some(path.as_path()))
                    .then(|| delta.old_file().path().map(Path::to_path_buf))
                    .flatten()
                });
                let mut index = repo.index()?;
                let mut original = git2::Index::new()?;
                if let Some(tree) = tree {
                    original.read_tree(&tree)?;
                }
                // A staged rename is one user action involving two index paths.
                // Restore both sides without changing either working-tree file.
                for path in std::iter::once(path.as_path()).chain(renamed_from.as_deref()) {
                    if let Some(entry) = original.get_path(path, 0) {
                        index.add(&entry)?;
                    } else {
                        index.remove_path(path)?;
                    }
                }
                index.write()?;
            }
            Action::SaveStash => {
                let signature = repo.signature()?;
                repo.stash_save2(&signature, None, Some(git2::StashFlags::INCLUDE_UNTRACKED))?;
            }
            Action::Apply(oid) | Action::Pop(oid) | Action::Drop(oid) => {
                let index = stash_index(&mut repo, *oid)?;
                match action {
                    Action::Apply(_) => repo.stash_apply(index, None)?,
                    Action::Pop(_) => repo.stash_pop(index, None)?,
                    Action::Drop(_) => repo.stash_drop(index)?,
                    _ => unreachable!(),
                }
            }
            Action::ShowStash(oid) => {
                let index = stash_index(&mut repo, *oid)?;
                let commit = repo.find_commit(*oid)?;
                let base_commit = commit.parent(0)?;
                let base = base_commit.tree()?;
                let staged = commit.parent(1)?.tree()?;
                let working = commit.tree()?;
                let author = commit.author();
                let time = commit.time();
                let offset = chrono::FixedOffset::east_opt(time.offset_minutes() * 60)
                    .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
                let date = chrono::DateTime::from_timestamp(time.seconds(), 0)
                    .map(|date| date.with_timezone(&offset).to_rfc3339())
                    .unwrap_or_else(|| "Unknown".into());
                let mut text = format!(
                    "stash@{{{index}}}\nCommit: {oid}\nAuthor: {}\nDate: {date}\nBase: {}\n\n{}\n",
                    author.name().unwrap_or("Unknown"),
                    base_commit.id(),
                    commit.message().unwrap_or("(No message)"),
                );
                document.rows.extend(
                    text.lines()
                        .filter(|line| !line.is_empty())
                        .map(|line| DiffRow::label(RowKind::Note, line)),
                );
                // The index parent preserves staged edits that a combined diff can hide.
                append_stash_section(
                    &repo,
                    renderer,
                    &mut document,
                    &mut text,
                    "Staged changes",
                    repo.diff_tree_to_tree(Some(&base), Some(&staged), None)?,
                )?;
                append_stash_section(
                    &repo,
                    renderer,
                    &mut document,
                    &mut text,
                    "Working tree changes",
                    repo.diff_tree_to_tree(Some(&staged), Some(&working), None)?,
                )?;
                if commit.parent_count() > 2 {
                    let untracked = commit.parent(2)?.tree()?;
                    append_stash_section(
                        &repo,
                        renderer,
                        &mut document,
                        &mut text,
                        "Untracked files",
                        repo.diff_tree_to_tree(None, Some(&untracked), None)?,
                    )?;
                }
                return Ok(text);
            }
        }
        Ok(String::new())
    })();
    result
        .map(|text| {
            document.text = text;
            document.limit();
            document
        })
        .map_err(|e| e.to_string())
}
fn stash_index(repo: &mut Repository, oid: Oid) -> Result<usize, git2::Error> {
    let mut found = None;
    repo.stash_foreach(|index, _, id| {
        if *id == oid {
            found = Some(index);
            false
        } else {
            true
        }
    })?;
    found.ok_or_else(|| git2::Error::from_str("Stash no longer exists; refresh and retry"))
}

fn append_stash_section(
    repo: &Repository,
    renderer: &DiffRenderer,
    document: &mut RenderedDiff,
    text: &mut String,
    title: &str,
    diff: git2::Diff<'_>,
) -> Result<(), git2::Error> {
    text.push_str(&format!("\n{title}\n"));
    if diff.deltas().len() == 0 {
        text.push_str("(none)\n");
        return Ok(());
    }
    let stats = diff.stats()?;
    text.push_str(&format!(
        "{} files changed, {} insertions(+), {} deletions(-)\n",
        stats.files_changed(),
        stats.insertions(),
        stats.deletions()
    ));
    // Cap the file summary as well as the patch so huge stashes remain readable.
    for delta in diff.deltas().take(200) {
        let path = delta.new_file().path().or_else(|| delta.old_file().path());
        text.push_str(&format!(
            "  {:?} {}\n",
            delta.status(),
            path.map(|p| p.display().to_string())
                .unwrap_or_else(|| "(unknown path)".into())
        ));
    }
    if diff.deltas().len() > 200 {
        text.push_str("  [File list limited to 200 entries]\n");
    }
    text.push('\n');
    let rendered = renderer.render(repo, diff, false)?;
    text.push_str(&rendered.text);
    document.section(title, rendered);
    Ok(())
}

fn head_tree(repo: &Repository) -> Result<Option<git2::Tree<'_>>, git2::Error> {
    match repo.head() {
        Ok(head) => Ok(Some(head.peel_to_tree()?)),
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}
pub fn load(path: &Path) -> Result<Snapshot, String> {
    let mut repo = Repository::open(path).map_err(|e| e.to_string())?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true);
    let changes = repo
        .statuses(Some(&mut opts))
        .map_err(|e| e.to_string())?
        .iter()
        .map(|entry| {
            let status = entry.status();
            let staged = status.intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE,
            );
            let unstaged = status.intersects(
                git2::Status::WT_NEW
                    | git2::Status::WT_MODIFIED
                    | git2::Status::WT_DELETED
                    | git2::Status::WT_RENAMED
                    | git2::Status::WT_TYPECHANGE
                    | git2::Status::CONFLICTED,
            );
            Ok(Change {
                path: {
                    let path = if status.contains(git2::Status::INDEX_RENAMED) {
                        entry
                            .head_to_index()
                            .and_then(|delta| delta.new_file().path().map(Path::to_path_buf))
                    } else {
                        Some(PathBuf::from(entry.path().map_err(|_| {
                            "A changed path is not valid UTF-8; use Git CLI for this repository"
                                .to_string()
                        })?))
                    }
                    .ok_or_else(|| "Changed file path is unavailable".to_string())?;
                    if path.to_str().is_none() {
                        return Err(
                            "A changed path is not valid UTF-8; use Git CLI for this repository"
                                .to_string(),
                        );
                    }
                    path
                },
                status: {
                    let mut labels = Vec::new();
                    if status.contains(git2::Status::CONFLICTED) {
                        labels.push("Conflicted");
                    }
                    for (flags, label) in [
                        (git2::Status::WT_NEW, "Untracked"),
                        (git2::Status::INDEX_NEW, "Added"),
                        (
                            git2::Status::INDEX_MODIFIED | git2::Status::WT_MODIFIED,
                            "Modified",
                        ),
                        (
                            git2::Status::INDEX_DELETED | git2::Status::WT_DELETED,
                            "Deleted",
                        ),
                        (
                            git2::Status::INDEX_RENAMED | git2::Status::WT_RENAMED,
                            "Renamed",
                        ),
                        (
                            git2::Status::INDEX_TYPECHANGE | git2::Status::WT_TYPECHANGE,
                            "Type changed",
                        ),
                    ] {
                        if status.intersects(flags) {
                            labels.push(label);
                        }
                    }
                    if staged {
                        labels.push("Staged");
                    }
                    if unstaged && !status.contains(git2::Status::WT_NEW) {
                        labels.push("Working tree");
                    }
                    labels.join(" · ")
                },
                staged,
                unstaged,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut stashes = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        stashes.push(Stash {
            index,
            message: message.into(),
            oid: *oid,
        });
        true
    })
    .map_err(|e| e.to_string())?;
    Ok(Snapshot {
        changes,
        stashes,
        diff: String::new(),
        diff_rows: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(commit: bool) -> Self {
            let path = std::env::temp_dir().join(format!(
                "git-master-worktree-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            let repo = Repository::init(&path).unwrap();
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "Git Master Tests").unwrap();
            config.set_bool("core.autocrlf", false).unwrap();
            config
                .set_str("user.email", "tests@example.invalid")
                .unwrap();
            if commit {
                std::fs::write(path.join("tracked.txt"), "base\n").unwrap();
                execute(&path, &Action::Stage("tracked.txt".into())).unwrap();
                let tree = repo.index().unwrap().write_tree().unwrap();
                let tree = repo.find_tree(tree).unwrap();
                let sig = repo.signature().unwrap();
                repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
                    .unwrap();
            }
            Self(path)
        }
        fn run(&self, action: Action) -> Snapshot {
            let (result, snapshot) = WorktreeHost {
                path: self.0.clone(),
                action,
            }
            .run_with_renderer(&DiffRenderer::default());
            result.unwrap();
            snapshot.unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn failed_difft_falls_back_to_the_original_diff() {
        let f = Fixture::new(true);
        std::fs::write(f.0.join("tracked.txt"), "modified\n").unwrap();
        let renderer = DiffRenderer::with_program(f.0.join("missing-difft"));
        let preview =
            execute_with_renderer(&f.0, &Action::Diff("tracked.txt".into()), &renderer).unwrap();
        assert!(preview.contains("difft could not render"));
        assert!(preview.contains("+modified"));
        let document =
            execute_document(&f.0, &Action::Diff("tracked.txt".into()), &renderer).unwrap();
        assert!(
            !document
                .rows
                .iter()
                .any(|row| row.kind == RowKind::Section && row.text == "Staged")
        );
        assert!(document.rows.iter().any(|row| row.kind == RowKind::Added
            && row.new_line == Some(1)
            && !row.highlights.is_empty()));
        assert!(
            document
                .rows
                .iter()
                .any(|row| row.kind == RowKind::Removed && row.old_line == Some(1))
        );

        assert!(preview.contains("Staged\nNo changes."));
    }

    #[test]
    fn unstage_rename_restores_both_index_paths_and_preserves_new_work() {
        let f = Fixture::new(true);
        std::fs::rename(f.0.join("tracked.txt"), f.0.join("renamed.txt")).unwrap();
        f.run(Action::Stage("tracked.txt".into()));
        f.run(Action::Stage("renamed.txt".into()));
        let snapshot = load(&f.0).unwrap();
        assert_eq!(snapshot.changes.len(), 1);
        assert_eq!(snapshot.changes[0].path, PathBuf::from("renamed.txt"));
        assert!(snapshot.changes[0].status.contains("Renamed"));
        std::fs::write(f.0.join("renamed.txt"), "newer working edit\n").unwrap();
        f.run(Action::Unstage("renamed.txt".into()));
        let repo = Repository::open(&f.0).unwrap();
        let tree = repo.head().unwrap().peel_to_tree().unwrap();
        assert_eq!(
            repo.diff_tree_to_index(Some(&tree), None, None)
                .unwrap()
                .deltas()
                .len(),
            0
        );
        assert!(!f.0.join("tracked.txt").exists());
        assert_eq!(
            std::fs::read_to_string(f.0.join("renamed.txt")).unwrap(),
            "newer working edit\n"
        );
    }

    #[test]
    fn installed_difft_renders_index_worktree_and_stash_contents() {
        let renderer = DiffRenderer::detect();
        // This integration case runs when difft is available; the remaining tests
        // always verify deterministic built-in and failure fallback behavior.
        if !renderer.is_available() {
            return;
        }
        let f = Fixture::new(true);
        std::fs::write(f.0.join("tracked.txt"), "staged version\n").unwrap();
        f.run(Action::Stage("tracked.txt".into()));
        std::fs::write(f.0.join("tracked.txt"), "working version\n").unwrap();
        let preview =
            execute_with_renderer(&f.0, &Action::Diff("tracked.txt".into()), &renderer).unwrap();
        assert_eq!(
            preview.matches("Diff renderer: difft").count(),
            2,
            "{preview}"
        );
        assert!(preview.contains("staged version") && preview.contains("working version"));
        assert!(!preview.contains('\u{1b}'));
        std::fs::write(
            f.0.join("new[1].rs"),
            "fn main() { println!(\"new file\"); }\n",
        )
        .unwrap();
        let preview =
            execute_with_renderer(&f.0, &Action::Diff("new[1].rs".into()), &renderer).unwrap();
        assert!(
            preview.contains("Diff renderer: difft") && preview.contains("new file"),
            "{preview}"
        );
        let id = f.run(Action::SaveStash).stashes[0].oid;
        let preview = execute_with_renderer(&f.0, &Action::ShowStash(id), &renderer).unwrap();
        assert_eq!(
            preview.matches("Diff renderer: difft").count(),
            3,
            "{preview}"
        );
        assert!(
            preview.contains("new file")
                && preview.contains("staged version")
                && preview.contains("working version")
        );
        std::fs::remove_file(f.0.join("tracked.txt")).unwrap();
        let preview =
            execute_with_renderer(&f.0, &Action::Diff("tracked.txt".into()), &renderer).unwrap();
        assert!(preview.contains("Diff renderer: difft"), "{preview}");
        assert!(preview.contains("base"));
    }

    #[test]
    fn staged_and_worktree_diffs_and_literal_paths_are_separate() {
        let f = Fixture::new(true);
        std::fs::write(f.0.join("tracked.txt"), "staged\n").unwrap();
        f.run(Action::Stage("tracked.txt".into()));
        std::fs::write(f.0.join("tracked.txt"), "working\n").unwrap();
        let snapshot = f.run(Action::Diff("tracked.txt".into()));
        assert!(snapshot.changes[0].staged && snapshot.changes[0].unstaged);
        assert!(snapshot.diff.contains("+staged") && snapshot.diff.contains("+working"));
        let snapshot = f.run(Action::Unstage("tracked.txt".into()));
        assert!(!snapshot.changes[0].staged);
        assert_eq!(
            std::fs::read_to_string(f.0.join("tracked.txt")).unwrap(),
            "working\n"
        );
        for file in ["file[1].txt", "file1.txt"] {
            std::fs::write(f.0.join(file), "new\n").unwrap();
        }
        let snapshot = f.run(Action::Stage("file[1].txt".into()));
        assert!(
            snapshot
                .changes
                .iter()
                .find(|c| c.path == Path::new("file[1].txt"))
                .unwrap()
                .staged
        );
        assert!(
            !snapshot
                .changes
                .iter()
                .find(|c| c.path == Path::new("file1.txt"))
                .unwrap()
                .staged
        );
        let snapshot = f.run(Action::Diff("file[1].txt".into()));
        assert!(!snapshot.diff.contains("b/file1.txt"));
        f.run(Action::Unstage("file[1].txt".into()));
    }
    #[test]
    fn diff_distinguishes_empty_sections_from_binary_changes() {
        let f = Fixture::new(true);
        std::fs::write(f.0.join("tracked.txt"), "modified\n").unwrap();
        let preview = f.run(Action::Diff("tracked.txt".into())).diff;
        assert!(preview.starts_with("Staged\nNo changes.\n"));
        f.run(Action::Stage("tracked.txt".into()));
        let preview = f.run(Action::Diff("tracked.txt".into())).diff;
        assert!(preview.ends_with("Working tree\nNo changes.\n"));
        std::fs::write(f.0.join("binary.dat"), [0, 1, 2, 0]).unwrap();
        let preview = f.run(Action::Diff("binary.dat".into())).diff;
        let working = preview.split_once("Working tree\n").unwrap().1;
        assert!(working.contains("binary.dat"));
        assert!(!working.contains("No changes."));
    }

    #[test]
    fn stages_deletions_and_unstages_before_first_commit() {
        let f = Fixture::new(true);
        std::fs::remove_file(f.0.join("tracked.txt")).unwrap();
        let snapshot = f.run(Action::Stage("tracked.txt".into()));
        assert!(snapshot.changes[0].staged);
        let snapshot = f.run(Action::Unstage("tracked.txt".into()));
        assert!(!snapshot.changes[0].staged && snapshot.changes[0].unstaged);
        let empty = Fixture::new(false);
        std::fs::write(empty.0.join("new.txt"), "new\n").unwrap();
        assert!(
            empty
                .run(Action::Diff("new.txt".into()))
                .diff
                .contains("+new")
        );
        empty.run(Action::Stage("new.txt".into()));
        assert!(!empty.run(Action::Unstage("new.txt".into())).changes[0].staged);
    }
    #[test]
    fn stash_roundtrip_uses_identity_and_includes_untracked_files() {
        let f = Fixture::new(true);
        std::fs::write(f.0.join("tracked.txt"), "saved\n").unwrap();
        std::fs::write(f.0.join("untracked.txt"), "untracked\n").unwrap();
        let snapshot = f.run(Action::SaveStash);
        assert!(snapshot.changes.is_empty());
        let id = snapshot.stashes[0].oid;
        let diff = f.run(Action::ShowStash(id)).diff;
        assert!(diff.contains("+saved") && diff.contains("+untracked"));
        assert_eq!(f.run(Action::Apply(id)).stashes.len(), 1);
        assert_eq!(
            std::fs::read_to_string(f.0.join("untracked.txt")).unwrap(),
            "untracked\n"
        );
        // Push another stash, then target the original by object identity.
        std::fs::write(f.0.join("another.txt"), "another\n").unwrap();
        let snapshot = f.run(Action::SaveStash);
        assert_eq!(snapshot.stashes.len(), 2);
        let new_id = snapshot.stashes[0].oid;
        let snapshot = f.run(Action::Drop(id));
        assert_eq!(snapshot.stashes.len(), 1);
        assert_eq!(snapshot.stashes[0].oid, new_id);
        assert!(f.run(Action::Pop(new_id)).stashes.is_empty());
        assert!(execute(&f.0, &Action::Apply(id)).is_err());
    }
    #[test]
    fn stash_preview_preserves_staged_changes_hidden_by_a_combined_diff() {
        let f = Fixture::new(true);
        std::fs::write(f.0.join("tracked.txt"), "staged-only-version\n").unwrap();
        f.run(Action::Stage("tracked.txt".into()));
        std::fs::write(f.0.join("tracked.txt"), "base\n").unwrap();
        std::fs::write(f.0.join("binary.dat"), [0, 1, 2, 0]).unwrap();
        let saved = f.run(Action::SaveStash);
        let before: Vec<_> = saved
            .changes
            .iter()
            .map(|change| (change.path.clone(), change.status.clone()))
            .collect();
        let id = saved.stashes[0].oid;
        let preview = f.run(Action::ShowStash(id)).diff;
        assert!(preview.contains(&format!("Commit: {id}")));
        assert!(preview.contains("Author: Git Master Tests"));
        assert!(preview.contains("Date:") && preview.contains("Base:"));
        let (_, staged) = preview.split_once("Staged changes\n").unwrap();
        let (staged, working) = staged.split_once("Working tree changes\n").unwrap();
        assert!(staged.contains("+staged-only-version"));
        assert!(working.contains("-staged-only-version") && working.contains("+base"));
        let untracked = preview.split_once("Untracked files\n").unwrap().1;
        assert!(untracked.contains("binary.dat") && untracked.contains("1 files changed"));
        let after: Vec<_> = f
            .run(Action::Refresh)
            .changes
            .iter()
            .map(|change| (change.path.clone(), change.status.clone()))
            .collect();
        assert_eq!(before, after);
        f.run(Action::Drop(id));
        assert!(execute(&f.0, &Action::ShowStash(id)).is_err());
    }

    #[test]
    fn failed_pop_preserves_stash_and_local_changes() {
        let f = Fixture::new(true);
        std::fs::write(f.0.join("tracked.txt"), "stashed\n").unwrap();
        let id = f.run(Action::SaveStash).stashes[0].oid;
        std::fs::write(f.0.join("tracked.txt"), "local\n").unwrap();
        assert!(execute(&f.0, &Action::Pop(id)).is_err());
        assert_eq!(load(&f.0).unwrap().stashes[0].oid, id);
        assert_eq!(
            std::fs::read_to_string(f.0.join("tracked.txt")).unwrap(),
            "local\n"
        );
    }
}
