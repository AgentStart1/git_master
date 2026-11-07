use super::*;
use crate::models::{CommitGraph, CommitLane, CommitLaneStatus, LogEntry};
use crate::ui::commit_canvas::{CommitCanvasState, NODE_HEIGHT, NODE_WIDTH, build_layout};

fn id(hash: &str) -> CanvasNodeId {
    CanvasNodeId {
        lane_id: "main".into(),
        commit_id: hash.into(),
    }
}
fn card(x: f32, y: f32) -> CardRect {
    CardRect::new(CanvasPoint::new(x, y), NODE_WIDTH, NODE_HEIGHT)
}

#[test]
fn narrow_reference_gap_has_no_horizontal_reversal() {
    for gap in [4.0, 12.0, 20.0] {
        let edge = CanvasEdge {
            from: id("head"),
            to: id("commit"),
            kind: CanvasEdgeKind::Reference,
        };
        let cards = HashMap::from([
            (
                edge.from.clone(),
                CardRect::new(CanvasPoint::new(NODE_WIDTH + gap, -60.0), 120.0, 50.0),
            ),
            (edge.to.clone(), card(0.0, 0.0)),
        ]);
        let route = edge_route(&edge, &cards);
        assert!(route.windows(2).all(|p| p[1].x <= p[0].x), "{route:?}");
        assert!(clear(&route, &cards.values().copied().collect::<Vec<_>>()));
        assert_eq!(route.len(), 4);
    }
}

#[test]
fn unchanged_geometry_reuses_routes() {
    let mut cache = RouteCache::default();
    let edges = vec![CanvasEdge {
        from: id("child"),
        to: id("parent"),
        kind: CanvasEdgeKind::Parent,
    }];
    let cards = HashMap::from([
        (id("child"), card(0.0, 0.0)),
        (id("parent"), card(0.0, 200.0)),
    ]);
    let first = cache.routes(&edges, cards.clone());
    assert!(std::rc::Rc::ptr_eq(&first, &cache.routes(&edges, cards)));
}

#[test]
fn moving_an_obstacle_invalidates_cached_routes() {
    let mut cache = RouteCache::default();
    let edges = vec![CanvasEdge {
        from: id("child"),
        to: id("parent"),
        kind: CanvasEdgeKind::Parent,
    }];
    let mut cards = HashMap::from([
        (id("child"), card(0.0, 0.0)),
        (id("parent"), card(0.0, 300.0)),
        (id("obstacle"), card(400.0, 150.0)),
    ]);
    let first = cache.routes(&edges, cards.clone());
    cards.insert(id("obstacle"), card(0.0, 150.0));
    let next = cache.routes(&edges, cards.clone());
    assert_ne!(*first, *next);
    assert!(clear(
        &next[0],
        &cards.values().copied().collect::<Vec<_>>()
    ));
}

#[test]
fn topology_changes_invalidate_cached_routes() {
    let mut cache = RouteCache::default();
    let cards = HashMap::from([
        (id("child"), card(0.0, 0.0)),
        (id("parent"), card(0.0, 200.0)),
    ]);
    let first = cache.routes(&[], cards.clone());
    let edges = vec![CanvasEdge {
        from: id("child"),
        to: id("parent"),
        kind: CanvasEdgeKind::Parent,
    }];
    let next = cache.routes(&edges, cards);
    assert!(first.is_empty());
    assert_eq!(next.len(), 1);
}

#[test]
fn parent_routes_leave_bottom_and_enter_top_even_after_dragging_above_source() {
    for (target_x, target_y) in [(0.0, 200.0), (380.0, 300.0), (0.0, -200.0)] {
        let edge = CanvasEdge {
            from: id("child"),
            to: id("parent"),
            kind: CanvasEdgeKind::Parent,
        };
        let cards = HashMap::from([
            (id("child"), card(0.0, 0.0)),
            (id("parent"), card(target_x, target_y)),
        ]);
        let route = edge_route(&edge, &cards);
        assert_eq!(
            route.first(),
            Some(&CanvasPoint::new(NODE_WIDTH / 2.0, NODE_HEIGHT))
        );
        assert_eq!(
            route.last(),
            Some(&CanvasPoint::new(target_x + NODE_WIDTH / 2.0, target_y))
        );
        assert!(route[1].y > route[0].y);
        assert!(route[route.len() - 2].y < route.last().unwrap().y);
        assert!(clear(&route, &cards.values().copied().collect::<Vec<_>>()));
    }
}

#[test]
fn submodule_route_joins_commit_sides_and_detours_around_reference_label() {
    let edge = CanvasEdge {
        from: id("commit"),
        to: id("module"),
        kind: CanvasEdgeKind::Submodule,
    };
    let cards = HashMap::from([
        (id("commit"), card(0.0, 0.0)),
        (id("module"), card(500.0, 0.0)),
        (
            id("HEAD-label"),
            CardRect::new(CanvasPoint::new(260.0, 0.0), 100.0, 64.0),
        ),
    ]);
    let route = edge_route(&edge, &cards);
    assert_eq!(
        route.first(),
        Some(&CanvasPoint::new(NODE_WIDTH, NODE_HEIGHT / 2.0))
    );
    assert_eq!(
        route.last(),
        Some(&CanvasPoint::new(500.0, NODE_HEIGHT / 2.0))
    );
    assert!(route.len() > 2);
    assert!(clear(&route, &cards.values().copied().collect::<Vec<_>>()));
}

#[test]
fn every_merge_parent_is_routed_without_crossing_cards_or_head_label() {
    let entry = |hash: &str, parents: &[&str]| LogEntry {
        full_hash: hash.into(),
        hash: hash.into(),
        parent_hashes: parents.iter().map(|p| p.to_string()).collect(),
        author: "Fixture".into(),
        date: String::new(),
        message: hash.into(),
    };
    let graph = CommitGraph {
        repository_path: "/fixture".into(),
        current_branch: "main".into(),
        lanes: vec![CommitLane {
            id: "main".into(),
            name: "fixture".into(),
            relative_path: None,
            status: CommitLaneStatus::Available,
            entries: vec![
                entry("merge", &["left", "right"]),
                entry("left", &["base"]),
                entry("right", &["base"]),
                entry("base", &[]),
            ],
        }],
        submodule_links: Vec::new(),
        head_labels: HashMap::from([("merge".into(), vec!["HEAD".into(), "main".into()])]),
    };
    let layout = build_layout(&graph);
    let state = CommitCanvasState::default();
    let cards = layout
        .nodes
        .iter()
        .map(|n| {
            (
                n.id.clone(),
                CardRect::new(state.position_for(n), n.width, n.height),
            )
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.kind == CanvasEdgeKind::Parent)
            .count(),
        4
    );
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.kind == CanvasEdgeKind::Reference)
            .count(),
        1
    );
    for edge in &layout.edges {
        let route = edge_route(edge, &cards);
        assert!(
            clear(&route, &cards.values().copied().collect::<Vec<_>>()),
            "{} -> {}: {route:?}",
            edge.from.commit_id,
            edge.to.commit_id
        );
        assert!(route.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
    }
}

#[test]
fn crossing_gitlinks_keep_separate_vertical_channels() {
    let cards = HashMap::from([
        (id("a"), card(0.0, 0.0)),
        (id("b"), card(0.0, 112.0)),
        (id("c"), card(380.0, 0.0)),
        (id("d"), card(380.0, 112.0)),
    ]);
    let route = |from, to| {
        edge_route(
            &CanvasEdge {
                from: id(from),
                to: id(to),
                kind: CanvasEdgeKind::Submodule,
            },
            &cards,
        )
    };
    let a = route("a", "d");
    let b = route("b", "c");
    let vertical_x = |points: &[CanvasPoint]| {
        points
            .windows(2)
            .find(|pair| pair[0].x == pair[1].x && pair[0].y != pair[1].y)
            .unwrap()[0]
            .x
    };
    assert_ne!(vertical_x(&a), vertical_x(&b));
    assert!(clear(&a, &cards.values().copied().collect::<Vec<_>>()));
    assert!(clear(&b, &cards.values().copied().collect::<Vec<_>>()));
}

#[test]
fn staggered_obstacles_allow_a_multi_bend_route_without_text_intersection() {
    let obstacles = [
        CardRect {
            left: -10.0,
            top: 20.0,
            right: 80.0,
            bottom: 40.0,
        },
        CardRect {
            left: 20.0,
            top: 60.0,
            right: 110.0,
            bottom: 80.0,
        },
    ];
    let start = CanvasPoint::new(0.0, 0.0);
    let end = CanvasPoint::new(100.0, 100.0);
    let route = route_between(start, end, &obstacles);
    assert_eq!(route.first(), Some(&start));
    assert_eq!(route.last(), Some(&end));
    assert!(clear(&route, &obstacles));
}

#[test]
fn reference_connectors_follow_moved_label_and_commit_card_boundaries() {
    for head_x in [-180.0, 400.0] {
        let edge = CanvasEdge {
            from: id("head:tip"),
            to: id("tip"),
            kind: CanvasEdgeKind::Reference,
        };
        let mut cards = HashMap::from([
            (
                edge.from.clone(),
                CardRect::new(CanvasPoint::new(head_x, 0.0), 120.0, 50.0),
            ),
            (edge.to.clone(), card(100.0, 150.0)),
        ]);
        let original = edge_route(&edge, &cards);
        let label = cards[&edge.from];
        let commit = cards[&edge.to];
        let right = head_x > 100.0;
        assert_eq!(
            original.first(),
            Some(&CanvasPoint::new(
                if right { label.left } else { label.right },
                25.0
            ))
        );
        assert_eq!(
            original.last(),
            Some(&CanvasPoint::new(
                if right { commit.right } else { commit.left },
                150.0 + NODE_HEIGHT / 2.0
            ))
        );
        for segment in original.windows(2) {
            assert!(
                !cards
                    .values()
                    .any(|rect| rect.blocks(segment[0], segment[1]))
            );
        }
        cards.insert(
            edge.from.clone(),
            CardRect::new(CanvasPoint::new(head_x, 40.0), 120.0, 50.0),
        );
        assert_ne!(edge_route(&edge, &cards), original);
    }
}

#[test]
fn centered_reference_detours_around_an_intervening_card() {
    let edge = CanvasEdge {
        from: id("head"),
        to: id("commit"),
        kind: CanvasEdgeKind::Reference,
    };
    let cards = HashMap::from([
        (
            edge.from.clone(),
            CardRect::new(CanvasPoint::new(500.0, 7.0), 100.0, 50.0),
        ),
        (edge.to.clone(), card(0.0, 0.0)),
        (
            id("obstacle"),
            CardRect::new(CanvasPoint::new(320.0, 0.0), 100.0, 64.0),
        ),
    ]);
    let route = edge_route(&edge, &cards);
    assert_eq!(route.first(), Some(&CanvasPoint::new(500.0, 32.0)));
    assert_eq!(route.last(), Some(&CanvasPoint::new(NODE_WIDTH, 32.0)));
    assert!(route.len() > 2);
    assert!(clear(&route, &cards.values().copied().collect::<Vec<_>>()));
}
