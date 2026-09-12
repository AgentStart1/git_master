use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::*;

use crate::app_state::GitMasterApp;
use crate::git_ops;
use crate::models::{CommitGraph, CommitLaneStatus, LogEntry, RepoInfo};
use crate::ui::theme;

pub const NODE_WIDTH: f32 = 236.0;
pub const NODE_HEIGHT: f32 = 96.0;
pub const LANE_SPACING: f32 = 316.0;
const LANE_LEFT: f32 = 52.0;
const FIRST_NODE_TOP: f32 = 92.0;
const ROW_SPACING: f32 = 128.0;

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
    Parent,
    Submodule,
    Head,
}

#[derive(Clone, Debug)]
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
            pan: CanvasPoint::new(18.0, 16.0),
            zoom: 1.0,
            node_positions: HashMap::new(),
            default_positions: HashMap::new(),
        }
    }
}

impl CommitCanvasState {
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
    git_ops::get_commit_graph(repo_info, limit).map(|graph| build_layout(&graph))
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
    git_ops::get_commit_graph_for_branches(&repo_info, branches, limit)
        .map(|graph| build_layout(&graph))
}

pub fn build_layout(graph: &CommitGraph) -> CommitCanvasLayout {
    let columns: HashMap<_, _> = graph
        .lanes
        .iter()
        .map(|lane| (lane.id.clone(), branch_columns(&lane.entries)))
        .collect();
    let mut next_x = LANE_LEFT;
    let mut lanes = graph
        .lanes
        .iter()
        .map(|lane| {
            let x = next_x;
            next_x +=
                (columns[&lane.id].iter().copied().max().unwrap_or(0) + 1) as f32 * LANE_SPACING;
            CanvasLane {
                id: lane.id.clone(),
                name: lane.name.clone(),
                relative_path: lane.relative_path.clone(),
                status: lane.status.clone(),
                x,
            }
        })
        .collect::<Vec<_>>();
    let head_lane_x = next_x;
    lanes.push(CanvasLane {
        id: "heads".to_string(),
        name: "Branch HEADs".to_string(),
        relative_path: None,
        status: CommitLaneStatus::Available,
        x: head_lane_x,
    });

    let lane_x = lanes
        .iter()
        .map(|lane| (lane.id.clone(), lane.x))
        .collect::<HashMap<_, _>>();
    let mut nodes = Vec::new();
    let mut known_nodes = HashSet::new();
    let mut head_edges = Vec::new();
    let mut next_head_top = FIRST_NODE_TOP;

    for lane in &graph.lanes {
        let x = lane_x.get(&lane.id).copied().unwrap_or(LANE_LEFT);
        for (index, entry) in lane.entries.iter().enumerate() {
            let commit_y = FIRST_NODE_TOP + index as f32 * ROW_SPACING;
            let id = CanvasNodeId {
                lane_id: lane.id.clone(),
                commit_id: entry.full_hash.clone(),
            };
            let head_labels = (lane.id == "main")
                .then(|| graph.head_labels.get(&entry.full_hash).cloned())
                .flatten()
                .unwrap_or_default();
            if !head_labels.is_empty() {
                let head_top = commit_y.max(next_head_top);
                let head_height = head_node_height(head_labels.len());
                let head_id = CanvasNodeId {
                    lane_id: "heads".to_string(),
                    commit_id: format!("head:{}", entry.full_hash),
                };
                nodes.push(CanvasNode {
                    id: head_id.clone(),
                    entry: None,
                    head_labels,
                    kind: CanvasNodeKind::Head,
                    width: NODE_WIDTH,
                    height: head_height,
                    default_position: CanvasPoint::new(head_lane_x, head_top),
                });
                next_head_top = head_top + head_height + 8.0;
                head_edges.push(CanvasEdge {
                    from: head_id,
                    to: id.clone(),
                    kind: CanvasEdgeKind::Head,
                });
            }
            known_nodes.insert(id.clone());
            nodes.push(CanvasNode {
                id,
                entry: Some(entry.clone()),
                head_labels: Vec::new(),
                kind: CanvasNodeKind::Commit,
                width: NODE_WIDTH,
                height: NODE_HEIGHT,
                default_position: CanvasPoint::new(
                    x + columns[&lane.id][index] as f32 * LANE_SPACING,
                    commit_y,
                ),
            });
        }
    }

    let default_positions = nodes
        .iter()
        .map(|node| (node.id.clone(), node.default_position))
        .collect::<HashMap<_, _>>();
    let mut edges = head_edges;
    for lane in &graph.lanes {
        for entry in &lane.entries {
            let from = CanvasNodeId {
                lane_id: lane.id.clone(),
                commit_id: entry.full_hash.clone(),
            };
            for parent_hash in &entry.parent_hashes {
                let to = CanvasNodeId {
                    lane_id: lane.id.clone(),
                    commit_id: parent_hash.clone(),
                };
                if known_nodes.contains(&to) {
                    edges.push(CanvasEdge {
                        from: from.clone(),
                        to,
                        kind: CanvasEdgeKind::Parent,
                    });
                }
            }
        }
    }

    for link in &graph.submodule_links {
        let from = CanvasNodeId {
            lane_id: "main".to_string(),
            commit_id: link.main_commit.clone(),
        };
        if !known_nodes.contains(&from) {
            continue;
        }
        let to = CanvasNodeId {
            lane_id: link.submodule_lane.clone(),
            commit_id: link.submodule_commit.clone(),
        };
        if !known_nodes.contains(&to) {
            let Some(x) = lane_x.get(&link.submodule_lane).copied() else {
                continue;
            };
            let source_y = default_positions
                .get(&from)
                .map(|position| position.y)
                .unwrap_or(FIRST_NODE_TOP);
            known_nodes.insert(to.clone());
            nodes.push(CanvasNode {
                id: to.clone(),
                entry: None,
                head_labels: Vec::new(),
                kind: CanvasNodeKind::Placeholder,
                width: NODE_WIDTH,
                height: NODE_HEIGHT,
                default_position: CanvasPoint::new(x, source_y),
            });
        }
        edges.push(CanvasEdge {
            from,
            to,
            kind: CanvasEdgeKind::Submodule,
        });
    }

    CommitCanvasLayout {
        repository_path: graph.repository_path.clone(),
        lanes,
        nodes,
        edges,
    }
}

fn head_node_height(label_count: usize) -> f32 {
    // Extra space absorbs platform text line-height rounding and card padding.
    62.0 + label_count as f32 * 30.0
}

/// Reserve separate columns for active ancestry paths, retaining the first
/// parent's column and joining paths when they reach a shared ancestor.
fn branch_columns(entries: &[LogEntry]) -> Vec<usize> {
    let mut active: Vec<Option<String>> = Vec::new();
    let mut result = Vec::new();
    for entry in entries {
        let column = active
            .iter()
            .position(|id| id.as_deref() == Some(&entry.full_hash))
            .unwrap_or_else(|| {
                let free = active
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(active.len());
                if free == active.len() {
                    active.push(None);
                }
                free
            });
        result.push(column);
        active[column] = None;
        for (index, parent) in entry.parent_hashes.iter().enumerate() {
            if active.iter().any(|id| id.as_ref() == Some(parent)) {
                continue;
            }
            let slot = if index == 0 {
                column
            } else {
                active
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(active.len())
            };
            if slot == active.len() {
                active.push(None);
            }
            active[slot] = Some(parent.clone());
        }
    }
    result
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
                .text_color(rgb(theme::TEXT_SUBTLE))
                .child("Commit graph is unavailable.")
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
        let edge_positions = positions.clone();
        let node_sizes = layout
            .nodes
            .iter()
            .map(|node| (node.id.clone(), (node.width, node.height)))
            .collect::<HashMap<_, _>>();
        let edges = layout.edges.clone();
        let paint_state = state.clone();

        let edge_canvas = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                paint_grid(bounds, &paint_state, window);
                paint_edges(
                    bounds,
                    &paint_state,
                    &edges,
                    &edge_positions,
                    &node_sizes,
                    window,
                );
            },
        )
        .absolute()
        .size_full();

        let lane_headers = layout.lanes.iter().map(|lane| {
            let left = state.pan.x + lane.x * state.zoom;
            let title = match lane.id.as_str() {
                "main" => format!("MAIN  {}", lane.name),
                "heads" => lane.name.clone(),
                _ => format!("SUBMODULE  {}", lane.name),
            };
            let path = lane
                .relative_path
                .as_ref()
                .map(|path| path.display().to_string());
            let (status_text, status_color) = match lane.status {
                CommitLaneStatus::Available => (None, rgb(theme::GREEN)),
                CommitLaneStatus::Uninitialized => (Some("Not initialized"), rgb(theme::YELLOW)),
                CommitLaneStatus::Unavailable => (Some("Repository unavailable"), rgb(theme::RED)),
            };

            div()
                .absolute()
                .left(px(left))
                .top(px(state.pan.y + 14.0))
                .w(px(NODE_WIDTH))
                .px(px(10.0))
                .py(px(7.0))
                .rounded(px(5.0))
                .border_1()
                .border_color(rgb(theme::BG_OVERLAY))
                .bg(rgb(theme::BG_SURFACE))
                .child(
                    div()
                        .text_xs()
                        .text_color(if lane.id == "main" || lane.id == "heads" {
                            rgb(theme::ACCENT)
                        } else {
                            rgb(theme::TEXT_SUBTLE)
                        })
                        .child(title),
                )
                .children(path.map(|path| {
                    div()
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SUBTLE))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(path)
                }))
                .children(
                    status_text
                        .map(|status| div().text_xs().text_color(status_color).child(status)),
                )
                .children((lane.status == CommitLaneStatus::Uninitialized).then(|| {
                    let path = layout.repository_path.clone();
                    let relative = lane.relative_path.clone();
                    div()
                        .id(ElementId::Name(format!("canvas-init-{}", lane.id).into()))
                        .mt(px(6.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(rgb(theme::ACCENT))
                        .text_color(rgb(theme::BG_BASE))
                        .text_xs()
                        .cursor_pointer()
                        .child(if self.busy {
                            "Working…"
                        } else {
                            "Initialize Submodule"
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_, _, _, cx| cx.stop_propagation()),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(relative) = relative.clone() {
                                this.init_canvas_submodule(path.clone(), relative, cx);
                            }
                            cx.stop_propagation();
                        }))
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
            let placeholder = node.is_placeholder();
            let is_head = node.is_head();
            let head_labels = node.head_labels.clone();
            let border_color = if placeholder {
                rgb(theme::YELLOW)
            } else if is_head {
                rgb(theme::GREEN)
            } else if node.id.lane_id == "main" {
                rgb(theme::ACCENT)
            } else {
                rgb(theme::BG_OVERLAY)
            };

            let content =
                if is_head {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(div().text_xs().text_color(rgb(theme::GREEN)).child("HEAD"))
                        .child(div().flex().flex_col().gap(px(4.0)).children(
                            head_labels.iter().map(|label| {
                                div()
                                    .px(px(5.0))
                                    .py(px(2.0))
                                    .rounded(px(3.0))
                                    .bg(rgb(theme::ACCENT))
                                    .text_xs()
                                    .text_color(rgb(theme::BG_BASE))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(label.clone())
                            }),
                        ))
                        .into_any_element()
                } else if let Some(entry) = node.entry.as_ref() {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(theme::YELLOW))
                                        .child(entry.hash.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(theme::TEXT_SUBTLE))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(entry.date.clone()),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(entry.message.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_SUBTLE))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(entry.author.clone()),
                        )
                        .into_any_element()
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(5.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::YELLOW))
                                .child(node.short_hash().to_string()),
                        )
                        .child(div().text_sm().child("Referenced commit not loaded"))
                        .into_any_element()
                };

            div()
                .id(ElementId::Name(
                    format!("commit-node-{}-{}", node.id.lane_id, node.id.commit_id).into(),
                ))
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(node.width))
                .h(px(node.height))
                .p(px(9.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(border_color)
                .bg(if placeholder {
                    rgba(0x25181825)
                } else {
                    rgb(theme::BG_SURFACE)
                })
                .cursor_move()
                .child(content)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                        this.commit_canvas_interaction = Some(CanvasInteraction::DragNode {
                            repository_path: drag_repository_path.clone(),
                            node: node_id.clone(),
                            last_pointer: canvas_point(event.position),
                        });
                        cx.stop_propagation();
                    }),
                )
        });

        div()
            .id("commit-canvas")
            .relative()
            .flex()
            .flex_grow()
            .overflow_hidden()
            .bg(rgb(theme::BG_BASE))
            .cursor_move()
            .child(edge_canvas)
            .children(lane_headers)
            .children(nodes)
            .child(
                div()
                    .absolute()
                    .right(px(12.0))
                    .bottom(px(10.0))
                    .px(px(8.0))
                    .py(px(5.0))
                    .rounded(px(4.0))
                    .bg(rgba(0xcc181825))
                    .text_xs()
                    .text_color(rgb(theme::TEXT_SUBTLE))
                    .child(format!(
                        "Drag nodes • Drag background to pan • Wheel to zoom  {}%",
                        (state.zoom * 100.0).round() as i32
                    )),
            )
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
                if this.update_commit_canvas_drag(event) {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event, _window, _cx| {
                    this.commit_canvas_interaction = None;
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _event, _window, _cx| {
                    this.commit_canvas_interaction = None;
                }),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
                this.zoom_commit_canvas(event);
                cx.stop_propagation();
                cx.notify();
            }))
            .into_any_element()
    }

    fn update_commit_canvas_drag(&mut self, event: &MouseMoveEvent) -> bool {
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

    fn zoom_commit_canvas(&mut self, event: &ScrollWheelEvent) {
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

fn canvas_point(point: Point<Pixels>) -> CanvasPoint {
    CanvasPoint::new(point.x.into(), point.y.into())
}

fn transformed_point(
    bounds: Bounds<Pixels>,
    state: &CommitCanvasState,
    point: CanvasPoint,
) -> Point<Pixels> {
    gpui::point(
        bounds.origin.x + px(state.pan.x + point.x * state.zoom),
        bounds.origin.y + px(state.pan.y + point.y * state.zoom),
    )
}

fn paint_grid(bounds: Bounds<Pixels>, state: &CommitCanvasState, window: &mut Window) {
    let spacing = 32.0 * state.zoom;
    let width: f32 = bounds.size.width.into();
    let height: f32 = bounds.size.height.into();
    let mut builder = PathBuilder::stroke(px(1.0));
    let start_x = state.pan.x.rem_euclid(spacing);
    let start_y = state.pan.y.rem_euclid(spacing);
    let mut x = start_x;
    while x <= width {
        builder.move_to(gpui::point(bounds.origin.x + px(x), bounds.origin.y));
        builder.line_to(gpui::point(
            bounds.origin.x + px(x),
            bounds.origin.y + bounds.size.height,
        ));
        x += spacing;
    }
    let mut y = start_y;
    while y <= height {
        builder.move_to(gpui::point(bounds.origin.x, bounds.origin.y + px(y)));
        builder.line_to(gpui::point(
            bounds.origin.x + bounds.size.width,
            bounds.origin.y + px(y),
        ));
        y += spacing;
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, rgba(0x18313244));
    }
}

fn paint_edges(
    bounds: Bounds<Pixels>,
    state: &CommitCanvasState,
    edges: &[CanvasEdge],
    positions: &HashMap<CanvasNodeId, CanvasPoint>,
    node_sizes: &HashMap<CanvasNodeId, (f32, f32)>,
    window: &mut Window,
) {
    let mut parent_paths = PathBuilder::stroke(px(1.5));
    let mut submodule_paths = PathBuilder::stroke(px(2.0)).dash_array(&[px(7.0), px(5.0)]);
    let mut head_paths = PathBuilder::stroke(px(2.0));

    for edge in edges {
        let (Some(from), Some(to)) = (positions.get(&edge.from), positions.get(&edge.to)) else {
            continue;
        };
        let (from_width, from_height) = node_sizes
            .get(&edge.from)
            .copied()
            .unwrap_or((NODE_WIDTH, NODE_HEIGHT));
        let (to_width, to_height) = node_sizes
            .get(&edge.to)
            .copied()
            .unwrap_or((NODE_WIDTH, NODE_HEIGHT));
        match edge.kind {
            CanvasEdgeKind::Parent => {
                let from_origin = transformed_point(bounds, state, *from);
                let to_origin = transformed_point(bounds, state, *to);
                let start = gpui::point(
                    from_origin.x + px(from_width / 2.0),
                    from_origin.y + px(from_height),
                );
                let end = gpui::point(to_origin.x + px(to_width / 2.0), to_origin.y);
                let middle_y = start.y + (end.y - start.y) / 2.0;
                parent_paths.move_to(start);
                parent_paths.cubic_bezier_to(
                    end,
                    gpui::point(start.x, middle_y),
                    gpui::point(end.x, middle_y),
                );
            }
            CanvasEdgeKind::Submodule => {
                let from_origin = transformed_point(bounds, state, *from);
                let to_origin = transformed_point(bounds, state, *to);
                let start = gpui::point(
                    from_origin.x + px(from_width),
                    from_origin.y + px(from_height / 2.0),
                );
                let end = gpui::point(to_origin.x, to_origin.y + px(to_height / 2.0));
                let middle_x = start.x + (end.x - start.x) / 2.0;
                submodule_paths.move_to(start);
                submodule_paths.cubic_bezier_to(
                    end,
                    gpui::point(middle_x, start.y),
                    gpui::point(middle_x, end.y),
                );
            }
            CanvasEdgeKind::Head => {
                let from_origin = transformed_point(bounds, state, *from);
                let to_origin = transformed_point(bounds, state, *to);
                let start = gpui::point(from_origin.x, from_origin.y + px(from_height / 2.0));
                let end = gpui::point(
                    to_origin.x + px(to_width),
                    to_origin.y + px(to_height / 2.0),
                );
                head_paths.move_to(start);
                head_paths.line_to(end);
            }
        }
    }

    if let Ok(path) = parent_paths.build() {
        window.paint_path(path, rgb(theme::TEXT_SUBTLE));
    }
    if let Ok(path) = submodule_paths.build() {
        window.paint_path(path, rgb(theme::YELLOW));
    }
    if let Ok(path) = head_paths.build() {
        window.paint_path(path, rgb(theme::GREEN));
    }
}

#[cfg(test)]
mod tests {
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
}
