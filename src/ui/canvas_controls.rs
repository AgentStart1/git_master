use super::focus_visibility::KeyboardFocusStyle;
use super::theme;
use crate::app_state::GitMasterApp;
use gpui::*;
impl GitMasterApp {
    pub(super) fn render_canvas_initializer(
        &self,
        layout: &super::commit_canvas::CommitCanvasLayout,
        lane: &super::commit_canvas::CanvasLane,
        cx: &Context<Self>,
    ) -> AnyElement {
        let path = layout.repository_path.clone();
        let relative = lane.relative_path.clone();
        let id = format!("canvas-init-{}", lane.id);
        let button = super::button_style::primary(
            div()
                .id(ElementId::Name(id.clone().into()))
                .flex_shrink_0()
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(3.0))
                .text_xs()
                .whitespace_nowrap(),
            !self.busy,
        )
        .child(if self.busy {
            "处理中…"
        } else {
            "初始化子模块"
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
        }));
        self.track(&id, button)
    }

    pub(super) fn render_canvas_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let zoom = self
            .commit_canvas_layout
            .as_ref()
            .and_then(|l| self.commit_canvas_states.get(&l.repository_path))
            .map(|s| s.zoom)
            .unwrap_or(1.0);
        let controls = [
            ("canvas-zoom-out", "−"),
            ("canvas-zoom-in", "+"),
            ("canvas-fit", "适应内容"),
            ("canvas-head", "回到 HEAD"),
            ("canvas-reset", "重置布局"),
        ];
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .px(px(18.0))
            .py(px(10.0))
            .gap(px(10.0))
            .border_t_1()
            .border_color(rgb(theme::palette().border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(10.0))
                    .child(div().text_size(px(16.0)).child(if zoom < 0.01 {
                        format!("{:.2}%", zoom * 100.0)
                    } else if zoom < 0.1 {
                        format!("{:.1}%", zoom * 100.0)
                    } else {
                        format!("{:.0}%", zoom * 100.0)
                    }))
                    .children(controls.into_iter().map(|(id, label)| {
                        self.track(
                            id,
                            div()
                                .id(id)
                                .px(px(14.0))
                                .py(px(6.0))
                                .rounded(px(3.0))
                                .border_1()
                                .border_color(rgb(theme::palette().border))
                                .bg(rgb(theme::palette().bg_elevated))
                                .text_size(px(16.0))
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| s.border_color(rgb(theme::palette().accent)))
                                .hover(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .active(|s| s.opacity(0.65))
                                .cursor_pointer()
                                .child(label)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.canvas_control(id, window);
                                    cx.notify();
                                })),
                        )
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(18.0))
                    .text_size(px(14.0))
                    .text_color(rgb(theme::palette().text_subtle))
                    .child(div().flex_1().child(
                        "实线：提交父子 · 细灰线：分支引用 · 灰虚线：子模块 · 蓝虚线：未加载",
                    ))
                    .child("拖动节点调整布局 · 拖动画布平移 · 滚轮缩放"),
            )
            .into_any_element()
    }
}
