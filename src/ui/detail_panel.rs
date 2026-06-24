use gpui::*;

use crate::app_state::{DetailTab, GitMasterApp};
use crate::models::RepoDetail;
use crate::ui::theme;

impl GitMasterApp {
    pub fn render_detail_panel(
        &self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<Div> {
        self.selected_index?;
        let detail = self.detail.as_ref()?;

        Some(
            div()
                .flex()
                .flex_col()
                .flex_grow()
                .bg(rgb(theme::BG_BASE))
                .child(self.render_tabs(cx))
                .child(match self.active_tab {
                    DetailTab::Info => self.render_info_tab(detail).into_any_element(),
                    DetailTab::GitLog => self.render_log_tab().into_any_element(),
                }),
        )
    }

    fn render_tabs(&self, cx: &mut Context<'_, Self>) -> impl IntoElement {
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

        div()
            .flex()
            .flex_row()
            .bg(rgb(theme::BG_SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BG_OVERLAY))
            .child(
                div()
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
                    })),
            )
            .child(
                div()
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
                    })),
            )
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
