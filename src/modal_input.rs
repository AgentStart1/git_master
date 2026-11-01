use std::collections::HashSet;

#[derive(Default)]
pub struct ModalInputHost {
    active: bool,
    pressed: HashSet<String>,
    guarded_releases: HashSet<String>,
    activation: Option<(String, usize)>,
}

pub struct KeyInput<'a> {
    pub key: &'a str,
    pub repeat: bool,
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub platform: bool,
}

impl KeyInput<'_> {
    fn unmodified(&self) -> bool {
        !self.shift && !self.control && !self.alt && !self.platform
    }
}

#[derive(Debug, PartialEq)]
pub enum ModalCommand {
    None,
    CycleFocus { reverse: bool },
    Cancel,
    Activate(usize),
    Scroll(Scroll),
}

#[derive(Debug, PartialEq)]
pub enum Scroll {
    Up,
    Down,
    PageUp,
    PageDown,
    Start,
    End,
}

impl ModalInputHost {
    pub fn begin(&mut self) {
        self.active = true;
        self.activation = None;
        // A release belonging to an earlier view cannot activate this dialog.
        self.guarded_releases.extend(self.pressed.iter().cloned());
    }

    pub fn close(&mut self) {
        self.active = false;
        self.activation = None;
        // The window retains this ownership after GPUI restores the trigger's focus.
        self.guarded_releases.extend(self.pressed.iter().cloned());
    }

    fn press(&mut self, input: &KeyInput<'_>) -> bool {
        self.pressed.insert(input.key.to_owned());
        if !input.repeat {
            // A fresh press also recovers from key-up lost during window deactivation.
            self.guarded_releases.remove(input.key);
        }
        self.guarded_releases.contains(input.key)
    }

    fn release(&mut self, key: &str) -> bool {
        self.pressed.remove(key);
        self.guarded_releases.remove(key)
    }

    /// Returns whether the main interface must consume this event before any control sees it.
    pub fn outside_key_down(&mut self, input: &KeyInput<'_>) -> bool {
        let guarded = self.press(input);
        self.active || guarded
    }

    pub fn outside_key_up(&mut self, key: &str) -> bool {
        let guarded = self.release(key);
        self.active || guarded
    }

    pub fn key_down(&mut self, input: &KeyInput<'_>, focused: usize) -> ModalCommand {
        if self.press(input) || !self.active {
            return ModalCommand::None;
        }
        match input.key {
            "escape" if !input.repeat && !input.control && !input.alt && !input.platform => {
                ModalCommand::Cancel
            }
            "f4" if !input.repeat && input.alt && !input.control && !input.platform => {
                ModalCommand::Cancel
            }
            "tab" if !input.control && !input.alt && !input.platform => {
                self.activation = None;
                ModalCommand::CycleFocus {
                    reverse: input.shift,
                }
            }
            "enter" | "space" if input.unmodified() && !input.repeat => {
                self.activation = Some((input.key.to_owned(), focused));
                ModalCommand::None
            }
            "up" => ModalCommand::Scroll(Scroll::Up),
            "down" => ModalCommand::Scroll(Scroll::Down),
            "pageup" => ModalCommand::Scroll(Scroll::PageUp),
            "pagedown" => ModalCommand::Scroll(Scroll::PageDown),
            "home" => ModalCommand::Scroll(Scroll::Start),
            "end" => ModalCommand::Scroll(Scroll::End),
            _ => ModalCommand::None,
        }
    }

    pub fn key_up(&mut self, input: &KeyInput<'_>, focused: usize) -> ModalCommand {
        let guarded = self.release(input.key);
        let armed = self
            .activation
            .as_ref()
            .is_some_and(|(key, _)| key == input.key);
        if !armed {
            return ModalCommand::None;
        }
        let (_, answer) = self.activation.take().expect("Matching activation exists");
        if self.active && !guarded && input.unmodified() && answer == focused {
            ModalCommand::Activate(answer)
        } else {
            ModalCommand::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: &str) -> KeyInput<'_> {
        KeyInput {
            key,
            repeat: false,
            shift: false,
            control: false,
            alt: false,
            platform: false,
        }
    }

    #[test]
    fn cancellation_owns_held_enter_until_its_release_after_focus_restoration() {
        let mut host = ModalInputHost::default();
        host.begin();
        host.key_down(&key("enter"), 1);
        assert_eq!(host.key_down(&key("escape"), 1), ModalCommand::Cancel);
        host.close();
        assert!(host.outside_key_up("escape"));
        let repeat = KeyInput {
            repeat: true,
            ..key("enter")
        };
        assert!(host.outside_key_down(&repeat));
        assert!(host.outside_key_up("enter"));
        assert!(!host.outside_key_down(&key("enter")));
        assert!(!host.outside_key_up("enter"));
    }

    #[test]
    fn a_new_dialog_does_not_accept_an_earlier_dialogs_key_release() {
        let mut host = ModalInputHost::default();
        host.begin();
        host.key_down(&key("space"), 0);
        host.close();
        host.begin();
        assert_eq!(host.key_up(&key("space"), 0), ModalCommand::None);
    }

    #[test]
    fn activation_requires_a_matching_press_and_release_on_the_same_button() {
        let mut host = ModalInputHost::default();
        host.begin();
        assert_eq!(host.key_up(&key("enter"), 0), ModalCommand::None);
        host.key_down(&key("enter"), 0);
        assert_eq!(host.key_up(&key("enter"), 1), ModalCommand::None);
        host.key_down(&key("space"), 0);
        assert_eq!(host.key_up(&key("space"), 0), ModalCommand::Activate(0));
        assert_eq!(host.key_up(&key("space"), 0), ModalCommand::None);
    }

    #[test]
    fn tab_cancels_pending_activation_instead_of_activating_the_new_focus() {
        let mut host = ModalInputHost::default();
        host.begin();
        host.key_down(&key("enter"), 1);
        assert_eq!(
            host.key_down(&key("tab"), 1),
            ModalCommand::CycleFocus { reverse: false }
        );
        assert_eq!(host.key_up(&key("enter"), 0), ModalCommand::None);
    }

    #[test]
    fn alt_f4_is_a_modal_cancellation_command() {
        let mut host = ModalInputHost::default();
        host.begin();
        let close = KeyInput {
            alt: true,
            ..key("f4")
        };
        assert_eq!(host.key_down(&close, 0), ModalCommand::Cancel);
        host.close();
        assert!(host.outside_key_up("f4"));
    }

    #[test]
    fn fresh_presses_recover_when_an_owned_release_was_lost_outside_the_window() {
        let mut host = ModalInputHost::default();
        host.begin();
        host.key_down(&key("enter"), 1);
        host.close();
        assert!(!host.outside_key_down(&key("enter")));
        assert!(!host.outside_key_up("enter"));
    }
}
