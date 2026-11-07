use super::*;
use crate::models::SubmoduleInfo;

#[::core::prelude::v1::test]
fn workspace_poll_waits_for_mutations_reads_and_user_interactions() {
    let blockers: &[fn(&mut GitMasterApp)] = &[
        |app| app.busy = true,
        |app| app.scanning = true,
        |app| app.loading_detail = true,
        |app| app.worktree_loading = true,
        |app| {
            app.history_host.start();
        },
        |app| app.branch_picker = Some(crate::ui::branch_filter::BranchPicker::Checkout),
        |app| app.open_context_menu(0, point(px(0.0), px(0.0))),
        |app| app.workspace_menu_open = true,
        |app| app.remote_menu = Some("origin".into()),
        |app| app.remote_picker_open = true,
        |app| app.overview_ref_picker = true,
    ];
    for block in blockers {
        let mut app = GitMasterApp::new();
        assert!(app.can_refresh_workspace());
        block(&mut app);
        assert!(!app.can_refresh_workspace());
    }
}

fn repo(name: &str, path: &str, submodules: Vec<SubmoduleInfo>) -> RepoInfo {
    RepoInfo {
        name: name.to_string(),
        path: PathBuf::from(path),
        is_dirty: false,
        current_branch: "main".to_string(),
        submodules,
        remote_statuses: Vec::new(),
    }
}

fn submodule(name: &str, path: &str, relative_path: &str) -> SubmoduleInfo {
    SubmoduleInfo {
        remote_statuses: Vec::new(),
        name: name.to_string(),
        path: PathBuf::from(path),
        relative_path: PathBuf::from(relative_path),
        url: None,
        is_initialized: true,
        is_dirty: false,
        current_branch: "main".to_string(),
    }
}

#[::core::prelude::v1::test]
fn repo_refresh_ignores_results_for_a_replaced_repo() {
    let mut app = GitMasterApp::new();
    app.repos = vec![repo("current", "/repos/current", Vec::new())];

    app.apply_repo_refresh(
        Path::new("/repos/previous"),
        Some(repo("stale", "/repos/previous", Vec::new())),
    );

    assert_eq!(app.repos[0].name, "current");
    assert_eq!(app.repos[0].path, PathBuf::from("/repos/current"));
}

#[::core::prelude::v1::test]
fn repo_refresh_reconciles_submodule_selection_by_relative_path() {
    let mut app = GitMasterApp::new();
    app.repos = vec![repo(
        "repo",
        "/repos/repo",
        vec![
            submodule("first", "/repos/repo/first", "first"),
            submodule("target", "/repos/repo/target", "target"),
        ],
    )];
    app.selected = Some(RepoSelection::Submodule {
        repo_index: 0,
        submodule_index: 1,
        relative_path: PathBuf::from("target"),
    });

    app.apply_repo_refresh(
        Path::new("/repos/repo"),
        Some(repo(
            "repo",
            "/repos/repo",
            vec![
                submodule("target", "/repos/repo/target", "target"),
                submodule("first", "/repos/repo/first", "first"),
            ],
        )),
    );

    assert_eq!(
        app.selected,
        Some(RepoSelection::Submodule {
            repo_index: 0,
            submodule_index: 0,
            relative_path: PathBuf::from("target"),
        })
    );
}

#[::core::prelude::v1::test]
fn pending_detail_survives_repository_reindexing() {
    let mut app = GitMasterApp::new();
    app.parent_dir = Some("/workspace".into());
    app.repos = vec![repo("target", "/workspace/target", vec![])];
    app.begin_select(0);
    let request = app.begin_detail_request("/workspace/target".into());
    app.apply_scan(
        Path::new("/workspace"),
        vec![
            repo("new", "/workspace/new", vec![]),
            repo("target", "/workspace/target", vec![]),
        ],
    );
    assert_eq!(app.selected, Some(RepoSelection::Repo(1)));
    assert!(app.owns_detail_request(&request));
    app.apply_detail(request, None, None, vec![], None);
    assert!(!app.loading_detail);
}
#[::core::prelude::v1::test]
fn pending_detail_survives_submodule_reordering() {
    let mut app = GitMasterApp::new();
    app.repos = vec![repo(
        "root",
        "/root",
        vec![submodule("target", "/root/target", "target")],
    )];
    app.begin_select_submodule(0, 0, "target".into());
    let request = app.begin_detail_request("/root/target".into());
    app.apply_repo_refresh(
        Path::new("/root"),
        Some(repo(
            "root",
            "/root",
            vec![
                submodule("new", "/root/new", "new"),
                submodule("target", "/root/target", "target"),
            ],
        )),
    );
    assert!(app.owns_detail_request(&request));
    app.apply_detail(request, None, None, vec![], None);
    assert!(!app.loading_detail);
}
#[::core::prelude::v1::test]
fn replaced_detail_request_and_navigation_reject_late_completion() {
    let mut app = GitMasterApp::new();
    app.repos = vec![repo("target", "/target", vec![])];
    app.begin_select(0);
    let old = app.begin_detail_request("/target".into());
    let current = app.begin_detail_request("/target".into());
    app.apply_detail(old, None, None, vec![], None);
    assert!(app.loading_detail);
    assert!(app.owns_detail_request(&current));
    app.begin_select(0);
    assert!(!app.owns_detail_request(&current));
}
#[::core::prelude::v1::test]
fn removing_selected_repository_cancels_pending_detail_and_worktree() {
    let mut app = GitMasterApp::new();
    app.parent_dir = Some("/workspace".into());
    app.repos = vec![repo("target", "/workspace/target", vec![])];
    app.begin_select(0);
    let request = app.begin_detail_request("/workspace/target".into());
    app.worktree_loading = true;
    app.apply_scan(Path::new("/workspace"), vec![]);
    assert!(app.selected.is_none());
    assert!(!app.loading_detail);
    assert!(!app.worktree_loading);
    assert!(!app.owns_detail_request(&request));
}

#[cfg(feature = "test-rpc")]
#[::core::prelude::v1::test]
fn test_rpc_selection_prepares_background_detail_work() {
    let mut app = GitMasterApp::new();
    app.repos = vec![repo("repo", "/repos/repo", Vec::new())];

    let request = app.prepare_test_command(crate::test_rpc::server::TestCommand::SelectRepo(0));

    assert!(request.is_some());
    assert!(app.loading_detail);
    assert_eq!(app.selected, Some(RepoSelection::Repo(0)));
}

#[cfg(feature = "test-rpc")]
#[::core::prelude::v1::test]
fn test_rpc_submodule_canvas_uses_the_selected_submodule_path() {
    let mut app = GitMasterApp::new();
    app.repos = vec![repo(
        "repo",
        "/repos/repo",
        vec![submodule("lib", "/repos/repo/lib", "lib")],
    )];

    let request = app
        .prepare_test_command(crate::test_rpc::server::TestCommand::SelectSubmodule {
            repo_index: 0,
            submodule_index: 0,
        })
        .unwrap();

    assert_eq!(request.graph_path, Some(PathBuf::from("/repos/repo/lib")));
}
#[::core::prelude::v1::test]
fn navigation_invalidates_read_ownership_even_after_returning_to_same_repo() {
    let mut app = GitMasterApp::new();
    app.begin_select(0);
    let original = app.worktree_revision;
    let navigation = app.navigation_revision;
    app.worktree_loading = true;
    app.begin_select(1);
    assert!(!app.worktree_loading);
    app.begin_select(0);
    assert!(!app.owns_worktree_request(original));
    assert!(app.navigation_revision > navigation);
    let selected = app.worktree_revision;
    app.begin_scan(PathBuf::from("other-workspace"));
    assert!(!app.owns_worktree_request(selected));
    app.begin_select_submodule(0, 0, PathBuf::from("module"));
    let read = app.worktree_revision;
    app.invalidate_worktree();
    assert!(!app.owns_worktree_request(read));
    app.repos = vec![repo(
        "repo",
        "/repo",
        vec![submodule("module", "/repo/module", "module")],
    )];
    app.begin_select_submodule(0, 0, PathBuf::from("module"));
    let removed = app.worktree_revision;
    app.worktree_loading = true;
    app.apply_repo_refresh(Path::new("/repo"), Some(repo("repo", "/repo", vec![])));
    assert!(app.selected.is_none());
    assert!(!app.worktree_loading);
    assert!(!app.owns_worktree_request(removed));
}
