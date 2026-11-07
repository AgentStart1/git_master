use super::theme;
use crate::markdown::{Block, Document, Kind};
use gpui::*;

pub fn render(document: &Document) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .gap(px(12.0))
        .children(
            document
                .0
                .iter()
                .enumerate()
                .map(|(i, block)| render_block(block, format!("md-{i}"))),
        )
        .into_any_element()
}
fn render_block(block: &Block, id: String) -> AnyElement {
    let p = theme::palette();
    if let Kind::List(start) = block.kind {
        return div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .children(block.children.iter().enumerate().map(|(i, child)| {
                div()
                    .flex()
                    .gap(px(7.0))
                    .items_start()
                    .child(
                        div().flex_shrink_0().min_w(px(18.0)).child(
                            start
                                .map(|n| format!("{}.", n + i as u64))
                                .unwrap_or_else(|| "•".into()),
                        ),
                    )
                    .child(render_block(child, format!("{id}-{i}")))
            }))
            .into_any_element();
    }
    let mut container = div()
        .id(ElementId::Name(id.clone().into()))
        .flex()
        .flex_col()
        .min_w_0()
        .gap(px(7.0));
    match block.kind {
        Kind::Heading(level) => {
            container = container
                .text_size(px(match level {
                    1 => 23.0,
                    2 => 21.0,
                    3 => 19.0,
                    _ => 17.0,
                }))
                .font_weight(FontWeight::SEMIBOLD)
        }
        Kind::Quote => {
            container = container
                .border_l_2()
                .border_color(rgb(p.accent))
                .pl(px(12.0))
                .text_color(rgb(p.text_subtle))
        }
        Kind::Code => {
            container = container
                .overflow_x_scroll()
                .p(px(10.0))
                .rounded(px(3.0))
                .bg(rgb(p.bg_elevated))
                .font_family(if cfg!(windows) {
                    "Consolas"
                } else if cfg!(target_os = "macos") {
                    "Menlo"
                } else {
                    "monospace"
                })
                .text_size(px(14.0))
        }
        Kind::Rule => return container.h(px(1.0)).bg(rgb(p.border)).into_any_element(),
        Kind::Table => {
            container = container
                .overflow_x_scroll()
                .border_1()
                .border_color(rgb(p.border))
                .gap_0()
        }
        Kind::TableHead | Kind::TableRow => {
            container = container
                .flex_row()
                .gap_0()
                .border_b_1()
                .border_color(rgb(p.border));
            if block.kind == Kind::TableHead {
                container = container
                    .bg(rgb(p.bg_elevated))
                    .font_weight(FontWeight::SEMIBOLD);
            }
        }
        Kind::TableCell => container = container.flex_1().min_w(px(100.0)).p(px(6.0)),
        Kind::Item => container = container.flex_1(),
        _ => {}
    }
    if !block.spans.is_empty() {
        let mut text = String::new();
        let mut highlights = Vec::new();
        let mut links = Vec::new();
        for span in &block.spans {
            let start = text.len();
            text.push_str(&span.text);
            let range = start..text.len();
            let mut style = HighlightStyle::default();
            if span.style.bold {
                style.font_weight = Some(FontWeight::BOLD);
            }
            if span.style.italic {
                style.font_style = Some(FontStyle::Italic);
            }
            if span.style.strike {
                style.strikethrough = Some(StrikethroughStyle {
                    thickness: px(1.0),
                    color: None,
                });
            }
            if span.style.code {
                style.background_color = Some(rgb(p.bg_overlay).into());
            }
            if let Some(url) = &span.style.link
                && crate::markdown::external_link(url)
            {
                style.color = Some(rgb(p.accent).into());
                style.underline = Some(UnderlineStyle {
                    thickness: px(1.0),
                    color: None,
                    wavy: false,
                });
                links.push((range.clone(), url.clone()));
            }
            highlights.push((range, style));
        }
        let styled = StyledText::new(text).with_highlights(highlights);
        let text = if links.is_empty() {
            styled.into_any_element()
        } else {
            let ranges = links.iter().map(|(range, _)| range.clone()).collect();
            InteractiveText::new(ElementId::Name(format!("{id}-text").into()), styled)
                .on_click(ranges, move |index, _, cx| cx.open_url(&links[index].1))
                .into_any_element()
        };
        let content = div().min_w_0().line_height(px(24.0)).child(text);
        container = container.child(if block.kind == Kind::Code {
            content.whitespace_nowrap()
        } else {
            content.whitespace_normal()
        });
    }
    container
        .children(
            block
                .children
                .iter()
                .enumerate()
                .map(|(i, child)| render_block(child, format!("{id}-{i}"))),
        )
        .into_any_element()
}
