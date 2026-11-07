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
        if let Some(picker) = self.branch_picker {
            self.close_branch_picker(picker, window);
        }
        self.close_context_menu();
        self.busy = true;
        self.set_status(format!("Confirm deletion of {branch}…"));
        cx.notify();
        let entity = cx.entity().downgrade();
        self.remote_action_prompt_task = Some(window.spawn(cx, async move |cx| {
            let check_path = expected.0.clone();
            let check_branch = branch.clone();
            let safe = cx
                .background_executor()
                .spawn(async move {
                    let repo = git2::Repository::open(check_path).map_err(|e| e.to_string())?;
                    let head = repo
                        .head()
                        .and_then(|h| h.peel_to_commit())
                        .map_err(|e| e.to_string())?;
                    let target = repo
                        .find_branch(&check_branch, git2::BranchType::Local)
                        .and_then(|b| b.into_reference().peel_to_commit())
                        .map_err(|e| e.to_string())?;
                    if target.id() == head.id()
                        || repo
                            .graph_descendant_of(head.id(), target.id())
                            .map_err(|e| e.to_string())?
                    {
                        Ok(())
                    } else {
                        Err("存在未合并提交；保留此分支，先查看历史。".to_owned())
                    }
                })
                .await;
            if let Err(error) = safe {
                entity
                    .update(cx, |this, cx| {
                        this.busy = false;
                        this.set_failure(format!("Branch deletion blocked: {error}"));
                        this.show_blocked(
                            super::blocked_action::OperationTarget {
                                path: expected.0.clone().into(),
                                branch: expected.1.clone(),
                            },
                            format!("无法安全删除 {branch}"),
                            error,
                            "仅删除非当前且已合并的本地分支，不删除远端分支。",
                            super::blocked_action::Recovery::History,
                        );
                        cx.notify();
                    })
                    .ok();
                return;
            }
            let description = super::confirmation::details(
                "trash",
                vec![("当前分支", expected.1.clone()), ("目标", branch.clone())],
                vec!["不删除远端分支。执行时仍由 Git 重新检查是否已合并。".into()],
                Some(("success", "已合并，可安全删除。".into())),
                vec![],
            );
            let answer = cx
                .prompt(
                    PromptLevel::Critical,
                    &format!("删除本地分支 '{branch}'？"),
                    Some(&description),
                    &[
                        PromptButton::cancel("取消"),
                        PromptButton::ok("删除本地分支"),
                    ],
                )
                .await;
            entity
                .update(cx, |this, cx| {
                    if answer == Ok(1) && this.delete_confirmation_matches(&expected) {
                        this.run_log_action(LogAction::Delete(branch), cx);
                    } else {
                        this.busy = false;
                        this.set_skipped("Branch deletion cancelled");
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
        if self.detail.as_ref().is_some_and(|d| d.git_state != "正常") {
            self.busy = false;
            self.set_warning("Git 操作尚未结束，请在外部处理后刷新，再执行分支或同步操作。");
            cx.notify();
            return;
        }
        let Some((_, root, target, _, submodule)) = self.selected_remote_action_target() else {
            return;
        };
        let Some(expected_branch) = self
            .detail
            .as_ref()
            .map(|detail| detail.current_branch.clone())
        else {
            return;
        };
        let failure_title = match &action {
            LogAction::Checkout(b) => format!("无法切换到 {b}"),
            LogAction::Delete(b) => format!("无法安全删除 {b}"),
            _ => "同步操作受阻".into(),
        };
        let deletion = matches!(action, LogAction::Delete(_));
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
        let request = self.begin_detail_request(target.clone());
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
                    if let Err(error) = &result {
                        this.show_blocked(
                            super::blocked_action::OperationTarget {
                                path: expected_target.clone(),
                                branch: expected_branch.clone(),
                            },
                            failure_title,
                            error.clone(),
                            if deletion {
                                "分支已保留。查看历史，确认合并后重试。"
                            } else {
                                "本地修改仍受保护。请先查看变更，在外部提交或保存到 Stash 后重试。"
                            },
                            if deletion {
                                super::blocked_action::Recovery::History
                            } else {
                                super::blocked_action::Recovery::Changes
                            },
                        );
                    }
                    this.set_status(match result {
                        Ok(_) => "Branch operation complete".into(),
                        Err(error) => format!("Branch operation failed: {error}"),
                    });
                    if !action_succeeded {
                        this.status_outcome = crate::app_state::TaskOutcome::Failure;
                    }
                    this.apply_repo_refresh(&expected_root, refreshed);
                    if this
                        .selected_remote_action_target()
                        .is_some_and(|(_, _, path, _, _)| path == expected_target)
                    {
                        if this.navigation_revision != navigation {
                            this.reload_current_detail(cx);
                        } else {
                            if action_succeeded {
                                this.log_visible_branches = visible_after;
                            }
                            this.apply_detail(request, detail, submodule, Vec::new(), layout);
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
            comparison_refs: vec![],
            upstream: None,
            head_oid: None,
            git_state: "正常".into(),
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
    #[::core::prelude::v1::test]
    fn blocked_result_keeps_original_repository_and_branch_after_navigation() {
        let mut app = GitMasterApp::new();
        app.detail = Some(detail("/repos/other", "other-branch"));
        let target = super::super::blocked_action::OperationTarget {
            path: "/repos/original".into(),
            branch: "original-branch".into(),
        };
        app.show_blocked(
            target.clone(),
            "Push failed",
            "rejected",
            "Review changes",
            super::super::blocked_action::Recovery::Changes,
        );
        let blocked = app.blocked_action.unwrap();
        assert_eq!(blocked.target, target);
        assert_eq!(blocked.fields[0].1, target.path.display().to_string());
        assert_eq!(blocked.fields[1].1, "original-branch");
    }
}
