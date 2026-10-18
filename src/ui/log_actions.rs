use gpui::*;

use super::commit_canvas;
use crate::{app_state::GitMasterApp, git_ops};

pub(super) enum LogAction {
    Checkout(String),
    Delete(String),
    Pull { remote: String },
    Push { remote: String },
}

impl GitMasterApp {
    fn delete_confirmation_matches(&self, expected: &(String, String)) -> bool {
        !self.loading_detail
            && self
                .detail
                .as_ref()
                .map(|detail| (detail.path.clone(), detail.current_branch.clone()))
                .as_ref()
                == Some(expected)
    }

    pub(super) fn confirm_delete_branch(
        &mut self,
        branch: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.busy || self.loading_detail {
            return;
        }
        let Some(expected) = self
            .detail
            .as_ref()
            .map(|detail| (detail.path.clone(), detail.current_branch.clone()))
        else {
            return;
        };
        self.busy = true;
        self.set_status(format!("Confirm deletion of {branch}…"));
        cx.notify();
        let entity = cx.entity().downgrade();
        self.remote_action_prompt_task = Some(window.spawn(cx, async move |cx| {
            let answer = cx
                .prompt(
                    PromptLevel::Critical,
                    &format!("Delete local branch '{branch}'?"),
                    Some("Git will refuse to delete the branch if it contains unmerged commits."),
                    &[
                        PromptButton::ok("Delete branch"),
                        PromptButton::cancel("Cancel"),
                    ],
                )
                .await;
            entity
                .update(cx, |this, cx| {
                    if answer == Ok(0) && this.delete_confirmation_matches(&expected) {
                        this.run_log_action(LogAction::Delete(branch), cx);
                    } else {
                        this.busy = false;
                        this.set_status("Branch deletion cancelled");
                        cx.notify();
                    }
                })
                .ok();
        }));
    }

    pub(super) fn run_log_action(&mut self, action: LogAction, cx: &mut Context<'_, Self>) {
        if (self.busy && !matches!(action, LogAction::Delete(_))) || self.loading_detail {
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
        let visible_before = self.log_visible_branches.clone();
        let mut visible_after = visible_before.clone();
        if let LogAction::Checkout(branch) = &action {
            if visible_after.is_empty() {
                visible_after = self
                    .detail
                    .as_ref()
                    .map(Self::default_log_visible_branches)
                    .unwrap_or_default();
            }
            visible_after.insert(branch.clone());
        }
        if let LogAction::Delete(branch) = &action {
            visible_after.remove(branch);
            if visible_after.is_empty() {
                visible_after.insert(expected_branch.clone());
            }
        }
        let branches = visible_after.iter().cloned().collect::<Vec<_>>();
        let failed_branches = visible_before.into_iter().collect::<Vec<_>>();
        self.busy = true;
        self.detail_task = None;
        self.close_context_menu();
        self.set_status(match &action {
            LogAction::Checkout(branch) => format!("Switching to {branch}…"),
            LogAction::Delete(branch) => format!("Deleting {branch}…"),
            LogAction::Pull { remote } => {
                format!("Pulling {remote}/{expected_branch} with rebase…")
            }
            LogAction::Push { remote } => format!("Pushing {expected_branch} to {remote}…"),
        });
        cx.notify();
        let navigation = self.navigation_revision;
        self.operation_task = Some(cx.spawn(async move |entity, cx| {
            let expected_target = target.clone();
            let expected_root = root.clone();
            let operation_branch = expected_branch.clone();
            let (result, refreshed, detail, layout) = cx
                .background_executor()
                .spawn(async move {
                    let current =
                        git_ops::get_repo_detail(&target).map(|detail| detail.current_branch);
                    let result = if current.as_deref() != Some(&operation_branch) {
                        Err("Branch changed; reload the repository before retrying".to_owned())
                    } else {
                        match action {
                            LogAction::Checkout(branch) => {
                                git_ops::checkout_branch(&target, &branch)
                            }
                            LogAction::Delete(branch) => git_ops::delete_branch(&target, &branch),
                            LogAction::Pull { remote } => {
                                git_ops::pull_remote_rebase(&target, &remote, &operation_branch)
                            }
                            LogAction::Push { remote } => {
                                git_ops::push_remote(&target, &remote, &operation_branch)
                            }
                        }
                    };
                    // Publish a layout for the actual outcome. A failed mutation
                    // must not leave the success-only branch filter behind.
                    let layout = commit_canvas::load_layout_for_branches(
                        &target,
                        if result.is_ok() {
                            &branches
                        } else {
                            &failed_branches
                        },
                        200,
                    );
                    (
                        result,
                        git_ops::build_repo_info(&root),
                        git_ops::get_repo_detail(&target),
                        layout,
                    )
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    let action_succeeded = result.is_ok();
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
                        if this.navigation_revision != navigation {
                            this.reload_current_detail(cx);
                        } else {
                            if action_succeeded {
                                this.log_visible_branches = visible_after;
                            }
                            this.apply_detail(selection, detail, submodule, Vec::new(), layout);
                        }
                    }
                    this.refresh_visible_worktree(cx);
                    cx.notify();
                })
                .ok();
        }));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::models::{FileStatusSummary, RepoDetail};

    fn detail(path: &str, branch: &str) -> RepoDetail {
        RepoDetail {
            path: path.into(),
            current_branch: branch.into(),
            branches: vec![branch.into()],
            remotes: Vec::new(),
            head_labels: HashMap::new(),
            file_status: FileStatusSummary::default(),
        }
    }

    #[::core::prelude::v1::test]
    fn delete_confirmation_requires_the_original_loaded_repository() {
        let mut app = GitMasterApp::new();
        app.detail = Some(detail("/repos/original", "main"));
        let expected = ("/repos/original".to_string(), "main".to_string());

        assert!(app.delete_confirmation_matches(&expected));

        app.detail = Some(detail("/repos/other", "main"));
        assert!(!app.delete_confirmation_matches(&expected));

        app.detail = Some(detail("/repos/original", "main"));
        app.loading_detail = true;
        assert!(!app.delete_confirmation_matches(&expected));
    }
}
