use super::log_geometry::{Observed, RowBounds, row_bounds};
use crate::app_state::GitMasterApp;
use gpui::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PopupKind {
    Workspace,
    Remote,
    RemotePicker,
    Reference,
}
impl PopupKind {
    pub const ALL: [Self; 4] = [
        Self::Workspace,
        Self::Remote,
        Self::RemotePicker,
        Self::Reference,
    ];
    fn is_open(self, app: &GitMasterApp) -> bool {
        match self {
            Self::Workspace => app.workspace_menu_open,
            Self::Remote => app.remote_menu.is_some(),
            Self::RemotePicker => app.remote_picker_open,
            Self::Reference => app.overview_ref_picker,
        }
    }
    fn close(self, app: &mut GitMasterApp) {
        match self {
            Self::Workspace => app.workspace_menu_open = false,
            Self::Remote => app.remote_menu = None,
            Self::RemotePicker => app.remote_picker_open = false,
            Self::Reference => app.overview_ref_picker = false,
        }
    }
}
pub struct PopupBoundary {
    bounds: RowBounds,
    kind: PopupKind,
    pub focus: FocusHandle,
}
impl PopupBoundary {
    pub fn new(app: &GitMasterApp, kind: PopupKind) -> Self {
        Self {
            bounds: Default::default(),
            kind,
            focus: app.popup_focus[&kind].clone(),
        }
    }
    pub(super) fn observe(&self, menu: impl IntoElement) -> Observed {
        row_bounds(menu, self.bounds.clone())
    }
    pub fn attach(&self, container: Div, cx: &mut Context<GitMasterApp>) -> Div {
        let kind = self.kind;
        let bounds = self.bounds.clone();
        container
            .on_mouse_down_out(
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if kind.is_open(this)
                        && !bounds
                            .get()
                            .is_some_and(|bounds| bounds.contains(&event.position))
                    {
                        this.close_popup(kind, window);
                        cx.notify();
                    }
                }),
            )
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if kind.is_open(this) && event.keystroke.key == "escape" {
                    this.close_popup(kind, window);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
    }
}
impl GitMasterApp {
    pub(crate) fn dismiss_open_popup(&mut self, window: &mut Window) -> bool {
        let Some(kind) = PopupKind::ALL.into_iter().find(|kind| kind.is_open(self)) else {
            return false;
        };
        self.close_popup(kind, window);
        true
    }
    pub(super) fn close_popup(&mut self, kind: PopupKind, window: &mut Window) {
        kind.close(self);
        if let Some(focus) = self.popup_focus.get(&kind) {
            window.focus(focus);
        }
    }
}
