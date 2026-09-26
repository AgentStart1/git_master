use crate::app_state::GitMasterApp;
use crate::ui::{commit_canvas::LogViewMode, theme};
use gpui::*;

impl GitMasterApp {
    pub(super) fn render_log_tab(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let list_bg = if self.log_view_mode == LogViewMode::List {
            rgb(theme::BG_OVERLAY)
        } else {
            rgb(theme::BG_SURFACE)
        };
        let canvas_bg = if self.log_view_mode == LogViewMode::Canvas {
            rgb(theme::BG_OVERLAY)
        } else {
            rgb(theme::BG_SURFACE)
        };
        let view_controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .px(px(10.0))
            .py(px(7.0))
            .bg(rgb(theme::BG_SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BG_OVERLAY))
            .child(
                div()
                    .id("log-view-list")
                    .px(px(10.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(list_bg)
                    .cursor_pointer()
                    .text_xs()
                    .child("List")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_log_view_mode(LogViewMode::List);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("log-view-canvas")
                    .px(px(10.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(canvas_bg)
                    .cursor_pointer()
                    .text_xs()
                    .child("Canvas")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_log_view_mode(LogViewMode::Canvas);
                        cx.notify();
                    })),
            );

        let remote_controls: Vec<AnyElement> = if let Some(detail) = self.detail.as_ref() {
            detail
                .remotes
                .iter()
                .map(|remote| {
                    let remote_name = remote.name.clone();
                    let fetch_id = format!("log-remote-{}-fetch", remote.name);
                    let reset_id = format!("log-remote-{}-reset", remote.name);
                    let fetch = div()
                        .id(ElementId::Name(fetch_id.clone().into()))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(rgb(theme::ACCENT))
                        .text_xs()
                        .text_color(rgb(theme::BG_BASE))
                        .cursor_pointer()
                        .child("Fetch")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.do_fetch_selected_remote(remote_name.clone(), cx);
                        }));
                    let remote_name = remote.name.clone();
                    let reset = div()
                        .id(ElementId::Name(reset_id.clone().into()))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(rgb(theme::RED))
                        .text_xs()
                        .text_color(rgb(theme::BG_BASE))
                        .cursor_pointer()
                        .child("Reset")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_reset_selected_to_remote(remote_name.clone(), window, cx);
                        }));

                    div()
                        .id(ElementId::Name(
                            format!("log-remote-{}", remote.name).into(),
                        ))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(8.0))
                        .py(px(5.0))
                        .rounded(px(4.0))
                        .bg(rgb(theme::BG_OVERLAY))
                        .child(div().text_xs().child(remote.name.clone()))
                        .child(self.track(&fetch_id, fetch))
                        .child(self.track(&reset_id, reset))
                        .into_any_element()
                })
                .collect()
        } else {
            Vec::new()
        };

        let branch_controls: Vec<AnyElement> = if let Some(detail) = self.detail.as_ref() {
            detail
                .branches
                .iter()
                .map(|branch| {
                    let branch_name = branch.clone();
                    let is_visible = if self.log_visible_branches.is_empty() {
                        Self::default_log_visible_branches(detail).contains(branch)
                    } else {
                        self.log_visible_branches.contains(branch)
                    };
                    let background = if is_visible {
                        rgb(theme::ACCENT)
                    } else {
                        rgb(theme::BG_OVERLAY)
                    };
                    let chip = div()
                        .id(ElementId::Name(format!("log-branch-{branch}").into()))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(background)
                        .text_xs()
                        .text_color(if is_visible {
                            rgb(theme::BG_BASE)
                        } else {
                            rgb(theme::TEXT_PRIMARY)
                        })
                        .cursor_pointer()
                        .child(format!(
                            "{} {}{}",
                            if is_visible { "☑" } else { "☐" },
                            branch,
                            if branch == &detail.current_branch {
                                " (current)"
                            } else {
                                ""
                            }
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_log_branch(branch_name.clone(), cx);
                        }));
                    let checkout_branch = branch.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.0))
                        .child(chip)
                        .children((branch != &detail.current_branch).then(|| {
                            div()
                                .id(ElementId::Name(format!("log-checkout-{branch}").into()))
                                .text_xs()
                                .px(px(6.0))
                                .py(px(4.0))
                                .cursor_pointer()
                                .child("Switch")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_log_action(
                                        super::log_actions::LogAction::Checkout(
                                            checkout_branch.clone(),
                                        ),
                                        cx,
                                    )
                                }))
                        }))
                        .into_any_element()
                })
                .collect()
        } else {
            Vec::new()
        };

        let toolbar = div()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BG_OVERLAY))
            .children((!branch_controls.is_empty()).then(|| {
                div()
                    .id("log-branches-scroll")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(10.0))
                    .pb(px(6.0))
                    .overflow_x_scroll()
                    .scrollbar_width(px(8.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_SUBTLE))
                            .child("Show branches"),
                    )
                    .children(branch_controls)
            }))
            .children((!remote_controls.is_empty()).then(|| {
                div()
                    .id("log-remotes-scroll")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(10.0))
                    .pb(px(7.0))
                    .overflow_x_scroll()
                    .scrollbar_width(px(8.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_SUBTLE))
                            .child("Remotes"),
                    )
                    .children(remote_controls)
            }));

        let toolbar = toolbar
            .child(self.render_branch_actions(cx))
            .child(view_controls);

        let body = match self.log_view_mode {
            LogViewMode::List => self.render_log_list(cx),
            LogViewMode::Canvas => self.render_commit_canvas(cx),
        };

        div()
            .flex()
            .flex_col()
            .flex_grow()
            .overflow_hidden()
            .child(toolbar)
            .child(body)
            .children(self.render_commit_details(cx))
            .into_any_element()
    }
}
