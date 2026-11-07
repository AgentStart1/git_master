use gpui::*;
use std::sync::atomic::{AtomicBool, Ordering};

// Input modality is shared by application windows, like the shared theme.
// Retain actual focus for activation/text entry; only the visual ring changes.
static KEYBOARD_NAVIGATION: AtomicBool = AtomicBool::new(false);

pub trait KeyboardFocusStyle: Sized {
    fn keyboard_focus(self, style: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self;
}
impl KeyboardFocusStyle for Stateful<Div> {
    fn keyboard_focus(self, style: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self {
        if KEYBOARD_NAVIGATION.load(Ordering::Relaxed) {
            self.focus(style)
        } else {
            self
        }
    }
}

fn navigation_key(key: &str) -> bool {
    matches!(
        key,
        "tab"
            | "left"
            | "right"
            | "up"
            | "down"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
            | "enter"
            | "space"
            | "escape"
    )
}
fn set_keyboard_navigation(visible: bool, cx: &mut App) {
    if KEYBOARD_NAVIGATION.swap(visible, Ordering::Relaxed) != visible {
        cx.refresh_windows();
    }
}

// Capture before child handlers, including menus that consume Tab/Escape.
// Do not consume input or blur the focused element.
pub fn track_input(root: Div) -> impl IntoElement {
    let root = root.capture_key_down(|event, _, cx| {
        if navigation_key(&event.keystroke.key) {
            set_keyboard_navigation(true, cx);
        }
    });
    super::log_geometry::paint_with(root, |window, _| {
        // Window-level capture also sees pointer input on occluding popovers.
        window.on_mouse_event(|_: &MouseDownEvent, phase, _, cx| {
            if phase == DispatchPhase::Capture {
                set_keyboard_navigation(false, cx);
            }
        });
    })
}

#[cfg(test)]
mod tests {
    #[::core::prelude::v1::test]
    fn navigation_enables_focus_rings_but_typing_or_modifiers_do_not() {
        for key in [
            "tab", "left", "right", "up", "down", "home", "end", "pageup", "pagedown", "enter",
            "space", "escape",
        ] {
            assert!(super::navigation_key(key), "{key}");
        }
        for key in ["a", "k", "1", "shift", "control", "alt", "cmd"] {
            assert!(!super::navigation_key(key), "{key}");
        }
    }
}
