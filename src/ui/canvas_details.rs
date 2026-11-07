use super::focus_visibility::KeyboardFocusStyle;
use super::{commit_canvas::CanvasNodeKind, icons::icon, theme};
use crate::{app_state::GitMasterApp, models::CommitLaneStatus};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::Path;
impl GitMasterApp {
    pub(super) fn render_placeholder_details(
        &self,
        path: &Path,
        hash: &str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let layout = self.commit_canvas_layout.as_ref()?;
        let node = layout.nodes.iter().find(|n| {
            n.kind == CanvasNodeKind::Placeholder
                && n.id.commit_id == hash
                && layout
                    .lanes
                    .iter()
                    .find(|l| l.id == n.id.lane_id)
                    .is_some_and(|l| {
                        layout
                            .repository_path
                            .join(l.relative_path.as_deref().unwrap_or(Path::new("")))
                            == path
                    })
        })?;
        let lane = layout.lanes.iter().find(|l| l.id == node.id.lane_id)?;
        let uninitialized = lane.status == CommitLaneStatus::Uninitialized;
        let initialization = self.initialization.get(path);
        let initializing = matches!(
            initialization,
            Some(super::state_views::Initialization::Running)
        );
        let failed = matches!(
            initialization,
            Some(super::state_views::Initialization::Failed(_))
        );
        let relative = lane.relative_path.clone();
        let root = layout.repository_path.clone();
        let source = self
            .repos
            .iter()
            .find(|r| r.path == root)
            .and_then(|r| {
                r.submodules
                    .iter()
                    .find(|s| Some(&s.relative_path) == relative.as_ref())
            })
            .and_then(|s| s.url.clone())
            .map(|url| crate::operation_log::redact(&url));
        let copied = source.clone().unwrap_or_else(|| hash.to_owned());
        let mut panel = div()
            .id("placeholder-details-body")
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .border_l_1()
            .border_color(rgb(theme::palette().border))
            .p(px(18.0))
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(div().text_size(px(23.0)).child(lane.name.clone()))
            .child(row(
                "状态",
                if uninitialized {
                    "未初始化"
                } else {
                    "不在已加载范围"
                },
            ))
            .child(row("路径", &path.display().to_string()))
            .children(source.map(|s| row("来源", &s)))
            .child(row("记录的引用", hash))
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(theme::palette().border))
                    .pt(px(16.0))
                    .flex()
                    .gap(px(10.0))
                    .child(icon("info-circle", 20.0))
                    .child(div().flex_1().min_w_0().child(if uninitialized {
                        "当前仅显示父仓库记录的引用；初始化后读取真实历史，不假造子模块提交。"
                    } else {
                        "该引用不在已加载范围，保留占位并可加载更多历史。它不表示提交丢失。"
                    })),
            );
        if let Some(state) = initialization {
            panel = panel.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(10.0))
                    .id("placeholder-initialization-error")
                    .max_h(px(150.0))
                    .overflow_y_scroll()
                    .flex_shrink_0()
                    .p(px(12.0))
                    .bg(rgb(theme::palette().bg_elevated))
                    .text_color(rgb(if failed {
                        theme::palette().red
                    } else {
                        theme::palette().accent
                    }))
                    .child(icon(if failed { "alert-circle" } else { "refresh" }, 20.0))
                    .child(div().flex_1().min_w_0().child(match state {
                        super::state_views::Initialization::Running => "正在初始化…".into(),
                        super::state_views::Initialization::Failed(e) => {
                            format!("初始化失败 · {}", crate::operation_log::redact(e))
                        }
                    })),
            );
        }
        let mut actions = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap(px(12.0))
            .p(px(18.0))
            .border_t_1()
            .border_color(rgb(theme::palette().border));
        if uninitialized {
            actions = actions.child(
                self.track(
                    "placeholder-init",
                    div()
                        .id("placeholder-init")
                        .p(px(10.0))
                        .map(|button| super::button_style::primary(button, !self.busy))
                        .child(if initializing {
                            "正在初始化…"
                        } else if failed {
                            "重试初始化"
                        } else {
                            "初始化子模块"
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(relative) = relative.clone() {
                                this.init_canvas_submodule(root.clone(), relative, cx);
                            }
                        })),
                ),
            );
        } else if layout.has_more {
            actions = actions.child(
                self.track(
                    "placeholder-load-more",
                    div()
                        .id("placeholder-load-more")
                        .p(px(10.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .child("加载更多历史")
                        .on_click(cx.listener(|this, _, _, cx| this.load_more_history(cx))),
                ),
            );
        }
        actions = actions.child(
            self.track(
                "placeholder-copy",
                div()
                    .id("placeholder-copy")
                    .p(px(10.0))
                    .border_1()
                    .border_color(rgb(theme::palette().border))
                    .focusable()
                    .tab_stop(true)
                    .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                    .cursor_pointer()
                    .child("复制地址 / 引用")
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                    }),
            ),
        );
        Some(
            self.track(
                "placeholder-details",
                div()
                    .id("placeholder-details")
                    .w(px(328.0))
                    .min_w_0()
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .border_l_1()
                    .border_color(rgb(theme::palette().border))
                    .child(
                        div()
                            .id("placeholder-details-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(panel),
                    )
                    .child(actions),
            ),
        )
    }
}
fn row(label: &str, value: &str) -> Div {
    div()
        .flex()
        .gap(px(12.0))
        .child(
            div()
                .w(px(82.0))
                .flex_shrink_0()
                .text_color(rgb(theme::palette().text_subtle))
                .child(label.to_owned()),
        )
        .child(div().flex_1().min_w_0().child(value.to_owned()))
}
