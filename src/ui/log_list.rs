use std::collections::HashMap;

use gpui::*;

use super::{commit_canvas::branch_columns, theme};
use crate::{app_state::GitMasterApp, models::LogEntry};

const ROW_HEIGHT: f32 = 46.0;
const COLUMN_WIDTH: f32 = 20.0;

#[derive(Clone, Debug, Default)]
pub struct LogListLayout {
    pub entries: Vec<LogEntry>,
    columns: Vec<usize>,
    edges: Vec<(usize, usize)>,
    routes: Vec<Vec<(f32, f32)>>,
    width_columns: f32,
    missing: Vec<usize>,
}

impl LogListLayout {
    pub fn new(entries: &[LogEntry]) -> Self {
        let indices: HashMap<_, _> = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.full_hash.as_str(), index))
            .collect();
        let mut edges = Vec::new();
        let mut missing = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            for parent in &entry.parent_hashes {
                if let Some(parent_index) = indices.get(parent.as_str()) {
                    edges.push((index, *parent_index));
                } else {
                    missing.push(index);
                }
            }
        }
        let columns = branch_columns(entries);
        let (routes, width_columns) = route_edges(&columns, &edges);
        Self {
            entries: entries.to_vec(),
            columns,
            edges,
            routes,
            width_columns,
            missing,
        }
    }
}

/// Route long edges between node columns, never through an unrelated node.
/// Channels can be reused only after the previous edge has ended.
fn route_edges(columns: &[usize], edges: &[(usize, usize)]) -> (Vec<Vec<(f32, f32)>>, f32) {
    let mut channel_ends: Vec<usize> = Vec::new();
    let mut width = columns.iter().copied().max().unwrap_or(0) as f32;
    let routes = edges
        .iter()
        .map(|&(from, to)| {
            let start = (columns[from] as f32, from as f32);
            let end = (columns[to] as f32, to as f32);
            if columns[from] == columns[to]
                && columns[from + 1..to]
                    .iter()
                    .all(|column| *column != columns[from])
            {
                return vec![start, end];
            }
            let channel = channel_ends
                .iter()
                .position(|last| *last <= from)
                .unwrap_or(channel_ends.len());
            if channel == channel_ends.len() {
                channel_ends.push(to);
            } else {
                channel_ends[channel] = to;
            }
            let x = channel as f32 + 0.5;
            width = width.max(x);
            vec![start, (x, from as f32 + 0.35), (x, to as f32 - 0.35), end]
        })
        .collect();
    (routes, width + 2.0)
}

impl GitMasterApp {
    pub(super) fn render_log_list(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let Some(layout) = self.commit_canvas_layout.as_ref() else {
            return div()
                .p(px(16.0))
                .child("No history available")
                .into_any_element();
        };
        let graph = layout.list.clone();
        let width = (graph.width_columns * COLUMN_WIDTH).max(60.0);
        let height = graph.entries.len() as f32 * ROW_HEIGHT;
        let paint_graph = graph.clone();
        let drawing = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let position = |index: usize| {
                    point(
                        bounds.origin.x
                            + px(28.0 + paint_graph.columns[index] as f32 * COLUMN_WIDTH),
                        bounds.origin.y + px(index as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0),
                    )
                };
                for (&(from, _), route) in paint_graph.edges.iter().zip(&paint_graph.routes) {
                    let mut path = PathBuilder::stroke(px(2.0));
                    for (index, &(column, row)) in route.iter().enumerate() {
                        let point = point(
                            bounds.origin.x + px(28.0 + column * COLUMN_WIDTH),
                            bounds.origin.y + px((row + 0.5) * ROW_HEIGHT),
                        );
                        if index == 0 {
                            path.move_to(point);
                        } else {
                            path.line_to(point);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(theme::graph_color(paint_graph.columns[from])));
                    }
                }
                for &index in &paint_graph.missing {
                    let start = position(index);
                    let mut path = PathBuilder::stroke(px(1.5)).dash_array(&[px(3.0), px(3.0)]);
                    path.move_to(start);
                    path.line_to(point(start.x, start.y + px(ROW_HEIGHT / 2.0)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(theme::palette().text_subtle));
                    }
                }
                for index in 0..paint_graph.entries.len() {
                    let center = position(index);
                    let mut node = fill(
                        Bounds::new(
                            point(center.x - px(5.0), center.y - px(5.0)),
                            size(px(10.0), px(10.0)),
                        ),
                        rgb(theme::graph_color(paint_graph.columns[index])),
                    );
                    node.corner_radii = px(5.0).into();
                    window.paint_quad(node);
                }
            },
        )
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w(px(width))
        .h(px(height));
        let rows = graph.entries.iter().map(|entry| {
            let path = layout.repository_path.clone();
            let hash = entry.full_hash.clone();
            let selected = self
                .selected_commit
                .as_ref()
                .is_some_and(|(_, hash)| hash == &entry.full_hash);
            let labels = self
                .detail
                .as_ref()
                .and_then(|detail| detail.head_labels.get(&entry.full_hash))
                .into_iter()
                .flatten()
                .filter(|label| label.as_str() != "HEAD");
            self.track(
                &format!("log-commit-{}", entry.full_hash),
                div()
                    .id(ElementId::Name(
                        format!("log-commit-{}", entry.full_hash).into(),
                    ))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_commit_details(path.clone(), hash.clone(), cx)
                    }))
                    .h(px(ROW_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .child(div().w(px(width)).h_full().flex_shrink_0())
                    .items_center()
                    .bg(rgb(if selected {
                        theme::palette().accent_soft
                    } else {
                        theme::palette().bg_base
                    }))
                    .hover(|style| style.bg(rgb(theme::palette().bg_elevated)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .flex_1()
                            .min_w(px(0.0))
                            .pr(px(12.0))
                            .child(
                                div()
                                    .id(ElementId::Name(
                                        format!("log-message-{}", entry.full_hash).into(),
                                    ))
                                    .text_size(px(17.0))
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .tooltip(super::tooltip::text(entry.message.clone()))
                                    .child(entry.message.clone()),
                            )
                            .children(labels.take(1).map(|label| {
                                div()
                                    .id(ElementId::Name(
                                        format!("log-label-{}-{label}", entry.full_hash).into(),
                                    ))
                                    .flex_shrink_0()
                                    .px(px(5.0))
                                    .py(px(1.0))
                                    .rounded(px(3.0))
                                    .border_1()
                                    .border_color(rgb(theme::palette().accent))
                                    .max_w(px(175.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .tooltip(super::tooltip::text(label.clone()))
                                    .text_size(px(15.0))
                                    .text_color(rgb(theme::palette().accent))
                                    .child(label.clone())
                            })),
                    )
                    .child(table_cell(&entry.author, 104.0))
                    .child(table_cell(&entry.hash, 90.0))
                    .child(table_cell(&relative_date(&entry.date), 90.0)),
            )
        });
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .child(
                div()
                    .h(px(48.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .child(table_cell("图", width))
                    .child(div().flex_1().text_size(px(17.0)).child("提交信息"))
                    .child(table_cell("作者", 104.0))
                    .child(table_cell("哈希", 90.0))
                    .child(table_cell("时间", 90.0)),
            )
            .child(
                div()
                    .id("log-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .relative()
                            .min_w(px(width + 650.0))
                            .h(px(height))
                            .child(div().flex().flex_col().children(rows))
                            .child(drawing),
                    ),
            )
            .into_any_element()
    }
}

fn table_cell(value: &str, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .pr(px(10.0))
        .text_size(px(16.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .child(value.to_owned())
}
pub(super) fn relative_date(value: &str) -> String {
    relative_date_at(value, chrono::Utc::now().naive_utc())
}

fn relative_date_at(value: &str, now: chrono::NaiveDateTime) -> String {
    let parsed = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M").ok();
    let Some(date) = parsed else {
        return value.to_owned();
    };
    let hours = (now - date).num_hours().max(0);
    if hours == 0 {
        "刚刚".into()
    } else if hours < 24 {
        format!("{hours} 小时前")
    } else {
        format!("{} 天前", hours / 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn relative_dates_compare_utc_commit_times_across_local_offsets() {
        for offset in ["+08:00", "-07:00", "+00:00"] {
            let now = chrono::DateTime::parse_from_rfc3339(&format!("2026-11-07T12:00:00{offset}"))
                .unwrap()
                .naive_utc();
            for (age, expected) in [(0, "刚刚"), (2, "2 小时前"), (48, "2 天前")] {
                let commit = (now - chrono::Duration::hours(age))
                    .format("%Y-%m-%d %H:%M")
                    .to_string();
                assert_eq!(relative_date_at(&commit, now), expected);
            }
        }
    }
    #[::core::prelude::v1::test]
    fn retains_both_merge_parents_and_truncated_history() {
        let entry = |hash: &str, parents: &[&str]| LogEntry {
            full_hash: hash.into(),
            hash: hash.into(),
            parent_hashes: parents.iter().map(|p| p.to_string()).collect(),
            author: String::new(),
            date: String::new(),
            message: String::new(),
        };
        let graph = LogListLayout::new(&[
            entry("merge", &["left", "right"]),
            entry("left", &["base"]),
            entry("right", &["base"]),
            entry("base", &["outside"]),
        ]);
        assert_eq!(graph.edges, [(0, 1), (0, 2), (1, 3), (2, 3)]);
        assert_ne!(graph.columns[1], graph.columns[2]);
        assert_eq!(graph.missing, [3]);
        assert_routes_avoid_nodes(&graph);
    }

    fn assert_routes_avoid_nodes(graph: &LogListLayout) {
        for (&(from, to), route) in graph.edges.iter().zip(&graph.routes) {
            for (row, &column) in graph.columns.iter().enumerate() {
                if row == from || row == to {
                    continue;
                }
                let (x, y) = (column as f32, row as f32);
                for pair in route.windows(2) {
                    let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
                    let cross = (x - ax) * (by - ay) - (y - ay) * (bx - ax);
                    let within =
                        x >= ax.min(bx) && x <= ax.max(bx) && y >= ay.min(by) && y <= ay.max(by);
                    assert!(
                        !(cross.abs() < 0.0001 && within),
                        "edge {from}->{to} crosses node {row}"
                    );
                }
            }
        }
    }

    #[::core::prelude::v1::test]
    fn routes_avoid_intermediate_nodes_and_reserve_overlapping_channels() {
        // A merge's second-parent edge must leave the first-parent column
        // before it encounters the intervening first-parent commit.
        let columns = vec![0, 0, 1, 0];
        let edges = vec![(0, 1), (0, 2), (1, 3), (2, 3)];
        let (routes, width_columns) = route_edges(&columns, &edges);
        let graph = LogListLayout {
            columns,
            edges,
            routes,
            width_columns,
            ..Default::default()
        };
        assert_routes_avoid_nodes(&graph);
        assert_eq!(graph.routes[1][1], (0.5, 0.35));
        let (routes, _) = route_edges(&[0, 1, 0, 1, 0], &[(0, 3), (1, 4)]);
        assert_ne!(routes[0][1].0, routes[1][1].0);
    }
}
