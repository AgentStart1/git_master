use gpui::*;

use crate::app_state::GitMasterApp;
use crate::git_ops;
use crate::ui::theme;

impl GitMasterApp {
    pub fn render_repo_list(
        &self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let repo_items: Vec<AnyElement> = self.repos.iter().enumerate().map(|(i, repo)| {
                let is_selected = self.selected_index == Some(i);
                let bg = if is_selected {
                    rgb(theme::BG_OVERLAY)
                } else {
                    rgb(theme::BG_BASE)
                };

                let dirty_color = if repo.is_dirty {
                    rgb(theme::RED)
                } else {
                    rgb(theme::GREEN)
                };
                let dirty_icon = if repo.is_dirty { "●" } else { "✓" };

                let ahead_behind: Option<String> = if repo.ahead > 0 || repo.behind > 0 {
                    Some(format!("↑{} ↓{}", repo.ahead, repo.behind))
                } else {
                    None
                };

                let item = div()
                    .id(ElementId::Name(format!("repo-{i}").into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .py(px(8.0))
                    .bg(bg)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let Some(repo) = this.repos.get(i) else {
                            return;
                        };
                        let path = repo.path.clone();
                        this.begin_select(i);
                        cx.notify();
                        cx.spawn(async move |entity, cx| {
                            let (detail, log_entries) = cx
                                .background_executor()
                                .spawn(async move {
                                    (
                                        git_ops::get_repo_detail(&path),
                                        git_ops::get_commit_log(&path, 200),
                                    )
                                })
                                .await;
                            entity
                                .update(cx, |this, cx| {
                                    this.apply_detail(i, detail, log_entries);
                                    cx.notify();
                                })
                                .ok();
                        })
                        .detach();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                            if this.busy {
                                return;
                            }
                            let Some(repo) = this.repos.get(i) else {
                                return;
                            };
                            let path = repo.path.clone();
                            let position = event.position;
                            cx.spawn(async move |entity, cx| {
                                let branches = cx
                                    .background_executor()
                                    .spawn(async move {
                                        git_ops::list_local_branches(&path)
                                    })
                                    .await;
                                entity
                                    .update(cx, |this, cx| {
                                        this.open_context_menu(i, position, branches);
                                        cx.notify();
                                    })
                                    .ok();
                            })
                            .detach();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_grow()
                            .overflow_x_hidden()
                            .child(div().text_sm().child(repo.name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme::TEXT_SUBTLE))
                                    .child(repo.current_branch.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(div().text_xs().text_color(dirty_color).child(dirty_icon))
                            .children(ahead_behind.map(|ab| {
                                div().text_xs().text_color(rgb(theme::YELLOW)).child(ab)
                            })),
                    );

                self.track(&format!("repo-{i}"), item)
            }).collect();

        div()
            .id("repo-list")
            .flex()
            .flex_col()
            .w(px(280.0))
            .min_w(px(280.0))
            .h_full()
            .bg(rgb(theme::BG_BASE))
            .border_r_1()
            .border_color(rgb(theme::BG_OVERLAY))
            .overflow_y_scroll()
            .children(self.scanning.then(|| {
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .text_xs()
                    .text_color(rgb(theme::TEXT_SUBTLE))
                    .child("Scanning…")
            }))
            .children(repo_items)
            .into_any_element()
    }

    pub fn render_context_menu(
        &self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        let menu = self.context_menu.as_ref()?;
        let repo_index = menu.repo_index;
        let position = menu.position;
        let show_branches = menu.show_branches;
        let current_branch = self
            .repos
            .get(repo_index)
            .map(|r| r.current_branch.clone())
            .unwrap_or_default();

        let branch_items: Vec<AnyElement> = if show_branches {
            menu.branches
                .iter()
                .filter(|b| **b != current_branch)
                .map(|branch| {
                    let branch_name = branch.clone();
                    let id_str = format!("ctx-branch-{branch_name}");
                    let item = div()
                        .id(ElementId::Name(id_str.clone().into()))
                        .px(px(24.0))
                        .py(px(6.0))
                        .text_xs()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::BG_OVERLAY)))
                        .child(branch_name.clone())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.close_context_menu();
                            this.do_checkout(repo_index, branch_name.clone(), cx);
                        }));
                    self.track(&id_str, item)
                })
                .collect()
        } else {
            Vec::new()
        };

        let switch_branch = div()
            .id("ctx-switch-branch")
            .px(px(12.0))
            .py(px(6.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_OVERLAY)))
            .child(if show_branches {
                "▾ Switch Branch"
            } else {
                "▸ Switch Branch"
            })
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(menu) = this.context_menu.as_mut() {
                    menu.show_branches = !menu.show_branches;
                }
                cx.notify();
            }));

        let pull_rebase = div()
            .id("ctx-pull-rebase")
            .px(px(12.0))
            .py(px(6.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_OVERLAY)))
            .child("Pull --rebase")
            .on_click(cx.listener(move |this, _, _, cx| {
                this.close_context_menu();
                this.do_pull_rebase(repo_index, cx);
            }));

        let push = div()
            .id("ctx-push")
            .px(px(12.0))
            .py(px(6.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_OVERLAY)))
            .child("Push")
            .on_click(cx.listener(move |this, _, window, cx| {
                this.close_context_menu();
                this.do_push(repo_index, window, cx);
            }));

        let menu_panel = div()
            .id("context-menu")
            .w(px(200.0))
            .bg(rgb(theme::BG_SURFACE))
            .border_1()
            .border_color(rgb(theme::BG_OVERLAY))
            .rounded(px(4.0))
            .py(px(4.0))
            .text_sm()
            .text_color(rgb(theme::TEXT_PRIMARY))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.close_context_menu();
                cx.notify();
            }))
            .child(self.track("ctx-switch-branch", switch_branch))
            .children(branch_items)
            .child(
                div()
                    .my(px(4.0))
                    .mx(px(8.0))
                    .h(px(1.0))
                    .bg(rgb(theme::BG_OVERLAY)),
            )
            .child(self.track("ctx-pull-rebase", pull_rebase))
            .child(self.track("ctx-push", push));

        Some(deferred(anchored().position(position).child(menu_panel)).into_any_element())
    }

    fn do_checkout(
        &mut self,
        repo_index: usize,
        branch: String,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(repo) = self.repos.get(repo_index) else {
            return;
        };
        let path = repo.path.clone();
        self.busy = true;
        self.set_status(format!("Checking out {branch}…"));
        cx.notify();
        let branch_clone = branch.clone();
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { git_ops::checkout_branch(&path, &branch_clone) })
                .await;
            entity
                .update(cx, |this, cx| {
                    match result {
                        Ok(_) => this.set_status(format!("Switched to {branch}")),
                        Err(e) => this.set_status(format!("Checkout failed: {e}")),
                    }
                    this.refresh_repo(repo_index);
                    this.busy = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn do_pull_rebase(
        &mut self,
        repo_index: usize,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(repo) = self.repos.get(repo_index) else {
            return;
        };
        let path = repo.path.clone();
        self.busy = true;
        self.set_status("Pulling --rebase…".to_string());
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { git_ops::pull_rebase(&path) })
                .await;
            entity
                .update(cx, |this, cx| {
                    match result {
                        Ok(msg) => {
                            if msg.is_empty() {
                                this.set_status("Pull --rebase done".to_string());
                            } else {
                                this.set_status(format!("Pull --rebase: {msg}"));
                            }
                        }
                        Err(e) => this.set_status(format!("Pull failed: {e}")),
                    }
                    this.refresh_repo(repo_index);
                    this.busy = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn do_push(
        &mut self,
        repo_index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(repo) = self.repos.get(repo_index) else {
            return;
        };
        let path = repo.path.clone();
        let branch = repo.current_branch.clone();

        let has_up = git_ops::has_upstream(&path, &branch);
        if has_up {
            self.do_push_inner(repo_index, path, false, cx);
        } else {
            let branch_display = branch.clone();
            let receiver = window.prompt(
                PromptLevel::Info,
                &format!("No upstream branch for '{branch_display}'"),
                Some(&format!(
                    "Create remote branch 'origin/{branch_display}' and push?"
                )),
                &[
                    PromptButton::ok("Push & Create"),
                    PromptButton::cancel("Cancel"),
                ],
                cx,
            );
            cx.spawn(async move |entity, cx| {
                if let Ok(answer) = receiver.await {
                    if answer == 0 {
                        entity
                            .update(cx, |this, cx| {
                                this.do_push_inner(repo_index, path, true, cx);
                            })
                            .ok();
                    }
                }
            })
            .detach();
        }
    }

    fn do_push_inner(
        &mut self,
        repo_index: usize,
        path: std::path::PathBuf,
        set_upstream: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let branch = self
            .repos
            .get(repo_index)
            .map(|r| r.current_branch.clone())
            .unwrap_or_default();
        self.busy = true;
        self.set_status("Pushing…".to_string());
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if set_upstream {
                        git_ops::push_set_upstream(&path, &branch)
                    } else {
                        git_ops::push(&path)
                    }
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    match result {
                        Ok(msg) => {
                            if msg.is_empty() {
                                this.set_status("Push done".to_string());
                            } else {
                                this.set_status(format!("Push: {msg}"));
                            }
                        }
                        Err(e) => this.set_status(format!("Push failed: {e}")),
                    }
                    this.refresh_repo(repo_index);
                    this.busy = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}
