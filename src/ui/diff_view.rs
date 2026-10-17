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
        .min_h(px(0.0))
        .overflow_y_scroll()
        .overflow_x_scroll()
        .border_1()
        .border_color(rgb(theme::BORDER))
        .rounded(px(10.0))
        .bg(rgb(theme::BG_SURFACE));
    if rows.is_empty() {
        return pane
            .child(
                div()
                    .p(px(16.0))
                    .text_sm()
                    .text_color(rgb(theme::TEXT_SUBTLE))
                    .child(if stash {
                        "Select a stash to view its saved changes."
                    } else {
                        "Select Diff beside a file to view its changes."
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
                .bg(rgb(theme::BG_ELEVATED))
                .border_b_1()
                .border_color(rgb(theme::BORDER))
                .text_color(rgb(theme::TEXT_PRIMARY))
                .font_weight(FontWeight::SEMIBOLD)
                .child(row.text.clone())
                .into_any_element();
        }
        RowKind::File => {
            return div()
                .flex()
                .items_center()
                .py(px(6.0))
                .bg(rgb(theme::BG_OVERLAY))
                .text_color(rgb(theme::TEXT_PRIMARY))
                .child(
                    div()
                        .w(px(48.0))
                        .flex_shrink_0()
                        .text_center()
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SUBTLE))
                        .child("OLD"),
                )
                .child(
                    div()
                        .w(px(48.0))
                        .flex_shrink_0()
                        .text_center()
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SUBTLE))
                        .child("NEW"),
                )
                .child(div().px(px(12.0)).child(row.text.clone()))
                .into_any_element();
        }
        RowKind::Hunk => {
            return div()
                .px(px(12.0))
                .py(px(3.0))
                .bg(rgb(theme::ACCENT_SOFT))
                .text_color(rgb(theme::ACCENT))
                .child(row.text.clone())
                .into_any_element();
        }
        RowKind::Note => {
            return div()
                .px(px(12.0))
                .py(px(2.0))
                .text_color(rgb(theme::TEXT_SUBTLE))
                .child(row.text.clone())
                .into_any_element();
        }
        _ => {}
    }
    let (background, foreground, emphasis, sign) = match row.kind {
        RowKind::Added => (0x122c25, 0xb4f0ce, 0x26563e, "+"),
        RowKind::Removed => (0x321c28, 0xffbdca, 0x703247, "−"),
        _ => (
            theme::BG_SURFACE,
            theme::TEXT_PRIMARY,
            theme::BG_SURFACE,
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
        .text_color(rgb(theme::TEXT_SUBTLE))
        .border_r_1()
        .border_color(rgb(theme::BORDER))
        .child(number.map(|n| n.to_string()).unwrap_or_default())
        .into_any_element()
}
