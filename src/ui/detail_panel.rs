use super::focus_visibility::KeyboardFocusStyle;
use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app_state::{DetailTab, GitMasterApp, RepoSelection};
use crate::git_ops;
use crate::models::{RepoDetail, SubmoduleDetail};
use crate::ui::{commit_canvas, theme};

impl GitMasterApp {
    pub fn render_detail_panel(
        &self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        let query = self
            .repo_search
            .as_ref()
            .map(|input| input.read(cx).content.to_string())
            .unwrap_or_default();
        if !self.scanning
            && !self.repos.is_empty()
            && !self
                .repos
                .iter()
                .any(|r| super::workspace_controls::matches(r, &query, self.repo_filter))
        {
            return Some(
                super::state_views::message("search", "0 个结果", "没有匹配的仓库。")
                    .child(
                        div()
                            .flex()
                            .gap(px(16.0))
                            .child(
                                self.track(
                                    "zero-clear-search",
                                    div()
                                        .id("zero-clear-search")
                                        .p(px(10.0))
                                        .border_1()
                                        .border_color(rgb(theme::palette().border))
                                        .focusable()
                                        .tab_stop(true)
                                        .cursor_pointer()
                                        .child("清空搜索")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(input) = &this.repo_search {
                                                input.update(cx, |input, cx| input.clear(cx));
                                            }
                                            cx.notify();
                                        })),
                                ),
                            )
                            .child(
                                self.track(
                                    "zero-reset-filter",
                                    div()
                                        .id("zero-reset-filter")
                                        .p(px(10.0))
                                        .border_1()
                                        .border_color(rgb(theme::palette().border))
                                        .focusable()
                                        .tab_stop(true)
                                        .cursor_pointer()
                                        .child("重置筛选")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.repo_filter = Default::default();
                                            cx.notify();
                                        })),
                                ),
                            ),
                    )
                    .into_any_element(),
            );
        }
        if self.selected.is_none() {
            return Some(self.render_empty_detail(cx));
        }

        let body = if self.loading_detail && self.detail.is_none() {
            super::state_views::loading("正在读取仓库", "读取状态与提交历史，请稍候")
        } else if let Some(submodule) = self.submodule_detail.as_ref()
            && !submodule.is_initialized
        {
            self.render_uninitialized_submodule(submodule, cx)
        } else if let Some(detail) = self.detail.as_ref() {
            match self.active_tab {
                DetailTab::Info => self.render_info_tab(detail, cx).into_any_element(),
                DetailTab::GitLog => self.render_log_tab(cx),
                DetailTab::GitChange => self.render_worktree_tab(false, cx),
                DetailTab::GitStash => self.render_worktree_tab(true, cx),
            }
        } else {
            let path = self
                .selected_remote_action_target()
                .map(|(_, _, p, _, _)| p.display().to_string())
                .unwrap_or_default();
            super::state_views::message(
                "alert-circle",
                "无法读取仓库",
                "路径不存在或无访问权限。请确认所选仓库目录。",
            )
            .child(
                div()
                    .p(px(12.0))
                    .bg(rgb(theme::palette().bg_elevated))
                    .child(path),
            )
            .child(
                div()
                    .flex()
                    .gap(px(16.0))
                    .child(
                        self.track(
                            "detail-retry",
                            div()
                                .id("detail-retry")
                                .p(px(10.0))
                                .map(|button| super::button_style::primary(button, true))
                                .child("重新读取")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.reload_current_detail(cx);
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        div()
                            .id("detail-change-workspace")
                            .p(px(10.0))
                            .border_1()
                            .border_color(rgb(theme::palette().border))
                            .focusable()
                            .tab_stop(true)
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .cursor_pointer()
                            .child("更换工作区")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workspace_menu_open = true;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
        };

        Some(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .bg(rgb(theme::palette().bg_base))
                .child(self.render_repository_header(cx))
                .child(self.render_tabs(cx))
                .child(body)
                .into_any_element(),
        )
    }

    fn render_empty_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.scanning {
            return super::state_views::loading("正在扫描仓库…", "已有仓库列表刷新时保留原内容。");
        }
        let (title, description) = if self.parent_dir.is_none() {
            ("工作区未打开".to_owned(), "选择包含 Git 仓库的父目录。")
        } else if self.repos.is_empty() {
            (
                format!("{} · 0 个仓库", self.parent_dir.as_ref().unwrap().display()),
                "当前目录的直接子目录中未发现 Git 仓库。不递归扫描更深目录。",
            )
        } else {
            ("选择一个仓库".to_owned(), "从左侧查看变更与历史。")
        };
        let mut panel = super::state_views::message("folder", &title, description);
        if self.parent_dir.is_none() || self.repos.is_empty() {
            panel = panel.child(
                div()
                    .flex()
                    .gap(px(16.0))
                    .child(
                        self.track(
                            "empty-workspace-menu",
                            div()
                                .id("empty-workspace-menu")
                                .px(px(22.0))
                                .py(px(10.0))
                                .rounded(px(4.0))
                                .map(|button| super::button_style::primary(button, true))
                                .child(if self.parent_dir.is_none() {
                                    "打开工作区"
                                } else {
                                    "更换目录"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.workspace_menu_open = true;
                                    cx.notify();
                                })),
                        ),
                    )
                    .children(self.parent_dir.clone().map(|path| {
                        self.track(
                            "empty-rescan",
                            div()
                                .id("empty-rescan")
                                .px(px(22.0))
                                .py(px(10.0))
                                .border_1()
                                .border_color(rgb(theme::palette().border))
                                .rounded(px(4.0))
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .cursor_pointer()
                                .child("重新扫描")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.start_scan(path.clone(), cx)
                                })),
                        )
                    })),
            );
            if self.parent_dir.is_none()
                && let Some(path) = self.recent_workspaces.first()
            {
                let path = path.clone();
                let label = format!("最近使用：{}", path.display());
                panel = panel.child(
                    div()
                        .id("empty-recent-workspace")
                        .pt(px(24.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .child(label)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.start_scan(path.clone(), cx)),
                        ),
                );
            }
        }
        panel.into_any_element()
    }

    fn render_tabs(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let tabs = [
            (DetailTab::GitChange, "tab-change", "变更"),
            (DetailTab::GitLog, "tab-log", "历史"),
            (DetailTab::GitStash, "tab-stash", "暂存记录"),
            (DetailTab::Info, "tab-info", "概览"),
        ];

        div()
            .flex()
            .flex_row()
            .bg(rgb(theme::palette().bg_base))
            .border_b_1()
            .border_color(rgb(theme::palette().border))
            .children(tabs.into_iter().map(|(tab, id, label)| {
                let element = div()
                    .id(id)
                    .px(px(16.0))
                    .py(px(11.0))
                    .focusable()
                    .tab_stop(true)
                    .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                    .cursor_pointer()
                    .border_b_2()
                    .border_color(rgb(if self.active_tab == tab {
                        theme::palette().accent
                    } else {
                        theme::palette().bg_surface
                    }))
                    .text_color(rgb(if self.active_tab == tab {
                        theme::palette().text_primary
                    } else {
                        theme::palette().text_subtle
                    }))
                    .text_size(px(17.0))
                    .font_weight(if self.active_tab == tab {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_tab(tab);
                        if matches!(tab, DetailTab::GitChange | DetailTab::GitStash) {
                            this.run_worktree(crate::worktree::Action::Refresh, cx);
                        }
                        cx.notify();
                    }));
                self.track(id, element)
            }))
            .into_any_element()
    }

    fn render_uninitialized_submodule(
        &self,
        detail: &SubmoduleDetail,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let initialization = self.initialization.get(std::path::Path::new(&detail.path));
        let initializing = matches!(
            initialization,
            Some(super::state_views::Initialization::Running)
        );
        let failed = matches!(
            initialization,
            Some(super::state_views::Initialization::Failed(_))
        );
        let button = div()
            .id("init-submodule-btn")
            .px(px(12.0))
            .py(px(6.0))
            .rounded(px(3.0))
            .map(|button| super::button_style::primary(button, !self.busy))
            .text_size(px(17.0))
            .child(if initializing {
                "正在初始化…"
            } else if failed {
                "重试初始化"
            } else {
                "初始化子模块"
            })
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.do_init_selected_submodule(cx);
            }));

        div()
            .id("submodule-info-content")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(
                div()
                    .id("submodule-info-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(16.0))
                    .gap(px(12.0))
                    .child(info_row("名称", &detail.name))
                    .child(info_row("路径", &detail.path))
                    .child(info_row(
                        "来源",
                        &detail
                            .url
                            .as_ref()
                            .map(|s| crate::operation_log::redact(s))
                            .unwrap_or_else(|| "未设置".into()),
                    ))
                    .child(info_row("状态", "未初始化"))
                    .children(initialization.map(|state| {
                        div()
                            .id("submodule-initialization-state")
                            .max_h(px(160.0))
                            .flex_shrink_0()
                            .overflow_y_scroll()
                            .p(px(14.0))
                            .bg(rgb(theme::palette().bg_elevated))
                            .text_color(rgb(if failed {
                                theme::palette().red
                            } else {
                                theme::palette().accent
                            }))
                            .flex()
                            .items_start()
                            .gap(px(10.0))
                            .child(
                                super::icons::icon(
                                    if failed { "alert-circle" } else { "refresh" },
                                    20.0,
                                )
                                .text_color(rgb(if failed {
                                    theme::palette().red
                                } else {
                                    theme::palette().accent
                                })),
                            )
                            .child(div().flex_1().min_w_0().child(match state {
                                super::state_views::Initialization::Running => {
                                    "正在初始化… 其他仓库导航仍可正常使用。".into()
                                }
                                super::state_views::Initialization::Failed(e) => format!(
                                    "初始化失败 · {} · 请核对后重试",
                                    crate::operation_log::redact(e)
                                ),
                            }))
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .p(px(16.0))
                    .gap(px(12.0))
                    .border_t_1()
                    .border_color(rgb(theme::palette().border))
                    .child(self.track("init-submodule-btn", button))
                    .child(
                        self.track(
                            "submodule-copy-source",
                            div()
                                .id("submodule-copy-source")
                                .p(px(10.0))
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .cursor_pointer()
                                .child("复制地址")
                                .on_click({
                                    let source = detail
                                        .url
                                        .as_ref()
                                        .map(|s| crate::operation_log::redact(s))
                                        .unwrap_or_default();
                                    move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            source.clone(),
                                        ))
                                    }
                                }),
                        ),
                    ),
            )
            .into_any_element()
    }
}

fn info_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(17.0))
                .text_color(rgb(theme::palette().text_subtle))
                .child(label.to_string()),
        )
        .child(div().text_size(px(17.0)).child(value.to_string()))
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
        let Some((_, root, target, _, submodule_detail)) = self.selected_remote_action_target()
        else {
            return;
        };
        if target != path {
            return;
        }
        let branches = self
            .log_visible_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let request = self.begin_detail_request(path.clone());
        let module_path = path.join(&relative);
        self.initialization.insert(
            module_path.clone(),
            super::state_views::Initialization::Running,
        );
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
                    this.apply_repo_refresh(&expected_root, refreshed);
                    if this
                        .selected_remote_action_target()
                        .is_some_and(|(_, _, path, _, _)| path == expected_path)
                    {
                        this.apply_detail(request, detail, submodule_detail, log, layout);
                    }
                    match &result {
                        Ok(_) => {
                            this.initialization.remove(&module_path);
                        }
                        Err(e) => {
                            this.initialization.insert(
                                module_path,
                                super::state_views::Initialization::Failed(e.clone()),
                            );
                        }
                    }
                    this.busy = false;
                    let failed = result.is_err();
                    this.set_status(match result {
                        Ok(_) => "Submodule initialized; canvas refreshed".into(),
                        Err(error) => format!("Submodule init failed: {error}"),
                    });
                    if failed {
                        this.status_outcome = crate::app_state::TaskOutcome::Failure;
                    }
                    this.refresh_visible_worktree(cx);
                    cx.notify();
                })
                .ok();
        }));
    }

    pub(super) fn default_log_visible_branches(
        detail: &RepoDetail,
    ) -> std::collections::BTreeSet<String> {
        let mut branches = std::collections::BTreeSet::new();
        if detail
            .branches
            .iter()
            .any(|branch| branch == &detail.current_branch)
        {
            branches.insert(detail.current_branch.clone());
        }
        if let Some(primary) = ["main", "master"]
            .into_iter()
            .find(|branch| detail.branches.iter().any(|candidate| candidate == branch))
        {
            branches.insert(primary.to_string());
        }
        branches
    }

    pub(super) fn toggle_log_branch(&mut self, branch: String, cx: &mut Context<'_, Self>) {
        if self.busy || self.loading_detail {
            return;
        }
        if self.log_visible_branches.is_empty()
            && let Some(default_branches) =
                self.detail.as_ref().map(Self::default_log_visible_branches)
        {
            self.log_visible_branches = default_branches;
        }
        if self.log_visible_branches.contains(&branch) {
            if self.log_visible_branches.len() == 1 {
                self.set_warning("At least one branch must remain visible");
                cx.notify();
                return;
            }
            self.log_visible_branches.remove(&branch);
        } else {
            self.log_visible_branches.insert(branch);
        }
        self.reload_log_for_visible_branches(cx);
    }

    pub(super) fn reload_log_for_visible_branches(&mut self, cx: &mut Context<'_, Self>) {
        self.cancel_history_load();
        let Some(path) = self.selected_path() else {
            return;
        };
        let request = self.begin_detail_request(path.clone());
        let branches = self
            .log_visible_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.loading_detail = true;
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
                    if !this.owns_detail_request(&request) {
                        return;
                    }
                    if let Some(layout) = layout.as_ref() {
                        this.commit_canvas_states
                            .entry(layout.repository_path.clone())
                            .or_default()
                            .ensure_layout(layout);
                    }
                    this.log_entries = layout
                        .as_ref()
                        .map(|layout| layout.list.entries.clone())
                        .unwrap_or_default();
                    this.commit_canvas_layout = layout;
                    this.commit_canvas_interaction = None;
                    this.loading_detail = false;
                    this.set_status("Git Log updated");
                    cx.notify();
                })
                .ok();
        }));
    }

    pub(crate) fn selected_remote_action_target(
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

    pub(super) fn do_fetch_selected_remote(&mut self, remote: String, cx: &mut Context<'_, Self>) {
        if self.busy {
            return;
        }
        self.perform_remote_action(remote, false, cx);
    }

    pub(super) fn confirm_reset_selected_to_remote(
        &mut self,
        remote: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.busy || self.loading_detail || self.selected_remote_action_target().is_none() {
            return;
        }

        if self.detail.as_ref().is_some_and(|d| d.git_state != "正常") {
            self.set_warning("Git 操作尚未结束，请处理并刷新后再 Reset。");
            cx.notify();
            return;
        }
        self.busy = true;
        self.set_status(format!("Confirm reset to {remote}…"));
        let expected = self
            .detail
            .as_ref()
            .map(|detail| (detail.path.clone(), detail.current_branch.clone()));
        cx.notify();
        let preview = self
            .detail
            .as_ref()
            .map(|d| (d.path.clone(), d.current_branch.clone()));
        let entity = cx.entity().downgrade();
        self.remote_action_prompt_task = Some(window.spawn(cx, async move |cx| {
            let preview_remote = remote.clone();
            let description = cx.background_executor().spawn(async move {
                let Some((path, branch)) = preview else { return String::new(); };
                let counts = git2::Repository::open(&path).ok().and_then(|r| crate::repo_actions::remote_statuses(&r).into_iter().find(|r| r.name == preview_remote).and_then(|r| r.counts));
                super::confirmation::details("refresh", vec![("仓库", path), ("目标", format!("{preview_remote}/{branch}"))], vec![match counts {
                    Some((ahead, _)) => format!("基于已获取的本地引用：当前分支有 {ahead} 个提交不在目标中。Reset 将替换当前分支位置。重新 Fetch 后目标与影响可能变化。"),
                    None => "目标未比较或未获取，无法确定本地提交影响。将先 Fetch 并重新校验目标。".into(),
                }], Some(("warning", "要求工作区与暂存区干净，且无进行中的 Git 操作。Fetch 前后均重新校验。".into())), vec![])
            }).await;
            let answer = cx
                .prompt(
                    PromptLevel::Critical,
                    &format!("Reset 当前分支至 '{remote}'？"),
                    Some(&description),
                    &[
                        PromptButton::cancel("取消"),
                        PromptButton::ok("Reset 并丢弃本地提交"),
                    ],
                )
                .await;
            entity
                .update(cx, |this, cx| {
                    if answer == Ok(1)
                        && !this.loading_detail
                        && this
                            .detail
                            .as_ref()
                            .map(|detail| (detail.path.clone(), detail.current_branch.clone()))
                            == expected
                    {
                        this.perform_remote_action(remote, true, cx);
                    } else {
                        this.busy = false;
                        this.set_skipped("Remote reset cancelled");
                        cx.notify();
                    }
                })
                .ok();
        }));
    }

    fn perform_remote_action(&mut self, remote: String, reset: bool, cx: &mut Context<'_, Self>) {
        let Some((_, root_path, target_path, _, submodule_detail)) =
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
            .log_visible_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if reset && branch.is_empty() {
            self.busy = false;
            self.set_failure("Cannot determine the current branch");
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
        let navigation = self.navigation_revision;
        let request = self.begin_detail_request(target_path.clone());
        let fetched_remote = remote.clone();
        self.operation_task = Some(cx.spawn(async move |entity, cx| {
            let refresh_path = root_path.clone();
            let detail_path = target_path.clone();
            let expected_detail_path = detail_path.clone();
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
                    if result.is_ok() {
                        this.last_fetch.insert(
                            (expected_detail_path.clone(), fetched_remote),
                            chrono::Local::now().format("%H:%M:%S").to_string(),
                        );
                    }
                    match result {
                        Ok(_) if reset => this.set_status("Reset to remote complete"),
                        Ok(_) => this.set_status("Fetch complete; Git Log refreshed"),
                        Err(error) => this.set_failure(format!("Remote action failed: {error}")),
                    }
                    this.apply_repo_refresh(&refresh_path, refreshed);
                    if this
                        .selected_remote_action_target()
                        .is_some_and(|(_, _, path, _, _)| path == expected_detail_path)
                    {
                        if this.navigation_revision != navigation {
                            this.reload_current_detail(cx);
                        } else {
                            this.apply_detail(
                                request,
                                detail,
                                submodule_detail,
                                log_entries,
                                canvas_layout,
                            );
                        }
                    }
                    this.busy = false;
                    this.refresh_visible_worktree(cx);
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

        let module_path = repo_path.join(&relative_path);
        self.initialization.insert(
            module_path.clone(),
            super::state_views::Initialization::Running,
        );
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
                    match &result {
                        Ok(_) => {
                            this.initialization.remove(&module_path);
                        }
                        Err(e) => {
                            this.initialization.insert(
                                module_path,
                                super::state_views::Initialization::Failed(e.clone()),
                            );
                        }
                    }
                    match result {
                        Ok(msg) => {
                            if msg.is_empty() {
                                this.set_status("Submodule initialized");
                            } else {
                                this.set_status(format!("Submodule initialized: {msg}"));
                            }
                            this.apply_repo_refresh(&refresh_path, refreshed);
                            let selected = this.selected_submodule();
                            if let Some((repo_index, _, relative_path)) = selected
                                && relative_path == selected_relative_path
                                && this
                                    .repos
                                    .get(repo_index)
                                    .is_some_and(|repo| repo.path == refresh_path)
                            {
                                this.reload_current_detail(cx);
                            }
                        }
                        Err(e) => this.set_failure(format!("Submodule init failed: {e}")),
                    }
                    this.busy = false;
                    this.refresh_visible_worktree(cx);
                    cx.notify();
                })
                .ok();
        }));
    }
}
