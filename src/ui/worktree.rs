use super::focus_visibility::KeyboardFocusStyle;
use super::theme;
use crate::app_state::GitMasterApp;
use crate::worktree::{Action, WorktreeHost};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl GitMasterApp {
    pub(crate) fn run_worktree(&mut self, action: Action, cx: &mut Context<'_, Self>) {
        self.start_worktree(action, true, cx);
    }

    pub(super) fn refresh_visible_worktree(&mut self, cx: &mut Context<'_, Self>) {
        if matches!(
            self.active_tab,
            crate::app_state::DetailTab::GitChange | crate::app_state::DetailTab::GitStash
        ) {
            self.start_worktree(Action::Refresh, false, cx);
        }
    }

    fn start_worktree(&mut self, action: Action, report_status: bool, cx: &mut Context<'_, Self>) {
        let mutating = action.mutates();
        let report_status = report_status && !self.busy;
        if (self.busy && mutating) || self.loading_detail {
            return;
        }
        let Some((_, root, path, _, _)) = self.selected_remote_action_target() else {
            return;
        };
        let operation_target = super::blocked_action::OperationTarget {
            path: path.clone(),
            branch: self
                .detail
                .as_ref()
                .map(|d| d.current_branch.clone())
                .unwrap_or_default(),
        };
        let failed_action = action.clone();
        let refresh_metadata = mutating || matches!(action, Action::Refresh);
        let action = if matches!(action, Action::Refresh) {
            match (
                self.worktree_preview.request().map(|r| &r.target),
                self.active_tab,
            ) {
                (
                    Some(crate::preview_state::PreviewTarget::Stash(oid)),
                    crate::app_state::DetailTab::GitStash,
                ) => Action::ShowStash(*oid),
                (
                    Some(crate::preview_state::PreviewTarget::File(file)),
                    crate::app_state::DetailTab::GitChange,
                ) => Action::Diff(file.clone()),
                _ => action,
            }
        } else {
            action
        };
        if mutating && self.worktree_loading {
            return;
        }
        self.worktree_task = None;
        self.worktree_revision += 1;
        let revision = self.worktree_revision;
        let preview_request = match &action {
            Action::Diff(file) => Some(crate::preview_state::PreviewTarget::File(file.clone())),
            Action::ShowStash(oid) => Some(crate::preview_state::PreviewTarget::Stash(*oid)),
            _ => None,
        }
        .map(|target| crate::preview_state::PreviewRequest {
            repository: path.clone(),
            target,
            revision,
        });
        self.worktree_preview.begin(preview_request.clone());
        self.worktree_loading = true;
        // Concurrent reads must preserve a running mutation's progress and outcome.
        if report_status {
            self.set_status(action.progress_message());
        }
        if mutating {
            self.busy = true;
        }
        if self
            .worktree
            .as_ref()
            .is_none_or(|(current, _)| current != &path)
        {
            self.worktree = Some((path.clone(), Ok(Default::default())));
        }
        cx.notify();
        let task = cx.spawn(async move |entity, cx| {
            let target = path.clone();
            let root_path = root.clone();
            let (result, snapshot, refreshed, detail) = cx
                .background_executor()
                .spawn(async move {
                    let (result, snapshot) = WorktreeHost {
                        path: target.clone(),
                        action,
                    }
                    .run();
                    let refreshed = refresh_metadata
                        .then(|| crate::git_ops::build_repo_info(&root_path))
                        .flatten();
                    let detail = refresh_metadata
                        .then(|| crate::git_ops::get_repo_detail(&target))
                        .flatten();
                    (result, snapshot, refreshed, detail)
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    let owns_request = this.owns_worktree_request(revision)
                        && this.selected_remote_action_target().is_some_and(
                            |(_, _, current, _, _)| current == path,
                        );
                    if !mutating && !owns_request {
                        return;
                    }
                    if mutating {
                        this.busy = false;
                    }
                    if refresh_metadata {
                        this.apply_repo_refresh(&root, refreshed);
                    }
                    if owns_request {
                        this.worktree_loading = false;
                        if mutating && let Err(error) = &result {
                            let (title, guidance, recovery) = match &failed_action {
                                Action::Pop(oid) => {
                                    let retained=snapshot.as_ref().ok().is_some_and(|s|s.stashes.iter().any(|e|e.oid==*oid));
                                    let conflicts=snapshot.as_ref().ok().map(|s|s.changes.iter().filter(|e|e.status.contains("Conflicted")).count()).unwrap_or(0);
                                    (if conflicts>0 {"应用暂存记录时发生冲突"}else{"应用暂存记录受阻"},format!("记录 {oid}：{}。{conflicts} 个冲突文件需要处理。已应用的部分可能仍在工作区，请查看实际状态后在外部处理并刷新。",if retained {"已保留，未删除"}else{"当前列表中无法确认，请刷新核对"}),super::blocked_action::Recovery::Changes)
                                },
                                Action::SaveStash => ("无法保存当前修改", "请在终端核对 Git 用户身份及错误原因，配置后重试。".into(), super::blocked_action::Recovery::Terminal(path.clone())),
                                _ => ("Git 操作受阻", "保留当前内容，请查看实际变更后重试。".into(), super::blocked_action::Recovery::Changes),
                            };
                            this.show_blocked(operation_target,title,error.clone(),guidance,recovery);
                        }
                        if let Some(request) = preview_request {
                            let preview = snapshot.clone().and_then(|s| match &s.preview_error {
                                Some(error) => Err(error.clone()), None => Ok(s),
                            });
                            this.worktree_preview.finish(request, preview);
                        }
                        this.worktree = Some((path, snapshot));
                        if refresh_metadata {
                            this.detail = detail;
                        }
                        if report_status {
                            match result {
                                Ok(()) => this.set_status(if mutating {
                                    "Git operation complete"
                                } else {
                                    "Repository changes and stashes refreshed"
                                }),
                                Err(error) => {
                                    this.set_failure(format!("Git operation failed: {error}"))
                                }
                            }
                        }
                    } else if mutating {
                        let failed=result.is_err();
                        this.set_status(match result {
                            Ok(()) => "Git operation complete".to_string(),
                            Err(error) => format!("Git operation failed: {error}"),
                        });
                        if failed {this.status_outcome=crate::app_state::TaskOutcome::Failure;}
                        this.refresh_visible_worktree(cx);
                    }
                    cx.notify();
                })
                .ok();
        });
        if mutating {
            self.operation_task = Some(task);
        } else {
            self.worktree_task = Some(task);
        }
    }

    fn confirm_drop_stash(
        &mut self,
        oid: git2::Oid,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.busy || self.worktree_loading {
            return;
        }
        let Some((_, _, path, _selection, _)) = self.selected_remote_action_target() else {
            return;
        };
        let record = self
            .worktree
            .as_ref()
            .and_then(|(_, r)| r.as_ref().ok())
            .and_then(|s| s.stashes.iter().find(|e| e.oid == oid));
        let title = record
            .map(|e| format!("删除 stash@{{{}}}？", e.index))
            .unwrap_or_else(|| "删除暂存记录？".into());
        let description = super::confirmation::details(
            "trash",
            vec![
                (
                    "记录",
                    record
                        .map(|e| e.message.clone())
                        .unwrap_or_else(|| oid.to_string()),
                ),
                ("提交", oid.to_string()),
            ],
            vec!["删除后无法从暂存记录列表恢复，当前工作区不变。".into()],
            None,
            vec![],
        );
        self.busy = true;
        self.set_status("Confirm stash deletion…");
        cx.notify();
        let entity = cx.entity().downgrade();
        self.remote_action_prompt_task = Some(window.spawn(cx, async move |cx| {
            let answer = cx
                .prompt(
                    PromptLevel::Critical,
                    &title,
                    Some(&description),
                    &[
                        PromptButton::cancel("取消"),
                        PromptButton::ok("删除暂存记录"),
                    ],
                )
                .await;
            entity
                .update(cx, |this, cx| {
                    this.busy = false;
                    if answer == Ok(1)
                        && this
                            .selected_remote_action_target()
                            .is_some_and(|(_, _, current, _, _)| current == path)
                    {
                        this.run_worktree(Action::Drop(oid), cx);
                    } else {
                        this.set_skipped("Stash deletion cancelled");
                    }
                    cx.notify();
                })
                .ok();
        }));
    }

    fn confirm_stage_conflict(
        &mut self,
        path: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let expected = self
            .selected_remote_action_target()
            .map(|(_, _, path, _, _)| path);
        self.busy = true;
        let entity = cx.entity().downgrade();
        cx.notify();
        self.remote_action_prompt_task=Some(window.spawn(cx,async move|cx|{
            let answer=cx.prompt(PromptLevel::Warning,"确认已在外部解决冲突？",Some(&format!("文件：{}\n将当前文件内容写入暂存区。此操作不会自动合并，也不会根据冲突标记的有无判断解决结果。请确认已检查并保存正确内容。",path.display())),&[PromptButton::cancel("取消"),PromptButton::ok("确认并暂存")]).await;
            entity.update(cx,|this,cx|{this.busy=false;if answer==Ok(1)&&this.selected_remote_action_target().map(|(_,_,path,_,_)|path)==expected {this.run_worktree(Action::Stage(path),cx);}else{this.set_skipped("已取消标记解决");}cx.notify();}).ok();
        }));
    }

    pub(super) fn worktree_button(
        &self,
        id: String,
        label: &str,
        action: Action,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let nothing_to_save = matches!(action, Action::SaveStash)
            && self
                .worktree
                .as_ref()
                .and_then(|(_, r)| r.as_ref().ok())
                .is_some_and(|s| s.changes.is_empty());
        let disabled = self.busy || self.worktree_loading || self.loading_detail || nothing_to_save;
        let heading = id.starts_with("stash-select-");
        let primary = matches!(action, Action::SaveStash | Action::Stage(_));
        let danger = matches!(action, Action::Drop(_));
        let background = if heading {
            theme::palette().bg_base
        } else if primary && !disabled {
            theme::palette().accent_fill
        } else {
            theme::palette().bg_elevated
        };
        let foreground = if disabled {
            theme::palette().text_muted
        } else if primary {
            0xffffff
        } else if danger {
            theme::palette().red
        } else {
            theme::palette().text_primary
        };
        let button = div()
            .id(ElementId::Name(id.clone().into()))
            .px(px(if heading { 0.0 } else { 11.0 }))
            .py(px(if heading { 0.0 } else { 6.0 }))
            .rounded(px(3.0))
            .border_1()
            .border_color(rgb(if heading {
                theme::palette().bg_surface
            } else if primary && !disabled {
                theme::palette().accent
            } else {
                theme::palette().border
            }))
            .bg(rgb(background))
            .text_color(rgb(foreground))
            .text_size(px(15.0))
            .when(heading, |button| {
                button
                    .text_size(px(17.0))
                    .bg(rgba(0x00000000))
                    .border_color(rgba(0x00000000))
                    .font_weight(FontWeight::MEDIUM)
            })
            .child(label.to_owned())
            .when(disabled, |s| {
                s.tooltip(super::tooltip::text(if nothing_to_save {
                    "当前没有可保存的修改"
                } else {
                    "当前操作尚未完成，请稍候"
                }))
            })
            .when(!disabled, |button| {
                button
                    .focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .when(primary, |button| super::button_style::primary(button, true))
                    .when(!primary, |button| {
                        button
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .hover(|s| {
                                s.bg(rgb(if heading {
                                    theme::palette().bg_elevated
                                } else {
                                    theme::palette().bg_overlay
                                }))
                            })
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Action::Drop(oid) = action {
                            this.confirm_drop_stash(oid, window, cx);
                        } else if let Action::Stage(path) = &action {
                            if this
                                .worktree
                                .as_ref()
                                .and_then(|(_, r)| r.as_ref().ok())
                                .is_some_and(|snapshot| {
                                    snapshot.changes.iter().any(|entry| {
                                        entry.path == *path && entry.status.contains("Conflicted")
                                    })
                                })
                            {
                                this.confirm_stage_conflict(path.clone(), window, cx);
                            } else {
                                this.run_worktree(action.clone(), cx);
                            }
                        } else {
                            this.run_worktree(action.clone(), cx);
                        }
                    }))
            });
        self.track(&id, button)
    }
}
