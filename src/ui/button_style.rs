use super::focus_visibility::KeyboardFocusStyle;
use super::{theme, tooltip};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Keep the foreground fixed while interaction styles vary the fill. Text is
/// shaped before hover painting, so changing inherited hover text is unreliable.
pub fn primary(button: Stateful<Div>, enabled: bool) -> Stateful<Div> {
    let p = theme::palette();
    button
        .border_1()
        .border_color(rgb(if enabled { p.accent_fill } else { p.border }))
        .bg(rgb(if enabled {
            p.accent_fill
        } else {
            p.bg_elevated
        }))
        .text_color(rgb(if enabled { 0xffffff } else { p.text_muted }))
        .tab_stop(enabled)
        .when(enabled, |button| {
            button
                .focusable()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(p.accent_pressed)))
                .active(|s| s.bg(rgb(p.accent_pressed)))
                .keyboard_focus(|s| {
                    s.border_color(rgb(p.text_primary)).shadow(vec![BoxShadow {
                        color: rgb(p.text_primary).into(),
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(1.0),
                        spread_radius: px(2.0),
                    }])
                })
        })
        .when(!enabled, |button| {
            button
                .cursor(CursorStyle::Arrow)
                .tooltip(tooltip::text("当前操作尚未完成，请稍候"))
        })
}
