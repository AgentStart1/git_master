use super::theme;
use crate::diff_model::{DiffRow, RowKind};
use gpui::*;

pub fn render(rows: &[DiffRow], stash: bool) -> AnyElement {
    let font = if cfg!(windows) {
        "Consolas"
    } else if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "monospace"
    };
    let mut pane = div()
        .id("worktree-diff")
        .flex_1()
        .min_w_0()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .overflow_x_scroll()
        .border_l_1()
        .border_color(rgb(theme::palette().border))
        .bg(rgb(theme::palette().bg_surface));
    if rows.is_empty() {
        return pane
            .child(
                div()
                    .p(px(16.0))
                    .text_size(px(16.0))
                    .text_color(rgb(theme::palette().text_subtle))
                    .child(if stash {
                        "选择一条暂存记录，查看保存的文件变更。"
                    } else {
                        "选择左侧文件，审阅文件变更。"
                    }),
            )
            .into_any_element();
    }
    let code_width = rows
        .iter()
        .filter(|row| {
            matches!(
                row.kind,
                RowKind::Context | RowKind::Added | RowKind::Removed
            )
        })
        .map(|row| row.text.len())
        .max()
        .unwrap_or(0);
    pane = pane.child(
        div()
            .flex()
            .flex_col()
            .w_full()
            .min_w(px(136.0 + code_width as f32 * 8.5))
            .font_family(font)
            .text_size(px(14.0))
            .line_height(px(24.0))
            .children(rows.iter().map(render_row)),
    );
    pane.into_any_element()
}

fn render_row(row: &DiffRow) -> AnyElement {
    match row.kind {
        RowKind::Section => {
            return div()
                .px(px(12.0))
                .py(px(7.0))
                .mt(px(8.0))
                .bg(rgb(theme::palette().bg_elevated))
                .border_b_1()
                .border_color(rgb(theme::palette().border))
                .text_color(rgb(theme::palette().text_primary))
                .font_weight(FontWeight::SEMIBOLD)
                .child(match row.text.as_str() {
                    "Staged" | "Staged changes" => "已暂存的变更".to_owned(),
                    "Working tree" | "Working tree changes" => "工作区变更".to_owned(),
                    "Untracked files" => "未跟踪文件".to_owned(),
                    _ => row.text.clone(),
                })
                .into_any_element();
        }
        RowKind::File => {
            return div()
                .flex()
                .items_center()
                .py(px(6.0))
                .bg(rgb(theme::palette().bg_overlay))
                .text_color(rgb(theme::palette().text_primary))
                .child(
                    div()
                        .w(px(48.0))
                        .flex_shrink_0()
                        .text_center()
                        .text_xs()
                        .text_color(rgb(theme::palette().text_subtle))
                        .child("旧"),
                )
                .child(
                    div()
                        .w(px(48.0))
                        .flex_shrink_0()
                        .text_center()
                        .text_xs()
                        .text_color(rgb(theme::palette().text_subtle))
                        .child("新"),
                )
                .child(div().px(px(12.0)).child(row.text.clone()))
                .into_any_element();
        }
        RowKind::Hunk => {
            return div()
                .px(px(12.0))
                .py(px(3.0))
                .bg(rgb(theme::palette().accent_soft))
                .text_color(rgb(theme::palette().accent))
                .child(row.text.clone())
                .into_any_element();
        }
        RowKind::Note => {
            return div()
                .px(px(12.0))
                .py(px(2.0))
                .text_color(rgb(theme::palette().text_subtle))
                .child(row.text.clone())
                .into_any_element();
        }
        _ => {}
    }
    let (background, foreground, emphasis, sign) = match row.kind {
        RowKind::Added => (
            theme::palette().diff_added,
            theme::palette().text_primary,
            theme::palette().diff_added_word,
            "+",
        ),
        RowKind::Removed => (
            theme::palette().diff_removed,
            theme::palette().text_primary,
            theme::palette().diff_removed_word,
            "−",
        ),
        _ => (
            theme::palette().bg_surface,
            theme::palette().text_primary,
            theme::palette().bg_surface,
            " ",
        ),
    };
    let text = if row.text.is_empty() {
        " ".to_owned()
    } else {
        row.text.clone()
    };
    let highlights = row.highlights.iter().cloned().map(|range| {
        (
            range,
            HighlightStyle {
                background_color: Some(rgb(emphasis).into()),
                font_weight: Some(FontWeight::BOLD),
                ..Default::default()
            },
        )
    });
    div()
        .flex()
        .flex_row()
        .w_full()
        .min_h(px(24.0))
        .flex_shrink_0()
        .bg(rgb(background))
        .text_color(rgb(foreground))
        .child(gutter(row.old_line))
        .child(gutter(row.new_line))
        .child(div().w(px(24.0)).flex_shrink_0().text_center().child(sign))
        .child(
            div()
                .flex_1()
                .pr(px(14.0))
                .whitespace_nowrap()
                .child(StyledText::new(text).with_highlights(highlights)),
        )
        .into_any_element()
}
fn gutter(number: Option<usize>) -> AnyElement {
    div()
        .w(px(48.0))
        .flex_shrink_0()
        .pr(px(10.0))
        .text_right()
        .text_color(rgb(theme::palette().text_subtle))
        .border_r_1()
        .border_color(rgb(theme::palette().border))
        .child(number.map(|n| n.to_string()).unwrap_or_default())
        .into_any_element()
}
