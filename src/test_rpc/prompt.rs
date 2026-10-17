use super::tracked::{self, BoundsRegistry};
use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct Description {
    pub message: String,
    pub detail: String,
    pub answers: Vec<String>,
}
pub type PromptState = Rc<RefCell<Option<Description>>>;
struct TestPrompt {
    description: Description,
    state: PromptState,
    bounds: BoundsRegistry,
    focus: FocusHandle,
}

// Tests select the GPUI custom-prompt boundary. Production keeps platform dialogs.
pub fn build(
    message: &str,
    detail: Option<&str>,
    answers: &[PromptButton],
    handle: PromptHandle,
    window: &mut Window,
    cx: &mut App,
    bounds: BoundsRegistry,
    state: PromptState,
) -> RenderablePromptHandle {
    let description = Description {
        message: message.into(),
        detail: detail.unwrap_or_default().into(),
        answers: answers.iter().map(|a| a.label().to_string()).collect(),
    };
    *state.borrow_mut() = Some(description.clone());
    let view = cx.new(|cx| TestPrompt {
        description,
        state,
        bounds,
        focus: cx.focus_handle(),
    });
    handle.with_view(view, window, cx)
}
impl EventEmitter<PromptResponse> for TestPrompt {}
impl Focusable for TestPrompt {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for TestPrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
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
                    .track_focus(&self.focus)
                    .w(px(440.0))
                    .p(px(20.0))
                    .rounded(px(10.0))
                    .bg(rgb(0x172033))
                    .text_color(rgb(0xe8eef8))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(self.description.message.clone())
                    .child(self.description.detail.clone())
                    .children(
                        self.description
                            .answers
                            .iter()
                            .enumerate()
                            .map(|(index, label)| {
                                let id = format!("test-prompt-answer-{index}");
                                tracked::tracked(
                                    &id,
                                    div()
                                        .id(ElementId::Name(id.clone().into()))
                                        .p(px(8.0))
                                        .bg(rgb(0x263248))
                                        .cursor_pointer()
                                        .child(label.clone())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            *this.state.borrow_mut() = None;
                                            cx.emit(PromptResponse(index));
                                            cx.notify();
                                        })),
                                    &self.bounds,
                                )
                            }),
                    ),
            )
    }
}
