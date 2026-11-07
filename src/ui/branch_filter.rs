use super::focus_visibility::KeyboardFocusStyle;
use super::icons::icon;
use super::{log_actions::LogAction, theme};
use crate::app_state::GitMasterApp;
use gpui::prelude::FluentBuilder;
use gpui::*;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchPicker {
    Display,
    Checkout,
}
impl GitMasterApp {
    pub(super) fn close_branch_picker(&mut self, picker: BranchPicker, window: &mut Window) {
        self.branch_picker = None;
        self.branch_keyboard_index = None;
        if let Some(focus) = self.branch_trigger_focus.get(&picker) {
            window.focus(focus);
        }
    }
    pub(super) fn render_repository_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(detail) = self.detail.as_ref() else {
            return div().into_any_element();
        };
        let name = std::path::Path::new(&detail.path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap(px(6.0))
            .px(px(20.0))
            .pt(px(12.0))
            .pb(px(4.0))
            .child(
                self.track(
                    "repository-menu",
                    div()
                        .id("repository-menu")
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .tooltip(super::tooltip::text(detail.path.clone()))
                        .on_click(cx.listener(|this, _, window, cx| {
                            if let Some((index, _, target, _, _)) =
                                this.selected_remote_action_target()
                            {
                                this.open_context_menu(index, window.mouse_position());
                                if let Some(menu) = &mut this.context_menu {
                                    menu.target_path = Some(target);
                                }
                                cx.notify();
                            }
                        }))
                        .text_size(px(24.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(icon("folder", 24.0))
                        .child(name)
                        .child(icon("chevron-down", 16.0)),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_size(px(17.0))
                            .text_color(rgb(theme::palette().text_subtle))
                            .child("当前分支"),
                    )
                    .child(div().text_size(px(18.0)).child(
                        if detail.current_branch == "HEAD detached" {
                            format!(
                                "HEAD 分离 · {}",
                                detail
                                    .head_oid
                                    .as_deref()
                                    .unwrap_or("")
                                    .chars()
                                    .take(7)
                                    .collect::<String>()
                            )
                        } else {
                            detail.current_branch.clone()
                        },
                    ))
                    .child(self.branch_picker_button(
                        "checkout-branches",
                        "切换分支",
                        BranchPicker::Checkout,
                        cx,
                    ))
                    .children(self.render_remote_controls(cx)),
            )
            .into_any_element()
    }
    fn branch_picker_button(
        &self,
        id: &'static str,
        label: &str,
        picker: BranchPicker,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.branch_picker == Some(picker);
        let button = self.track(
            id,
            div()
                .id(id)
                .px(px(9.0))
                .py(px(5.0))
                .text_size(px(15.0))
                .track_focus(
                    self.branch_trigger_focus
                        .get(&picker)
                        .expect("branch inputs initialized"),
                )
                .tab_stop(true)
                .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                .cursor_pointer()
                .border_1()
                .border_color(rgb(theme::palette().border))
                .rounded(px(3.0))
                .child(format!("{label}  {}", if open { "▴" } else { "▾" }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.branch_keyboard_index = None;
                    if open {
                        this.close_branch_picker(picker, window);
                    } else {
                        this.branch_picker = Some(picker);
                        if let Some(focus) = &this.branch_menu_focus {
                            window.focus(focus);
                        }
                    }
                    cx.notify();
                })),
        );
        div()
            .relative()
            .child(button)
            .children(open.then(|| deferred(self.render_branch_popover(picker, cx))))
            .into_any_element()
    }
    pub(super) fn render_history_filter(&self, cx: &mut Context<Self>) -> Div {
        let visible = self
            .detail
            .as_ref()
            .map(|detail| {
                if self.log_visible_branches.is_empty() {
                    Self::default_log_visible_branches(detail)
                } else {
                    self.log_visible_branches.clone()
                }
            })
            .unwrap_or_default();
        let chips = visible
            .iter()
            .take(if self.narrow_window { 0 } else { 3 })
            .map(|branch| {
                let name = branch.clone();
                div()
                    .id(ElementId::Name(format!("branch-chip-{branch}").into()))
                    .text_size(px(15.0))
                    .focusable()
                    .tab_stop(true)
                    .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                    .cursor_pointer()
                    .text_color(rgb(theme::palette().text_subtle))
                    .child(format!("{branch}  ×"))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_log_branch(name.clone(), cx)),
                    )
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(60.0))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(theme::palette().border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .min_w(px(0.0))
                    .gap(px(12.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .child(
                        div()
                            .text_size(px(15.0))
                            .text_color(rgb(theme::palette().text_subtle))
                            .child("显示分支"),
                    )
                    .child(self.branch_picker_button(
                        "log-branch-picker",
                        &format!("已选 {} 个", visible.len()),
                        BranchPicker::Display,
                        cx,
                    ))
                    .children(chips),
            )
    }
    fn render_branch_popover(&self, picker: BranchPicker, cx: &mut Context<Self>) -> AnyElement {
        let query = self
            .branch_search
            .as_ref()
            .map(|input| input.read(cx).content.to_lowercase())
            .unwrap_or_default();
        let detail = self
            .detail
            .as_ref()
            .expect("branch controls need repository details");
        let visible = if self.log_visible_branches.is_empty() {
            Self::default_log_visible_branches(detail)
        } else {
            self.log_visible_branches.clone()
        };
        let rows = detail
            .branches
            .iter()
            .filter(|branch| branch.to_lowercase().contains(&query))
            .enumerate()
            .map(|(index, branch)| {
                let name = branch.clone();
                let selected = visible.contains(branch);
                let current = branch == &detail.current_branch;
                let id = if picker == BranchPicker::Display {
                    format!("log-branch-{branch}")
                } else {
                    format!("log-checkout-{branch}")
                };
                let label = format!("{branch}{}", if current { "  · 当前" } else { "" });
                let row = self.track(
                    &id,
                    div()
                        .id(ElementId::Name(id.clone().into()))
                        .flex_1()
                        .min_w(px(0.0))
                        .px(px(8.0))
                        .py(px(7.0))
                        .text_size(px(17.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .when(self.branch_keyboard_index == Some(index), |row| {
                            row.bg(rgb(theme::palette().accent_soft))
                                .border_1()
                                .border_color(rgb(theme::palette().accent))
                        })
                        .hover(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .children((picker == BranchPicker::Display).then(|| {
                            div()
                                .size(px(20.0))
                                .flex_shrink_0()
                                .rounded(px(3.0))
                                .border_1()
                                .border_color(rgb(if selected {
                                    theme::palette().accent
                                } else {
                                    theme::palette().text_muted
                                }))
                                .bg(rgb(if selected {
                                    theme::palette().accent
                                } else {
                                    theme::palette().bg_surface
                                }))
                                .children(selected.then(|| {
                                    icon("check", 18.0).text_color(rgb(theme::palette().bg_base))
                                }))
                        }))
                        .child(icon("git-branch", 18.0))
                        .tooltip(super::tooltip::text(branch.clone()))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(label),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if picker == BranchPicker::Display {
                                this.toggle_log_branch(name.clone(), cx);
                            } else if !current {
                                this.close_branch_picker(picker, window);
                                this.run_log_action(LogAction::Checkout(name.clone()), cx);
                            }
                        })),
                );
                div().flex().items_center().child(row).children(
                    (picker == BranchPicker::Checkout && !current).then(|| {
                        let name = branch.clone();
                        let id = format!("log-delete-{branch}");
                        self.track(
                            &id,
                            div()
                                .id(ElementId::Name(id.clone().into()))
                                .text_size(px(15.0))
                                .px(px(8.0))
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .cursor_pointer()
                                .text_color(rgb(theme::palette().red))
                                .child("删除")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.confirm_delete_branch(name.clone(), window, cx);
                                })),
                        )
                    }),
                )
            });
        div()
            .id("branch-popover")
            .on_mouse_down_out(cx.listener(move |this, _, window, cx| {
                if this.branch_picker == Some(picker) {
                    this.close_branch_picker(picker, window);
                    cx.notify();
                }
            }))
            .when_some(self.branch_menu_focus.as_ref(), |menu, focus| {
                menu.track_focus(focus)
            })
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.close_branch_picker(picker, window);
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                let query = this
                    .branch_search
                    .as_ref()
                    .map(|input| input.read(cx).content.to_lowercase())
                    .unwrap_or_default();
                let names = this
                    .detail
                    .as_ref()
                    .map(|d| {
                        d.branches
                            .iter()
                            .filter(|b| b.to_lowercase().contains(&query))
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if names.is_empty() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "tab" | "down" | "up" => {
                        let backwards =
                            event.keystroke.key == "up" || event.keystroke.modifiers.shift;
                        let next = match this.branch_keyboard_index {
                            None => 0,
                            Some(i) if backwards => (i + names.len() - 1) % names.len(),
                            Some(i) => (i + 1) % names.len(),
                        };
                        this.branch_keyboard_index = Some(next);
                        if let Some(f) = &this.branch_menu_focus {
                            window.focus(f);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }
                    "space" | "enter"
                        if this
                            .branch_menu_focus
                            .as_ref()
                            .is_some_and(|f| f.is_focused(window)) =>
                    {
                        if let Some(name) = this
                            .branch_keyboard_index
                            .and_then(|i| names.get(i))
                            .cloned()
                        {
                            if picker == BranchPicker::Display {
                                this.toggle_log_branch(name, cx);
                            } else {
                                this.close_branch_picker(picker, window);
                                this.run_log_action(LogAction::Checkout(name), cx);
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }
                    _ => {}
                }
            }))
            .absolute()
            .left(px(0.0))
            .top(px(38.0))
            .w(px(330.0))
            .occlude()
            .bg(rgb(theme::palette().bg_surface))
            .border_1()
            .border_color(rgb(theme::palette().border))
            .rounded(px(3.0))
            .p(px(10.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(div().text_size(px(17.0)).pb(px(4.0)).child(
                if picker == BranchPicker::Display {
                    "显示分支"
                } else {
                    "切换分支"
                },
            ))
            .child(
                div()
                    .text_size(px(15.0))
                    .pb(px(8.0))
                    .text_color(rgb(theme::palette().text_subtle))
                    .child(if picker == BranchPicker::Display {
                        "仅筛选历史，不切换分支"
                    } else {
                        "切换当前工作分支"
                    }),
            )
            .children(
                self.branch_search
                    .clone()
                    .map(|input| self.track("branch-search", div().child(input))),
            )
            .children((picker == BranchPicker::Display).then(|| {
                div()
                    .flex()
                    .gap(px(14.0))
                    .py(px(7.0))
                    .child(
                        self.track(
                            "log-branches-all",
                            div()
                                .id("log-branches-all")
                                .text_size(px(15.0))
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .cursor_pointer()
                                .child("全选")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if this.busy || this.loading_detail {
                                        return;
                                    }
                                    if let Some(detail) = &this.detail {
                                        this.log_visible_branches =
                                            detail.branches.iter().cloned().collect();
                                        this.reload_log_for_visible_branches(cx);
                                    }
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        self.track(
                            "log-branches-current",
                            div()
                                .id("log-branches-current")
                                .text_size(px(15.0))
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .cursor_pointer()
                                .child("仅当前")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if this.busy || this.loading_detail {
                                        return;
                                    }
                                    if let Some(detail) = &this.detail {
                                        this.log_visible_branches =
                                            [detail.current_branch.clone()].into_iter().collect();
                                        this.reload_log_for_visible_branches(cx);
                                    }
                                    cx.notify();
                                })),
                        ),
                    )
            }))
            .child(
                div()
                    .id("branch-options")
                    .max_h(px(300.0))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .pt(px(8.0))
                    .text_color(rgb(theme::palette().text_subtle))
                    .child("筛选同时应用于列表与关系画布。"),
            )
            .into_any_element()
    }
}
