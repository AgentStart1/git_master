use super::focus_visibility::KeyboardFocusStyle;
use super::icons::icon;
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
        self.commit_file_task = None;
        self.commit_file_preview = None;
        self.commit_expanded_file = None;
        self.selected_commit = None;
        self.commit_text = None;
        self.commit_presentation = None;
        self.commit_message_source = false;
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
        self.canvas_reveal_selection = true;
        self.commit_text = None;
        self.commit_file_task = None;
        self.commit_file_preview = None;
        self.commit_expanded_file = None;
        self.commit_presentation = None;
        self.commit_message_source = false;
        cx.notify();
        if self.commit_canvas_layout.as_ref().is_some_and(|layout| {
            layout.nodes.iter().any(|n| {
                n.is_placeholder()
                    && n.id.commit_id == key.1
                    && layout
                        .lanes
                        .iter()
                        .find(|l| l.id == n.id.lane_id)
                        .is_some_and(|l| {
                            layout.repository_path.join(
                                l.relative_path
                                    .as_deref()
                                    .unwrap_or(std::path::Path::new("")),
                            ) == key.0
                        })
            })
        }) {
            return;
        }
        self.commit_task = Some(cx.spawn(async move |entity, cx| {
            let request = key.clone();
            let (result, presentation) = cx
                .background_executor()
                .spawn(async move {
                    let result = crate::commit_details::load(&request.0, &request.1);
                    let presentation = result.as_ref().ok().and_then(|_| {
                        crate::commit_details::Presentation::load(&request.0, &request.1).ok()
                    });
                    (result, presentation)
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.selected_commit.as_ref() == Some(&key) {
                        this.commit_text = Some(result);
                        this.commit_presentation = presentation;
                        cx.notify();
                    }
                })
                .ok();
        }));
    }

    fn toggle_commit_file(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        self.commit_file_task = None;
        self.commit_file_preview = None;
        if self.commit_expanded_file.as_ref() == Some(&file) {
            self.commit_expanded_file = None;
            cx.notify();
            return;
        }
        let Some(key) = self.selected_commit.clone() else {
            return;
        };
        self.commit_expanded_file = Some(file.clone());
        cx.notify();
        self.commit_file_task = Some(cx.spawn(async move |entity, cx| {
            let request = key.clone();
            let selected = file.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::commit_details::load_file_diff(&request.0, &request.1, &selected)
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.selected_commit.as_ref() == Some(&key)
                        && this.commit_expanded_file.as_ref() == Some(&file)
                    {
                        this.commit_file_preview = Some(result);
                        cx.notify();
                    }
                })
                .ok();
        }));
    }

    pub(super) fn render_commit_details(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let (selected_path, hash) = self.selected_commit.as_ref()?;
        if let Some(panel) = self.render_placeholder_details(selected_path, hash, cx) {
            return Some(panel);
        }
        let text = match &self.commit_text {
            None => "正在读取提交…".to_owned(),
            Some(Ok(text)) => text.clone(),
            Some(Err(error)) => format!("暂时无法读取这条提交。它可能不在本地对象库中。\n{error}"),
        };
        Some(
            self.track(
                "commit-details",
                div()
                    .id("commit-details")
                    .flex()
                    .flex_col()
                    .min_w(px(0.0))
                    .w(px(328.0))
                    .h_full()
                    .flex_shrink_0()
                    .border_l_1()
                    .border_color(rgb(theme::palette().bg_overlay))
                    .bg(rgb(theme::palette().bg_base))
                    .child(
                        div()
                            .id("commit-details-scroll")
                            .flex_1()
                            .min_h(px(0.0))
                            .overflow_y_scroll()
                            .overflow_x_scroll()
                            .px(px(18.0))
                            .py(px(6.0))
                            .text_sm()
                            .child(render_commit_content(
                                self,
                                cx,
                                self.commit_presentation.as_ref(),
                                &text,
                                self.detail
                                    .as_ref()
                                    .and_then(|d| d.head_labels.get(hash))
                                    .cloned()
                                    .unwrap_or_default(),
                            ))
                            .children(self.commit_canvas_layout.as_ref().map(|layout| {
                                let links = layout
                                    .edges
                                    .iter()
                                    .filter(|edge| {
                                        edge.from.commit_id == *hash
                                            && edge.kind
                                                == super::commit_canvas::CanvasEdgeKind::Submodule
                                    })
                                    .collect::<Vec<_>>();
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.0))
                                    .children((!links.is_empty()).then(|| {
                                        div()
                                            .mt(px(16.0))
                                            .text_size(px(17.0))
                                            .child("子模块引用 · gitlink")
                                    }))
                                    .children(links.into_iter().map(|edge| {
                                        let target = edge.to.commit_id.clone();
                                        let lane = edge.to.lane_id.clone();
                                        let copied = format!("{lane}: {target}");
                                        div()
                                            .id(ElementId::Name(
                                                format!("gitlink-copy-{lane}").into(),
                                            ))
                                            .text_size(px(14.0))
                                            .focusable()
                                            .tab_stop(true)
                                            .keyboard_focus(|s| {
                                                s.border_1()
                                                    .border_color(rgb(theme::palette().accent))
                                            })
                                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                            .cursor_pointer()
                                            .text_color(rgb(theme::palette().accent))
                                            .child(format!(
                                                "{lane} · {} · 复制",
                                                target.chars().take(10).collect::<String>()
                                            ))
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    copied.clone(),
                                                ))
                                            })
                                    }))
                            }))
                            .children(
                                (matches!(self.commit_text, Some(Err(_)))
                                    && self
                                        .commit_canvas_layout
                                        .as_ref()
                                        .is_some_and(|layout| layout.has_more))
                                .then(|| {
                                    div()
                                        .id("placeholder-load-more")
                                        .py(px(12.0))
                                        .focusable()
                                        .tab_stop(true)
                                        .keyboard_focus(|s| {
                                            s.border_1().border_color(rgb(theme::palette().accent))
                                        })
                                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                        .cursor_pointer()
                                        .text_color(rgb(theme::palette().accent))
                                        .child("加载更多历史")
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.load_more_history(cx)
                                            }),
                                        )
                                }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .p(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(14.0))
                                    .child(format!("提交 {}", &hash[..hash.len().min(10)])),
                            )
                            .child(
                                div()
                                    .id("copy-commit-details")
                                    .text_size(px(14.0))
                                    .focusable()
                                    .tab_stop(true)
                                    .keyboard_focus(|s| {
                                        s.border_1().border_color(rgb(theme::palette().accent))
                                    })
                                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                    .cursor_pointer()
                                    .child("复制")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(Ok(text)) = &this.commit_text {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                text.clone(),
                                            ));
                                        } else if let Some((_, hash)) = &this.selected_commit {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                hash.clone(),
                                            ));
                                        }
                                    })),
                            )
                            .child(
                                self.track(
                                    "close-commit-details",
                                    div()
                                        .id("close-commit-details")
                                        .text_size(px(14.0))
                                        .focusable()
                                        .tab_stop(true)
                                        .keyboard_focus(|s| {
                                            s.border_1().border_color(rgb(theme::palette().accent))
                                        })
                                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                        .cursor_pointer()
                                        .child("关闭")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.close_commit_details();
                                            cx.notify();
                                        })),
                                ),
                            ),
                    ),
            ),
        )
    }
}

fn render_commit_content(
    app: &GitMasterApp,
    cx: &mut Context<GitMasterApp>,
    data: Option<&crate::commit_details::Presentation>,
    fallback: &str,
    branches: Vec<String>,
) -> AnyElement {
    let Some(data) = data else {
        return div().child(fallback.to_owned()).into_any_element();
    };
    let metadata = [
        (
            "作者",
            data.author.split(" <").next().unwrap_or("").to_owned(),
        ),
        (
            "日期",
            data.date.replace('T', " ").trim_end_matches('Z').to_owned(),
        ),
        ("提交", data.hash.clone()),
        (
            "父提交",
            if data.parents.is_empty() {
                "根提交".into()
            } else {
                data.parents
                    .split_whitespace()
                    .map(|h| h.chars().take(7).collect::<String>())
                    .collect::<Vec<_>>()
                    .join("、")
            },
        ),
        (
            "分支",
            branches
                .into_iter()
                .filter(|b| b != "HEAD")
                .collect::<Vec<_>>()
                .join("、"),
        ),
    ];
    let added: u64 = data
        .files
        .iter()
        .filter_map(|f| f.added.parse::<u64>().ok())
        .sum();
    let removed: u64 = data
        .files
        .iter()
        .filter_map(|f| f.removed.parse::<u64>().ok())
        .sum();
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(
            div()
                .text_size(px(21.0))
                .font_weight(FontWeight::MEDIUM)
                .child(data.title.clone()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .children(metadata.into_iter().map(|(label, value)| {
                    let copied = value.clone();
                    div()
                        .flex()
                        .gap(px(12.0))
                        .text_size(px(16.0))
                        .child(
                            div()
                                .w(px(76.0))
                                .flex_shrink_0()
                                .text_color(rgb(theme::palette().text_subtle))
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(icon(
                                    match label {
                                        "作者" => "user",
                                        "日期" => "calendar",
                                        "提交" => "code",
                                        _ => "git-branch",
                                    },
                                    18.0,
                                ))
                                .child(label),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .id(ElementId::Name(format!("commit-metadata-{label}").into()))
                                .tooltip(super::tooltip::text(value.clone()))
                                .child(value),
                        )
                        .children((label == "提交").then(|| {
                            app.track(
                                "commit-inline-copy",
                                div()
                                    .id("commit-inline-copy")
                                    .focusable()
                                    .tab_stop(true)
                                    .cursor_pointer()
                                    .child(icon("copy", 16.0))
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            copied.clone(),
                                        ))
                                    }),
                            )
                        }))
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(rgb(theme::palette().border))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .h(px(48.0))
                        .border_b_1()
                        .border_color(rgb(theme::palette().border))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(17.0))
                                .child(format!("{} 个文件已更改", data.files.len())),
                        )
                        .child(stat(format!("+{added}"), theme::palette().green))
                        .child(stat(format!("-{removed}"), theme::palette().red)),
                )
                .children(data.files.iter().enumerate().map(|(index, file)| {
                    let key = file.path.clone();
                    let expanded = app.commit_expanded_file.as_ref() == Some(&key);
                    let id = format!("commit-file-{index}");
                    let mut item = div().flex().flex_col().child(
                        app.track(
                            &id,
                            div()
                                .id(ElementId::Name(id.clone().into()))
                                .flex()
                                .items_center()
                                .h(px(38.0))
                                .gap(px(8.0))
                                .border_b_1()
                                .border_color(rgb(theme::palette().border))
                                .focusable()
                                .tab_stop(true)
                                .cursor_pointer()
                                .child(icon(
                                    if expanded {
                                        "chevron-down"
                                    } else {
                                        "chevron-right"
                                    },
                                    16.0,
                                ))
                                .child(icon("file", 18.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(16.0))
                                        .child(file.display_name.clone()),
                                )
                                .child(stat(format!("+{}", file.added), theme::palette().green))
                                .child(stat(format!("-{}", file.removed), theme::palette().red))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_commit_file(key.clone(), cx)
                                })),
                        ),
                    );
                    if expanded {
                        item = item.child(match &app.commit_file_preview {
                            None => div()
                                .p(px(12.0))
                                .child("正在读取只读差异…")
                                .into_any_element(),
                            Some(Err(e)) => div()
                                .p(px(12.0))
                                .text_color(rgb(theme::palette().red))
                                .child(e.clone())
                                .into_any_element(),
                            Some(Ok(rows)) => div()
                                .h(px(280.0))
                                .child(super::diff_view::render(rows, false))
                                .into_any_element(),
                        });
                    }
                    item
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .border_t_1()
                .border_color(rgb(theme::palette().border))
                .pt(px(8.0))
                .text_size(px(16.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex_1()
                                .text_color(rgb(theme::palette().text_subtle))
                                .child("提交正文"),
                        )
                        .child(
                            app.track(
                                "commit-message-source",
                                div()
                                    .id("commit-message-source")
                                    .focusable()
                                    .tab_stop(true)
                                    .keyboard_focus(|s| {
                                        s.border_1().border_color(rgb(theme::palette().accent))
                                    })
                                    .cursor_pointer()
                                    .text_size(px(14.0))
                                    .text_color(rgb(theme::palette().accent))
                                    .child(if app.commit_message_source {
                                        "预览"
                                    } else {
                                        "原文"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.commit_message_source = !this.commit_message_source;
                                        cx.notify();
                                    })),
                            ),
                        )
                        .child(
                            app.track(
                                "commit-message-copy",
                                div()
                                    .id("commit-message-copy")
                                    .focusable()
                                    .tab_stop(true)
                                    .keyboard_focus(|s| {
                                        s.border_1().border_color(rgb(theme::palette().accent))
                                    })
                                    .cursor_pointer()
                                    .text_size(px(14.0))
                                    .child("复制正文")
                                    .on_click({
                                        let source = data.description.clone();
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                source.clone(),
                                            ))
                                        }
                                    }),
                            ),
                        ),
                )
                .child(app.track(
                    "commit-message-body",
                    if app.commit_message_source {
                        div()
                            .whitespace_normal()
                            .child(data.description.clone())
                            .into_any_element()
                    } else {
                        super::markdown::render(&data.markdown)
                    },
                )),
        )
        .into_any_element()
}
fn stat(value: String, color: u32) -> Div {
    div()
        .w(px(44.0))
        .text_size(px(15.0))
        .text_right()
        .text_color(rgb(color))
        .child(value)
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
