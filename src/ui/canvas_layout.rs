use super::commit_canvas::*;
use crate::models::{CommitGraph, LogEntry};
use std::collections::{HashMap, HashSet};

const LANE_LEFT: f32 = 36.0;
const FIRST_NODE_TOP: f32 = 124.0;
const ROW_SPACING: f32 = 112.0;

pub fn build_layout(graph: &CommitGraph) -> CommitCanvasLayout {
    let columns: HashMap<_, _> = graph
        .lanes
        .iter()
        .map(|lane| (lane.id.clone(), branch_columns(&lane.entries)))
        .collect();
    let mut next_x = LANE_LEFT;
    let lanes = graph
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
    let lane_x = lanes
        .iter()
        .map(|lane| (lane.id.clone(), lane.x))
        .collect::<HashMap<_, _>>();
    let mut nodes = Vec::new();
    let mut known_nodes = HashSet::new();

    for lane in &graph.lanes {
        let x = lane_x.get(&lane.id).copied().unwrap_or(LANE_LEFT);
        for (index, entry) in lane.entries.iter().enumerate() {
            let commit_y = FIRST_NODE_TOP + index as f32 * ROW_SPACING;
            let id = CanvasNodeId {
                lane_id: lane.id.clone(),
                commit_id: entry.full_hash.clone(),
            };
            let mut head_labels = (lane.id == "main")
                .then(|| graph.head_labels.get(&entry.full_hash).cloned())
                .flatten()
                .unwrap_or_default();
            head_labels.sort_by_key(|label| (label != "HEAD", label != &graph.current_branch));
            if !head_labels.is_empty() {
                let head_height = head_node_height(
                    head_labels
                        .iter()
                        .filter(|label| label.as_str() != "HEAD")
                        .count(),
                );
                let head_width = (head_labels
                    .iter()
                    .map(|label| label.chars().count())
                    .max()
                    .unwrap_or(0) as f32
                    * 7.0
                    + 30.0)
                    .clamp(100.0, 120.0);
                let head_id = CanvasNodeId {
                    lane_id: "heads".to_string(),
                    commit_id: format!("head:{}", entry.full_hash),
                };
                nodes.push(CanvasNode {
                    id: head_id.clone(),
                    entry: None,
                    head_labels,
                    kind: CanvasNodeKind::Head,
                    width: head_width,
                    height: head_height,
                    default_position: default_head_position(
                        CanvasPoint::new(
                            x + columns[&lane.id][index] as f32 * LANE_SPACING,
                            commit_y,
                        ),
                        head_height,
                    ),
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

    // Place labels beside their commits, moving down only when another card blocks them.
    for index in 0..nodes.len() {
        if nodes[index].is_head() {
            nodes[index].default_position = free_head_position(
                &nodes[index],
                nodes.iter().enumerate().filter_map(|(other_index, node)| {
                    (other_index != index && (!node.is_head() || other_index < index))
                        .then_some(node)
                }),
            );
        }
    }
    let default_positions = nodes
        .iter()
        .map(|node| (node.id.clone(), node.default_position))
        .collect::<HashMap<_, _>>();
    let mut edges = nodes
        .iter()
        .filter(|node| node.is_head())
        .filter_map(|node| {
            let commit_id = node.id.commit_id.strip_prefix("head:")?;
            Some(CanvasEdge {
                from: node.id.clone(),
                to: CanvasNodeId {
                    lane_id: "main".into(),
                    commit_id: commit_id.into(),
                },
                kind: CanvasEdgeKind::Reference,
            })
        })
        .collect::<Vec<_>>();
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
                if !known_nodes.contains(&to) {
                    let y = FIRST_NODE_TOP + lane.entries.len() as f32 * ROW_SPACING;
                    let position = free_placeholder_position(&nodes, lane_x[&lane.id], y);
                    known_nodes.insert(to.clone());
                    nodes.push(CanvasNode {
                        id: to.clone(),
                        entry: None,
                        head_labels: vec![],
                        kind: CanvasNodeKind::Placeholder,
                        width: NODE_WIDTH,
                        height: NODE_HEIGHT,
                        default_position: position,
                    });
                }
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
            let position = free_placeholder_position(&nodes, x, source_y);
            known_nodes.insert(to.clone());
            nodes.push(CanvasNode {
                id: to.clone(),
                entry: None,
                head_labels: Vec::new(),
                kind: CanvasNodeKind::Placeholder,
                width: NODE_WIDTH,
                height: NODE_HEIGHT,
                default_position: position,
            });
        }
        edges.push(CanvasEdge {
            from,
            to,
            kind: CanvasEdgeKind::Submodule,
        });
    }

    CommitCanvasLayout {
        history_limit: 200,
        has_more: false,
        repository_path: graph.repository_path.clone(),
        lanes,
        nodes,
        edges,
        list: crate::ui::log_list::LogListLayout::new(
            graph
                .lanes
                .iter()
                .find(|lane| lane.id == "main")
                .map(|lane| lane.entries.as_slice())
                .unwrap_or(&[]),
        ),
    }
}

// A referenced-but-unloaded OID is a separate card, not a replacement for the
// loaded commit at the source row. Reserve a free row for every placeholder.
fn free_placeholder_position(nodes: &[CanvasNode], x: f32, mut y: f32) -> CanvasPoint {
    loop {
        let next = nodes
            .iter()
            .filter(|node| {
                let p = node.default_position;
                x < p.x + node.width + 8.0
                    && x + NODE_WIDTH + 8.0 > p.x
                    && y < p.y + node.height + 8.0
                    && y + NODE_HEIGHT + 8.0 > p.y
            })
            .map(|node| node.default_position.y + ROW_SPACING)
            .max_by(f32::total_cmp);
        match next {
            Some(next) => y = next,
            None => return CanvasPoint::new(x, y),
        }
    }
}

pub(super) fn default_head_position(commit: CanvasPoint, head_height: f32) -> CanvasPoint {
    CanvasPoint::new(
        commit.x + NODE_WIDTH + 12.0,
        commit.y + (NODE_HEIGHT - head_height) / 2.0,
    )
}

pub(super) fn free_head_position<'a>(
    head: &CanvasNode,
    obstacles: impl Iterator<Item = &'a CanvasNode> + Clone,
) -> CanvasPoint {
    let mut p = head.default_position;
    loop {
        let collision = obstacles.clone().find(|other| {
            let q = other.default_position;
            p.x < q.x + other.width + 8.0
                && p.x + head.width + 8.0 > q.x
                && p.y < q.y + other.height + 8.0
                && p.y + head.height + 8.0 > q.y
        });
        match collision {
            Some(other) => p.y = other.default_position.y + other.height + 12.0,
            None => return p,
        }
    }
}

fn head_node_height(label_count: usize) -> f32 {
    26.0 + label_count as f32 * 16.0
}

/// Reserve separate columns for active ancestry paths, retaining the first
/// parent's column and joining paths when they reach a shared ancestor.
pub(super) fn branch_columns(entries: &[LogEntry]) -> Vec<usize> {
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
            if let Some(existing_column) = active.iter().position(|id| id.as_ref() == Some(parent))
            {
                // Lower-numbered columns represent the earlier (primary) path.
                // Let a primary path reclaim a shared ancestor from a side path,
                // but never let the side path steal it back afterward.
                if index == 0 && column < existing_column {
                    active[existing_column] = None;
                    active[column] = Some(parent.clone());
                }
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
