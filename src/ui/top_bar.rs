use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app_state::GitMasterApp;
use crate::git_ops;
use crate::ui::theme;

impl GitMasterApp {
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

        let status_msg = self.status_message.clone();
        let can_fetch = !self.busy && !self.scanning && !self.repos.is_empty();
        let fetch_btn = div()
            .id("fetch-all-btn")
            .px(px(12.0))
            .py(px(6.0))
            .rounded(px(4.0))
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
            .px(px(12.0))
            .py(px(6.0))
            .rounded(px(4.0))
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
            .child("Switch All to Main")
            .when(can_fetch, |button| button.cursor_pointer())
            .on_click(cx.listener(|this, _, _, cx| {
                this.run_batch(crate::batch_ops::BatchAction::SwitchMain, cx)
            }));

        let btn = div()
            .id("change-dir-btn")
            .px(px(12.0))
            .py(px(6.0))
            .bg(rgb(theme::ACCENT))
            .text_color(rgb(theme::BG_BASE))
            .rounded(px(4.0))
            .cursor_pointer()
            .text_sm()
            .child("Open Directory")
            .on_click(cx.listener(|this, _event, _window, cx| {
                let receiver = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Select parent directory".into()),
                });
                this.scan_task =
                    Some(cx.spawn(async move |entity: WeakEntity<GitMasterApp>, cx| {
                        if let Ok(Ok(Some(paths))) = receiver.await
                            && let Some(path) = paths.into_iter().next()
                        {
                            entity
                                .update(cx, |this, cx| {
                                    this.begin_scan(path.clone());
                                    cx.notify();
                                })
                                .ok();
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
                        }
                    }));
            }));

        div()
            .flex()
            .flex_shrink_0()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .p(px(12.0))
            .bg(rgb(theme::BG_SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BG_OVERLAY))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .child(div().text_sm().child(dir_label))
                    .children(
                        status_msg
                            .map(|msg| div().text_xs().text_color(rgb(theme::YELLOW)).child(msg)),
                    ),
            )
            .child(self.track("fetch-all-btn", fetch_btn))
            .child(self.track("switch-all-main-btn", main_btn))
            .child(self.track("change-dir-btn", btn))
            .into_any_element()
    }
}
