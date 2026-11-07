use super::commit_canvas::{CanvasInteraction, canvas_point};
use crate::app_state::GitMasterApp;
use gpui::*;

impl GitMasterApp {
    pub(super) fn canvas_control(&mut self, action: &str, window: &Window) {
        let Some(layout) = self.commit_canvas_layout.as_ref() else {
            return;
        };
        let state = self
            .commit_canvas_states
            .entry(layout.repository_path.clone())
            .or_default();
        match action {
            "canvas-reset" => {
                *state = Default::default();
                state.ensure_layout(layout);
            }
            "canvas-zoom-in" => state.zoom = (state.zoom * 1.2).min(1.75),
            "canvas-zoom-out" => state.zoom = (state.zoom / 1.2).max(f32::MIN_POSITIVE),
            "canvas-head" => {
                let head = self.detail.as_ref().and_then(|d| d.head_oid.as_ref());
                if let Some(node) = layout
                    .nodes
                    .iter()
                    .find(|n| n.id.lane_id == "main" && Some(&n.id.commit_id) == head)
                {
                    let p = state.position_for(node);
                    state.pan.x = 36.0 - p.x * state.zoom;
                    state.pan.y = 92.0 - p.y * state.zoom;
                }
            }
            "canvas-fit" => {
                let viewport = self
                    .canvas_viewport_size
                    .unwrap_or_else(|| window.viewport_size());
                state.fit_content(layout, viewport.width.into(), viewport.height.into());
            }
            _ => {}
        }
        self.commit_canvas_interaction = None;
    }

    pub(super) fn update_commit_canvas_drag(&mut self, event: &MouseMoveEvent) -> bool {
        if !event.dragging() {
            return self.commit_canvas_interaction.take().is_some();
        }
        let Some(interaction) = self.commit_canvas_interaction.take() else {
            return false;
        };
        let pointer = canvas_point(event.position);
        self.commit_canvas_interaction = match interaction {
            CanvasInteraction::Pan {
                repository_path,
                last_pointer,
            } => {
                if let Some(state) = self.commit_canvas_states.get_mut(&repository_path) {
                    state.pan.x += pointer.x - last_pointer.x;
                    state.pan.y += pointer.y - last_pointer.y;
                }
                Some(CanvasInteraction::Pan {
                    repository_path,
                    last_pointer: pointer,
                })
            }
            CanvasInteraction::DragNode {
                repository_path,
                node,
                last_pointer,
            } => {
                if let Some(state) = self.commit_canvas_states.get_mut(&repository_path) {
                    let zoom = state.zoom;
                    if let Some(position) = state.node_positions.get_mut(&node) {
                        position.x += (pointer.x - last_pointer.x) / zoom;
                        position.y += (pointer.y - last_pointer.y) / zoom;
                    }
                }
                Some(CanvasInteraction::DragNode {
                    repository_path,
                    node,
                    last_pointer: pointer,
                })
            }
        };
        true
    }

    pub(super) fn zoom_commit_canvas(&mut self, event: &ScrollWheelEvent) {
        let Some(repository_path) = self
            .commit_canvas_layout
            .as_ref()
            .map(|layout| layout.repository_path.clone())
        else {
            return;
        };
        let Some(state) = self.commit_canvas_states.get_mut(&repository_path) else {
            return;
        };
        let delta: f32 = event.delta.pixel_delta(px(16.0)).y.into();
        let factor = (-delta * 0.0025).exp();
        state.zoom = (state.zoom * factor).clamp(f32::MIN_POSITIVE, 1.75);
    }
}
