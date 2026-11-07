use super::{icons::icon, theme};
use crate::app_state::{DetailTab, GitMasterApp};
use gpui::*;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationTarget {
    pub path: PathBuf,
    pub branch: String,
}

pub struct BlockedAction {
    pub target: OperationTarget,
    pub fields: Vec<(String, String)>,
    pub selected: usize,
    pub title: String,
    pub error: String,
    pub guidance: String,
    pub recovery: Recovery,
}
pub enum Recovery {
    Changes,
    History,
    Terminal(PathBuf),
}
impl GitMasterApp {
    pub(crate) fn show_blocked(
        &mut self,
        target: OperationTarget,
        title: impl Into<String>,
        error: impl Into<String>,
        guidance: impl Into<String>,
        recovery: Recovery,
    ) {
        self.branch_picker = None;
        self.workspace_menu_open = false;
        self.close_context_menu();
        self.blocked_focus = None;
        self.blocked_action = Some(BlockedAction {
            fields: vec![
                ("仓库".into(), target.path.display().to_string()),
                ("当前分支".into(), target.branch.clone()),
            ],
            target,
            selected: 0,
            title: title.into(),
            error: crate::operation_log::redact(&error.into()),
            guidance: guidance.into(),
            recovery,
        });
    }
    fn select_operation_target(&mut self, path: &std::path::Path) -> bool {
        if self.selected_path().as_deref() == Some(path) {
            return true;
        }
        let target = self.repos.iter().enumerate().find_map(|(i, repo)| {
            if repo.path == path {
                Some((i, None))
            } else {
                repo.submodules
                    .iter()
                    .enumerate()
                    .find(|(_, sub)| sub.path == path)
                    .map(|(j, sub)| (i, Some((j, sub.relative_path.clone()))))
            }
        });
        match target {
            Some((i, None)) => self.begin_select(i),
            Some((i, Some((j, relative)))) => self.begin_select_submodule(i, j, relative),
            None => return false,
        }
        true
    }
    fn recover_blocked(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.blocked_action.take() {
            if !matches!(state.recovery, Recovery::Terminal(_))
                && !self.select_operation_target(&state.target.path)
            {
                self.set_warning(format!(
                    "原仓库已不在当前工作区：{}",
                    state.target.path.display()
                ));
                return;
            }
            match state.recovery {
                Recovery::Changes => {
                    self.set_tab(DetailTab::GitChange);
                    self.reload_current_detail(cx);
                }
                Recovery::History => {
                    self.set_tab(DetailTab::GitLog);
                    self.reload_current_detail(cx);
                }
                Recovery::Terminal(path) => self.open_overview_location(path, true, cx),
            }
        }
    }
    pub(crate) fn render_blocked_action(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.blocked_action.as_ref()?;
        let p = theme::palette();
        let label = match state.recovery {
            Recovery::Changes => "查看变更",
            Recovery::History => "查看历史",
            Recovery::Terminal(_) => "打开终端",
        };
        Some(
            div()
                .absolute()
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .inset_0()
                .bg(rgba(0x00000055))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .track_focus(self.blocked_focus.as_ref()?)
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            match event.keystroke.key.as_str() {
                                "escape" => {
                                    this.blocked_action = None;
                                }
                                "tab" | "left" | "right" => {
                                    if let Some(s) = &mut this.blocked_action {
                                        s.selected = 1 - s.selected;
                                    }
                                }
                                "enter" | "space" => {}
                                _ => return,
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                            match event.keystroke.key.as_str() {
                                "enter" | "space" => {
                                    if this
                                        .blocked_action
                                        .as_ref()
                                        .is_some_and(|s| s.selected == 1)
                                    {
                                        this.recover_blocked(cx);
                                    } else {
                                        this.blocked_action = None;
                                    }
                                }
                                _ => return,
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .w(px(480.0))
                        .max_w(relative(0.95))
                        .p(px(24.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(rgb(p.border))
                        .bg(rgb(p.bg_base))
                        .shadow_lg()
                        .flex()
                        .flex_col()
                        .gap(px(22.0))
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .items_start()
                                .gap(px(14.0))
                                .text_size(px(23.0))
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_color(rgb(p.red))
                                        .child(icon("alert-circle", 26.0)),
                                )
                                .child(
                                    div()
                                        .id("blocked-title")
                                        .flex_1()
                                        .min_w_0()
                                        .max_h(px(120.0))
                                        .overflow_y_scroll()
                                        .whitespace_normal()
                                        .child(state.title.clone()),
                                ),
                        )
                        .children(state.fields.iter().map(|(label, value)| {
                            div()
                                .flex()
                                .gap(px(12.0))
                                .text_size(px(16.0))
                                .child(
                                    div()
                                        .w(px(78.0))
                                        .flex_shrink_0()
                                        .text_color(rgb(p.text_subtle))
                                        .child(label.clone()),
                                )
                                .child(div().flex_1().min_w_0().child(value.clone()))
                        }))
                        .child(
                            div()
                                .id("blocked-error")
                                .max_h(px(180.0))
                                .overflow_y_scroll()
                                .p(px(12.0))
                                .bg(rgb(p.bg_elevated))
                                .text_size(px(15.0))
                                .child(state.error.clone()),
                        )
                        .child(
                            div()
                                .border_t_1()
                                .border_color(rgb(p.border))
                                .pt(px(16.0))
                                .child(state.guidance.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(16.0))
                                .child(
                                    self.track(
                                        "blocked-close",
                                        div()
                                            .id("blocked-close")
                                            .flex_1()
                                            .p(px(10.0))
                                            .border_1()
                                            .border_color(rgb(if state.selected == 0 {
                                                p.accent
                                            } else {
                                                p.border
                                            }))
                                            .rounded(px(4.0))
                                            .text_center()
                                            .cursor_pointer()
                                            .child("关闭")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.blocked_action = None;
                                                cx.notify();
                                            })),
                                    ),
                                )
                                .child(
                                    self.track(
                                        "blocked-recover",
                                        div()
                                            .id("blocked-recover")
                                            .flex_1()
                                            .p(px(10.0))
                                            .border_2()
                                            .border_color(rgb(if state.selected == 1 {
                                                p.text_primary
                                            } else {
                                                p.accent
                                            }))
                                            .bg(rgb(p.accent_fill))
                                            .text_color(rgb(0xffffff))
                                            .rounded(px(4.0))
                                            .text_center()
                                            .cursor_pointer()
                                            .child(label)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.recover_blocked(cx);
                                                cx.notify();
                                            })),
                                    ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
