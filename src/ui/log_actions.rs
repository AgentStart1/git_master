use gpui::*;

use super::{commit_canvas, theme};
use crate::{
    app_state::{GitMasterApp, RepoSelection},
    git_ops,
};

pub(super) enum LogAction {
    Checkout(String),
    Pull,
    Push,
}

impl GitMasterApp {
    pub(super) fn render_branch_actions(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let branch = self
            .detail
            .as_ref()
            .map(|detail| detail.current_branch.clone())
            .unwrap_or_default();
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(6.0))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme::TEXT_SUBTLE))
                    .child(format!("Current: {branch}")),
            )
            .child(
                div()
                    .id("log-pull")
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(rgb(theme::BG_OVERLAY))
                    .text_xs()
                    .cursor_pointer()
                    .child("Pull --rebase")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.run_log_action(LogAction::Pull, cx)),
                    ),
            )
            .child(
                div()
                    .id("log-push")
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(rgb(theme::BG_OVERLAY))
                    .text_xs()
                    .cursor_pointer()
                    .child("Push")
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.busy || this.loading_detail {
                            return;
                        }
                        if let Some(RepoSelection::Repo(index)) = this.selected {
                            this.do_push(index, window, cx);
                        } else {
                            this.run_log_action(LogAction::Push, cx);
                        }
                    })),
            )
            .into_any_element()
    }

    pub(super) fn run_log_action(&mut self, action: LogAction, cx: &mut Context<'_, Self>) {
        if self.busy || self.loading_detail {
            return;
        }
        let Some((index, root, target, selection, submodule)) =
            self.selected_remote_action_target()
        else {
            return;
        };
        let Some(expected_branch) = self
            .detail
            .as_ref()
            .map(|detail| detail.current_branch.clone())
        else {
            return;
        };
        if let LogAction::Checkout(branch) = &action {
            if self.log_visible_branches.is_empty() {
                self.log_visible_branches = self
                    .detail
                    .as_ref()
                    .map(Self::default_log_visible_branches)
                    .unwrap_or_default();
            }
            self.log_visible_branches.insert(branch.clone());
        }
        let branches = self
            .log_visible_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.busy = true;
        self.detail_task = None;
        self.close_context_menu();
        self.set_status(match &action {
            LogAction::Checkout(branch) => format!("Switching to {branch}…"),
            LogAction::Pull => format!("Pulling {expected_branch}…"),
            LogAction::Push => format!("Pushing {expected_branch}…"),
        });
        cx.notify();
        self.operation_task = Some(cx.spawn(async move |entity, cx| {
            let expected_target = target.clone();
            let expected_root = root.clone();
            let (result, refreshed, detail, layout) = cx
                .background_executor()
                .spawn(async move {
                    let current =
                        git_ops::get_repo_detail(&target).map(|detail| detail.current_branch);
                    let result = if current.as_deref() != Some(&expected_branch) {
                        Err("Branch changed; reload the repository before retrying".to_owned())
                    } else {
                        match action {
                            LogAction::Checkout(branch) => {
                                git_ops::checkout_branch(&target, &branch)
                            }
                            LogAction::Pull => git_ops::pull_rebase(&target),
                            LogAction::Push => git_ops::push(&target),
                        }
                    };
                    (
                        result,
                        git_ops::build_repo_info(&root),
                        git_ops::get_repo_detail(&target),
                        commit_canvas::load_layout_for_branches(&target, &branches, 200),
                    )
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    this.busy = false;
                    this.set_status(match result {
                        Ok(_) => "Branch operation complete".into(),
                        Err(error) => format!("Branch operation failed: {error}"),
                    });
                    this.apply_repo_refresh(index, &expected_root, refreshed);
                    if this.selected_remote_action_target().is_some_and(
                        |(_, _, path, selected, _)| {
                            path == expected_target && selected == selection
                        },
                    ) {
                        this.apply_detail(selection, detail, submodule, Vec::new(), layout);
                    }
                    cx.notify();
                })
                .ok();
        }));
    }
}
