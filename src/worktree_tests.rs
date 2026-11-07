use super::*;
struct Fixture(PathBuf);
impl Fixture {
    fn new(commit: bool) -> Self {
        let path = tempfile::Builder::new()
            .prefix("git-master-worktree-")
            .tempdir()
            .unwrap()
            .keep();
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
    let document = execute_document(&f.0, &Action::Diff("tracked.txt".into()), &renderer).unwrap();
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
fn mixed_index_and_worktree_statuses_keep_each_partitions_git_meaning() {
    let f = Fixture::new(true);
    let repo = Repository::open(&f.0).unwrap();
    std::fs::rename(f.0.join("tracked.txt"), f.0.join("renamed.txt")).unwrap();
    f.run(Action::Stage("tracked.txt".into()));
    f.run(Action::Stage("renamed.txt".into()));
    std::fs::write(f.0.join("renamed.txt"), "working modification\n").unwrap();
    std::fs::write(f.0.join("added.txt"), "index addition\n").unwrap();
    f.run(Action::Stage("added.txt".into()));
    std::fs::write(f.0.join("added.txt"), "working edit of addition\n").unwrap();
    let head = repo.head().unwrap().target();
    let index = repo.index().unwrap().write_tree().unwrap();
    let snapshot = load(&f.0).unwrap();
    for (path, expected_index) in [
        ("renamed.txt", ChangeKind::Renamed),
        ("added.txt", ChangeKind::Added),
    ] {
        let change = snapshot
            .changes
            .iter()
            .find(|c| c.path == Path::new(path))
            .unwrap();
        assert_eq!(change.index_kind, Some(expected_index), "{path}");
        assert_eq!(change.worktree_kind, Some(ChangeKind::Modified), "{path}");
        assert!(change.staged && change.unstaged);
    }
    assert_eq!(repo.head().unwrap().target(), head);
    assert_eq!(repo.index().unwrap().write_tree().unwrap(), index);
    assert_eq!(
        std::fs::read_to_string(f.0.join("renamed.txt")).unwrap(),
        "working modification\n"
    );
    assert_eq!(
        std::fs::read_to_string(f.0.join("added.txt")).unwrap(),
        "working edit of addition\n"
    );
}

#[test]
fn rename_preview_keeps_both_paths_edits_and_literal_selection() {
    let f = Fixture::new(true);
    let base = (0..20)
        .map(|n| format!("unchanged line {n}\n"))
        .collect::<String>();
    std::fs::write(f.0.join("tracked.txt"), &base).unwrap();
    f.run(Action::Stage("tracked.txt".into()));
    let repo = Repository::open(&f.0).unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let parent = repo.head().unwrap().peel_to_commit().unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "multiline", &tree, &[&parent])
        .unwrap();
    std::fs::rename(f.0.join("tracked.txt"), f.0.join("renamed[1].txt")).unwrap();
    f.run(Action::Stage("tracked.txt".into()));
    f.run(Action::Stage("renamed[1].txt".into()));
    for edited in [false, true] {
        let staged = if edited {
            base.replace("unchanged line 10", "staged rename edit")
        } else {
            base.clone()
        };
        std::fs::write(f.0.join("renamed[1].txt"), &staged).unwrap();
        f.run(Action::Stage("renamed[1].txt".into()));
        std::fs::write(
            f.0.join("renamed[1].txt"),
            staged.replace("unchanged line 11", "working rename edit"),
        )
        .unwrap();
        std::fs::write(f.0.join("renamed1.txt"), "unrelated sentinel\n").unwrap();
        for (mode, renderer) in [
            ("git", DiffRenderer::default()),
            (
                "fallback",
                DiffRenderer::with_program(f.0.join("missing-difft")),
            ),
            ("installed", DiffRenderer::detect()),
        ] {
            for path in ["tracked.txt", "renamed[1].txt"] {
                let doc = execute_document(&f.0, &Action::Diff(path.into()), &renderer).unwrap();
                let (index, work) = doc.text.split_once("Working tree\n").unwrap();
                let expected_renders = if path == "tracked.txt" { 1 } else { 2 };
                if mode == "fallback" {
                    assert_eq!(
                        doc.text.matches("difft could not render").count(),
                        expected_renders
                    );
                } else if renderer.is_available() {
                    assert_eq!(
                        doc.text.matches("Diff renderer: difft").count(),
                        expected_renders,
                        "installed difft must not silently fall back: {}",
                        doc.text
                    );
                }
                let work_start = doc
                    .rows
                    .iter()
                    .position(|row| row.kind == RowKind::Section && row.text == "Working tree");
                let index_rows = &doc.rows[1..work_start.unwrap_or(doc.rows.len())];
                let work_rows = work_start
                    .map(|start| &doc.rows[start + 1..])
                    .unwrap_or(&[]);
                assert!(
                    !index_rows
                        .iter()
                        .any(|row| row.text == "working rename edit")
                );
                if !edited {
                    assert!(
                        !index_rows
                            .iter()
                            .any(|row| matches!(row.kind, RowKind::Added | RowKind::Removed))
                    );
                }
                if path == "renamed[1].txt" {
                    assert!(work_rows.iter().any(|row| row.kind == RowKind::Removed
                        && row.text == "unchanged line 11"
                        && row.old_line == Some(12)));
                    assert!(work_rows.iter().any(|row| row.kind == RowKind::Added
                        && row.text == "working rename edit"
                        && row.new_line == Some(12)));
                } else {
                    assert!(work_rows.is_empty());
                }
                assert!(
                    index.contains("tracked.txt") && index.contains("renamed[1].txt"),
                    "{}",
                    doc.text
                );
                assert!(!doc.text.contains("unrelated sentinel"));
                assert!(!doc.text.contains("renamed1.txt"));
                assert!(
                    !doc.rows
                        .iter()
                        .any(|r| r.kind == RowKind::Added && r.text == "unchanged line 0")
                );
                if edited {
                    assert!(index_rows.iter().any(|r| r.kind == RowKind::Removed
                        && r.text == "unchanged line 10"
                        && r.old_line == Some(11)));
                    assert!(index_rows.iter().any(|r| r.kind == RowKind::Added
                        && r.text == "staged rename edit"
                        && r.new_line == Some(11)));
                }
                if path == "renamed[1].txt" {
                    assert!(work.contains("working rename edit"));
                }
            }
        }
    }
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
fn conflicted_fixture() -> Fixture {
    let f = Fixture::new(true);
    let branch = Repository::open(&f.0)
        .unwrap()
        .head()
        .unwrap()
        .shorthand()
        .unwrap()
        .to_owned();
    crate::git_ops::run_git(&f.0, ["checkout", "-b", "topic"]).unwrap();
    std::fs::write(f.0.join("tracked.txt"), "topic version\n").unwrap();
    crate::git_ops::run_git(&f.0, ["commit", "-am", "topic"]).unwrap();
    crate::git_ops::run_git(&f.0, ["checkout", &branch]).unwrap();
    std::fs::write(f.0.join("tracked.txt"), "main version\n").unwrap();
    crate::git_ops::run_git(&f.0, ["commit", "-am", "main"]).unwrap();
    assert!(crate::git_ops::run_git(&f.0, ["merge", "topic"]).is_err());
    f
}

#[test]
fn conflict_preview_reads_current_file_without_resolving_index() {
    let f = conflicted_fixture();
    let before = std::fs::read(f.0.join("tracked.txt")).unwrap();
    let snapshot = f.run(Action::Diff("tracked.txt".into()));
    assert!(snapshot.diff.contains("<<<<<<<"));
    assert!(snapshot.diff.contains("topic version"));
    assert!(snapshot.diff.contains("main version"));
    let current = &snapshot.sections["当前内容（索引 stage 2）"];
    let incoming = &snapshot.sections["传入内容（索引 stage 3）"];
    assert!(
        current
            .iter()
            .any(|r| r.text == "main version" && r.kind == RowKind::Removed)
    );
    assert!(
        incoming
            .iter()
            .any(|r| r.text == "topic version" && r.kind == RowKind::Added)
    );
    assert!(!current.iter().any(|r| r.text == "topic version"));
    assert!(!incoming.iter().any(|r| r.text == "main version"));

    assert!(snapshot.diff_rows.iter().any(|r| r.new_line == Some(1)));
    assert!(
        Repository::open(&f.0)
            .unwrap()
            .index()
            .unwrap()
            .has_conflicts()
    );
    assert_eq!(std::fs::read(f.0.join("tracked.txt")).unwrap(), before);
}

#[test]
fn deleted_conflict_file_keeps_both_index_sides_visible_without_writes() {
    let f = conflicted_fixture();
    std::fs::remove_file(f.0.join("tracked.txt")).unwrap();
    let repo = Repository::open(&f.0).unwrap();
    let index_before = std::fs::read(repo.path().join("index")).unwrap();

    let snapshot = f.run(Action::Diff("tracked.txt".into()));

    assert!(snapshot.preview_error.is_none());
    assert!(
        snapshot.sections["冲突文件 · 当前工作区内容（只读）"]
            .iter()
            .any(|r| r.kind == RowKind::Note && r.text.contains("工作区文件已删除"))
    );
    for (section, expected, kind) in [
        ("当前内容（索引 stage 2）", "main version", RowKind::Removed),
        ("传入内容（索引 stage 3）", "topic version", RowKind::Added),
    ] {
        assert!(
            snapshot.sections[section]
                .iter()
                .any(|r| r.kind == kind && r.text == expected)
        );
    }
    assert!(!f.0.join("tracked.txt").exists());
    assert!(repo.index().unwrap().has_conflicts());
    assert_eq!(
        std::fs::read(repo.path().join("index")).unwrap(),
        index_before
    );
}

#[test]
fn pop_merge_conflict_retains_record_and_reports_failure() {
    let f = Fixture::new(true);
    std::fs::write(f.0.join("tracked.txt"), "saved side\n").unwrap();
    let saved = f.run(Action::SaveStash).stashes[0].oid;
    std::fs::write(f.0.join("tracked.txt"), "conflicting side\n").unwrap();
    execute(&f.0, &Action::Stage("tracked.txt".into())).unwrap();
    crate::git_ops::run_git(&f.0, ["commit", "-m", "conflicting committed side"]).unwrap();
    let (result, snapshot) = WorktreeHost {
        path: f.0.clone(),
        action: Action::Pop(saved),
    }
    .run_with_renderer(&DiffRenderer::default());
    assert!(
        result.is_err(),
        "unmerged index must never report a successful Pop"
    );
    let snapshot = snapshot.unwrap();
    assert!(
        snapshot.stashes.iter().any(|s| s.oid == saved),
        "conflict must keep the original record identity"
    );
    assert!(
        snapshot
            .changes
            .iter()
            .any(|c| c.status.contains("Conflicted"))
    );
    assert!(
        Repository::open(&f.0)
            .unwrap()
            .index()
            .unwrap()
            .has_conflicts()
    );
}
