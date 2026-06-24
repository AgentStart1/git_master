use gpui::*;

use crate::app_state::GitMasterApp;
use crate::git_ops;
use crate::ui::theme;

impl GitMasterApp {
    pub fn render_top_bar(&self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let dir_label = self
            .parent_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "No directory selected".into());

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .p(px(12.0))
            .bg(rgb(theme::BG_SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BG_OVERLAY))
            .child(div().flex_grow().text_sm().child(dir_label))
            .child(
                div()
                    .id("change-dir-btn")
                    .px(px(12.0))
                    .py(px(6.0))
                    .bg(rgb(theme::ACCENT))
                    .text_color(rgb(theme::BG_BASE))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_sm()
                    .child("Open Directory")
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        let receiver = cx.prompt_for_paths(PathPromptOptions {
                            files: false,
                            directories: true,
                            multiple: false,
                            prompt: Some("Select parent directory".into()),
                        });
                        cx.spawn(async move |entity: WeakEntity<GitMasterApp>, cx| {
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
                        })
                        .detach();
                    })),
            )
    }
}
