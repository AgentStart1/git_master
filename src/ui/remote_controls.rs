use super::focus_visibility::KeyboardFocusStyle;
use super::icons::icon;
use super::theme;
use crate::app_state::GitMasterApp;
use gpui::prelude::FluentBuilder;
use gpui::*;
impl GitMasterApp {
    pub(super) fn render_remote_controls(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.detail.as_ref().is_some_and(|d| d.remotes.is_empty()) {
            return vec![
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(icon("cloud", 22.0))
                    .child("无远端")
                    .children(
                        [
                            ("refresh", "Fetch"),
                            ("download", "Pull"),
                            ("upload", "Push"),
                        ]
                        .into_iter()
                        .map(|(symbol, label)| {
                            div()
                                .id(ElementId::Name(format!("no-remote-{label}").into()))
                                .opacity(0.45)
                                .px(px(14.0))
                                .py(px(6.0))
                                .border_1()
                                .border_color(rgb(theme::palette().border))
                                .bg(rgb(theme::palette().bg_elevated))
                                .tooltip(super::tooltip::text("尚未配置远端；请在终端配置后刷新。"))
                                .flex()
                                .gap(px(8.0))
                                .child(icon(symbol, 20.0))
                                .child(label)
                        }),
                    )
                    .into_any_element(),
            ];
        }
        let remote_controls: Vec<AnyElement> = if let Some(detail) = self.detail.as_ref() {
            detail
                .remotes
                .iter()
                .filter(|remote| {
                    Some(&remote.name)
                        == detail
                            .remotes
                            .iter()
                            .find(|r| Some(&r.name) == self.overview_remote.as_ref())
                            .or_else(|| detail.remotes.first())
                            .map(|r| &r.name)
                })
                .map(|remote| {
                    let remote_name = remote.name.clone();
                    let fetch_id = format!("log-remote-{}-fetch", remote.name);
                    let pull_id = format!("log-remote-{}-pull", remote.name);
                    let push_id = format!("log-remote-{}-push", remote.name);
                    let reset_id = format!("log-remote-{}-reset", remote.name);
                    let fetch_disabled = self.busy;
                    let fetch = div()
                        .id(ElementId::Name(fetch_id.clone().into()))
                        .px(px(20.0))
                        .py(px(6.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(rgb(theme::palette().border))
                        .bg(rgb(theme::palette().bg_elevated))
                        .text_size(px(17.0))
                        .text_color(rgb(theme::palette().text_primary))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon("refresh", 20.0))
                        .child("Fetch")
                        .opacity(if fetch_disabled { 0.45 } else { 1.0 })
                        .tab_stop(!fetch_disabled)
                        .cursor(if fetch_disabled {
                            CursorStyle::Arrow
                        } else {
                            CursorStyle::PointingHand
                        })
                        .when(fetch_disabled, |s| {
                            s.tooltip(super::tooltip::text("当前操作尚未完成"))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.do_fetch_selected_remote(remote_name.clone(), cx);
                        }));
                    let remote_name = remote.name.clone();
                    let write_disabled = self.busy
                        || detail.git_state != "正常"
                        || detail.current_branch == "HEAD detached";
                    let reason = if self.busy {
                        "当前操作尚未完成"
                    } else if detail.current_branch == "HEAD detached" {
                        "HEAD 分离，请先切换到本地分支"
                    } else {
                        "存在进行中的 Git 操作，请先处理并刷新"
                    };
                    let pull = div()
                        .id(ElementId::Name(pull_id.clone().into()))
                        .opacity(if write_disabled { 0.45 } else { 1.0 })
                        .when(write_disabled, |s| s.tooltip(super::tooltip::text(reason)))
                        .px(px(20.0))
                        .py(px(6.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(rgb(theme::palette().border))
                        .bg(rgb(theme::palette().bg_overlay))
                        .text_size(px(17.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon("download", 20.0))
                        .child("Pull")
                        .tab_stop(!write_disabled)
                        .cursor(if write_disabled {
                            CursorStyle::Arrow
                        } else {
                            CursorStyle::PointingHand
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if write_disabled {
                                return;
                            }
                            this.run_log_action(
                                super::log_actions::LogAction::Pull {
                                    remote: remote_name.clone(),
                                },
                                cx,
                            );
                        }));
                    let remote_name = remote.name.clone();
                    let push = div()
                        .id(ElementId::Name(push_id.clone().into()))
                        .opacity(if write_disabled { 0.45 } else { 1.0 })
                        .when(write_disabled, |s| s.tooltip(super::tooltip::text(reason)))
                        .px(px(20.0))
                        .py(px(6.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(rgb(theme::palette().border))
                        .bg(rgb(theme::palette().bg_overlay))
                        .text_size(px(17.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon("upload", 20.0))
                        .child("Push")
                        .tab_stop(!write_disabled)
                        .cursor(if write_disabled {
                            CursorStyle::Arrow
                        } else {
                            CursorStyle::PointingHand
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if write_disabled {
                                return;
                            }
                            this.run_log_action(
                                super::log_actions::LogAction::Push {
                                    remote: remote_name.clone(),
                                },
                                cx,
                            );
                        }));
                    let remote_name = remote.name.clone();
                    let reset = div()
                        .id(ElementId::Name(reset_id.clone().into()))
                        .opacity(if write_disabled { 0.45 } else { 1.0 })
                        .px(px(20.0))
                        .py(px(6.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(rgb(theme::palette().border))
                        .bg(rgb(theme::palette().bg_elevated))
                        .text_size(px(17.0))
                        .text_color(rgb(theme::palette().red))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .border_t_1()
                        .child(icon("refresh", 18.0).text_color(rgb(theme::palette().red)))
                        .tooltip(super::tooltip::text(if write_disabled {
                            format!("重置到 {}/{}\n{reason}", remote.name, detail.current_branch)
                        } else {
                            format!("重置到 {}/{}", remote.name, detail.current_branch)
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(format!("重置到 {}/{}", remote.name, detail.current_branch)),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if write_disabled {
                                return;
                            }
                            this.close_popup(super::popup::PopupKind::Remote, window);
                            this.confirm_reset_selected_to_remote(remote_name.clone(), window, cx);
                        }));

                    let safe_url = remote
                        .url
                        .as_ref()
                        .map(|url| crate::operation_log::redact(url))
                        .unwrap_or_default();
                    let menu_name = remote.name.clone();
                    let is_open = self.remote_menu.as_ref() == Some(&remote.name);
                    let more_id = format!("remote-more-{}", remote.name);
                    let popup =
                        super::popup::PopupBoundary::new(self, super::popup::PopupKind::Remote);
                    let picker_popup = super::popup::PopupBoundary::new(
                        self,
                        super::popup::PopupKind::RemotePicker,
                    );
                    let more = popup.attach(
                        div()
                            .relative()
                            .child(
                                self.track(
                                    &more_id,
                                    div()
                                        .id(ElementId::Name(more_id.clone().into()))
                                        .track_focus(&popup.focus)
                                        .px(px(6.0))
                                        .py(px(6.0))
                                        .focusable()
                                        .tab_stop(true)
                                        .keyboard_focus(|s| {
                                            s.border_1().border_color(rgb(theme::palette().accent))
                                        })
                                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                        .cursor_pointer()
                                        .child("⋯")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.remote_menu = if is_open {
                                                None
                                            } else {
                                                Some(menu_name.clone())
                                            };
                                            cx.notify();
                                        })),
                                ),
                            )
                            .children(is_open.then(|| {
                                deferred(
                                    popup.observe(
                                        div()
                                            .absolute()
                                            .right(px(0.0))
                                            .top(px(36.0))
                                            .w(px(288.0))
                                            .flex_shrink_0()
                                            .whitespace_nowrap()
                                            .occlude()
                                            .p(px(8.0))
                                            .bg(rgb(theme::palette().bg_surface))
                                            .border_1()
                                            .border_color(rgb(theme::palette().border))
                                            .shadow_md()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(
                                                self.track(
                                                    &format!("remote-copy-{}", remote.name),
                                                    div()
                                                        .id(ElementId::Name(
                                                            format!("remote-copy-{}", remote.name)
                                                                .into(),
                                                        ))
                                                        .px(px(12.0))
                                                        .py(px(6.0))
                                                        .focusable()
                                                        .tab_stop(true)
                                                        .keyboard_focus(|s| {
                                                            s.border_1().border_color(rgb(
                                                                theme::palette().accent,
                                                            ))
                                                        })
                                                        .active(|s| {
                                                            s.bg(rgb(theme::palette().bg_overlay))
                                                        })
                                                        .cursor_pointer()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(10.0))
                                                        .child(icon("copy", 18.0))
                                                        .child("复制脱敏地址")
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.close_popup(
                                                                    super::popup::PopupKind::Remote,
                                                                    window,
                                                                );
                                                                cx.notify();
                                                                cx.write_to_clipboard(
                                                                    ClipboardItem::new_string(
                                                                        safe_url.clone(),
                                                                    ),
                                                                )
                                                            },
                                                        )),
                                                ),
                                            )
                                            .child(
                                                self.track(
                                                    &format!("remote-refresh-{}", remote.name),
                                                    div()
                                                        .id(ElementId::Name(
                                                            format!(
                                                                "remote-refresh-{}",
                                                                remote.name
                                                            )
                                                            .into(),
                                                        ))
                                                        .px(px(12.0))
                                                        .py(px(6.0))
                                                        .focusable()
                                                        .tab_stop(true)
                                                        .keyboard_focus(|s| {
                                                            s.border_1().border_color(rgb(
                                                                theme::palette().accent,
                                                            ))
                                                        })
                                                        .active(|s| {
                                                            s.bg(rgb(theme::palette().bg_overlay))
                                                        })
                                                        .cursor_pointer()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(10.0))
                                                        .child(icon("refresh", 18.0))
                                                        .child("刷新本地状态")
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.close_popup(
                                                                    super::popup::PopupKind::Remote,
                                                                    window,
                                                                );
                                                                if !this.busy {
                                                                    this.reload_current_detail(cx);
                                                                }
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                            )
                                            .child(self.track(&reset_id, reset)),
                                    ),
                                )
                            })),
                        cx,
                    );
                    div()
                        .id(ElementId::Name(
                            format!("log-remote-{}", remote.name).into(),
                        ))
                        .flex()
                        .flex_row()
                        .flex_1()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(14.0))
                        .py(px(0.0))
                        .bg(rgb(theme::palette().bg_base))
                        .child(
                            picker_popup.attach(
                                div()
                                    .relative()
                                    .child(
                                        self.track(
                                            "remote-picker",
                                            div()
                                                .id("remote-picker")
                                                .track_focus(&picker_popup.focus)
                                                .flex()
                                                .items_center()
                                                .gap(px(10.0))
                                                .mr(px(18.0))
                                                .text_size(px(17.0))
                                                .focusable()
                                                .tab_stop(true)
                                                .keyboard_focus(|s| {
                                                    s.border_1()
                                                        .border_color(rgb(theme::palette().accent))
                                                })
                                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                                .cursor_pointer()
                                                .child(icon("cloud", 22.0))
                                                .child(format!("{} ▾", remote.name))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.remote_picker_open =
                                                        !this.remote_picker_open;
                                                    cx.notify();
                                                })),
                                        ),
                                    )
                                    .children(self.remote_picker_open.then(|| {
                                        deferred(
                                            picker_popup.observe(
                                                div()
                                                    .absolute()
                                                    .top(px(32.0))
                                                    .left(px(0.0))
                                                    .w(px(220.0))
                                                    .p(px(8.0))
                                                    .flex()
                                                    .flex_col()
                                                    .bg(rgb(theme::palette().bg_surface))
                                                    .border_1()
                                                    .border_color(rgb(theme::palette().border))
                                                    .occlude()
                                                    .children(detail.remotes.iter().map(
                                                        |remote| {
                                                            let name = remote.name.clone();
                                                            let id =
                                                                format!("remote-select-{name}");
                                                            self.track(
                                                    &id,
                                                    div()
                                                        .id(ElementId::Name(id.clone().into()))
                                                        .px(px(10.0))
                                                        .py(px(8.0))
                                                        .focusable()
                                                        .tab_stop(true)
                                                        .keyboard_focus(|s| {
                                                            s.border_1().border_color(rgb(
                                                                theme::palette().accent,
                                                            ))
                                                        })
                                                        .active(|s| {
                                                            s.bg(rgb(theme::palette().bg_overlay))
                                                        })
                                                        .cursor_pointer()
                                                        .child(name.clone())
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.overview_remote =
                                                                    Some(name.clone());
                                                                this.overview_reference = None;
                                                                this.close_popup(super::popup::PopupKind::RemotePicker, window);
                                                                this.remote_menu = None;
                                                                cx.notify();
                                                            },
                                                        )),
                                                )
                                                        },
                                                    )),
                                            ),
                                        )
                                    })),
                                cx,
                            ),
                        )
                        .child(div().flex_1())
                        .child(self.track(&fetch_id, fetch))
                        .child(self.track(&pull_id, pull))
                        .child(self.track(&push_id, push))
                        .child(more)
                        .into_any_element()
                })
                .collect()
        } else {
            Vec::new()
        };

        remote_controls
    }
}
