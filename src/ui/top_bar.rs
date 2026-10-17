use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app_state::GitMasterApp;
use crate::git_ops;
use crate::ui::theme;

impl GitMasterApp {
    pub(crate) fn start_scan(&mut self, path: std::path::PathBuf, cx: &mut Context<'_, Self>) {
        self.begin_scan(path.clone());
        cx.notify();
        self.scan_task = Some(cx.spawn(async move |entity, cx| {
            let scan_path = path.clone();
            let repos = cx
                .background_executor()
                .spawn(async move { git_ops::scan_repos(&scan_path) })
                .await;
            entity
                .update(cx, |this, cx| {
                    this.apply_scan(&path, repos);
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
                        this.set_status(result.message());
                        for (path, refreshed) in result.refreshed {
                            if let Some(index) =
                                this.repos.iter().position(|repo| repo.path == path)
                            {
                                this.apply_repo_refresh(index, &path, refreshed);
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
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "No directory selected".into());

        let can_fetch = !self.busy && !self.scanning && !self.repos.is_empty();
        let fetch_btn = div()
            .id("fetch-all-btn")
            .flex_shrink_0()
            .px(px(12.0))
            .py(px(7.0))
            .rounded(px(7.0))
            .text_sm()
            .bg(rgb(if can_fetch {
                theme::ACCENT
            } else {
                theme::BG_OVERLAY
            }))
            .text_color(rgb(if can_fetch {
                theme::BG_BASE
            } else {
                theme::TEXT_SUBTLE
            }))
            .child("Fetch All")
            .when(can_fetch, |button| button.cursor_pointer())
            .on_click(cx.listener(|this, _, _, cx| {
                this.run_batch(crate::batch_ops::BatchAction::Fetch, cx)
            }));

        let main_btn = div()
            .id("switch-all-main-btn")
            .flex_shrink_0()
            .px(px(12.0))
            .py(px(7.0))
            .rounded(px(7.0))
            .text_sm()
            .bg(rgb(if can_fetch {
                theme::ACCENT
            } else {
                theme::BG_OVERLAY
            }))
            .text_color(rgb(if can_fetch {
                theme::BG_BASE
            } else {
                theme::TEXT_SUBTLE
            }))
            .child("Use Main")
            .when(can_fetch, |button| button.cursor_pointer())
            .on_click(cx.listener(|this, _, _, cx| {
                this.run_batch(crate::batch_ops::BatchAction::SwitchMain, cx)
            }));

        let btn = div()
            .id("change-dir-btn")
            .flex_shrink_0()
            .px(px(13.0))
            .py(px(7.0))
            .bg(rgb(theme::ACCENT))
            .text_color(rgb(theme::BG_BASE))
            .rounded(px(7.0))
            .cursor_pointer()
            .text_sm()
            .child("Open Workspace")
            .on_click(cx.listener(|this, _event, _window, cx| {
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

        let header = div()
            .flex()
            .flex_shrink_0()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .px(px(16.0))
            .py(px(12.0))
            .bg(rgb(theme::BG_SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(32.0))
                    .rounded(px(9.0))
                    .bg(rgb(theme::ACCENT_SOFT))
                    .text_color(rgb(theme::ACCENT))
                    .text_sm()
                    .child("GM"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .min_w(px(0.0))
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("WORKSPACE"),
                    )
                    .child(div().text_sm().text_ellipsis().child(dir_label)),
            )
            .child(
                self.track(
                    "open-logs-btn",
                    div()
                        .id("open-logs-btn")
                        .flex_shrink_0()
                        .px(px(12.0))
                        .py(px(7.0))
                        .rounded(px(7.0))
                        .text_sm()
                        .bg(rgb(if self.log_window.is_some() {
                            theme::ACCENT_SOFT
                        } else {
                            theme::BG_OVERLAY
                        }))
                        .text_color(rgb(theme::ACCENT))
                        .cursor_pointer()
                        .child("Logs")
                        .on_click(cx.listener(|this, _, _, cx| this.open_logs(cx))),
                ),
            )
            .child(self.track("fetch-all-btn", fetch_btn))
            .child(self.track("switch-all-main-btn", main_btn))
            .child(self.track("change-dir-btn", btn));

        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .child(header)
            .children(self.render_task_status(cx))
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

    fn render_task_status(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let running = self.busy || self.scanning || self.loading_detail || self.worktree_loading;
        let summary = self.task_status_summary()?;
        let normalized = summary.to_ascii_lowercase();
        let failed =
            if normalized.starts_with("fetch all:") || normalized.starts_with("switch to main:") {
                !normalized.contains(", 0 failed")
            } else {
                ["failed:", "error:", "cannot ", "conflict"]
                    .iter()
                    .any(|word| normalized.contains(word))
            };
        let color = if running {
            theme::ACCENT
        } else if failed {
            theme::RED
        } else {
            theme::TEXT_SUBTLE
        };
        let mut strip = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .bg(rgb(theme::BG_SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BORDER));
        let row = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .h(px(34.0))
            .px(px(16.0))
            .child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(rgb(color))
                    .child(if running {
                        "◌  In progress"
                    } else if failed {
                        "●  Failed"
                    } else {
                        "●  Result"
                    }),
            )
            .child(
                self.track(
                    "status-summary",
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_xs()
                        .text_color(rgb(theme::TEXT_PRIMARY))
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
                            .cursor_pointer()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(rgb(theme::ACCENT))
                            .child(if self.status_expanded {
                                "Hide details"
                            } else {
                                "Details"
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
                            .cursor_pointer()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_SUBTLE))
                            .child("Copy")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(message) = &this.status_message {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        message.clone(),
                                    ));
                                }
                            })),
                    ),
                )
            });
        strip = strip.child(row);
        if running {
            strip = strip.child(
                div().h(px(2.0)).w_full().with_animation(
                    "task-activity",
                    Animation::new(std::time::Duration::from_millis(1400))
                        .repeat()
                        .with_easing(pulsating_between(0.25, 0.85)),
                    |element, opacity| element.bg(rgb(theme::ACCENT)).opacity(opacity),
                ),
            );
        }
        if self.status_expanded
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
                        .bg(rgb(theme::BG_BASE))
                        .font_family(if cfg!(windows) {
                            "Consolas"
                        } else if cfg!(target_os = "macos") {
                            "Menlo"
                        } else {
                            "monospace"
                        })
                        .text_xs()
                        .text_color(rgb(theme::TEXT_SUBTLE))
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
        Some(strip.into_any_element())
    }
}
