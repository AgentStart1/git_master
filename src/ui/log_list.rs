use std::collections::HashMap;

use gpui::*;

use super::{commit_canvas::branch_columns, theme};
use crate::{app_state::GitMasterApp, models::LogEntry};

const ROW_HEIGHT: f32 = 86.0;
const COLUMN_WIDTH: f32 = 20.0;

#[derive(Clone, Debug, Default)]
pub struct LogListLayout {
    pub entries: Vec<LogEntry>,
    columns: Vec<usize>,
    edges: Vec<(usize, usize)>,
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
        Self {
            entries: entries.to_vec(),
            columns: branch_columns(entries),
            edges,
            missing,
        }
    }
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
        let width = (graph.columns.iter().copied().max().unwrap_or(0) + 2) as f32 * COLUMN_WIDTH;
        let height = graph.entries.len() as f32 * ROW_HEIGHT;
        let paint_graph = graph.clone();
        let drawing = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let position = |index: usize| {
                    point(
                        bounds.origin.x
                            + px(16.0 + paint_graph.columns[index] as f32 * COLUMN_WIDTH),
                        bounds.origin.y + px(index as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0),
                    )
                };
                for &(from, to) in &paint_graph.edges {
                    let start = position(from);
                    let end = position(to);
                    let mut path = PathBuilder::stroke(px(2.0));
                    path.move_to(start);
                    path.line_to(point(start.x, end.y - px(ROW_HEIGHT / 2.0)));
                    path.line_to(end);
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
                        window.paint_path(path, rgb(theme::TEXT_SUBTLE));
                    }
                }
                for index in 0..paint_graph.entries.len() {
                    let center = position(index);
                    let mut path = PathBuilder::stroke(px(3.0));
                    path.move_to(point(center.x, center.y - px(4.0)));
                    path.line_to(point(center.x + px(4.0), center.y));
                    path.line_to(point(center.x, center.y + px(4.0)));
                    path.line_to(point(center.x - px(4.0), center.y));
                    path.line_to(point(center.x, center.y - px(4.0)));
                    if let Ok(path) = path.build() {
                        window
                            .paint_path(path, rgb(theme::graph_color(paint_graph.columns[index])));
                    }
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
            div()
                .id(ElementId::Name(
                    format!("log-commit-{}", entry.full_hash).into(),
                ))
                .cursor_pointer()
                .hover(|style| style.bg(rgba(0x31324466)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_commit_details(path.clone(), hash.clone(), cx)
                }))
                .h(px(ROW_HEIGHT))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .justify_center()
                .gap(px(3.0))
                .pl(px(width + 12.0))
                .pr(px(12.0))
                .border_b_1()
                .border_color(rgb(theme::BG_OVERLAY))
                .child(
                    div()
                        .text_sm()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(entry.message.clone()),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(6.0))
                        .text_xs()
                        .text_color(rgb(theme::ACCENT))
                        .children(
                            self.detail
                                .as_ref()
                                .and_then(|detail| detail.head_labels.get(&entry.full_hash))
                                .into_iter()
                                .flatten()
                                .map(|label| div().child(label.clone())),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SUBTLE))
                        .child(format!("{}  {} — {}", entry.hash, entry.author, entry.date)),
                )
        });
        div()
            .id("log-scroll")
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .overflow_x_scroll()
            .child(
                div()
                    .relative()
                    .min_w(px(width + 420.0))
                    .h(px(height))
                    .child(drawing)
                    .child(div().flex().flex_col().children(rows)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }
}
