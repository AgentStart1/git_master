use super::commit_canvas::{CanvasInteraction, canvas_point};
use crate::app_state::GitMasterApp;
use gpui::*;

impl GitMasterApp {
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
        state.zoom = (state.zoom * factor).clamp(0.75, 1.75);
    }
}
