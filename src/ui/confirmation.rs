use super::{icons::icon, theme};
use gpui::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub struct Description {
    pub message: String,
    pub detail: String,
    pub answers: Vec<String>,
}
pub type PromptState = Rc<RefCell<Option<Description>>>;
#[derive(Clone, Default)]
pub struct Instrumentation {
    #[cfg(feature = "test-rpc")]
    pub bounds: crate::test_rpc::tracked::BoundsRegistry,
    pub state: PromptState,
}

/// Structured content crosses GPUI's string-based prompt boundary without losing field identity.
pub fn details(
    icon: &str,
    rows: Vec<(&str, String)>,
    paragraphs: Vec<String>,
    notice: Option<(&str, String)>,
    table: Vec<Vec<String>>,
) -> String {
    serde_json::json!({"icon":icon,"rows":rows,"paragraphs":paragraphs,"notice":notice,"table":table}).to_string()
}

struct Confirmation {
    description: Description,
    instrumentation: Instrumentation,
    focus: FocusHandle,
    selected: usize,
    critical: bool,
}

pub fn build(
    level: PromptLevel,
    message: &str,
    detail: Option<&str>,
    answers: &[PromptButton],
    handle: PromptHandle,
    window: &mut Window,
    cx: &mut App,
    instrumentation: Instrumentation,
) -> RenderablePromptHandle {
    let description = Description {
        message: message.into(),
        detail: detail.unwrap_or_default().into(),
        answers: answers.iter().map(|a| a.label().to_string()).collect(),
    };
    *instrumentation.state.borrow_mut() = Some(description.clone());
    let view = cx.new(|cx| Confirmation {
        description,
        instrumentation,
        focus: cx.focus_handle(),
        selected: 0,
        critical: matches!(level, PromptLevel::Critical),
    });
    handle.with_view(view, window, cx)
}
impl EventEmitter<PromptResponse> for Confirmation {}
impl Focusable for Confirmation {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Confirmation {
    fn answer(&mut self, index: usize, cx: &mut Context<Self>) {
        *self.instrumentation.state.borrow_mut() = None;
        cx.emit(PromptResponse(index));
        cx.notify();
    }
}
impl Render for Confirmation {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette();
        let content = serde_json::from_str::<serde_json::Value>(&self.description.detail).ok();
        let icon_name = match content
            .as_ref()
            .and_then(|v| v["icon"].as_str())
            .unwrap_or("info-circle")
        {
            "trash" => "trash",
            "refresh" => "refresh",
            "arrows-exchange" => "arrows-exchange",
            "alert-circle" => "alert-circle",
            _ => "info-circle",
        };
        let has_table = content
            .as_ref()
            .and_then(|v| v["table"].as_array())
            .is_some_and(|a| !a.is_empty());
        let width: f32 = if has_table { 680.0 } else { 460.0 };
        let viewport_width: f32 = window.viewport_size().width.into();
        let viewport_height: f32 = window.viewport_size().height.into();
        let card_width = width.min(viewport_width - 40.0);
        let text_width = (card_width - 80.0).max(150.0);
        let mut body = div()
            .id("confirmation-content")
            .flex()
            .flex_col()
            .w_full()
            .gap(px(20.0))
            .min_h_0()
            .max_h(px((viewport_height - 220.0).max(140.0)))
            .overflow_y_scroll();
        if let Some(content) = content {
            if let Some(rows) = content["rows"].as_array() {
                body = body.children(rows.iter().map(|row| {
                    let label = row[0].as_str().unwrap_or("");
                    div()
                        .flex()
                        .gap(px(14.0))
                        .items_start()
                        .child(icon(
                            match label {
                                "仓库" | "记录" | "文件" => "folder",
                                "目标" | "当前分支" => "git-branch",
                                _ => "info-circle",
                            },
                            20.0,
                        ))
                        .child(div().w(px(65.0)).flex_shrink_0().child(label.to_owned()))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(row[1].as_str().unwrap_or("").to_owned()),
                        )
                }));
            }
            if let Some(table) = content["table"].as_array().filter(|t| !t.is_empty()) {
                body = body.child(
                    div()
                        .flex()
                        .py(px(8.0))
                        .bg(rgb(p.bg_elevated))
                        .child(div().flex_1().child("仓库"))
                        .child(div().w(px(150.0)).child("目标分支"))
                        .child(div().w(px(170.0)).child("状态")),
                );
                body = body.children(table.iter().map(|row| {
                    div()
                        .flex()
                        .gap(px(12.0))
                        .py(px(8.0))
                        .border_b_1()
                        .border_color(rgb(p.border))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(row[0].as_str().unwrap_or("").to_owned()),
                        )
                        .child(
                            div()
                                .w(px(150.0))
                                .child(row[1].as_str().unwrap_or("").to_owned()),
                        )
                        .child(
                            div()
                                .w(px(170.0))
                                .text_color(rgb(if row[2].as_str() == Some("可切换") {
                                    p.green
                                } else {
                                    p.yellow
                                }))
                                .child(row[2].as_str().unwrap_or("").to_owned()),
                        )
                }));
            }
            if let Some(paragraphs) = content["paragraphs"].as_array() {
                body = body.children(paragraphs.iter().map(|v| {
                    div()
                        .flex()
                        .flex_shrink_0()
                        .gap(px(12.0))
                        .pt(px(12.0))
                        .border_t_1()
                        .border_color(rgb(p.border))
                        .child(icon("info-circle", 20.0))
                        .child(
                            div()
                                .w(px(text_width))
                                .flex_shrink_0()
                                .whitespace_normal()
                                .line_height(px(24.0))
                                .child(v.as_str().unwrap_or("").to_owned()),
                        )
                }));
            }
            if content["notice"].is_array() {
                let ok = content["notice"][0].as_str() == Some("success");
                body = body.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(10.0))
                        .p(px(10.0))
                        .rounded(px(4.0))
                        .bg(rgb(if ok { p.diff_added } else { p.bg_elevated }))
                        .text_color(rgb(if ok { p.green } else { p.yellow }))
                        .child(
                            icon(if ok { "circle-check" } else { "alert-triangle" }, 20.0)
                                .text_color(rgb(if ok { p.green } else { p.yellow })),
                        )
                        .child(
                            div()
                                .w(px((text_width - 20.0).max(130.0)))
                                .flex_shrink_0()
                                .whitespace_normal()
                                .line_height(px(24.0))
                                .child(content["notice"][1].as_str().unwrap_or("").to_owned()),
                        ),
                );
            }
        } else {
            body = body.child(self.description.detail.clone());
        }
        let buttons = self
            .description
            .answers
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let id = format!("test-prompt-answer-{index}");
                let primary = index > 0;
                let danger = primary
                    && self.critical
                    && (label.contains("删除")
                        || label.contains("重置")
                        || label.contains("Reset"));
                let bg = if danger {
                    p.danger_fill
                } else if primary {
                    p.accent_fill
                } else {
                    p.bg_elevated
                };
                let button = div()
                    .id(ElementId::Name(id.clone().into()))
                    .flex_1()
                    .min_h(px(40.0))
                    .px(px(14.0))
                    .py(px(9.0))
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(rgb(if self.selected == index {
                        p.accent
                    } else {
                        p.border
                    }))
                    .bg(rgb(bg))
                    .text_color(rgb(if primary { 0xffffff } else { p.text_primary }))
                    .text_center()
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(rgb(if danger {
                            p.danger_pressed
                        } else if primary {
                            p.accent_pressed
                        } else {
                            p.bg_overlay
                        }))
                    })
                    .active(|s| {
                        s.bg(rgb(if danger {
                            p.danger_pressed
                        } else if primary {
                            p.accent_pressed
                        } else {
                            p.bg_overlay
                        }))
                    })
                    .child(label.clone())
                    .on_click(cx.listener(move |this, _, _, cx| this.answer(index, cx)));
                #[cfg(feature = "test-rpc")]
                {
                    crate::test_rpc::tracked::tracked(&id, button, &self.instrumentation.bounds)
                }
                #[cfg(not(feature = "test-rpc"))]
                {
                    button.into_any_element()
                }
            })
            .collect::<Vec<_>>();
        div()
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .bg(rgba(0x00000055))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .track_focus(&self.focus)
                    .w(px(width.min(viewport_width - 40.0)))
                    .p(px(24.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(rgb(p.border))
                    .shadow_lg()
                    .bg(rgb(p.bg_base))
                    .text_color(rgb(p.text_primary))
                    .font_family("Segoe UI")
                    .text_size(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(24.0))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => this.answer(0, cx),
                            "tab" | "left" | "right" => {
                                this.selected =
                                    (this.selected + 1) % this.description.answers.len().max(1);
                                cx.notify();
                            }
                            // Keep the modal mounted until key-up, so restoring focus
                            // cannot activate the button that opened it with that key-up.
                            "enter" | "space" => {}
                            _ => return,
                        }
                        cx.stop_propagation();
                    }))
                    .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            this.answer(this.selected, cx);
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(14.0))
                            .pb(px(20.0))
                            .border_b_1()
                            .border_color(rgb(p.border))
                            .child(icon(icon_name, 26.0))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(23.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.description.message.clone()),
                            ),
                    )
                    .child(body)
                    .child(div().flex().gap(px(16.0)).pt(px(8.0)).children(buttons)),
            )
    }
}
