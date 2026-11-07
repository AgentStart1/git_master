use crate::diff_model::{DiffRow, RenderedDiff, RowKind};
use crate::diff_preview::DiffRenderer;
use git2::{DiffOptions, Oid, Repository, StatusOptions};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Change {
    pub path: PathBuf,
    pub status: String,
    pub index_kind: Option<ChangeKind>,
    pub worktree_kind: Option<ChangeKind>,
    pub staged: bool,
    pub unstaged: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
    Conflicted,
    Untracked,
}
impl ChangeKind {
    pub fn badge(self) -> &'static str {
        match self {
            Self::Added | Self::Untracked => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::TypeChanged => "T",
            Self::Conflicted => "!",
        }
    }

    fn for_partition(status: git2::Status, index: bool) -> Option<Self> {
        use git2::Status as S;
        let kinds = if index {
            [
                (S::INDEX_NEW, Self::Added),
                (S::INDEX_MODIFIED, Self::Modified),
                (S::INDEX_DELETED, Self::Deleted),
                (S::INDEX_RENAMED, Self::Renamed),
                (S::INDEX_TYPECHANGE, Self::TypeChanged),
            ]
        } else {
            if status.contains(S::CONFLICTED) {
                return Some(Self::Conflicted);
            }
            [
                (S::WT_NEW, Self::Untracked),
                (S::WT_MODIFIED, Self::Modified),
                (S::WT_DELETED, Self::Deleted),
                (S::WT_RENAMED, Self::Renamed),
                (S::WT_TYPECHANGE, Self::TypeChanged),
            ]
        };
        kinds
            .into_iter()
            .find_map(|(flag, kind)| status.contains(flag).then_some(kind))
    }
}
#[derive(Clone, Debug)]
pub struct Stash {
    pub index: usize,
    pub oid: Oid,
    pub message: String,
    pub author: String,
    pub branch: String,
    pub date: String,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub change_lines: Option<(usize, usize)>,
    pub changes: Vec<Change>,
    pub stashes: Vec<Stash>,
    pub diff: String,
    pub diff_rows: Vec<DiffRow>,
    pub sections: std::collections::BTreeMap<String, Vec<DiffRow>>,
    pub preview_files: Vec<(String, usize, usize)>,
    pub preview_error: Option<String>,
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
        #[cfg(feature = "test-rpc")]
        if !self.action.mutates() {
            crate::test_rpc::worker_gate::wait("GIT_MASTER_TEST_WORKTREE_READ_GATE");
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
            snapshot.preview_error = result.as_ref().err().cloned();
            if let Ok(text) = &result {
                snapshot.diff = text.text.clone();
                snapshot.diff_rows = text.rows.clone();
                let mut section = String::new();
                for row in &text.rows {
                    if row.kind == RowKind::Section {
                        section = row.text.clone();
                    }
                    snapshot
                        .sections
                        .entry(section.clone())
                        .or_default()
                        .push(row.clone());
                    if row.kind == RowKind::File {
                        snapshot.preview_files.push((row.text.clone(), 0, 0));
                    }
                    if let Some((_, added, removed)) = snapshot.preview_files.last_mut() {
                        *added += usize::from(row.kind == RowKind::Added);
                        *removed += usize::from(row.kind == RowKind::Removed);
                    }
                }
            }
            let mut totals = std::collections::BTreeMap::<String, (usize, usize)>::new();
            for (path, added, removed) in std::mem::take(&mut snapshot.preview_files) {
                let value = totals.entry(path).or_default();
                value.0 += added;
                value.1 += removed;
            }
            snapshot.preview_files = totals
                .into_iter()
                .map(|(path, (added, removed))| (path, added, removed))
                .collect();
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
                if repo
                    .status_file(path)
                    .is_ok_and(|status| status.contains(git2::Status::CONFLICTED))
                {
                    use std::io::Read;
                    let file = repo
                        .workdir()
                        .ok_or_else(|| git2::Error::from_str("No working directory"))?
                        .join(path);
                    let metadata = match std::fs::symlink_metadata(&file) {
                        Ok(metadata) => Some(metadata),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                        Err(error) => return Err(git2::Error::from_str(&error.to_string())),
                    };
                    let mut bytes = Vec::new();
                    if metadata
                        .as_ref()
                        .is_some_and(|m| m.file_type().is_symlink())
                    {
                        bytes = std::fs::read_link(&file)
                            .map_err(|error| git2::Error::from_str(&error.to_string()))?
                            .to_string_lossy()
                            .as_bytes()
                            .to_vec();
                    } else if metadata.is_some() {
                        std::fs::File::open(&file)
                            .and_then(|file| file.take(256 * 1024).read_to_end(&mut bytes))
                            .map_err(|error| git2::Error::from_str(&error.to_string()))?;
                    }
                    document.rows.push(DiffRow::label(
                        RowKind::Section,
                        "冲突文件 · 当前工作区内容（只读）",
                    ));
                    let deletion_notice = "工作区文件已删除；索引中仍有冲突记录。";
                    if metadata.is_none() {
                        document
                            .rows
                            .push(DiffRow::label(RowKind::Note, deletion_notice));
                    }
                    if bytes.contains(&0) {
                        document.rows.push(DiffRow::label(
                            RowKind::Note,
                            "二进制冲突，请在外部工具中查看。",
                        ));
                        return Ok("Binary conflict preview".into());
                    }
                    let content = String::from_utf8_lossy(&bytes);
                    let mut region = RowKind::Context;
                    for (index, line) in content.lines().take(600).enumerate() {
                        if line.starts_with("<<<<<<<") {
                            region = RowKind::Removed;
                        } else if line.starts_with("=======") {
                            region = RowKind::Added;
                        }
                        let kind = region;
                        if line.starts_with(">>>>>>>") {
                            region = RowKind::Context;
                        }
                        document.rows.push(DiffRow {
                            kind,
                            old_line: None,
                            new_line: Some(index + 1),
                            text: line.into(),
                            highlights: Vec::new(),
                        });
                    }
                    if metadata.as_ref().is_some_and(|m| m.len() > 256 * 1024)
                        || content.lines().count() > 600
                    {
                        document.rows.push(DiffRow::label(
                            RowKind::Note,
                            "预览已截断，请在外部打开完整文件。",
                        ));
                    }
                    let index = repo.index()?;
                    for (stage, title, kind) in [
                        (2, "当前内容（索引 stage 2）", RowKind::Removed),
                        (3, "传入内容（索引 stage 3）", RowKind::Added),
                    ] {
                        document.rows.push(DiffRow::label(RowKind::Section, title));
                        if let Some(entry) = index.get_path(path, stage) {
                            let blob = repo.find_blob(entry.id)?;
                            let bytes = blob.content();
                            if bytes.contains(&0) {
                                document.rows.push(DiffRow::label(
                                    RowKind::Note,
                                    "二进制内容，请在外部工具中查看。",
                                ));
                            } else {
                                for (line, text) in
                                    String::from_utf8_lossy(&bytes[..bytes.len().min(128 * 1024)])
                                        .lines()
                                        .take(600)
                                        .enumerate()
                                {
                                    document.rows.push(DiffRow {
                                        kind,
                                        old_line: if stage == 2 { Some(line + 1) } else { None },
                                        new_line: if stage == 3 { Some(line + 1) } else { None },
                                        text: text.into(),
                                        highlights: vec![],
                                    });
                                }
                                if bytes.len() > 128 * 1024
                                    || bytes.split(|b| *b == b'\n').count() > 600
                                {
                                    document.rows.push(DiffRow::label(
                                        RowKind::Note,
                                        "此侧预览已截断，请在外部查看完整内容。",
                                    ));
                                }
                            }
                        } else {
                            document.rows.push(DiffRow::label(
                                RowKind::Note,
                                "此侧无索引条目（可能为删除冲突）。",
                            ));
                        }
                    }
                    document.limit();
                    return Ok(format!(
                        "Conflict preview (read-only)\n{}",
                        if metadata.is_none() {
                            deletion_notice
                        } else {
                            &content
                        }
                    ));
                }
                let tree = head_tree(&repo)?;
                // Detect across the complete index diff before selecting either side of a rename.
                let mut staged = repo.diff_tree_to_index(tree.as_ref(), None, None)?;
                staged.find_similar(Some(git2::DiffFindOptions::new().renames(true)))?;
                // Worktree entries are separate paths; keep content reads scoped to the selection.
                let mut opts = DiffOptions::new();
                opts.pathspec(path)
                    .disable_pathspec_match(true)
                    .include_untracked(true)
                    .recurse_untracked_dirs(true)
                    .show_untracked_content(true);
                let work = repo.diff_index_to_workdir(None, Some(&mut opts))?;
                let staged = renderer.render_selected(&repo, staged, false, Some(path))?;
                let work = renderer.render_selected(&repo, work, true, Some(path))?;
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
                    Action::Apply(_) | Action::Pop(_) => {
                        // libgit2 may return Ok after creating unmerged index entries.
                        // Keep the record until the actual resulting index is conflict-free.
                        repo.stash_apply(index, None)?;
                        if repo.index()?.has_conflicts() {
                            return Err(git2::Error::from_str(
                                "Stash application created conflicts; the original stash is retained. Some contents may already be applied.",
                            ));
                        }
                        if matches!(action, Action::Pop(_)) {
                            let current = stash_index(&mut repo, *oid)?;
                            repo.stash_drop(current)?;
                        }
                    }
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
                index_kind: ChangeKind::for_partition(status, true),
                worktree_kind: ChangeKind::for_partition(status, false),
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
            author: String::new(),
            branch: message
                .strip_prefix("On ")
                .or_else(|| message.strip_prefix("WIP on "))
                .and_then(|m| m.split_once(": "))
                .map(|(b, _)| b.to_owned())
                .unwrap_or_default(),
            date: String::new(),
        });
        true
    })
    .map_err(|e| e.to_string())?;
    for stash in &mut stashes {
        if let Ok(commit) = repo.find_commit(stash.oid) {
            if let Ok(message) = commit.message() {
                if let Some((_, title)) = message
                    .trim()
                    .strip_prefix("On ")
                    .and_then(|m| m.split_once(": "))
                {
                    stash.message = title.to_owned();
                }
            }
            stash.author = commit.author().name().unwrap_or("未知作者").to_owned();
            stash.date = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default();
        }
    }
    let change_lines = (|| {
        let tree = head_tree(&repo).ok()?;
        let staged = repo
            .diff_tree_to_index(tree.as_ref(), None, None)
            .ok()?
            .stats()
            .ok()?;
        let mut options = DiffOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .show_untracked_content(true);
        let working = repo
            .diff_index_to_workdir(None, Some(&mut options))
            .ok()?
            .stats()
            .ok()?;
        Some((
            staged.insertions() + working.insertions(),
            staged.deletions() + working.deletions(),
        ))
    })();
    Ok(Snapshot {
        change_lines,
        changes,
        stashes,
        diff: String::new(),
        diff_rows: vec![],
        sections: Default::default(),
        preview_files: Vec::new(),
        preview_error: None,
    })
}

#[cfg(test)]
#[path = "worktree_tests.rs"]
mod tests;
