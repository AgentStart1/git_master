use super::{commit_canvas::*, theme};
use gpui::*;
use std::collections::HashSet;

pub(super) fn paint_edges(
    bounds: Bounds<Pixels>,
    state: &CommitCanvasState,
    edges: &[CanvasEdge],
    routes: &[Vec<CanvasPoint>],
    unloaded: &HashSet<CanvasNodeId>,
    window: &mut Window,
) {
    for (edge, route) in edges.iter().zip(routes) {
        let pending_parent = edge.kind == CanvasEdgeKind::Parent && unloaded.contains(&edge.to);
        let mut path = PathBuilder::stroke(px(if edge.kind == CanvasEdgeKind::Reference {
            1.2
        } else {
            1.8
        }));
        if edge.kind == CanvasEdgeKind::Submodule || pending_parent {
            path = path.dash_array(&[px(7.0), px(5.0)]);
        }
        for (index, p) in route.iter().enumerate() {
            let p = gpui::point(
                bounds.origin.x + px(state.pan.x + p.x * state.zoom),
                bounds.origin.y + px(state.pan.y + p.y * state.zoom),
            );
            if index == 0 {
                path.move_to(p);
            } else {
                path.line_to(p);
            }
        }
        let color = if edge.kind == CanvasEdgeKind::Reference {
            theme::palette().text_subtle
        } else if edge.kind == CanvasEdgeKind::Submodule {
            theme::palette().text_muted
        } else if edge.from.lane_id == "main" || pending_parent {
            theme::palette().accent
        } else {
            theme::palette().green
        };
        if let Ok(path) = path.build() {
            window.paint_path(path, rgb(color));
        }
    }
}
