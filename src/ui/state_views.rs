use super::{icons::icon, theme};
use gpui::*;

pub fn message(symbol: &'static str, title: &str, description: &str) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(20.0))
        .p(px(32.0))
        .child(
            div()
                .text_color(rgb(theme::palette().text_subtle))
                .child(icon(symbol, 48.0)),
        )
        .child(
            div()
                .text_size(px(23.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_size(px(16.0))
                .text_color(rgb(theme::palette().text_subtle))
                .child(description.to_owned()),
        )
}
pub fn loading(title: &str, description: &str) -> AnyElement {
    let p = theme::palette();
    div()
        .flex_1()
        .p(px(28.0))
        .flex()
        .flex_col()
        .gap(px(18.0))
        .child(div().text_size(px(20.0)).child(title.to_owned()))
        .children((0..5).map(|i| {
            div()
                .flex()
                .gap(px(16.0))
                .py(px(10.0))
                .border_b_1()
                .border_color(rgb(p.border))
                .child(icon("folder", 24.0))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .h(px(12.0))
                                .w(px(170.0 + i as f32 * 16.0))
                                .rounded(px(5.0))
                                .bg(rgb(p.bg_overlay)),
                        )
                        .child(
                            div()
                                .h(px(10.0))
                                .w(relative(0.7))
                                .rounded(px(5.0))
                                .bg(rgb(p.bg_elevated)),
                        ),
                )
        }))
        .child(
            div()
                .p(px(14.0))
                .bg(rgb(p.accent_soft))
                .text_color(rgb(p.accent))
                .child(description.to_owned()),
        )
        .into_any_element()
}

#[derive(Clone)]
pub enum Initialization {
    Running,
    Failed(String),
}
