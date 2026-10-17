use super::theme;
use crate::app_state::GitMasterApp;
use crate::worktree::{Action, WorktreeHost};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl GitMasterApp {
    pub(crate) fn run_worktree(&mut self, action: Action, cx: &mut Context<'_, Self>) {
        if self.busy || self.loading_detail {
            return;
        }
        let Some((index, root, path, selection, _)) = self.selected_remote_action_target() else {
            return;
        };
        let mutating = action.mutates();
        if mutating && self.worktree_loading {
            return;
        }
        self.worktree_task = None;
        self.worktree_loading = true;
        if mutating {
            self.busy = true;
        }
        self.worktree = Some((path.clone(), Ok(Default::default())));
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
                    let refreshed = mutating
                        .then(|| crate::git_ops::build_repo_info(&root_path))
                        .flatten();
                    let detail = mutating
                        .then(|| crate::git_ops::get_repo_detail(&target))
                        .flatten();
                    (result, snapshot, refreshed, detail)
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    if mutating {
                        this.busy = false;
                        this.apply_repo_refresh(index, &root, refreshed);
                    }
                    if this.selected_remote_action_target().is_some_and(
                        |(_, _, current, selected, _)| current == path && selected == selection,
                    ) {
                        this.worktree_loading = false;
                        this.worktree = Some((path, snapshot));
                        if mutating {
                            this.detail = detail;
                        }
                        match result {
                            Ok(()) => this.set_status(if mutating {
                                "Git operation complete"
                            } else {
                                "Repository changes and stashes refreshed"
                            }),
                            Err(error) => this.set_status(format!("Git operation failed: {error}")),
                        }
                    } else {
                        this.worktree_loading = false;
                        if let Err(error) = result {
                            this.set_status(format!("Git operation failed: {error}"));
                        }
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
        let Some((_, _, path, selection, _)) = self.selected_remote_action_target() else {
            return;
        };
        self.busy = true;
        cx.notify();
        let entity = cx.entity().downgrade();
        self.remote_action_prompt_task = Some(window.spawn(cx, async move |cx| {
            let answer = cx.prompt(PromptLevel::Critical, "Delete this stash?", Some("The saved changes will be removed from the stash list. This cannot be undone from Git Master."), &[PromptButton::ok("Delete stash"), PromptButton::cancel("Cancel")]).await;
            entity.update(cx, |this, cx| {
                this.busy = false;
                if answer == Ok(0) && this.selected_remote_action_target().is_some_and(|(_, _, current, selected, _)| current == path && selected == selection) {
                    this.run_worktree(Action::Drop(oid), cx);
                }
                cx.notify();
            }).ok();
        }));
    }

    fn worktree_button(
        &self,
        id: String,
        label: &str,
        action: Action,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let disabled = self.busy || self.worktree_loading || self.loading_detail;
        let button = div()
            .id(ElementId::Name(id.clone().into()))
            .px(px(8.0))
            .py(px(4.0))
            .bg(rgb(theme::BG_OVERLAY))
            .text_xs()
            .child(label.to_owned())
            .when(!disabled, |button| {
                button
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Action::Drop(oid) = action {
                            this.confirm_drop_stash(oid, window, cx);
                        } else {
                            this.run_worktree(action.clone(), cx);
                        }
                    }))
            })
            .when(disabled, |button| {
                button.text_color(rgb(theme::TEXT_SUBTLE))
            });
        self.track(&id, button)
    }

    pub(super) fn render_worktree_tab(
        &self,
        stash: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let mut body = div()
            .id("worktree-content")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .gap(px(8.0))
            .p(px(12.0));
        let mut toolbar = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(self.worktree_button("worktree-refresh".into(), "Refresh", Action::Refresh, cx));
        if stash {
            toolbar = toolbar.child(self.worktree_button(
                "stash-save".into(),
                "Stash changes + untracked files",
                Action::SaveStash,
                cx,
            ));
        }
        body = body.child(toolbar);
        let current_path = self
            .selected_remote_action_target()
            .map(|(_, _, path, _, _)| path);
        let data = self
            .worktree
            .as_ref()
            .filter(|(path, _)| Some(path) == current_path.as_ref());
        if self.worktree_loading {
            return body.child("Loading…").into_any_element();
        }
        let Some((_, result)) = data else {
            return body
                .child("Click Refresh to load repository changes and stashes.")
                .into_any_element();
        };
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return body
                    .child(format!("Cannot load repository: {error}"))
                    .into_any_element();
            }
        };
        let mut list = div()
            .id("worktree-list")
            .flex()
            .flex_col()
            .gap(px(6.0))
            .max_h(px(280.0))
            .overflow_y_scroll();
        if stash {
            if snapshot.stashes.is_empty() {
                list = list.child("No stashes.");
            }
            for entry in &snapshot.stashes {
                let id = entry.oid;
                let row = div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_sm()
                    .child(
                        div()
                            .flex_1()
                            .child(format!("stash@{{{}}} {}", entry.index, entry.message)),
                    )
                    .child(self.worktree_button(
                        format!("stash-show-{id}"),
                        "View",
                        Action::ShowStash(id),
                        cx,
                    ))
                    .child(self.worktree_button(
                        format!("stash-apply-{id}"),
                        "Apply",
                        Action::Apply(id),
                        cx,
                    ))
                    .child(self.worktree_button(
                        format!("stash-pop-{id}"),
                        "Pop",
                        Action::Pop(id),
                        cx,
                    ))
                    .child(self.worktree_button(
                        format!("stash-drop-{id}"),
                        "Delete",
                        Action::Drop(id),
                        cx,
                    ));
                list = list.child(row);
            }
        } else {
            if snapshot.changes.is_empty() {
                list = list.child("Working tree clean.");
            }
            for (index, entry) in snapshot.changes.iter().enumerate() {
                let mut row = div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_sm()
                    .child(div().flex_1().child(format!(
                        "{} — {}",
                        entry.path.display(),
                        entry.status
                    )))
                    .child(self.worktree_button(
                        format!("change-diff-{index}"),
                        "Diff",
                        Action::Diff(entry.path.clone()),
                        cx,
                    ));
                if entry.unstaged {
                    row = row.child(self.worktree_button(
                        format!("change-stage-{index}"),
                        "Stage",
                        Action::Stage(entry.path.clone()),
                        cx,
                    ));
                }
                if entry.staged {
                    row = row.child(self.worktree_button(
                        format!("change-unstage-{index}"),
                        "Unstage",
                        Action::Unstage(entry.path.clone()),
                        cx,
                    ));
                }
                list = list.child(row);
            }
        }
        body.child(list)
            .child(
                div()
                    .id("worktree-diff")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .overflow_x_scroll()
                    .p(px(8.0))
                    .bg(rgb(theme::BG_SURFACE))
                    .text_sm()
                    .child(snapshot.diff.clone()),
            )
            .into_any_element()
    }
}
