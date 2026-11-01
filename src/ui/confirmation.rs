use super::theme;
use crate::modal_input::{KeyInput, ModalCommand, ModalInputHost, Scroll};
use gpui::*;
use std::{cell::RefCell, rc::Rc};

pub type ModalInput = Rc<RefCell<ModalInputHost>>;

pub fn key_input(event: &KeyDownEvent) -> KeyInput<'_> {
    stroke_input(&event.keystroke, event.is_held)
}

fn stroke_input(stroke: &Keystroke, repeat: bool) -> KeyInput<'_> {
    KeyInput {
        key: &stroke.key,
        repeat,
        shift: stroke.modifiers.shift,
        control: stroke.modifiers.control,
        alt: stroke.modifiers.alt,
        platform: stroke.modifiers.platform,
    }
}

struct Confirmation {
    message: String,
    detail: Option<String>,
    answers: Vec<PromptButton>,
    cancel: usize,
    input: ModalInput,
    buttons: Vec<FocusHandle>,
    detail_scroll: ScrollHandle,
    #[cfg(feature = "test-rpc")]
    observation: crate::test_rpc::prompt::Observation,
}

pub fn build(
    message: &str,
    detail: Option<&str>,
    answers: &[PromptButton],
    handle: PromptHandle,
    window: &mut Window,
    cx: &mut App,
    input: ModalInput,
    #[cfg(feature = "test-rpc")] observation: crate::test_rpc::prompt::Observation,
) -> RenderablePromptHandle {
    assert!(
        !answers.is_empty(),
        "A confirmation requires at least one answer"
    );
    input.borrow_mut().begin();
    let view = cx.new(|cx| Confirmation {
        message: message.into(),
        detail: detail.map(str::to_owned),
        answers: answers.to_vec(),
        cancel: answers
            .iter()
            .position(|answer| matches!(answer, PromptButton::Cancel(_)))
            .unwrap_or(answers.len().saturating_sub(1)),
        input,
        buttons: answers.iter().map(|_| cx.focus_handle()).collect(),
        detail_scroll: ScrollHandle::new(),
        #[cfg(feature = "test-rpc")]
        observation,
    });
    // GPUI captures the previous focus here and restores it on PromptResponse.
    handle.with_view(view, window, cx)
}

impl Confirmation {
    fn respond(&mut self, index: usize, cx: &mut Context<Self>) {
        self.input.borrow_mut().close();
        #[cfg(feature = "test-rpc")]
        {
            *self.observation.state.borrow_mut() = None;
            self.observation.owner.update(cx, |_, cx| cx.notify()).ok();
        }
        cx.emit(PromptResponse(index));
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let command = self
            .input
            .borrow_mut()
            .key_down(&key_input(event), self.focused(window));
        self.apply_command(command, window, cx);
    }

    fn key_up(&mut self, event: &KeyUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Capture before GPUI's generic key-up click handler: activation must
        // belong to a matching press in this dialog on the same button.
        cx.stop_propagation();
        let command = self
            .input
            .borrow_mut()
            .key_up(&stroke_input(&event.keystroke, false), self.focused(window));
        self.apply_command(command, window, cx);
    }

    fn focused(&self, window: &Window) -> usize {
        self.buttons
            .iter()
            .position(|focus| focus.is_focused(window))
            .unwrap_or(self.cancel)
    }

    fn apply_command(
        &mut self,
        command: ModalCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focused = self
            .buttons
            .iter()
            .position(|focus| focus.is_focused(window))
            .unwrap_or(self.cancel);
        match command {
            ModalCommand::CycleFocus { reverse } => {
                let next = if reverse {
                    (focused + self.buttons.len() - 1) % self.buttons.len()
                } else {
                    (focused + 1) % self.buttons.len()
                };
                window.focus(&self.buttons[next]);
                cx.notify();
            }
            ModalCommand::Cancel => self.respond(self.cancel, cx),
            ModalCommand::Activate(index) => self.respond(index, cx),
            ModalCommand::Scroll(direction) => {
                let scroll = &self.detail_scroll;
                let offset = scroll.offset();
                let max = scroll.max_offset().height;
                let page = scroll.bounds().size.height * 0.9;
                let y = match direction {
                    Scroll::Up => offset.y + px(24.0),
                    Scroll::Down => offset.y - px(24.0),
                    Scroll::PageUp => offset.y + page,
                    Scroll::PageDown => offset.y - page,
                    Scroll::Start => px(0.0),
                    Scroll::End => -max,
                };
                scroll.set_offset(point(offset.x, y.max(-max).min(px(0.0))));
                cx.notify();
            }
            ModalCommand::None => {}
        }
    }
}

impl EventEmitter<PromptResponse> for Confirmation {}

impl Focusable for Confirmation {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.buttons[self.cancel].clone()
    }
}

impl Render for Confirmation {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(feature = "test-rpc")]
        {
            let description = crate::test_rpc::prompt::Description {
                message: self.message.clone(),
                detail: self.detail.clone().unwrap_or_default(),
                answers: self
                    .answers
                    .iter()
                    .map(|answer| answer.label().to_string())
                    .collect(),
                focused_answer: self
                    .buttons
                    .iter()
                    .position(|focus| focus.is_focused(window)),
                detail_scroll_y: self.detail_scroll.offset().y.into(),
            };
            if self.observation.state.borrow().as_ref() != Some(&description) {
                *self.observation.state.borrow_mut() = Some(description);
                let owner = self.observation.owner.clone();
                cx.defer(move |cx| {
                    owner.update(cx, |_, cx| cx.notify()).ok();
                });
            }
        }
        let viewport = window.viewport_size();
        let buttons = self
            .answers
            .iter()
            .enumerate()
            .map(|(index, answer)| {
                let focus = self.buttons[index].clone();
                let button = div()
                    .id(ElementId::Name(
                        format!("confirmation-answer-{index}").into(),
                    ))
                    .track_focus(&focus)
                    .px(px(14.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .border_2()
                    .border_color(if focus.is_focused(window) {
                        rgb(theme::ACCENT)
                    } else {
                        rgb(theme::BORDER)
                    })
                    .bg(rgb(theme::BG_OVERLAY))
                    .text_sm()
                    .text_color(rgb(if index == self.cancel {
                        theme::TEXT_PRIMARY
                    } else {
                        theme::RED
                    }))
                    .cursor_pointer()
                    .child(answer.label().to_string())
                    .on_click(cx.listener(move |this, _, _, cx| this.respond(index, cx)));
                #[cfg(feature = "test-rpc")]
                let button = crate::test_rpc::tracked::tracked(
                    &format!("test-prompt-answer-{index}"),
                    button,
                    &self.observation.bounds,
                );
                button
            })
            .collect::<Vec<_>>();
        div()
            .id("confirmation-overlay")
            .key_context("Confirmation")
            .capture_key_down(cx.listener(Self::key_down))
            .capture_key_up(cx.listener(Self::key_up))
            .occlude()
            .on_any_mouse_down(|_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .bg(rgba(0x000000aa))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(520.0).min(viewport.width - px(32.0)))
                    .max_h(viewport.height - px(32.0))
                    .p(px(20.0))
                    .rounded(px(10.0))
                    .bg(rgb(theme::BG_ELEVATED))
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .child(div().flex_shrink_0().text_lg().child(self.message.clone()))
                    .children(self.detail.as_ref().map(|detail| {
                        div()
                            .id("confirmation-detail")
                            .min_h(px(0.0))
                            .max_h(viewport.height * 0.5)
                            .overflow_y_scroll()
                            .track_scroll(&self.detail_scroll)
                            .text_sm()
                            .child(detail.clone())
                    }))
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_wrap()
                            .justify_end()
                            .gap(px(10.0))
                            .children(buttons),
                    ),
            )
    }
}
