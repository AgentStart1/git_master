use super::canvas_paint::paint_edges;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::*;

use crate::app_state::GitMasterApp;
use crate::git_ops;
use crate::models::{CommitLaneStatus, LogEntry, RepoInfo};
use crate::ui::theme;

pub const NODE_WIDTH: f32 = 236.0;
pub const NODE_HEIGHT: f32 = 64.0;
pub const LANE_SPACING: f32 = 380.0;
pub(super) use super::canvas_layout::branch_columns;
pub use super::canvas_layout::build_layout;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CanvasPoint {
    pub x: f32,
    pub y: f32,
}

impl CanvasPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CanvasNodeId {
    pub lane_id: String,
    pub commit_id: String,
}

#[derive(Clone, Debug)]
pub struct CanvasNode {
    pub id: CanvasNodeId,
    pub entry: Option<LogEntry>,
    pub head_labels: Vec<String>,
    pub kind: CanvasNodeKind,
    pub width: f32,
    pub height: f32,
    pub default_position: CanvasPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasNodeKind {
    Commit,
    Placeholder,
    Head,
}

impl CanvasNode {
    pub fn short_hash(&self) -> &str {
        self.entry
            .as_ref()
            .map(|entry| entry.hash.as_str())
            .unwrap_or_else(|| &self.id.commit_id[..7.min(self.id.commit_id.len())])
    }

    pub fn is_placeholder(&self) -> bool {
        self.kind == CanvasNodeKind::Placeholder
    }

    pub fn is_head(&self) -> bool {
        self.kind == CanvasNodeKind::Head
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasEdgeKind {
    Reference,
    Parent,
    Submodule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasEdge {
    pub from: CanvasNodeId,
    pub to: CanvasNodeId,
    pub kind: CanvasEdgeKind,
}

#[derive(Clone, Debug)]
pub struct CanvasLane {
    pub id: String,
    pub name: String,
    pub relative_path: Option<PathBuf>,
    pub status: CommitLaneStatus,
    pub x: f32,
}

#[derive(Clone, Debug)]
pub struct CommitCanvasLayout {
    pub repository_path: PathBuf,
    pub lanes: Vec<CanvasLane>,
    pub nodes: Vec<CanvasNode>,
    pub edges: Vec<CanvasEdge>,
    pub list: crate::ui::log_list::LogListLayout,
    pub history_limit: usize,
    pub has_more: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogViewMode {
    List,
    Canvas,
}

#[derive(Clone, Debug)]
pub struct CommitCanvasState {
    pub pan: CanvasPoint,
    pub zoom: f32,
    pub node_positions: HashMap<CanvasNodeId, CanvasPoint>,
    default_positions: HashMap<CanvasNodeId, CanvasPoint>,
}

impl Default for CommitCanvasState {
    fn default() -> Self {
        Self {
            pan: CanvasPoint::new(0.0, 0.0),
            zoom: 1.0,
            node_positions: HashMap::new(),
            default_positions: HashMap::new(),
        }
    }
}

impl CommitCanvasState {
    pub fn fit_content(&mut self, layout: &CommitCanvasLayout, width: f32, height: f32) {
        if layout.nodes.is_empty() {
            return;
        }
        let mut minimum = CanvasPoint::new(f32::MAX, f32::MAX);
        let mut maximum = CanvasPoint::new(f32::MIN, f32::MIN);
        for node in &layout.nodes {
            let p = self.position_for(node);
            minimum.x = minimum.x.min(p.x);
            minimum.y = minimum.y.min(p.y);
            maximum.x = maximum.x.max(p.x + node.width);
            maximum.y = maximum.y.max(p.y + node.height);
        }
        self.zoom = ((width - 48.0).max(1.0) / (maximum.x - minimum.x).max(1.0))
            .min((height - 112.0).max(1.0) / (maximum.y - minimum.y).max(1.0))
            .min(1.75);
        self.pan = CanvasPoint::new(24.0 - minimum.x * self.zoom, 62.0 - minimum.y * self.zoom);
    }

    pub fn ensure_layout(&mut self, layout: &CommitCanvasLayout) {
        for node in &layout.nodes {
            let old_default = self
                .default_positions
                .insert(node.id.clone(), node.default_position);
            let position = self
                .node_positions
                .entry(node.id.clone())
                .or_insert(node.default_position);
            if old_default == Some(*position) {
                *position = node.default_position;
            }
        }
    }

    pub fn position_for(&self, node: &CanvasNode) -> CanvasPoint {
        self.node_positions
            .get(&node.id)
            .copied()
            .unwrap_or(node.default_position)
    }
}

#[derive(Clone, Debug)]
pub enum CanvasInteraction {
    Pan {
        repository_path: PathBuf,
        last_pointer: CanvasPoint,
    },
    DragNode {
        repository_path: PathBuf,
        node: CanvasNodeId,
        last_pointer: CanvasPoint,
    },
}

/// Loads Git data and prepares an immutable, UI-ready layout. Call this only
/// from a background executor.
pub fn load_layout(repo_info: &RepoInfo, limit: usize) -> Option<CommitCanvasLayout> {
    git_ops::get_commit_graph_page(repo_info, &[], limit).map(|(graph, has_more)| {
        let mut layout = build_layout(&graph);
        layout.history_limit = limit;
        layout.has_more = has_more;
        layout
    })
}

/// Discovers a repository and prepares its graph layout. Call this only from
/// a background executor when the selected repository is not already loaded.
pub fn load_layout_for_path(
    repo_path: &std::path::Path,
    limit: usize,
) -> Option<CommitCanvasLayout> {
    let repo_info = git_ops::build_repo_info(repo_path)?;
    load_layout(&repo_info, limit)
}

/// Builds a graph whose main lane contains the histories reachable from the
/// selected local branches. An empty selection preserves the current-HEAD view.
pub fn load_layout_for_branches(
    repo_path: &std::path::Path,
    branches: &[String],
    limit: usize,
) -> Option<CommitCanvasLayout> {
    let repo_info = git_ops::build_repo_info(repo_path)?;
    git_ops::get_commit_graph_page(&repo_info, branches, limit).map(|(graph, has_more)| {
        let mut layout = build_layout(&graph);
        layout.history_limit = limit;
        layout.has_more = has_more;
        layout
    })
}

impl GitMasterApp {
    pub fn render_commit_canvas(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let Some(layout) = self.commit_canvas_layout.as_ref().cloned() else {
            return div()
                .flex()
                .flex_grow()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(rgb(theme::palette().text_subtle))
                .child("暂无提交关系图")
                .into_any_element();
        };
        let state = self
            .commit_canvas_states
            .get(&layout.repository_path)
            .cloned()
            .unwrap_or_default();
        let repository_path = layout.repository_path.clone();

        let positions = layout
            .nodes
            .iter()
            .map(|node| (node.id.clone(), state.position_for(node)))
            .collect::<HashMap<_, _>>();
        let cards = layout
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.clone(),
                    super::canvas_routes::CardRect::new(
                        state.position_for(node),
                        node.width,
                        node.height,
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        let routes = self
            .canvas_route_cache
            .borrow_mut()
            .routes(&layout.edges, cards);
        let unloaded = layout
            .nodes
            .iter()
            .filter(|n| n.is_placeholder())
            .map(|n| n.id.clone())
            .collect::<HashSet<_>>();
        let edges = layout.edges.clone();
        let paint_state = state.clone();

        // An overview fits loaded content; it must not immediately move its
        // frontier by treating the entire visible history as a scroll-to-end.
        let auto_load = layout.has_more && self.history_host.can_auto_load() && state.zoom >= 0.35;
        let frontier = layout
            .nodes
            .iter()
            .filter(|node| node.kind == CanvasNodeKind::Commit)
            .map(|node| state.pan.y + (state.position_for(node).y + node.height) * state.zoom)
            .fold(0.0, f32::max);
        let entity = cx.entity().downgrade();
        let expected_path = layout.repository_path.clone();
        let expected_limit = layout.history_limit;
        let edge_canvas = canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let size = bounds.size;
                let measure_entity = entity.clone();
                cx.defer(move |cx| {
                    measure_entity
                        .update(cx, |this, cx| {
                            this.canvas_viewport_size = Some(size);
                            if std::mem::take(&mut this.canvas_reveal_selection)
                                && let (Some((path, hash)), Some(layout)) =
                                    (&this.selected_commit, &this.commit_canvas_layout)
                            {
                                if let Some(node) = layout.nodes.iter().find(|n| {
                                    n.id.commit_id == *hash
                                        && layout
                                            .lanes
                                            .iter()
                                            .find(|l| l.id == n.id.lane_id)
                                            .is_some_and(|l| {
                                                layout.repository_path.join(
                                                    l.relative_path
                                                        .as_deref()
                                                        .unwrap_or(std::path::Path::new("")),
                                                ) == *path
                                            })
                                }) {
                                    let state = this
                                        .commit_canvas_states
                                        .entry(layout.repository_path.clone())
                                        .or_default();
                                    let p = state.position_for(node);
                                    let x = state.pan.x + p.x * state.zoom;
                                    let y = state.pan.y + p.y * state.zoom;
                                    let right =
                                        (f32::from(size.width) - node.width * state.zoom - 18.0)
                                            .max(18.0);
                                    let bottom =
                                        (f32::from(size.height) - node.height * state.zoom - 80.0)
                                            .max(75.0);
                                    state.pan.x += x.clamp(18.0, right) - x;
                                    state.pan.y += y.clamp(75.0, bottom) - y;
                                    cx.notify();
                                }
                            }
                        })
                        .ok();
                });
                if auto_load && frontier >= 0.0 && frontier <= f32::from(bounds.size.height) + 256.0
                {
                    let entity = entity.clone();
                    let expected_path = expected_path.clone();
                    cx.defer(move |cx| {
                        entity
                            .update(cx, |this, cx| {
                                if this.history_host.can_auto_load()
                                    && this.commit_canvas_layout.as_ref().is_some_and(|layout| {
                                        layout.repository_path == expected_path
                                            && layout.history_limit == expected_limit
                                    })
                                {
                                    this.load_more_history(cx);
                                }
                            })
                            .ok();
                    });
                }

                paint_edges(bounds, &paint_state, &edges, &routes, &unloaded, window);
            },
        )
        .absolute()
        .size_full();

        let overview = state.zoom < 0.35;
        let header_width = if overview {
            ((self
                .canvas_viewport_size
                .map(|s| f32::from(s.width))
                .unwrap_or(800.0)
                - 48.0)
                / layout.lanes.len().max(1) as f32)
                .min(NODE_WIDTH)
                .max(1.0)
        } else {
            NODE_WIDTH
        };
        let lane_headers = layout.lanes.iter().enumerate().map(|(index, lane)| {
            let left = if overview {
                24.0 + index as f32 * header_width
            } else {
                state.pan.x + lane.x * state.zoom
            };
            let submodule_lane = lane.relative_path.is_some();
            let title = lane.name.clone();
            let path = lane
                .relative_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| layout.repository_path.display().to_string());
            let (status_text, status_color) = match lane.status {
                CommitLaneStatus::Available => (None, rgb(theme::palette().green)),
                CommitLaneStatus::Uninitialized => (Some("未初始化"), rgb(theme::palette().yellow)),
                CommitLaneStatus::Unavailable => (Some("仓库不可用"), rgb(theme::palette().red)),
            };

            div()
                .id(ElementId::Name(format!("canvas-lane-{}", lane.id).into()))
                .tooltip(super::tooltip::text(path))
                .absolute()
                .left(px(left))
                .top(px(if overview { 7.0 } else { state.pan.y + 14.0 }))
                .w(px(header_width))
                .px(px(10.0))
                .py(px(7.0))
                .rounded(px(3.0))
                .border_1()
                .border_color(rgb(theme::palette().bg_base))
                .bg(rgb(theme::palette().bg_base))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(super::icons::icon("folder", 20.0).flex_shrink_0())
                        .text_size(px(18.0))
                        .text_color(if lane.id == "main" || lane.id == "heads" {
                            rgb(theme::palette().accent)
                        } else {
                            rgb(theme::palette().text_subtle)
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(title),
                        )
                        .children(
                            (lane.status == CommitLaneStatus::Uninitialized && !overview)
                                .then(|| self.render_canvas_initializer(&layout, lane, cx)),
                        ),
                )
                .children((submodule_lane || status_text.is_some()).then(|| {
                    div()
                        .text_xs()
                        .text_color(if status_text.is_some() {
                            status_color
                        } else {
                            rgb(theme::palette().text_subtle)
                        })
                        .child(match (submodule_lane, status_text) {
                            (true, Some(status)) => format!("子模块 · {status}"),
                            (true, None) => "子模块".into(),
                            (false, Some(status)) => status.into(),
                            (false, None) => String::new(),
                        })
                }))
        });

        let nodes = layout.nodes.iter().map(|node| {
            let world_position = positions
                .get(&node.id)
                .copied()
                .unwrap_or(node.default_position);
            let left = state.pan.x + world_position.x * state.zoom;
            let top = state.pan.y + world_position.y * state.zoom;
            let node_id = node.id.clone();
            let drag_repository_path = repository_path.clone();
            let commit_path = layout
                .lanes
                .iter()
                .find(|lane| lane.id == node.id.lane_id)
                .and_then(|lane| lane.relative_path.as_ref())
                .map(|relative| repository_path.join(relative))
                .unwrap_or_else(|| repository_path.clone());
            let commit_hash = node
                .id
                .commit_id
                .strip_prefix("head:")
                .unwrap_or(&node.id.commit_id)
                .to_owned();
            let placeholder = node.is_placeholder();
            let uninitialized = placeholder
                && layout.lanes.iter().any(|lane| {
                    lane.id == node.id.lane_id && lane.status == CommitLaneStatus::Uninitialized
                });
            let is_head = node.is_head();
            let head_labels = node.head_labels.clone();
            let selected = self
                .selected_commit
                .as_ref()
                .is_some_and(|(path, hash)| path == &commit_path && hash == &commit_hash);
            let border_color = if selected {
                rgb(theme::palette().accent)
            } else if placeholder {
                rgb(theme::palette().yellow)
            } else if is_head {
                rgb(theme::palette().accent)
            } else if node.id.lane_id == "main" {
                rgb(theme::palette().border)
            } else {
                rgb(theme::palette().bg_overlay)
            };

            let content = if is_head {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0 * state.zoom))
                    .child(
                        div()
                            .text_size(px(11.0 * state.zoom))
                            .line_height(px(12.0 * state.zoom))
                            .text_color(rgb(theme::palette().accent))
                            .child(if head_labels.iter().any(|label| label == "HEAD") {
                                "HEAD"
                            } else {
                                "分支引用"
                            }),
                    )
                    .child(
                        div().flex().flex_col().gap(px(2.0 * state.zoom)).children(
                            head_labels
                                .iter()
                                .filter(|label| label.as_str() != "HEAD")
                                .map(|label| {
                                    div()
                                        .id(ElementId::Name(
                                            format!("canvas-head-label-{label}").into(),
                                        ))
                                        .text_size(px(11.0 * state.zoom))
                                        .line_height(px(14.0 * state.zoom))
                                        .text_color(rgb(theme::palette().accent))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .tooltip(super::tooltip::text(label.clone()))
                                        .child(label.clone())
                                }),
                        ),
                    )
                    .into_any_element()
            } else if let Some(entry) = node.entry.as_ref() {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0 * state.zoom))
                    .child(
                        div()
                            .text_size(px(17.0 * state.zoom))
                            .line_height(px(22.0 * state.zoom))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(entry.message.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(14.0 * state.zoom))
                            .line_height(px(20.0 * state.zoom))
                            .text_color(rgb(theme::palette().text_subtle))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(format!(
                                "{} · {} · {}",
                                entry.hash,
                                entry.author,
                                super::log_list::relative_date(&entry.date)
                            )),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.0 * state.zoom))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0 * state.zoom))
                            .text_size(px(12.0 * state.zoom))
                            .text_color(rgb(theme::palette().yellow))
                            .child(
                                super::icons::icon(
                                    if uninitialized {
                                        "alert-circle"
                                    } else {
                                        "info-circle"
                                    },
                                    16.0 * state.zoom,
                                )
                                .flex_shrink_0(),
                            )
                            .child(node.short_hash().to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(14.0 * state.zoom))
                            .child(if uninitialized {
                                "子模块未初始化"
                            } else {
                                "引用的提交尚未加载"
                            }),
                    )
                    .into_any_element()
            };

            let id = format!("commit-node-{}-{}", node.id.lane_id, node.id.commit_id);
            let element = div()
                .id(ElementId::Name(id.clone().into()))
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(node.width * state.zoom))
                .h(px(node.height * state.zoom))
                .p(px(if is_head { 5.0 } else { 9.0 } * state.zoom))
                .rounded(px(6.0 * state.zoom))
                .border(px(state.zoom))
                .border_color(border_color)
                .bg(if selected {
                    rgb(theme::palette().accent_soft)
                } else if placeholder {
                    rgb(theme::palette().bg_elevated)
                } else {
                    rgb(theme::palette().bg_surface)
                })
                .cursor_move()
                .child(content)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                        this.commit_press = Some(super::commit_details::CommitPress {
                            path: commit_path.clone(),
                            hash: commit_hash.clone(),
                            start: event.position,
                            dragged: false,
                        });
                        this.commit_canvas_interaction = Some(CanvasInteraction::DragNode {
                            repository_path: drag_repository_path.clone(),
                            node: node_id.clone(),
                            last_pointer: canvas_point(event.position),
                        });
                        cx.stop_propagation();
                    }),
                );
            self.track(&id, element)
        });

        let canvas = div()
            .id("commit-canvas")
            .relative()
            .flex()
            .flex_grow()
            .overflow_hidden()
            .bg(rgb(theme::palette().bg_base))
            .cursor_move()
            .child(edge_canvas)
            .children(lane_headers)
            .children(nodes)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _window, _cx| {
                    this.commit_canvas_interaction = Some(CanvasInteraction::Pan {
                        repository_path: repository_path.clone(),
                        last_pointer: canvas_point(event.position),
                    });
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if let Some(press) = this.commit_press.as_mut() {
                    press.update(event.position);
                }
                if this.update_commit_canvas_drag(event) {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, _window, cx| {
                    this.commit_canvas_interaction = None;
                    if let Some(mut press) = this.commit_press.take() {
                        press.update(event.position);
                        if !press.dragged {
                            this.open_commit_details(press.path, press.hash, cx);
                        }
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _event, _window, _cx| {
                    this.commit_press = None;
                    this.commit_canvas_interaction = None;
                }),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
                this.zoom_commit_canvas(event);
                cx.stop_propagation();
                cx.notify();
            }))
            .into_any_element();
        self.track("commit-canvas", canvas)
    }
}

pub(super) fn canvas_point(point: Point<Pixels>) -> CanvasPoint {
    CanvasPoint::new(point.x.into(), point.y.into())
}

#[cfg(test)]
#[path = "commit_canvas_tests.rs"]
mod tests;
