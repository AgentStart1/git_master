use super::*;

impl TestViewTreeSnapshot {
    pub(super) fn build_commit_canvas_node(&self) -> ViewNode {
        let Some(layout) = self.commit_canvas_layout.as_ref() else {
            return ViewNode::new("panel")
                .with_id("commit-canvas")
                .with_child(ViewNode::new("text").with_text("Commit graph is unavailable."));
        };
        let lanes = layout.lanes.iter().map(|lane| {
            ViewNode::new("group")
                .with_id(&format!("commit-lane-{}", lane.id))
                .with_text(&lane.name)
        });
        let canvas_state = self.commit_canvas_state.clone().unwrap_or_default();
        let cards = layout
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.clone(),
                    crate::ui::canvas_routes::CardRect::new(
                        canvas_state.position_for(node),
                        node.width,
                        node.height,
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        let nodes = layout.nodes.iter().map(|node| {
            let mut view = ViewNode::new("commit-node")
                .with_id(&format!(
                    "commit-node-{}-{}",
                    node.id.lane_id, node.id.commit_id
                ))
                .with_interactive()
                .with_state(serde_json::json!({"lane":node.id.lane_id,"oid":node.id.commit_id,
                    "world_x":cards[&node.id].left,"world_y":cards[&node.id].top,"width":node.width,"height":node.height,
                    "kind":if node.is_head(){"reference"}else if node.is_placeholder(){"placeholder"}else{"commit"}}))
                .with_child(ViewNode::new("text").with_text(node.short_hash()));
            match node.kind {
                CanvasNodeKind::Commit => {
                    if let Some(entry) = node.entry.as_ref() {
                        view = view.with_child(ViewNode::new("text").with_text(&entry.message));
                    }
                }
                CanvasNodeKind::Placeholder => {
                    view = view.with_child(
                        ViewNode::new("text").with_text("Referenced commit not loaded"),
                    );
                }
                CanvasNodeKind::Head => {
                    view = view.with_children(
                        node.head_labels
                            .iter()
                            .map(|label| ViewNode::new("label").with_text(label))
                            .collect(),
                    );
                }
            }
            view
        });
        let edges = layout.edges.iter().map(|edge| {
            ViewNode::new("graph-edge").with_text(&format!(
                "{}:{} -> {}:{} ({})",
                edge.from.lane_id,
                &edge.from.commit_id[..7.min(edge.from.commit_id.len())],
                edge.to.lane_id,
                &edge.to.commit_id[..7.min(edge.to.commit_id.len())],
                match edge.kind {
                    CanvasEdgeKind::Reference => "reference",
                    CanvasEdgeKind::Parent => "parent",
                    CanvasEdgeKind::Submodule => "submodule",
                }
            )).with_state(serde_json::json!({
                "from_lane":edge.from.lane_id,"from_oid":edge.from.commit_id,"to_lane":edge.to.lane_id,"to_oid":edge.to.commit_id,
                "kind":match edge.kind { CanvasEdgeKind::Reference => "reference", CanvasEdgeKind::Parent => "parent", CanvasEdgeKind::Submodule => "submodule" },
                "route":crate::ui::canvas_routes::edge_route(edge,&cards).iter().map(|p|[p.x,p.y]).collect::<Vec<_>>()
            }))
        });

        ViewNode::new("canvas")
            .with_id("commit-canvas")
            .with_interactive()
            .with_state(serde_json::json!({"pan_x":canvas_state.pan.x,"pan_y":canvas_state.pan.y,"zoom":canvas_state.zoom}))
            .with_children(lanes.collect())
            .with_children(nodes.collect())
            .with_children(edges.collect())
    }
}
