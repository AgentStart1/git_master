use super::focus_visibility::KeyboardFocusStyle;
use super::icons::icon;
use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app_state::GitMasterApp;
use crate::git_ops;
use crate::ui::theme;

impl GitMasterApp {
    pub(crate) fn start_scan(&mut self, path: std::path::PathBuf, cx: &mut Context<'_, Self>) {
        self.scan_workspace(path, false, cx);
    }

    pub(crate) fn start_workspace_polling(&mut self, cx: &mut Context<Self>) {
        self.workspace_poll_task = Some(cx.spawn(async move |entity, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(30))
                    .await;
                if entity
                    .update(cx, |this, cx| {
                        if this.can_refresh_workspace()
                            && let Some(path) = this.parent_dir.clone()
                        {
                            this.scan_workspace(path, true, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    pub(crate) fn can_refresh_workspace(&self) -> bool {
        !self.busy
            && !self.scanning
            && !self.loading_detail
            && !self.worktree_loading
            && !self.history_host.is_loading()
            && self.branch_picker.is_none()
            && self.context_menu.is_none()
            && !self.workspace_menu_open
            && self.remote_menu.is_none()
            && !self.remote_picker_open
            && !self.overview_ref_picker
            && self.blocked_action.is_none()
            && self.commit_canvas_interaction.is_none()
    }

    fn scan_workspace(
        &mut self,
        path: std::path::PathBuf,
        automatic: bool,
        cx: &mut Context<Self>,
    ) {
        if self.parent_dir.as_deref() == Some(path.as_path()) && !self.repos.is_empty() {
            self.scanning = true;
        } else {
            self.begin_scan(path.clone());
        }
        cx.notify();
        self.scan_task = Some(cx.spawn(async move |entity, cx| {
            let scan_path = path.clone();
            let (repos, recent) = cx
                .background_executor()
                .spawn(async move {
                    (
                        git_ops::scan_repos(&scan_path),
                        (!automatic).then(|| crate::preferences::remember_workspace(&scan_path)),
                    )
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.parent_dir.as_ref() != Some(&path) {
                        return;
                    }
                    if automatic {
                        this.scanning = false;
                        if !this.can_refresh_workspace() {
                            cx.notify();
                            return;
                        }
                    }
                    this.apply_scan(&path, repos);
                    if let Some(Ok(recent)) = recent {
                        this.recent_workspaces = recent;
                    }
                    if !this.busy && (!automatic || !this.loading_detail) {
                        this.reload_current_detail(cx);
                    }
                    cx.notify();
                })
                .ok();
        }));
    }

    fn run_batch(&mut self, action: crate::batch_ops::BatchAction, cx: &mut Context<'_, Self>) {
        if self.busy || self.scanning || self.repos.is_empty() {
            return;
        }
        let host = crate::batch_ops::BatchHost::new(
            self.repos.iter().map(|repo| repo.path.clone()).collect(),
            action,
        );
        self.start_batch_host(host, action, cx);
    }
    pub(super) fn start_batch_host(
        &mut self,
        host: crate::batch_ops::BatchHost,
        action: crate::batch_ops::BatchAction,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let parent = self.parent_dir.clone();
        self.close_context_menu();
        self.busy = true;
        self.set_status(match action {
            crate::batch_ops::BatchAction::Fetch => "Fetching all projects and remotes…",
            crate::batch_ops::BatchAction::SwitchMain => "Switching all projects to main branch…",
        });
        cx.notify();
        self.operation_task = Some(cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { host.run() })
                .await;
            entity
                .update(cx, |this, cx| {
                    this.busy = false;
                    if this.parent_dir == parent {
                        if matches!(action, crate::batch_ops::BatchAction::Fetch) {
                            for item in &result.items {
                                if item.outcome.is_ok() && !item.skipped {
                                    if let Some(remote) = &item.remote {
                                        this.last_fetch.insert(
                                            (item.path.clone(), remote.clone()),
                                            chrono::Local::now().format("%H:%M:%S").to_string(),
                                        );
                                    }
                                }
                            }
                        }
                        this.set_status(result.message());
                        this.status_outcome = if result.failed > 0 {
                            if result.succeeded > 0 {
                                crate::app_state::TaskOutcome::Partial
                            } else {
                                crate::app_state::TaskOutcome::Failure
                            }
                        } else if result.succeeded == 0 {
                            crate::app_state::TaskOutcome::Skipped
                        } else {
                            crate::app_state::TaskOutcome::Success
                        };
                        this.batch_expanded.clear();
                        this.batch_result = Some(result.clone());
                        this.status_expanded = true;
                        for (path, refreshed) in result.refreshed {
                            if let Some(index) =
                                this.repos.iter().position(|repo| repo.path == path)
                            {
                                this.apply_repo_refresh(&path, refreshed);
                                this.refresh_pushed_repo_details(index, &path, cx);
                            }
                        }
                        this.refresh_visible_worktree(cx);
                    }
                    cx.notify();
                })
                .ok();
        }));
    }

    pub fn render_top_bar(&self, _window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let dir_label = self
            .parent_dir
            .as_ref()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_else(|| "选择工作区".into());

        let can_fetch = !self.busy && !self.scanning && !self.repos.is_empty();
        let fetch_btn = div()
            .id("fetch-all-btn")
            .flex_shrink_0()
            .px(px(12.0))
            .py(px(7.0))
            .rounded(px(3.0))
            .text_size(px(17.0))
            .bg(rgb(if can_fetch {
                theme::palette().bg_elevated
            } else {
                theme::palette().bg_overlay
            }))
            .text_color(rgb(if can_fetch {
                theme::palette().text_primary
            } else {
                theme::palette().text_subtle
            }))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(icon("refresh", 18.0))
            .child("全部 Fetch")
            .when(can_fetch, |button| {
                button
                    .focusable()
                    .tab_stop(true)
                    .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                    .cursor_pointer()
            })
            .on_click(cx.listener(|this, _, window, cx| {
                this.close_popup(super::popup::PopupKind::Workspace, window);
                this.run_batch(crate::batch_ops::BatchAction::Fetch, cx)
            }));

        let main_btn = div()
            .id("switch-all-main-btn")
            .flex_shrink_0()
            .px(px(12.0))
            .py(px(7.0))
            .rounded(px(3.0))
            .text_size(px(17.0))
            .bg(rgb(if can_fetch {
                theme::palette().bg_elevated
            } else {
                theme::palette().bg_overlay
            }))
            .text_color(rgb(if can_fetch {
                theme::palette().text_primary
            } else {
                theme::palette().text_subtle
            }))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(icon("git-branch", 18.0))
            .child("全部切换至默认分支")
            .when(can_fetch, |button| {
                button
                    .focusable()
                    .tab_stop(true)
                    .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                    .cursor_pointer()
            })
            .on_click(cx.listener(|this, _, window, cx| {
                this.close_popup(super::popup::PopupKind::Workspace, window);
                this.confirm_batch_switch(window, cx);
            }));

        let btn = div()
            .id("change-dir-btn")
            .flex_shrink_0()
            .px(px(13.0))
            .py(px(7.0))
            .bg(rgb(theme::palette().bg_elevated))
            .text_color(rgb(theme::palette().text_primary))
            .rounded(px(3.0))
            .focusable()
            .tab_stop(true)
            .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
            .cursor_pointer()
            .text_size(px(17.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(icon("folder", 18.0))
            .child("打开工作区")
            .on_click(cx.listener(|this, _event, window, cx| {
                this.close_popup(super::popup::PopupKind::Workspace, window);
                cx.notify();
                let receiver = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Select parent directory".into()),
                });
                this.directory_prompt_task =
                    Some(cx.spawn(async move |entity: WeakEntity<GitMasterApp>, cx| {
                        if let Ok(Ok(Some(paths))) = receiver.await
                            && let Some(path) = paths.into_iter().next()
                        {
                            entity
                                .update(cx, |this, cx| {
                                    this.start_scan(path, cx);
                                })
                                .ok();
                        }
                    }));
            }));

        let popup = super::popup::PopupBoundary::new(self, super::popup::PopupKind::Workspace);
        let mut menu = div()
            .id("workspace-menu-popup")
            .occlude()
            .on_click(|_, _, cx| cx.stop_propagation())
            .absolute()
            .top(px(44.0))
            .left(px(0.0))
            .w(px(260.0))
            .p(px(8.0))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .bg(rgb(theme::palette().bg_surface))
            .border_1()
            .border_color(rgb(theme::palette().border))
            .rounded(px(4.0))
            .shadow_md()
            .child(self.track("change-dir-btn", btn))
            .child(self.track("fetch-all-btn", fetch_btn))
            .child(self.track("switch-all-main-btn", main_btn))
            .child(
                self.track(
                    "open-logs-btn",
                    div()
                        .id("open-logs-btn")
                        .p(px(12.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon("file", 18.0))
                        .child("操作日志")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_popup(super::popup::PopupKind::Workspace, window);
                            this.open_logs(cx);
                            cx.notify();
                        })),
                ),
            );
        if !self.recent_workspaces.is_empty() {
            menu = menu.child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .text_size(px(13.0))
                    .text_color(rgb(theme::palette().text_subtle))
                    .child("最近工作区"),
            );
            for (index, path) in self.recent_workspaces.iter().enumerate() {
                let path = path.clone();
                let id = format!("recent-workspace-{index}");
                let label = path.display().to_string();
                menu = menu.child(
                    self.track(
                        &id,
                        div()
                            .id(ElementId::Name(id.clone().into()))
                            .px(px(12.0))
                            .py(px(8.0))
                            .text_size(px(14.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .focusable()
                            .tab_stop(true)
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .cursor_pointer()
                            .child(label)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.close_popup(super::popup::PopupKind::Workspace, window);
                                this.start_scan(path.clone(), cx);
                            })),
                    ),
                );
            }
        }
        let header = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(24.0))
            .h(px(50.0))
            .px(px(20.0))
            .bg(rgb(theme::palette().bg_surface))
            .border_b_1()
            .border_color(rgb(theme::palette().border))
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(icon("git-branch", 24.0).text_color(rgb(theme::palette().accent)))
                    .child("Git Master"),
            )
            .child(
                div()
                    .h(px(26.0))
                    .border_l_1()
                    .border_color(rgb(theme::palette().border)),
            )
            .child(
                popup.attach(
                    div()
                        .relative()
                        .child(
                            self.track(
                                "workspace-menu",
                                div()
                                    .id("workspace-menu")
                                    .track_focus(&popup.focus)
                                    .py(px(9.0))
                                    .border_1()
                                    .border_color(rgba(0x00000000))
                                    .focusable()
                                    .tab_stop(true)
                                    .keyboard_focus(|s| {
                                        s.border_1().border_color(rgb(theme::palette().accent))
                                    })
                                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                    .cursor_pointer()
                                    .text_size(px(18.0))
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(icon("stack-2", 20.0))
                                    .child(dir_label)
                                    .child(icon("chevron-down", 16.0))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workspace_menu_open = !this.workspace_menu_open;
                                        cx.notify();
                                    })),
                            ),
                        )
                        .children(
                            self.workspace_menu_open
                                .then(|| deferred(popup.observe(menu))),
                        ),
                    cx,
                ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(self.render_theme_controls(cx))
            .when(cfg!(windows), |header| {
                header.child(
                    div()
                        .flex()
                        .h_full()
                        .gap(px(4.0))
                        .child(
                            div()
                                .id("window-minimize")
                                .w(px(46.0))
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .child(icon("minus", 20.0))
                                .window_control_area(WindowControlArea::Min),
                        )
                        .child(
                            div()
                                .id("window-maximize")
                                .w(px(46.0))
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .child(icon("square", 18.0))
                                .window_control_area(WindowControlArea::Max),
                        )
                        .child(
                            div()
                                .id("window-close")
                                .w(px(46.0))
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgb(theme::palette().red)))
                                .child(icon("x", 22.0))
                                .window_control_area(WindowControlArea::Close),
                        ),
                )
            });

        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .child(header)
            .into_any_element()
    }
}

impl GitMasterApp {
    pub(crate) fn task_status_summary(&self) -> Option<String> {
        let message = if self.busy {
            self.status_message.as_deref().unwrap_or("Working…")
        } else if self.scanning {
            "Scanning workspace…"
        } else if self.loading_detail {
            "Loading repository…"
        } else if self.worktree_loading {
            "Loading changes…"
        } else {
            self.status_message.as_deref()?
        };
        Some(
            message
                .lines()
                .next()
                .unwrap_or("Task completed")
                .chars()
                .take(180)
                .collect(),
        )
    }

    pub(crate) fn render_task_status(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let appending = self.history_host.is_loading();
        let running =
            self.busy || self.scanning || self.loading_detail || self.worktree_loading || appending;
        let idle = self.status_message.is_none() && !running;
        let skipped = self.status_outcome == crate::app_state::TaskOutcome::Skipped;
        let summary = if appending {
            "正在加载更多历史…".into()
        } else {
            self.task_status_summary().unwrap_or_else(|| "就绪".into())
        };
        let summary = match summary.as_str() {
            "Git Log updated" => "提交历史已更新".into(),
            "Repository changes and stashes refreshed" => "已刷新工作区与暂存记录".into(),
            "Git operation complete" => "Git 操作完成".into(),
            "Branch operation complete" => "分支操作完成".into(),
            "Loading repository…" => "正在读取仓库…".into(),
            "Loading changes…" => "正在读取变更…".into(),
            "Stash deletion cancelled" => "已取消删除暂存记录".into(),
            _ => summary,
        };
        let failed = self.status_outcome == crate::app_state::TaskOutcome::Failure;
        let partial = self.status_outcome == crate::app_state::TaskOutcome::Partial;
        let warning = self.status_outcome == crate::app_state::TaskOutcome::Warning;
        let color = if running {
            theme::palette().accent
        } else if partial || warning {
            theme::palette().yellow
        } else if failed {
            theme::palette().red
        } else if idle || skipped {
            theme::palette().text_subtle
        } else {
            theme::palette().green
        };
        let mut strip = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .bg(rgb(theme::palette().bg_surface))
            .border_t_1()
            .border_color(rgb(theme::palette().border));
        let row = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .h(px(48.0))
            .px(px(16.0))
            .child(
                icon(
                    if running {
                        "refresh"
                    } else if partial || warning {
                        "alert-triangle"
                    } else if failed {
                        "alert-circle"
                    } else if idle {
                        "info-circle"
                    } else if skipped {
                        "minus"
                    } else {
                        "check"
                    },
                    20.0,
                )
                .text_color(rgb(color)),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(15.0))
                    .text_color(rgb(color))
                    .child(if running {
                        "进行中"
                    } else if partial {
                        "部分失败"
                    } else if failed {
                        "失败"
                    } else if warning {
                        "提示"
                    } else if idle {
                        "空闲"
                    } else if skipped {
                        "跳过"
                    } else {
                        "成功"
                    }),
            )
            .child(
                self.track(
                    "status-summary",
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(15.0))
                        .text_color(rgb(theme::palette().text_primary))
                        .text_ellipsis()
                        .child(summary),
                ),
            )
            .when(self.status_message.is_some(), |row| {
                row.child(
                    self.track(
                        "status-toggle",
                        div()
                            .id("status-toggle")
                            .focusable()
                            .tab_stop(true)
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .cursor_pointer()
                            .flex_shrink_0()
                            .text_size(px(15.0))
                            .text_color(rgb(theme::palette().accent))
                            .child(if self.status_expanded {
                                "收起详情"
                            } else {
                                "查看详情"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.status_expanded = !this.status_expanded;
                                cx.notify();
                            })),
                    ),
                )
                .child(
                    self.track(
                        "status-copy",
                        div()
                            .id("status-copy")
                            .focusable()
                            .tab_stop(true)
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .cursor_pointer()
                            .flex_shrink_0()
                            .text_size(px(15.0))
                            .text_color(rgb(theme::palette().text_subtle))
                            .child("复制")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(message) = &this.status_message {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        message.clone(),
                                    ));
                                }
                            })),
                    ),
                )
            })
            .child(
                div()
                    .flex_shrink_0()
                    .ml(px(14.0))
                    .text_size(px(15.0))
                    .child(chrono::Local::now().format("%Y-%m-%d").to_string()),
            );
        if self.status_expanded && self.batch_result.is_some() {
            strip = strip.child(self.render_batch_results(cx));
        }
        if self.status_expanded
            && self.batch_result.is_none()
            && let Some(message) = &self.status_message
        {
            strip = strip.child(
                self.track(
                    "status-details",
                    div()
                        .id("status-details")
                        .h(px(160.0))
                        .flex_shrink_0()
                        .overflow_y_scroll()
                        .overflow_x_scroll()
                        .px(px(16.0))
                        .py(px(10.0))
                        .bg(rgb(theme::palette().bg_base))
                        .font_family(if cfg!(windows) {
                            "Consolas"
                        } else if cfg!(target_os = "macos") {
                            "Menlo"
                        } else {
                            "monospace"
                        })
                        .text_size(px(15.0))
                        .text_color(rgb(theme::palette().text_subtle))
                        .flex()
                        .flex_col()
                        .child(
                            div().flex_shrink_0().child(
                                message
                                    .rsplit_once('\n')
                                    .map_or("", |(body, _)| body)
                                    .to_owned(),
                            ),
                        )
                        .child(
                            self.track(
                                "status-detail-end",
                                div().flex_shrink_0().child(
                                    message
                                        .rsplit_once('\n')
                                        .map_or(message.as_str(), |(_, end)| end)
                                        .to_owned(),
                                ),
                            ),
                        ),
                ),
            );
        }
        let mut footer = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .when(self.status_expanded, |footer| {
                footer
                    .border_t_1()
                    .border_color(rgb(theme::palette().border))
            })
            .child(row);
        if running {
            footer = footer.child(
                self.track(
                    "task-activity",
                    div().h(px(2.0)).flex_shrink_0().w_full().with_animation(
                        "task-activity",
                        Animation::new(std::time::Duration::from_millis(1400))
                            .repeat()
                            .with_easing(pulsating_between(0.25, 0.85)),
                        |element, opacity| {
                            element.bg(rgb(theme::palette().accent)).opacity(opacity)
                        },
                    ),
                ),
            );
        }
        strip = strip.child(self.track("status-footer", footer));
        Some(strip.into_any_element())
    }
}
