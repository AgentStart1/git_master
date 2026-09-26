use super::theme;
use crate::app_state::GitMasterApp;
use gpui::*;
use std::path::PathBuf;

pub struct CommitPress {
    pub path: PathBuf,
    pub hash: String,
    pub start: Point<Pixels>,
    pub dragged: bool,
}

impl CommitPress {
    pub fn update(&mut self, position: Point<Pixels>) {
        let dx: f32 = (position.x - self.start.x).into();
        let dy: f32 = (position.y - self.start.y).into();
        self.dragged |= dx * dx + dy * dy > 16.0;
    }
}

impl GitMasterApp {
    pub fn close_commit_details(&mut self) {
        self.commit_task = None;
        self.selected_commit = None;
        self.commit_text = None;
        self.commit_press = None;
    }

    pub(super) fn open_commit_details(
        &mut self,
        path: PathBuf,
        hash: String,
        cx: &mut Context<'_, Self>,
    ) {
        let key = (path, hash);
        self.selected_commit = Some(key.clone());
        self.commit_text = None;
        cx.notify();
        self.commit_task = Some(cx.spawn(async move |entity, cx| {
            let request = key.clone();
            let result = cx
                .background_executor()
                .spawn(async move { crate::commit_details::load(&request.0, &request.1) })
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.selected_commit.as_ref() == Some(&key) {
                        this.commit_text = Some(result);
                        cx.notify();
                    }
                })
                .ok();
        }));
    }

    pub(super) fn render_commit_details(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let (path, hash) = self.selected_commit.as_ref()?;
        let text = match &self.commit_text {
            None => "Loading commit…".to_owned(),
            Some(Ok(text)) => text.clone(),
            Some(Err(error)) => format!("Cannot load commit: {error}"),
        };
        Some(
            div()
                .id("commit-details")
                .flex()
                .flex_col()
                .h(px(280.0))
                .flex_shrink_0()
                .border_t_1()
                .border_color(rgb(theme::BG_OVERLAY))
                .bg(rgb(theme::BG_SURFACE))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .child(format!("Commit {hash} — {}", path.display())),
                        )
                        .child(
                            div()
                                .id("copy-commit-details")
                                .text_xs()
                                .cursor_pointer()
                                .child("Copy")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(Ok(text)) = &this.commit_text {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            text.clone(),
                                        ));
                                    }
                                })),
                        )
                        .child(
                            div()
                                .id("close-commit-details")
                                .text_xs()
                                .cursor_pointer()
                                .child("Close")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_commit_details();
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .id("commit-details-scroll")
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .overflow_x_scroll()
                        .p(px(12.0))
                        .text_sm()
                        .child(text),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn drag_stays_a_drag_even_after_returning_to_start() {
        let mut press = CommitPress {
            path: PathBuf::new(),
            hash: String::new(),
            start: point(px(0.0), px(0.0)),
            dragged: false,
        };
        press.update(point(px(2.0), px(1.0)));
        assert!(!press.dragged);
        press.update(point(px(8.0), px(0.0)));
        press.update(point(px(0.0), px(0.0)));
        assert!(press.dragged);
    }
}
