use gpui::*;

use crate::app_state::{DetailTab, GitMasterApp, RepoSelection};
use crate::git_ops;
use crate::models::{RepoDetail, SubmoduleDetail};
use crate::ui::commit_canvas::LogViewMode;
use crate::ui::{commit_canvas, theme};

impl GitMasterApp {
    pub fn render_detail_panel(
        &self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        self.selected.as_ref()?;

        let body = if self.loading_detail {
            div()
                .p(px(16.0))
                .text_sm()
                .text_color(rgb(theme::TEXT_SUBTLE))
                .child("Loading…")
                .into_any_element()
        } else if let Some(submodule) = self.submodule_detail.as_ref()
            && !submodule.is_initialized
        {
            self.render_uninitialized_submodule(submodule, cx)
        } else if let Some(detail) = self.detail.as_ref() {
            match self.active_tab {
                DetailTab::Info => self.render_info_tab(detail).into_any_element(),
                DetailTab::GitLog => self.render_log_tab(cx),
            }
        } else {
            div()
                .p(px(16.0))
                .text_sm()
                .text_color(rgb(theme::TEXT_SUBTLE))
                .child("Failed to open repository.")
                .into_any_element()
        };

        Some(
            div()
                .flex()
                .flex_col()
                .flex_grow()
                .bg(rgb(theme::BG_BASE))
                .child(self.render_tabs(cx))
                .child(body)
                .into_any_element(),
        )
    }

    fn render_tabs(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let info_bg = if self.active_tab == DetailTab::Info {
            rgb(theme::BG_OVERLAY)
        } else {
            rgb(theme::BG_SURFACE)
        };
        let log_bg = if self.active_tab == DetailTab::GitLog {
            rgb(theme::BG_OVERLAY)
        } else {
            rgb(theme::BG_SURFACE)
        };

        let tab_info = div()
            .id("tab-info")
            .px(px(16.0))
            .py(px(8.0))
            .cursor_pointer()
            .bg(info_bg)
            .text_sm()
            .child("Info")
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_tab(DetailTab::Info);
                cx.notify();
            }));

        let tab_log = div()
            .id("tab-log")
            .px(px(16.0))
            .py(px(8.0))
            .cursor_pointer()
            .bg(log_bg)
            .text_sm()
            .child("Git Log")
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_tab(DetailTab::GitLog);
                cx.notify();
            }));

        div()
            .flex()
            .flex_row()
            .bg(rgb(theme::BG_SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BG_OVERLAY))
            .child(self.track("tab-info", tab_info))
            .child(self.track("tab-log", tab_log))
            .into_any_element()
    }

    fn render_info_tab(&self, detail: &RepoDetail) -> impl IntoElement {
        let remote_rows: Vec<AnyElement> = if detail.remotes.is_empty() {
            vec![
                div()
                    .text_sm()
                    .text_color(rgb(theme::TEXT_SUBTLE))
                    .child("(none)")
                    .into_any_element(),
            ]
        } else {
            detail
                .remotes
                .iter()
                .map(|remote| {
                    div()
                        .id(ElementId::Name(format!("remote-{}", remote.name).into()))
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .p(px(10.0))
                        .bg(rgb(theme::BG_SURFACE))
                        .rounded(px(4.0))
                        .child(div().text_sm().child(remote.name.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_SUBTLE))
                                .child(remote.url.clone().unwrap_or_else(|| "(no URL)".into())),
                        )
                        .into_any_element()
                })
                .collect()
        };

        div()
            .id("repo-info-scroll")
            .flex()
            .flex_col()
            .flex_grow()
            .p(px(16.0))
            .gap(px(12.0))
            .overflow_y_scroll()
            .scrollbar_width(px(10.0))
            .child(info_row("Path", &detail.path))
            .child(info_row("Branch", &detail.current_branch))
            .child(info_row(
                "Remotes",
                &format!("{} configured", detail.remotes.len()),
            ))
            .children(remote_rows)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme::TEXT_SUBTLE))
                            .child("File Status"),
                    )
                    .child(div().text_sm().child(format!(
                        "{} new, {} modified, {} deleted, {} renamed, {} conflicted",
                        detail.file_status.new_files,
                        detail.file_status.modified,
                        detail.file_status.deleted,
                        detail.file_status.renamed,
                        detail.file_status.conflicted,
                    ))),
            )
    }

    fn render_uninitialized_submodule(
        &self,
        detail: &SubmoduleDetail,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let button = div()
            .id("init-submodule-btn")
            .px(px(12.0))
            .py(px(6.0))
            .bg(rgb(theme::ACCENT))
            .text_color(rgb(theme::BG_BASE))
            .rounded(px(4.0))
            .cursor_pointer()
            .text_sm()
            .child("Initialize Submodule")
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.do_init_selected_submodule(cx);
            }));

        div()
            .id("submodule-info-content")
            .flex()
            .flex_col()
            .p(px(16.0))
            .gap(px(12.0))
            .child(info_row("Name", &detail.name))
            .child(info_row("Path", &detail.path))
            .child(info_row("URL", detail.url.as_deref().unwrap_or("(none)")))
            .child(info_row("Status", "Not initialized"))
            .child(self.track("init-submodule-btn", button))
            .into_any_element()
    }

    fn render_log_tab(&self, cx: &mut Context<'_, Self>) -> AnyElement {
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

        let canvas_remote_controls: Vec<AnyElement> = if self.log_view_mode != LogViewMode::Canvas {
            Vec::new()
        } else if let Some(detail) = self.detail.as_ref() {
            detail
                .remotes
                .iter()
                .map(|remote| {
                    let remote_name = remote.name.clone();
                    let fetch_id = format!("canvas-remote-{}-fetch", remote.name);
                    let reset_id = format!("canvas-remote-{}-reset", remote.name);
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
                            format!("canvas-remote-{}", remote.name).into(),
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

        let canvas_branch_controls: Vec<AnyElement> = if self.log_view_mode != LogViewMode::Canvas {
            Vec::new()
        } else if let Some(detail) = self.detail.as_ref() {
            detail
                .branches
                .iter()
                .map(|branch| {
                    let branch_name = branch.clone();
                    let is_visible = if self.canvas_visible_branches.is_empty() {
                        Self::default_canvas_visible_branches(detail).contains(branch)
                    } else {
                        self.canvas_visible_branches.contains(branch)
                    };
                    let background = if is_visible {
                        rgb(theme::ACCENT)
                    } else {
                        rgb(theme::BG_OVERLAY)
                    };
                    div()
                        .id(ElementId::Name(format!("canvas-branch-{branch}").into()))
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
                        .child(branch.clone())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_canvas_branch(branch_name.clone(), cx);
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
            .child(view_controls)
            .children((!canvas_branch_controls.is_empty()).then(|| {
                div()
                    .id("canvas-branches-scroll")
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
                            .child("Branches"),
                    )
                    .children(canvas_branch_controls)
            }))
            .children((!canvas_remote_controls.is_empty()).then(|| {
                div()
                    .id("canvas-remotes-scroll")
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
                    .children(canvas_remote_controls)
            }));

        let body = match self.log_view_mode {
            LogViewMode::List => self.render_log_list(),
            LogViewMode::Canvas => self.render_commit_canvas(cx),
        };

        div()
            .flex()
            .flex_col()
            .flex_grow()
            .overflow_hidden()
            .child(toolbar)
            .child(body)
            .into_any_element()
    }

    fn render_log_list(&self) -> AnyElement {
        div()
            .id("log-scroll")
            .flex()
            .flex_col()
            .flex_grow()
            .overflow_y_scroll()
            .children(self.log_entries.iter().map(|entry| {
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.0))
                    .px(px(12.0))
                    .py(px(6.0))
                    .border_b_1()
                    .border_color(rgb(theme::BG_OVERLAY))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::YELLOW))
                            .w(px(56.0))
                            .child(entry.hash.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_grow()
                            .gap(px(2.0))
                            .child(div().text_sm().child(entry.message.clone()))
                            .children(
                                self.detail
                                    .as_ref()
                                    .and_then(|detail| detail.head_labels.get(&entry.full_hash))
                                    .map(|labels| {
                                        div().flex().flex_wrap().gap(px(4.0)).children(
                                            labels.iter().map(|label| {
                                                div()
                                                    .px(px(5.0))
                                                    .py(px(2.0))
                                                    .rounded(px(3.0))
                                                    .bg(rgb(theme::ACCENT))
                                                    .text_xs()
                                                    .text_color(rgb(theme::BG_BASE))
                                                    .child(label.clone())
                                            }),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme::TEXT_SUBTLE))
                                    .child(format!("{} — {}", entry.author, entry.date)),
                            ),
                    )
            }))
            .into_any_element()
    }
}

fn info_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_sm()
                .text_color(rgb(theme::TEXT_SUBTLE))
                .child(label.to_string()),
        )
        .child(div().text_sm().child(value.to_string()))
}

impl GitMasterApp {
    pub fn init_canvas_submodule(
        &mut self,
        path: std::path::PathBuf,
        relative: std::path::PathBuf,
        cx: &mut Context<'_, Self>,
    ) {
        if self.busy {
            return;
        }
        let Some((index, root, target, selection, submodule_detail)) =
            self.selected_remote_action_target()
        else {
            return;
        };
        if target != path {
            return;
        }
        let branches = self
            .canvas_visible_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.busy = true;
        self.detail_task = None;
        self.set_status(format!("Initializing {}…", relative.display()));
        cx.notify();
        self.operation_task = Some(cx.spawn(async move |entity, cx| {
            let expected_root = root.clone();
            let expected_path = path.clone();
            let (result, refreshed, detail, log, layout) = cx
                .background_executor()
                .spawn(async move {
                    let result = git_ops::init_submodule(&path, &relative);
                    (
                        result,
                        git_ops::build_repo_info(&root),
                        git_ops::get_repo_detail(&path),
                        git_ops::get_commit_log(&path, 200),
                        commit_canvas::load_layout_for_branches(&path, &branches, 200),
                    )
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    this.apply_repo_refresh(index, &expected_root, refreshed);
                    if this.selected_remote_action_target().is_some_and(
                        |(_, _, path, current, _)| path == expected_path && current == selection,
                    ) {
                        this.apply_detail(selection, detail, submodule_detail, log, layout);
                    }
                    this.busy = false;
                    this.set_status(match result {
                        Ok(_) => "Submodule initialized; canvas refreshed".into(),
                        Err(error) => format!("Submodule init failed: {error}"),
                    });
                    cx.notify();
                })
                .ok();
        }));
    }

    fn default_canvas_visible_branches(detail: &RepoDetail) -> std::collections::BTreeSet<String> {
        let mut branches = std::collections::BTreeSet::from([detail.current_branch.clone()]);
        if let Some(primary) = ["main", "master"]
            .into_iter()
            .find(|branch| detail.branches.iter().any(|candidate| candidate == branch))
        {
            branches.insert(primary.to_string());
        }
        branches
    }

    fn toggle_canvas_branch(&mut self, branch: String, cx: &mut Context<'_, Self>) {
        if self.busy {
            return;
        }
        if self.canvas_visible_branches.is_empty()
            && let Some(default_branches) = self
                .detail
                .as_ref()
                .map(Self::default_canvas_visible_branches)
        {
            self.canvas_visible_branches = default_branches;
        }
        if self.canvas_visible_branches.contains(&branch) {
            if self.canvas_visible_branches.len() == 1 {
                self.set_status("At least one branch must remain visible");
                cx.notify();
                return;
            }
            self.canvas_visible_branches.remove(&branch);
        } else {
            self.canvas_visible_branches.insert(branch);
        }
        self.reload_canvas_for_visible_branches(cx);
    }

    fn reload_canvas_for_visible_branches(&mut self, cx: &mut Context<'_, Self>) {
        let Some((selection, path)) =
            self.selected
                .as_ref()
                .and_then(|selection| match selection {
                    RepoSelection::Repo(repo_index) => self
                        .repos
                        .get(*repo_index)
                        .map(|repo| (selection.clone(), repo.path.clone())),
                    RepoSelection::Submodule {
                        repo_index,
                        submodule_index,
                        ..
                    } => self
                        .repos
                        .get(*repo_index)
                        .and_then(|repo| repo.submodules.get(*submodule_index))
                        .map(|submodule| (selection.clone(), submodule.path.clone())),
                })
        else {
            return;
        };
        let branches = self
            .canvas_visible_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.set_status(format!("Loading {} branch history…", branches.len()));
        cx.notify();
        self.detail_task = Some(cx.spawn(async move |entity, cx| {
            let layout = cx
                .background_executor()
                .spawn(
                    async move { commit_canvas::load_layout_for_branches(&path, &branches, 200) },
                )
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.selected.as_ref() != Some(&selection) {
                        return;
                    }
                    if let Some(layout) = layout.as_ref() {
                        this.commit_canvas_states
                            .entry(layout.repository_path.clone())
                            .or_default()
                            .ensure_layout(layout);
                    }
                    this.commit_canvas_layout = layout;
                    this.commit_canvas_interaction = None;
                    this.set_status("Canvas updated");
                    cx.notify();
                })
                .ok();
        }));
    }

    fn selected_remote_action_target(
        &self,
    ) -> Option<(
        usize,
        std::path::PathBuf,
        std::path::PathBuf,
        RepoSelection,
        Option<SubmoduleDetail>,
    )> {
        match self.selected.clone()? {
            selection @ RepoSelection::Repo(repo_index) => self.repos.get(repo_index).map(|repo| {
                (
                    repo_index,
                    repo.path.clone(),
                    repo.path.clone(),
                    selection,
                    None,
                )
            }),
            selection @ RepoSelection::Submodule {
                repo_index,
                submodule_index,
                ..
            } => self.repos.get(repo_index).and_then(|repo| {
                repo.submodules.get(submodule_index).map(|submodule| {
                    (
                        repo_index,
                        repo.path.clone(),
                        submodule.path.clone(),
                        selection,
                        Some(SubmoduleDetail {
                            name: submodule.name.clone(),
                            path: submodule.path.display().to_string(),
                            url: submodule.url.clone(),
                            is_initialized: submodule.is_initialized,
                        }),
                    )
                })
            }),
        }
    }

    fn do_fetch_selected_remote(&mut self, remote: String, cx: &mut Context<'_, Self>) {
        if self.busy {
            return;
        }
        self.perform_remote_action(remote, false, cx);
    }

    fn confirm_reset_selected_to_remote(
        &mut self,
        remote: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.busy || self.loading_detail || self.selected_remote_action_target().is_none() {
            return;
        }

        self.busy = true;
        self.set_status(format!("Confirm reset to {remote}…"));
        let expected = self
            .detail
            .as_ref()
            .map(|detail| (detail.path.clone(), detail.current_branch.clone()));
        cx.notify();
        let entity = cx.entity().downgrade();
        self.remote_action_prompt_task = Some(window.spawn(cx, async move |cx| {
            let answer = cx
                .prompt(
                    PromptLevel::Critical,
                    &format!("Reset current branch to '{remote}'?"),
                    Some(
                        "This fetches the remote, then permanently discards local commits and uncommitted changes on the current branch.",
                    ),
                    &[
                        PromptButton::ok("Reset and discard changes"),
                        PromptButton::cancel("Cancel"),
                    ],
                )
                .await;
            entity
                .update(cx, |this, cx| {
                    if answer == Ok(0) && !this.loading_detail
                        && this.detail.as_ref().map(|detail| (detail.path.clone(), detail.current_branch.clone())) == expected {
                        this.perform_remote_action(remote, true, cx);
                    } else {
                        this.busy = false;
                        this.set_status("Remote reset cancelled");
                        cx.notify();
                    }
                })
                .ok();
        }));
    }

    fn perform_remote_action(&mut self, remote: String, reset: bool, cx: &mut Context<'_, Self>) {
        let Some((repo_index, root_path, target_path, selection, submodule_detail)) =
            self.selected_remote_action_target()
        else {
            self.busy = false;
            return;
        };
        let branch = self
            .detail
            .as_ref()
            .map(|detail| detail.current_branch.clone())
            .unwrap_or_default();
        let canvas_branches = self
            .canvas_visible_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if reset && branch.is_empty() {
            self.busy = false;
            self.set_status("Cannot determine the current branch");
            cx.notify();
            return;
        }

        self.busy = true;
        self.set_status(if reset {
            format!("Resetting to {remote}/{branch}…")
        } else {
            format!("Fetching {remote}…")
        });
        cx.notify();
        self.operation_task = Some(cx.spawn(async move |entity, cx| {
            let refresh_path = root_path.clone();
            let detail_path = target_path.clone();
            let (result, refreshed, detail, log_entries, canvas_layout) = cx
                .background_executor()
                .spawn(async move {
                    let result = if reset {
                        git_ops::reset_to_remote_branch(&target_path, &remote, &branch)
                    } else {
                        git_ops::fetch_remote(&target_path, &remote)
                    };
                    (
                        result,
                        git_ops::build_repo_info(&root_path),
                        git_ops::get_repo_detail(&detail_path),
                        git_ops::get_commit_log(&detail_path, 200),
                        commit_canvas::load_layout_for_branches(
                            &detail_path,
                            &canvas_branches,
                            200,
                        ),
                    )
                })
                .await;

            entity
                .update(cx, |this, cx| {
                    match result {
                        Ok(_) if reset => this.set_status("Reset to remote complete"),
                        Ok(_) => this.set_status("Fetch complete; canvas refreshed"),
                        Err(error) => this.set_status(format!("Remote action failed: {error}")),
                    }
                    this.apply_repo_refresh(repo_index, &refresh_path, refreshed);
                    this.apply_detail(
                        selection,
                        detail,
                        submodule_detail,
                        log_entries,
                        canvas_layout,
                    );
                    this.busy = false;
                    cx.notify();
                })
                .ok();
        }));
    }

    fn do_init_selected_submodule(&mut self, cx: &mut Context<'_, Self>) {
        if self.busy {
            return;
        }
        let Some((repo_index, submodule_index, selected_relative_path)) = self.selected_submodule()
        else {
            return;
        };
        let Some((repo_path, relative_path)) = self.repos.get(repo_index).and_then(|repo| {
            repo.submodules
                .get(submodule_index)
                .map(|submodule| (repo.path.clone(), submodule.relative_path.clone()))
        }) else {
            return;
        };

        self.busy = true;
        self.set_status("Initializing submodule…");
        cx.notify();

        self.operation_task = Some(cx.spawn(async move |entity, cx| {
            let refresh_path = repo_path.clone();
            let (result, refreshed) = cx
                .background_executor()
                .spawn(async move {
                    let result = git_ops::init_submodule(&repo_path, &relative_path);
                    let refreshed = git_ops::build_repo_info(&repo_path);
                    (result, refreshed)
                })
                .await;

            entity
                .update(cx, |this, cx| {
                    match result {
                        Ok(msg) => {
                            if msg.is_empty() {
                                this.set_status("Submodule initialized");
                            } else {
                                this.set_status(format!("Submodule initialized: {msg}"));
                            }
                            this.apply_repo_refresh(repo_index, &refresh_path, refreshed);
                            let selected = this.selected_submodule();
                            if let Some((repo_index, submodule_index, relative_path)) = selected
                                && relative_path == selected_relative_path
                                && let Some(submodule) = this
                                    .repos
                                    .get(repo_index)
                                    .and_then(|repo| repo.submodules.get(submodule_index))
                            {
                                let path = submodule.path.clone();
                                let submodule_detail = Some(SubmoduleDetail {
                                    name: submodule.name.clone(),
                                    path: submodule.path.display().to_string(),
                                    url: submodule.url.clone(),
                                    is_initialized: submodule.is_initialized,
                                });
                                let selection = RepoSelection::Submodule {
                                    repo_index,
                                    submodule_index,
                                    relative_path,
                                };
                                this.loading_detail = true;
                                cx.notify();
                                this.detail_task = Some(cx.spawn(async move |entity, cx| {
                                    let (detail, log_entries, canvas_layout) = cx
                                        .background_executor()
                                        .spawn(async move {
                                            (
                                                git_ops::get_repo_detail(&path),
                                                git_ops::get_commit_log(&path, 200),
                                                commit_canvas::load_layout_for_path(&path, 200),
                                            )
                                        })
                                        .await;
                                    entity
                                        .update(cx, |this, cx| {
                                            this.apply_detail(
                                                selection,
                                                detail,
                                                submodule_detail,
                                                log_entries,
                                                canvas_layout,
                                            );
                                            cx.notify();
                                        })
                                        .ok();
                                }));
                            }
                        }
                        Err(e) => this.set_status(format!("Submodule init failed: {e}")),
                    }
                    this.busy = false;
                    cx.notify();
                })
                .ok();
        }));
    }
}
