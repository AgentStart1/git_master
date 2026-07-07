use gpui::*;

use crate::app_state::{DetailTab, GitMasterApp, RepoSelection};
use crate::git_ops;
use crate::models::{RepoDetail, SubmoduleDetail};
use crate::ui::theme;

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
                DetailTab::GitLog => self.render_log_tab().into_any_element(),
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
        div()
            .flex()
            .flex_col()
            .p(px(16.0))
            .gap(px(12.0))
            .child(info_row("Path", &detail.path))
            .child(info_row("Branch", &detail.current_branch))
            .child(info_row(
                "Remote",
                detail.remote_url.as_deref().unwrap_or("(none)"),
            ))
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

    fn render_log_tab(&self) -> impl IntoElement {
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
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme::TEXT_SUBTLE))
                                    .child(format!("{} — {}", entry.author, entry.date)),
                            ),
                    )
            }))
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
    fn do_init_selected_submodule(&mut self, cx: &mut Context<'_, Self>) {
        if self.busy {
            return;
        }
        let Some((repo_index, submodule_index)) = self.selected_submodule() else {
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

        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { git_ops::init_submodule(&repo_path, &relative_path) })
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
                            this.refresh_repo(repo_index);
                            if let Some(submodule) = this
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
                                let detail = git_ops::get_repo_detail(&path);
                                let log_entries = git_ops::get_commit_log(&path, 200);
                                this.apply_detail(
                                    RepoSelection::Submodule {
                                        repo_index,
                                        submodule_index,
                                    },
                                    detail,
                                    submodule_detail,
                                    log_entries,
                                );
                            }
                        }
                        Err(e) => this.set_status(format!("Submodule init failed: {e}")),
                    }
                    this.busy = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}
