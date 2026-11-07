use super::theme;
use gpui::*;
pub struct Tooltip(String);
impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(560.0))
            .p(px(9.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(theme::palette().border))
            .bg(rgb(theme::palette().bg_elevated))
            .text_color(rgb(theme::palette().text_primary))
            .text_size(px(14.0))
            .shadow_md()
            .child(self.0.clone())
    }
}
pub fn text(value: impl Into<String>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let value = value.into();
    move |_, cx| cx.new(|_| Tooltip(value.clone())).into()
}
