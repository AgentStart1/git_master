use super::*;

#[test]
fn worktree_snapshot_exposes_actions_and_hides_other_repository_data() {
    let app = GitMasterApp::new();
    let mut snapshot = app.test_view_tree_snapshot();
    let path = std::path::PathBuf::from("fixture");
    snapshot.detail = Some(RepoDetail {
        comparison_refs: vec![],
        upstream: None,
        head_oid: None,
        git_state: "正常".into(),
        path: "fixture".into(),
        current_branch: "main".into(),
        branches: vec![],
        remotes: vec![],
        head_labels: Default::default(),
        file_status: Default::default(),
    });
    snapshot.worktree = Some((
        path,
        Ok(crate::worktree::Snapshot {
            change_lines: None,
            changes: vec![crate::worktree::Change {
                path: "file.txt".into(),
                status: "Modified".into(),
                index_kind: Some(crate::worktree::ChangeKind::Modified),
                worktree_kind: Some(crate::worktree::ChangeKind::Modified),
                staged: true,
                unstaged: true,
            }],
            stashes: vec![crate::worktree::Stash {
                branch: "main".into(),
                index: 0,
                oid: git2::Oid::ZERO_SHA1,
                message: "Saved edits".into(),
                author: "Fixture".into(),
                date: String::new(),
            }],
            diff: "+preview".into(),
            sections: Default::default(),
            preview_files: Vec::new(),
            preview_error: None,
            diff_rows: vec![crate::diff_model::DiffRow::label(
                crate::diff_model::RowKind::Note,
                "+preview",
            )],
        }),
    ));
    snapshot.worktree_preview = crate::preview_state::PreviewState::Ready {
        request: crate::preview_state::PreviewRequest {
            repository: "fixture".into(),
            target: crate::preview_state::PreviewTarget::File("file.txt".into()),
            revision: 1,
        },
        snapshot: snapshot
            .worktree
            .as_ref()
            .unwrap()
            .1
            .as_ref()
            .unwrap()
            .clone(),
    };
    let find = |node: &ViewNode, id: &str| {
        node.children
            .iter()
            .find(|child| child.id.as_deref() == Some(id))
            .cloned()
    };
    let tree = snapshot.build_worktree_node(false, &HashMap::new());
    assert!(find(&tree, "change-stage-0").unwrap().interactive);
    assert!(find(&tree, "change-unstage-0").unwrap().interactive);
    assert_eq!(
        find(&tree, "worktree-diff").unwrap().text.as_deref(),
        Some("+preview")
    );
    let stash_tree = snapshot.build_worktree_node(true, &HashMap::new());
    for action in ["select", "show", "apply", "pop", "drop"] {
        assert!(
            find(
                &stash_tree,
                &format!("stash-{action}-{}", git2::Oid::ZERO_SHA1)
            )
            .unwrap()
            .interactive
        );
    }
    snapshot.busy = true;
    let tree = snapshot.build_worktree_node(false, &HashMap::new());
    assert!(!find(&tree, "change-stage-0").unwrap().interactive);
    snapshot.detail.as_mut().unwrap().path = "other-repo".into();
    assert!(
        find(
            &snapshot.build_worktree_node(false, &HashMap::new()),
            "worktree-diff"
        )
        .is_none()
    );
}

#[test]
fn snapshot_builds_a_view_tree_without_accessing_live_ui_state() {
    let app = GitMasterApp::new();

    let tree = app.test_view_tree_snapshot().build();

    assert_eq!(tree.node_type, "window");
    assert_eq!(tree.id.as_deref(), Some("root"));
    assert!(!tree.children.is_empty());
}
