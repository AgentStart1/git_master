use super::*;
use crate::models::{CommitLane, SubmoduleCommitLink};

fn entry(hash: &str, parents: &[&str]) -> LogEntry {
    LogEntry {
        full_hash: hash.to_string(),
        hash: hash[..7.min(hash.len())].to_string(),
        parent_hashes: parents.iter().map(|parent| (*parent).to_string()).collect(),
        author: "Developer".to_string(),
        date: "2026-01-01 10:00".to_string(),
        message: "Commit".to_string(),
    }
}

#[::core::prelude::v1::test]
fn forks_and_merges_use_distinct_columns() {
    let entries = vec![
        entry("merge000", &["left0000", "right000"]),
        entry("left0000", &["base0000"]),
        entry("right000", &["base0000"]),
        entry("base0000", &[]),
    ];
    let columns = branch_columns(&entries);
    assert_eq!(columns[0], columns[1]);
    assert_ne!(columns[1], columns[2]);
    assert_eq!(columns[1], columns[3]);
    let divergent = branch_columns(&[
        entry("local000", &["base0000"]),
        entry("remote00", &["base0000"]),
        entry("base0000", &[]),
    ]);
    assert_ne!(divergent[0], divergent[1]);
}

#[::core::prelude::v1::test]
fn first_parent_reclaims_its_column_when_merge_paths_converge() {
    let entries = vec![
        entry("merge000", &["left0000", "right000"]),
        entry("right000", &["base0000"]),
        entry("left0000", &["base0000"]),
        entry("base0000", &[]),
    ];

    let columns = branch_columns(&entries);

    assert_eq!(columns[0], columns[2]);
    assert_ne!(columns[1], columns[2]);
    assert_eq!(columns[2], columns[3]);
}

#[::core::prelude::v1::test]
fn layout_connects_parents_and_creates_missing_submodule_placeholders() {
    let graph = CommitGraph {
        repository_path: PathBuf::from("/repo"),
        lanes: vec![
            CommitLane {
                id: "main".to_string(),
                name: "repo".to_string(),
                relative_path: None,
                status: CommitLaneStatus::Available,
                entries: vec![entry("aaaaaaaa", &["bbbbbbbb"]), entry("bbbbbbbb", &[])],
            },
            CommitLane {
                id: "submodule:lib".to_string(),
                name: "lib".to_string(),
                relative_path: Some(PathBuf::from("lib")),
                status: CommitLaneStatus::Uninitialized,
                entries: Vec::new(),
            },
        ],
        submodule_links: vec![SubmoduleCommitLink {
            main_commit: "aaaaaaaa".to_string(),
            submodule_lane: "submodule:lib".to_string(),
            submodule_commit: "cccccccc".to_string(),
        }],
        head_labels: HashMap::from([("aaaaaaaa".to_string(), vec!["main".to_string()])]),
    };

    let layout = build_layout(&graph);

    assert_eq!(layout.nodes.len(), 4);
    assert!(!layout.lanes.iter().any(|lane| lane.id == "heads"));
    assert_eq!(layout.list.entries.len(), 2);
    let mut app = GitMasterApp::new();
    app.begin_select(0);
    app.apply_detail(
        crate::app_state::RepoSelection::Repo(0),
        None,
        None,
        vec![entry("stale", &[])],
        Some(layout.clone()),
    );
    assert_eq!(
        app.log_entries
            .iter()
            .map(|entry| entry.full_hash.as_str())
            .collect::<Vec<_>>(),
        ["aaaaaaaa", "bbbbbbbb"]
    );
    app.set_log_view_mode(LogViewMode::Canvas);
    app.set_log_view_mode(LogViewMode::List);
    assert_eq!(app.log_entries.len(), layout.list.entries.len());
    assert!(layout.nodes.iter().any(|node| {
        node.kind == CanvasNodeKind::Head
            && node.head_labels == ["main".to_string()]
            && node.id.lane_id == "heads"
            && node.id.commit_id == "head:aaaaaaaa"
    }));
    assert!(layout.edges.iter().any(|edge| {
        edge.kind == CanvasEdgeKind::Head
            && edge.from.commit_id == "head:aaaaaaaa"
            && edge.to.commit_id == "aaaaaaaa"
    }));
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.kind == CanvasEdgeKind::Parent)
            .count(),
        1
    );
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.kind == CanvasEdgeKind::Submodule)
            .count(),
        1
    );
    assert!(layout.nodes.iter().any(|node| {
        node.id.commit_id == "cccccccc"
            && node.id.lane_id == "submodule:lib"
            && node.is_placeholder()
    }));
}

#[::core::prelude::v1::test]
fn dragging_a_node_updates_only_its_saved_world_position() {
    let mut app = GitMasterApp::new();
    let repository_path = PathBuf::from("/repo");
    let node = CanvasNodeId {
        lane_id: "main".to_string(),
        commit_id: "aaaaaaaa".to_string(),
    };
    let mut state = CommitCanvasState {
        zoom: 2.0,
        ..Default::default()
    };
    state
        .node_positions
        .insert(node.clone(), CanvasPoint::new(20.0, 30.0));
    let original_pan = state.pan;
    app.commit_canvas_states
        .insert(repository_path.clone(), state);
    app.commit_canvas_interaction = Some(CanvasInteraction::DragNode {
        repository_path: repository_path.clone(),
        node: node.clone(),
        last_pointer: CanvasPoint::new(100.0, 100.0),
    });

    let changed = app.update_commit_canvas_drag(&MouseMoveEvent {
        position: gpui::point(px(120.0), px(110.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
    });

    let state = app.commit_canvas_states.get(&repository_path).unwrap();
    assert!(changed);
    assert_eq!(state.pan, original_pan);
    assert_eq!(
        state.node_positions.get(&node),
        Some(&CanvasPoint::new(30.0, 35.0))
    );
}

#[::core::prelude::v1::test]
fn dragging_the_background_pans_the_repository_canvas() {
    let mut app = GitMasterApp::new();
    let repository_path = PathBuf::from("/repo");
    app.commit_canvas_states
        .insert(repository_path.clone(), CommitCanvasState::default());
    app.commit_canvas_interaction = Some(CanvasInteraction::Pan {
        repository_path: repository_path.clone(),
        last_pointer: CanvasPoint::new(50.0, 70.0),
    });

    app.update_commit_canvas_drag(&MouseMoveEvent {
        position: gpui::point(px(62.0), px(65.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
    });

    assert_eq!(
        app.commit_canvas_states.get(&repository_path).unwrap().pan,
        CanvasPoint::new(30.0, 11.0)
    );
}
