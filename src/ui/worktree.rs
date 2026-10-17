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
        let Some((index, root, path, selection, _)) = self.selected_remote_action_target() else {
            return;
        };
        let refresh_metadata = mutating || matches!(action, Action::Refresh);
        if mutating && self.worktree_loading {
            return;
        }
        self.worktree_task = None;
        self.worktree_revision += 1;
        let revision = self.worktree_revision;
        self.worktree_loading = true;
        // Concurrent reads must preserve a running mutation's progress and outcome.
        if report_status {
            self.set_status(action.progress_message());
        }
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
                            |(_, _, current, selected, _)| current == path && selected == selection,
                        );
                    if !mutating && !owns_request {
                        return;
                    }
                    if mutating {
                        this.busy = false;
                    }
                    if refresh_metadata {
                        this.apply_repo_refresh(index, &root, refreshed);
                    }
                    if owns_request {
                        this.worktree_loading = false;
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
                                    this.set_status(format!("Git operation failed: {error}"))
                                }
                            }
                        }
                    } else if mutating {
                        this.set_status(match result {
                            Ok(()) => "Git operation complete".to_string(),
                            Err(error) => format!("Git operation failed: {error}"),
                        });
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
        let Some((_, _, path, selection, _)) = self.selected_remote_action_target() else {
            return;
        };
        self.busy = true;
        self.set_status("Confirm stash deletion…");
        cx.notify();
        let entity = cx.entity().downgrade();
        self.remote_action_prompt_task = Some(window.spawn(cx, async move |cx| {
            let answer = cx.prompt(PromptLevel::Critical, "Delete this stash?", Some("The saved changes will be removed from the stash list. This cannot be undone from Git Master."), &[PromptButton::ok("Delete stash"), PromptButton::cancel("Cancel")]).await;
            entity.update(cx, |this, cx| {
                this.busy = false;
                if answer == Ok(0) && this.selected_remote_action_target().is_some_and(|(_, _, current, selected, _)| current == path && selected == selection) {
                    this.run_worktree(Action::Drop(oid), cx);
                } else {
                    this.set_status("Stash deletion cancelled");
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
        let heading = id.starts_with("stash-select-");
        let primary = matches!(action, Action::SaveStash | Action::Stage(_));
        let danger = matches!(action, Action::Drop(_));
        let background = if heading {
            theme::BG_SURFACE
        } else if primary && !disabled {
            theme::ACCENT
        } else {
            theme::BG_ELEVATED
        };
        let foreground = if disabled {
            theme::TEXT_MUTED
        } else if primary {
            theme::BG_BASE
        } else if danger {
            theme::RED
        } else {
            theme::TEXT_PRIMARY
        };
        let button = div()
            .id(ElementId::Name(id.clone().into()))
            .px(px(if heading { 0.0 } else { 11.0 }))
            .py(px(if heading { 0.0 } else { 6.0 }))
            .rounded(px(7.0))
            .border_1()
            .border_color(rgb(if heading {
                theme::BG_SURFACE
            } else if primary && !disabled {
                theme::ACCENT
            } else {
                theme::BORDER
            }))
            .bg(rgb(background))
            .text_color(rgb(foreground))
            .text_xs()
            .when(heading, |button| {
                button.text_sm().font_weight(FontWeight::SEMIBOLD)
            })
            .child(label.to_owned())
            .when(!disabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| {
                        style.bg(rgb(if heading {
                            theme::BG_ELEVATED
                        } else if primary {
                            0x8bbaff
                        } else {
                            theme::BG_OVERLAY
                        }))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Action::Drop(oid) = action {
                            this.confirm_drop_stash(oid, window, cx);
                        } else {
                            this.run_worktree(action.clone(), cx);
                        }
                    }))
            });
        self.track(&id, button)
    }

    pub(super) fn render_worktree_tab(
        &self,
        stash: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let current_path = self
            .selected_remote_action_target()
            .map(|(_, _, path, _, _)| path);
        let data = self
            .worktree
            .as_ref()
            .filter(|(path, _)| Some(path) == current_path.as_ref());
        let snapshot = data.and_then(|(_, result)| result.as_ref().ok());
        let count = snapshot.filter(|_| !self.worktree_loading).map(|data| {
            if stash {
                data.stashes.len()
            } else {
                data.changes.len()
            }
        });
        let mut toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.0))
            .p(px(16.0))
            .bg(rgb(theme::BG_SURFACE))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .rounded(px(10.0))
            .child(
                div()
                    .flex_1()
                    .min_w(px(180.0))
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if stash { "Git Stash" } else { "Git Change" }),
                            )
                            .children(count.map(|count| {
                                badge(
                                    &format!(
                                        "{count} {}",
                                        if stash {
                                            "saved"
                                        } else if count == 1 {
                                            "file"
                                        } else {
                                            "files"
                                        }
                                    ),
                                    theme::ACCENT,
                                )
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_SUBTLE))
                            .child(if stash {
                                "Save and restore staged, working-tree and untracked changes."
                            } else {
                                "Review file changes and manage the staging area."
                            }),
                    ),
            )
            .child(self.worktree_button("worktree-refresh".into(), "Refresh", Action::Refresh, cx));
        if stash {
            toolbar = toolbar.child(self.worktree_button(
                "stash-save".into(),
                "Save stash",
                Action::SaveStash,
                cx,
            ));
        }
        let body = div()
            .id("worktree-content")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .gap(px(12.0))
            .p(px(20.0))
            .child(toolbar);
        if self.worktree_loading {
            return body
                .child(state_card(
                    "Reading repository",
                    "Loading file changes and saved stashes…",
                    theme::ACCENT,
                ))
                .into_any_element();
        }
        let Some((_, result)) = data else {
            return body
                .child(state_card(
                    "Ready to review",
                    "Refresh to load this repository's changes and stashes.",
                    theme::ACCENT,
                ))
                .into_any_element();
        };
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return body
                    .child(state_card("Cannot load repository", error, theme::RED))
                    .into_any_element();
            }
        };
        let mut list = div()
            .id("worktree-list")
            .flex()
            .flex_col()
            .gap(px(8.0))
            .max_h(px(240.0))
            .overflow_y_scroll();
        if stash {
            if snapshot.stashes.is_empty() {
                return body.child(state_card("No saved stashes", "Use Save stash to set aside your current changes, including untracked files.", theme::ACCENT)).into_any_element();
            }
            for entry in &snapshot.stashes {
                let id = entry.oid;
                let row = item_card()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .child(self.worktree_button(
                                format!("stash-select-{id}"),
                                &entry.message,
                                Action::ShowStash(id),
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(badge(
                                        &format!("stash@{{{}}}", entry.index),
                                        theme::ACCENT,
                                    ))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(theme::TEXT_MUTED))
                                            .child(id.to_string()[..7].to_owned()),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.0))
                            .items_center()
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
                            )),
                    );
                list = list.child(row);
            }
        } else {
            if snapshot.changes.is_empty() {
                return body
                    .child(state_card(
                        "Working tree clean",
                        "There are no file changes to review or stage.",
                        theme::GREEN,
                    ))
                    .into_any_element();
            }
            for (index, entry) in snapshot.changes.iter().enumerate() {
                let status_color =
                    if entry.status.contains("Conflicted") || entry.status.contains("Deleted") {
                        theme::RED
                    } else if entry.status.contains("Untracked") || entry.status.contains("Added") {
                        theme::GREEN
                    } else {
                        theme::YELLOW
                    };
                let mut actions = div().flex().flex_wrap().items_center().gap(px(6.0)).child(
                    self.worktree_button(
                        format!("change-diff-{index}"),
                        "Diff",
                        Action::Diff(entry.path.clone()),
                        cx,
                    ),
                );
                if entry.unstaged {
                    actions = actions.child(self.worktree_button(
                        format!("change-stage-{index}"),
                        "Stage",
                        Action::Stage(entry.path.clone()),
                        cx,
                    ));
                }
                if entry.staged {
                    actions = actions.child(self.worktree_button(
                        format!("change-unstage-{index}"),
                        "Unstage",
                        Action::Unstage(entry.path.clone()),
                        cx,
                    ));
                }
                list = list.child(
                    item_card()
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap(px(5.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(entry.path.display().to_string()),
                                )
                                .child(div().flex().child(badge(&entry.status, status_color))),
                        )
                        .child(actions),
                );
            }
        }
        body.child(list)
            .child(super::diff_view::render(&snapshot.diff_rows, stash))
            .into_any_element()
    }
}

fn badge(label: &str, color: u32) -> AnyElement {
    div()
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(5.0))
        .bg(rgb(theme::BG_ELEVATED))
        .text_xs()
        .text_color(rgb(color))
        .child(label.to_owned())
        .into_any_element()
}
fn item_card() -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(12.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(rgb(theme::BG_SURFACE))
        .border_1()
        .border_color(rgb(theme::BORDER))
}
fn state_card(title: &str, message: &str, color: u32) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .p(px(24.0))
        .bg(rgb(theme::BG_SURFACE))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .rounded(px(10.0))
        .child(
            div()
                .size(px(42.0))
                .rounded(px(12.0))
                .bg(rgb(theme::BG_ELEVATED))
                .text_color(rgb(color))
                .flex()
                .items_center()
                .justify_center()
                .text_lg()
                .child(if color == theme::GREEN {
                    "✓"
                } else if color == theme::RED {
                    "!"
                } else {
                    "◈"
                }),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .id("worktree-state-message")
                .max_h(px(160.0))
                .max_w_full()
                .overflow_y_scroll()
                .overflow_x_scroll()
                .text_xs()
                .text_color(rgb(theme::TEXT_SUBTLE))
                .child(message.to_owned()),
        )
        .into_any_element()
}
