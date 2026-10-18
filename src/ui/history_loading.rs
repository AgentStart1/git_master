use std::collections::HashMap;

use gpui::*;

use super::commit_canvas::{self, CanvasNodeKind, CommitCanvasLayout};
use crate::app_state::GitMasterApp;

const HISTORY_BATCH: usize = 200;

/// Keep existing nodes fixed even when expanding another branch inserts commits
/// earlier in the combined topological order. Append new rows below old content.
pub(super) fn stabilize_history_layout(
    previous: &CommitCanvasLayout,
    next: &mut CommitCanvasLayout,
) {
    let positions = previous
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.default_position))
        .collect::<HashMap<_, _>>();
    let lane_offsets = next
        .lanes
        .iter()
        .filter_map(|lane| {
            previous
                .lanes
                .iter()
                .find(|old| old.id == lane.id)
                .map(|old| (lane.id.clone(), old.x - lane.x))
        })
        .collect::<HashMap<_, _>>();
    for lane in &mut next.lanes {
        if let Some(offset) = lane_offsets.get(&lane.id) {
            lane.x += offset;
        }
    }
    let mut bottom = previous
        .nodes
        .iter()
        .map(|node| node.default_position.y + node.height + 32.0)
        .fold(92.0, f32::max);
    for node in &mut next.nodes {
        if let Some(position) = positions.get(&node.id) {
            node.default_position = *position;
        } else if node.kind != CanvasNodeKind::Head {
            node.default_position.x += lane_offsets.get(&node.id.lane_id).copied().unwrap_or(0.0);
            node.default_position.y = bottom;
            bottom += node.height + 32.0;
        }
    }
    let commits = next
        .nodes
        .iter()
        .filter(|node| node.kind == CanvasNodeKind::Commit)
        .map(|node| (node.id.commit_id.clone(), node.default_position))
        .collect::<HashMap<_, _>>();
    for node in &mut next.nodes {
        if node.kind == CanvasNodeKind::Head
            && !positions.contains_key(&node.id)
            && let Some(commit) = node.id.commit_id.strip_prefix("head:")
            && let Some(position) = commits.get(commit)
        {
            node.default_position.x = position.x - node.width - 28.0;
            node.default_position.y = bottom;
            bottom += node.height + 32.0;
        }
    }
}

impl GitMasterApp {
    pub(crate) fn cancel_history_load(&mut self) {
        self.history_host.cancel();
        self.history_task = None;
    }

    pub(super) fn load_more_history(&mut self, cx: &mut Context<'_, Self>) {
        if self.busy || self.loading_detail {
            return;
        }
        let Some(previous) = self
            .commit_canvas_layout
            .clone()
            .filter(|layout| layout.has_more)
        else {
            return;
        };
        let Some(request) = self.history_host.start() else {
            return;
        };
        let path = previous.repository_path.clone();
        let branches = self.log_visible_branches.clone();
        let selection = self.selected.clone();
        let navigation = self.navigation_revision;
        let limit = previous.history_limit.saturating_add(HISTORY_BATCH);
        self.history_task = Some(cx.spawn(async move |entity, cx| {
            let expected_path = path.clone();
            let expected_branches = branches.clone();
            let expected_limit = previous.history_limit;
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut layout = commit_canvas::load_layout_for_branches(
                        &path,
                        &branches.into_iter().collect::<Vec<_>>(),
                        limit,
                    )?;
                    stabilize_history_layout(&previous, &mut layout);
                    Some(layout)
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    if !this.history_host.finish(request) {
                        return;
                    }
                    if this.busy
                        || this.loading_detail
                        || this.navigation_revision != navigation
                        || this.selected != selection
                        || this.log_visible_branches != expected_branches
                        || !this.commit_canvas_layout.as_ref().is_some_and(|layout| {
                            layout.repository_path == expected_path
                                && layout.history_limit == expected_limit
                        })
                    {
                        cx.notify();
                        return;
                    }
                    match result {
                        Some(layout) => {
                            this.commit_canvas_states
                                .entry(layout.repository_path.clone())
                                .or_default()
                                .ensure_layout(&layout);
                            this.log_entries = layout.list.entries.clone();
                            this.commit_canvas_layout = Some(layout);
                            this.set_status("More history loaded");
                        }
                        None => {
                            this.history_host.failed();
                            this.set_status("Could not load more history; retry Load more");
                        }
                    }
                    cx.notify();
                })
                .ok();
        }));
        cx.notify();
    }
}
