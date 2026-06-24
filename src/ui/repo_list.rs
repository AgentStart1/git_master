use gpui::*;

use crate::app_state::GitMasterApp;
use crate::git_ops;
use crate::ui::theme;

impl GitMasterApp {
    pub fn render_repo_list(
        &self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> impl IntoElement {
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
            .children(self.repos.iter().enumerate().map(|(i, repo)| {
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

                div()
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
                    )
            }))
    }
}
