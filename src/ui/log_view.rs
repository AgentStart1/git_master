use super::focus_visibility::KeyboardFocusStyle;
use super::icons::icon;
use crate::app_state::GitMasterApp;
use crate::ui::{commit_canvas::LogViewMode, theme};
use gpui::*;

impl GitMasterApp {
    pub(super) fn render_log_tab(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let list_bg = if self.log_view_mode == LogViewMode::List {
            rgb(theme::palette().accent)
        } else {
            rgb(theme::palette().bg_base)
        };
        let canvas_bg = if self.log_view_mode == LogViewMode::Canvas {
            rgb(theme::palette().accent)
        } else {
            rgb(theme::palette().bg_base)
        };
        let view_controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .px(px(10.0))
            .py(px(7.0))
            .bg(rgb(theme::palette().bg_base))
            .flex_shrink_0()
            .child(
                self.track(
                    "log-view-list",
                    div()
                        .id("log-view-list")
                        .px(px(10.0))
                        .py(px(4.0))
                        .rounded(px(3.0))
                        .border_b_2()
                        .border_color(list_bg)
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .text_size(px(16.0))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon("list", 20.0))
                        .child("列表")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_log_view_mode(LogViewMode::List);
                            cx.notify();
                        })),
                ),
            )
            .child(
                self.track(
                    "log-view-canvas",
                    div()
                        .id("log-view-canvas")
                        .px(px(10.0))
                        .py(px(4.0))
                        .rounded(px(3.0))
                        .border_b_2()
                        .border_color(canvas_bg)
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .text_size(px(16.0))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon("git-fork", 20.0))
                        .child("关系画布")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_log_view_mode(LogViewMode::Canvas);
                            cx.notify();
                        })),
                ),
            );

        let loading = self.history_host.is_loading();
        let more = self
            .commit_canvas_layout
            .as_ref()
            .is_some_and(|layout| layout.has_more);
        let view_controls = if loading || more {
            view_controls.child(
                self.track(
                    "log-load-more",
                    div()
                        .id("log-load-more")
                        .px(px(8.0))
                        .text_size(px(14.0))
                        .child(if loading {
                            "加载中…"
                        } else {
                            "加载更多"
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.load_more_history(cx))),
                ),
            )
        } else {
            view_controls
        };
        let mut toolbar = self.render_history_filter(cx).child(view_controls);
        if self.narrow_window && self.selected_commit.is_some() {
            toolbar = toolbar.child(
                self.track(
                    "details-toggle",
                    div()
                        .id("details-toggle")
                        .px(px(8.0))
                        .text_size(px(14.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .child(if self.narrow_details_open {
                            "收起详情"
                        } else {
                            "展开详情"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.narrow_details_open = !this.narrow_details_open;
                            cx.notify();
                        })),
                ),
            );
        }

        let body = if self.detail.as_ref().is_some_and(|d| d.head_oid.is_none()) {
            let path = self
                .detail
                .as_ref()
                .map(|d| std::path::PathBuf::from(&d.path));
            super::state_views::message("git-branch", "尚无提交记录", "完成首次提交后显示历史。")
                .children(path.map(|path| {
                    div()
                        .id("unborn-terminal")
                        .p(px(10.0))
                        .border_1()
                        .border_color(rgb(theme::palette().border))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .child("打开终端")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_overview_location(path.clone(), true, cx)
                        }))
                }))
                .into_any_element()
        } else {
            match self.log_view_mode {
                LogViewMode::List => self.render_log_list(cx),
                LogViewMode::Canvas => self.render_commit_canvas(cx),
            }
        };
        let content = div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .child(body),
            )
            .children(
                (!self.narrow_window || self.narrow_details_open)
                    .then(|| self.render_commit_details(cx))
                    .flatten(),
            );
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .child(toolbar)
            .child(content)
            .children(
                (self.log_view_mode == LogViewMode::Canvas).then(|| self.render_canvas_footer(cx)),
            )
            .children(
                self.commit_canvas_layout
                    .as_ref()
                    .filter(|l| l.history_limit > 200 && !l.has_more)
                    .map(|_| {
                        div()
                            .p(px(12.0))
                            .bg(rgb(theme::palette().diff_added))
                            .text_color(rgb(theme::palette().green))
                            .child("全部加载完成")
                    }),
            )
            .children(self.history_host.has_error().then(|| {
                self.track(
                    "history-append-error",
                    div()
                        .id("history-append-error")
                        .p(px(14.0))
                        .bg(rgb(theme::palette().bg_elevated))
                        .text_color(rgb(theme::palette().yellow))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .child("更早历史加载失败 · 重试   （保留现有节点位置、缩放与筛选）")
                        .on_click(cx.listener(|this, _, _, cx| this.load_more_history(cx))),
                )
            }))
            .into_any_element()
    }
}
