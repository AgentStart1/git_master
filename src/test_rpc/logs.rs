use super::view_tree::{Rect, ViewNode};
use gpui::{Bounds, Pixels};
use std::collections::HashMap;

pub struct Observation {
    pub completed_reads: u64,
    pub generation: u64,
    pub poll_ticks: u64,
    pub active_read: Option<u64>,
    pub active_read_manual: Option<bool>,
    pub manual_queued: bool,
}

pub fn build_tree(
    viewer: &crate::log_viewer::Viewer,
    reg: &HashMap<String, Bounds<Pixels>>,
    revision: u64,
    observation: Observation,
) -> ViewNode {
    use crate::log_viewer::State;
    let state = &viewer.state;
    let message = state.display_text();
    let last = match state {
        State::Ready(snapshot) if !snapshot.text.is_empty() => {
            snapshot.rows.last().map(String::as_str).unwrap_or_default()
        }
        _ => message,
    };
    let phase = match state {
        State::Loading => "loading",
        State::Failed(_) => "failed",
        State::Ready(_) => "ready",
    };
    let mut panel = ViewNode::new("panel")
        .with_id("logs-panel")
        .with_state(serde_json::json!({"phase":phase}))
        .with_child(
            ViewNode::new("panel")
                .with_id("logs-content")
                .with_interactive()
                .with_text(&message)
                .with_child(ViewNode::new("text").with_id("logs-end").with_text(last)),
        );
    if let State::Ready(snapshot) = state {
        panel = panel.with_state(
            serde_json::json!({"phase":phase,"path":snapshot.path,"truncated":snapshot.truncated}),
        );
    }
    for (id, enabled) in [
        ("logs-close", true),
        ("logs-follow", true),
        ("logs-refresh", true),
        (
            "logs-copy",
            matches!(state, State::Ready(snapshot) if !snapshot.text.is_empty()),
        ),
    ] {
        let node = ViewNode::new("button").with_id(id);
        panel = panel.with_child(if enabled {
            node.with_interactive()
        } else {
            node
        });
    }

    if let Some(error) = &viewer.refresh_error {
        panel = panel.with_child(
            ViewNode::new("text")
                .with_id("logs-refresh-error")
                .with_text(error),
        );
    }

    if let Some(viewport) = reg.get("logs-content") {
        let anchor = reg
            .iter()
            .filter_map(|(id, bounds)| {
                let index = id.strip_prefix("logs-row-")?.parse::<usize>().ok()?;
                (bounds.origin.y >= viewport.origin.y && bounds.bottom() <= viewport.bottom())
                    .then_some((index, bounds))
            })
            .min_by_key(|(index, _)| *index);
        if let Some((index, bounds)) = anchor {
            let row = match state {
                State::Ready(snapshot) if !snapshot.text.is_empty() => snapshot
                    .rows
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or_default(),
                _ => message,
            };
            let mut node = ViewNode::new("text")
                .with_id("logs-visible-anchor")
                .with_text(row)
                .with_state(serde_json::json!({"index":index}));
            node.bounds = Some(Rect::from_gpui(bounds));
            panel = panel.with_child(node);
        }
    }

    if let Some(state) = &mut panel.state {
        state["following"] = serde_json::json!(viewer.following);
        state["pending"] = serde_json::json!(viewer.pending.is_some());
        state["refresh_error"] = serde_json::json!(viewer.refresh_error);
        state["completed_reads"] = serde_json::json!(observation.completed_reads);
        state["poll_ticks"] = serde_json::json!(observation.poll_ticks);
        state["active_read"] = serde_json::json!(observation.active_read);
        state["active_read_manual"] = serde_json::json!(observation.active_read_manual);
        state["manual_queued"] = serde_json::json!(observation.manual_queued);
        state["painted_rows"] =
            serde_json::json!(reg.keys().filter(|id| id.starts_with("logs-row-")).count());
        state["display_rows"] = serde_json::json!(match &viewer.state {
            State::Ready(snapshot) if !snapshot.text.is_empty() => snapshot.rows.len(),
            _ => 1,
        });
    }

    fn attach(node: &mut ViewNode, reg: &HashMap<String, Bounds<Pixels>>) {
        if let Some(bounds) = node.id.as_ref().and_then(|id| reg.get(id)) {
            node.bounds = Some(Rect::from_gpui(bounds));
        }
        for child in &mut node.children {
            attach(child, reg);
        }
    }
    let mut root = ViewNode::new("window")
        .with_id("logs-root")
        .with_state(serde_json::json!({"view_revision":revision, "window_generation":observation.generation}))
        .with_child(panel);
    attach(&mut root, reg);
    root
}
