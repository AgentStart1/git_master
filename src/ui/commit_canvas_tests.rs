use super::*;
use crate::models::CommitGraph;
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
fn placeholder_cards_reserve_distinct_rows_and_keep_them_when_history_loads() {
    let mut graph = CommitGraph {
        repository_path: "/fixture".into(),
        current_branch: "main".into(),
        head_labels: HashMap::new(),
        lanes: vec![
            CommitLane {
                id: "main".into(),
                name: "parent".into(),
                relative_path: None,
                status: CommitLaneStatus::Available,
                entries: vec![entry("main-tip", &["parent-a", "parent-b"])],
            },
            CommitLane {
                id: "submodule:lib".into(),
                name: "lib".into(),
                relative_path: Some("lib".into()),
                status: CommitLaneStatus::Available,
                entries: vec![entry("mod-tip", &["mod-parent"])],
            },
        ],
        submodule_links: vec![SubmoduleCommitLink {
            main_commit: "main-tip".into(),
            submodule_lane: "submodule:lib".into(),
            submodule_commit: "mod-old".into(),
        }],
    };
    let previous = build_layout(&graph);
    let assert_separate = |layout: &CommitCanvasLayout| {
        for (index, a) in layout.nodes.iter().enumerate() {
            for b in layout.nodes.iter().skip(index + 1) {
                let p = a.default_position;
                let q = b.default_position;
                assert!(
                    p.x + a.width <= q.x
                        || q.x + b.width <= p.x
                        || p.y + a.height <= q.y
                        || q.y + b.height <= p.y,
                    "{} overlaps {}",
                    a.id.commit_id,
                    b.id.commit_id
                );
            }
        }
    };
    assert_separate(&previous);
    assert_eq!(
        previous
            .edges
            .iter()
            .filter(|e| e.kind == CanvasEdgeKind::Parent)
            .count(),
        3
    );
    assert_eq!(
        previous
            .edges
            .iter()
            .filter(|e| e.kind == CanvasEdgeKind::Submodule)
            .count(),
        1
    );
    let mut state = CommitCanvasState::default();
    state.ensure_layout(&previous);
    let positions = state.node_positions.clone();
    graph.lanes[0]
        .entries
        .extend([entry("parent-a", &[]), entry("parent-b", &[])]);
    graph.lanes[1]
        .entries
        .extend([entry("mod-parent", &["mod-old"]), entry("mod-old", &[])]);
    let mut next = build_layout(&graph);
    super::super::history_loading::stabilize_history_layout(&previous, &mut next);
    state.ensure_layout(&next);
    assert_separate(&next);
    for (id, position) in positions {
        assert_eq!(state.node_positions[&id], position);
    }
}

#[::core::prelude::v1::test]
fn fitting_long_history_contains_every_node_including_manual_positions() {
    let graph = CommitGraph {
        repository_path: PathBuf::from("/repo"),
        current_branch: "main".into(),
        lanes: vec![CommitLane {
            id: "main".into(),
            name: "repo".into(),
            relative_path: None,
            status: CommitLaneStatus::Available,
            entries: (0..500)
                .map(|i| entry(&format!("node{i:06}"), &[]))
                .collect(),
        }],
        submodule_links: vec![],
        head_labels: HashMap::new(),
    };
    let layout = build_layout(&graph);
    let mut state = CommitCanvasState::default();
    state.ensure_layout(&layout);
    state.node_positions.insert(
        layout.nodes[0].id.clone(),
        CanvasPoint::new(-12340.0, -200.0),
    );
    let positions = state.node_positions.clone();
    for (width, height) in [(1068.0, 609.0), (480.0, 360.0)] {
        state.fit_content(&layout, width, height);
        assert!(state.zoom < 0.15);
        assert!(state.zoom > 0.0);
        for node in &layout.nodes {
            let p = state.position_for(node);
            let x = state.pan.x + p.x * state.zoom;
            let y = state.pan.y + p.y * state.zoom;
            assert!(x >= 23.99 && y >= 61.99);
            assert!(x + node.width * state.zoom <= width - 23.99);
            assert!(y + node.height * state.zoom <= height - 49.99);
        }
        assert_eq!(state.node_positions, positions);
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
        current_branch: "main".into(),
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
        head_labels: HashMap::from([(
            "aaaaaaaa".to_string(),
            vec!["HEAD".to_string(), "main".to_string()],
        )]),
    };

    let layout = build_layout(&graph);

    assert_eq!(layout.nodes.len(), 4);
    assert!(!layout.lanes.iter().any(|lane| lane.id == "heads"));
    assert_eq!(layout.list.entries.len(), 2);
    let mut app = GitMasterApp::new();
    app.begin_select(0);
    app.repos = vec![crate::models::RepoInfo {
        path: layout.repository_path.clone(),
        name: "repo".into(),
        current_branch: "main".into(),
        submodules: vec![],
        is_dirty: false,
        remote_statuses: vec![],
    }];
    let request = app.begin_detail_request(layout.repository_path.clone());
    app.apply_detail(
        request,
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
            && node.head_labels == ["HEAD".to_string(), "main".to_string()]
            && node.id.lane_id == "heads"
            && node.id.commit_id == "head:aaaaaaaa"
    }));
    let label = layout.nodes.iter().find(|node| node.is_head()).unwrap();
    let commit = layout
        .nodes
        .iter()
        .find(|node| node.id.commit_id == "aaaaaaaa")
        .unwrap();
    assert_eq!(
        label.default_position.y + label.height / 2.0,
        commit.default_position.y + commit.height / 2.0
    );
    assert!(label.default_position.x > commit.default_position.x + commit.width);
    assert!(label.default_position.x + label.width < commit.default_position.x + LANE_SPACING);
    assert!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.kind != CanvasEdgeKind::Reference)
            .all(|edge| edge.from.lane_id != "heads" && edge.to.lane_id != "heads")
    );
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
    app.commit_canvas_states.insert(
        repository_path.clone(),
        CommitCanvasState {
            pan: CanvasPoint::new(18.0, 16.0),
            ..Default::default()
        },
    );
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

#[::core::prelude::v1::test]
fn expanding_history_preserves_existing_nodes_pan_zoom_and_dragged_positions() {
    let mut graph = CommitGraph {
        current_branch: "main".into(),
        repository_path: PathBuf::from("/repo"),
        lanes: vec![CommitLane {
            id: "main".into(),
            name: "repo".into(),
            relative_path: None,
            status: CommitLaneStatus::Available,
            entries: vec![
                entry("new-tip", &["new-parent"]),
                entry("old-tip", &["old-parent"]),
            ],
        }],
        submodule_links: vec![],
        head_labels: HashMap::from([("old-tip".into(), vec!["older".into()])]),
    };
    let previous = build_layout(&graph);
    let mut state = CommitCanvasState::default();
    state.ensure_layout(&previous);
    state.pan = CanvasPoint::new(-70.0, -1200.0);
    state.zoom = 1.3;
    let dragged = previous.nodes[0].id.clone();
    state
        .node_positions
        .insert(dragged, CanvasPoint::new(815.0, 456.0));
    let positions = state.node_positions.clone();
    graph.lanes[0].entries.insert(1, entry("new-parent", &[]));
    graph.lanes[0].entries.push(entry("old-parent", &[]));
    let mut next = build_layout(&graph);
    super::super::history_loading::stabilize_history_layout(&previous, &mut next);
    state.ensure_layout(&next);
    assert_eq!(state.pan, CanvasPoint::new(-70.0, -1200.0));
    assert_eq!(state.zoom, 1.3);
    for (id, position) in positions {
        assert_eq!(state.node_positions[&id], position);
    }
    let old_bottom = previous
        .nodes
        .iter()
        .map(|node| node.default_position.y + node.height)
        .fold(0.0, f32::max);
    for node in next
        .nodes
        .iter()
        .filter(|node| !previous.nodes.iter().any(|old| old.id == node.id))
    {
        assert!(node.default_position.y > old_bottom);
    }
    assert!(
        next.edges
            .iter()
            .any(|edge| edge.from.commit_id == "old-tip" && edge.to.commit_id == "old-parent")
    );
}

#[::core::prelude::v1::test]
fn reference_cards_preserve_local_and_remote_tips_and_connect_to_their_commit() {
    let graph = CommitGraph {
        repository_path: "/fixture".into(),
        current_branch: "feature".into(),
        head_labels: HashMap::from([
            (
                "aaaaaaaa".into(),
                vec!["HEAD".into(), "feature".into(), "origin/feature".into()],
            ),
            (
                "bbbbbbbb".into(),
                vec![
                    "main".into(),
                    "origin/main".into(),
                    "origin/HEAD".into(),
                    "backup/main".into(),
                ],
            ),
        ]),
        lanes: vec![CommitLane {
            id: "main".into(),
            name: "repo".into(),
            relative_path: None,
            status: CommitLaneStatus::Available,
            entries: vec![entry("aaaaaaaa", &["bbbbbbbb"]), entry("bbbbbbbb", &[])],
        }],
        submodule_links: vec![],
    };
    let layout = build_layout(&graph);
    let labels = layout
        .nodes
        .iter()
        .filter(|node| node.is_head())
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 2);
    for (hash, expected) in &graph.head_labels {
        let label = labels
            .iter()
            .find(|node| node.id.commit_id == format!("head:{hash}"))
            .unwrap();
        assert_eq!(&label.head_labels, expected);
        assert!(
            layout
                .edges
                .iter()
                .any(|edge| edge.kind == CanvasEdgeKind::Reference
                    && edge.from == label.id
                    && edge.to.lane_id == "main"
                    && edge.to.commit_id == *hash)
        );
    }
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.kind == CanvasEdgeKind::Reference)
            .count(),
        2
    );
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.kind == CanvasEdgeKind::Parent)
            .count(),
        1
    );
}

fn reference_graph(labels: Vec<String>) -> CommitGraph {
    CommitGraph {
        repository_path: "/fixture".into(),
        current_branch: "main".into(),
        lanes: vec![CommitLane {
            id: "main".into(),
            name: "fixture".into(),
            relative_path: None,
            status: CommitLaneStatus::Available,
            entries: vec![entry("tip", &["older"])],
        }],
        submodule_links: vec![],
        head_labels: HashMap::from([("tip".into(), labels)]),
    }
}

fn reference_route(layout: &CommitCanvasLayout, state: &CommitCanvasState) -> Vec<CanvasPoint> {
    let cards = layout
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.clone(),
                super::super::canvas_routes::CardRect::new(
                    state.position_for(node),
                    node.width,
                    node.height,
                ),
            )
        })
        .collect();
    super::super::canvas_routes::edge_route(
        layout
            .edges
            .iter()
            .find(|edge| edge.kind == CanvasEdgeKind::Reference)
            .unwrap(),
        &cards,
    )
}

#[::core::prelude::v1::test]
fn default_references_are_centered_and_connect_with_a_single_horizontal_segment() {
    for count in [0, 1, 4, 8] {
        let mut labels = vec!["HEAD".into()];
        labels.extend((0..count).map(|i| format!("branch-{i}")));
        let layout = build_layout(&reference_graph(labels));
        let head = layout.nodes.iter().find(|node| node.is_head()).unwrap();
        let commit = layout
            .nodes
            .iter()
            .find(|node| node.id.commit_id == "tip")
            .unwrap();
        let y = commit.default_position.y + commit.height / 2.0;
        assert_eq!(head.default_position.y + head.height / 2.0, y);
        assert_eq!(
            head.default_position.x,
            commit.default_position.x + commit.width + 12.0
        );
        assert_eq!(
            reference_route(&layout, &CommitCanvasState::default()),
            vec![
                CanvasPoint::new(head.default_position.x, y),
                CanvasPoint::new(commit.default_position.x + commit.width, y),
            ]
        );
    }
}

#[::core::prelude::v1::test]
fn reference_layout_moves_colliding_labels_below_existing_cards() {
    let mut graph = reference_graph((0..14).map(|i| format!("branch-{i}")).collect());
    graph.lanes[0].entries.push(entry("older", &[]));
    graph
        .head_labels
        .insert("older".into(), vec!["older-branch".into()]);
    let layout = build_layout(&graph);
    assert_reference_labels_separate(&layout);
}

fn assert_reference_labels_separate(layout: &CommitCanvasLayout) {
    let head = layout
        .nodes
        .iter()
        .find(|node| node.id.commit_id == "head:older")
        .unwrap();
    let commit = layout
        .nodes
        .iter()
        .find(|node| node.id.commit_id == "older")
        .unwrap();
    assert!(
        head.default_position.y > commit.default_position.y + (commit.height - head.height) / 2.0
    );
    for other in layout.nodes.iter().filter(|node| node.id != head.id) {
        let p = head.default_position;
        let q = other.default_position;
        assert!(
            p.x >= q.x + other.width + 8.0
                || p.x + head.width + 8.0 <= q.x
                || p.y >= q.y + other.height + 8.0
                || p.y + head.height + 8.0 <= q.y
        );
    }
}

#[::core::prelude::v1::test]
fn incremental_history_centers_new_references_without_moving_existing_nodes() {
    let mut graph = reference_graph(vec!["HEAD".into()]);
    let previous = build_layout(&graph);
    graph.lanes[0].entries.push(entry("older", &[]));
    graph
        .head_labels
        .insert("older".into(), vec!["older-branch".into()]);
    let mut next = build_layout(&graph);
    super::super::history_loading::stabilize_history_layout(&previous, &mut next);
    for old in &previous.nodes {
        assert_eq!(
            next.nodes
                .iter()
                .find(|node| node.id == old.id)
                .unwrap()
                .default_position,
            old.default_position
        );
    }
    let head = next
        .nodes
        .iter()
        .find(|node| node.id.commit_id == "head:older")
        .unwrap();
    let commit = next
        .nodes
        .iter()
        .find(|node| node.id.commit_id == "older")
        .unwrap();
    assert_eq!(
        head.default_position.x,
        commit.default_position.x + commit.width + 12.0
    );
    assert_eq!(
        head.default_position.y + head.height / 2.0,
        commit.default_position.y + commit.height / 2.0
    );
}

#[::core::prelude::v1::test]
fn incremental_history_avoids_existing_reference_cards() {
    let mut graph = reference_graph((0..14).map(|i| format!("branch-{i}")).collect());
    let previous = build_layout(&graph);
    graph.lanes[0].entries.push(entry("older", &[]));
    graph
        .head_labels
        .insert("older".into(), vec!["older-branch".into()]);
    let mut next = build_layout(&graph);
    super::super::history_loading::stabilize_history_layout(&previous, &mut next);
    assert_reference_labels_separate(&next);
    for old in &previous.nodes {
        assert_eq!(
            next.nodes
                .iter()
                .find(|node| node.id == old.id)
                .unwrap()
                .default_position,
            old.default_position
        );
    }
}

#[::core::prelude::v1::test]
fn dragging_commit_keeps_head_fixed_and_updates_cached_reference_route() {
    let layout = build_layout(&reference_graph(vec!["HEAD".into(), "main".into()]));
    let head = layout.nodes.iter().find(|node| node.is_head()).unwrap();
    let commit = layout
        .nodes
        .iter()
        .find(|node| node.id.commit_id == "tip")
        .unwrap();
    let mut state = CommitCanvasState::default();
    state.ensure_layout(&layout);
    let original_positions = state.node_positions.clone();
    let cards = |state: &CommitCanvasState| {
        layout
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.clone(),
                    super::super::canvas_routes::CardRect::new(
                        state.position_for(node),
                        node.width,
                        node.height,
                    ),
                )
            })
            .collect()
    };
    let mut app = GitMasterApp::new();
    let before = app
        .canvas_route_cache
        .borrow_mut()
        .routes(&layout.edges, cards(&state));
    app.commit_canvas_states
        .insert(layout.repository_path.clone(), state);
    app.commit_canvas_interaction = Some(CanvasInteraction::DragNode {
        repository_path: layout.repository_path.clone(),
        node: commit.id.clone(),
        last_pointer: CanvasPoint::new(100.0, 100.0),
    });
    assert!(app.update_commit_canvas_drag(&MouseMoveEvent {
        position: gpui::point(px(70.0), px(150.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
    }));
    let state = &app.commit_canvas_states[&layout.repository_path];
    assert_eq!(state.position_for(head), original_positions[&head.id]);
    for node in layout.nodes.iter().filter(|node| node.id != commit.id) {
        assert_eq!(state.position_for(node), original_positions[&node.id]);
    }
    assert_eq!(
        state.position_for(commit),
        CanvasPoint::new(
            commit.default_position.x - 30.0,
            commit.default_position.y + 50.0
        )
    );
    let after = app
        .canvas_route_cache
        .borrow_mut()
        .routes(&layout.edges, cards(state));
    assert!(!std::rc::Rc::ptr_eq(&before, &after));
    assert_ne!(before[0], after[0]);
    assert_eq!(before[0].first(), after[0].first());
    assert_eq!(
        after[0].last(),
        Some(&CanvasPoint::new(
            state.position_for(commit).x + commit.width,
            state.position_for(commit).y + commit.height / 2.0
        ))
    );
}
