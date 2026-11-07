use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

// Shared by the main window, auxiliary windows and canvas paint callbacks.
// Only the application UI changes the mode; palettes themselves are immutable.
static LIGHT: AtomicBool = AtomicBool::new(false);
pub fn mode() -> ThemeMode {
    if LIGHT.load(Ordering::Relaxed) {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}
pub fn set_mode(mode: ThemeMode) {
    LIGHT.store(mode == ThemeMode::Light, Ordering::Relaxed);
}
pub fn palette() -> &'static Palette {
    palette_for(mode())
}
pub fn palette_for(mode: ThemeMode) -> &'static Palette {
    match mode {
        ThemeMode::Light => &LIGHT_PALETTE,
        ThemeMode::Dark => &DARK_PALETTE,
    }
}

pub struct Palette {
    pub bg_base: u32,
    pub bg_surface: u32,
    pub bg_elevated: u32,
    pub bg_overlay: u32,
    pub border: u32,
    pub text_primary: u32,
    pub text_subtle: u32,
    pub text_muted: u32,
    pub accent_fill: u32,
    pub danger_fill: u32,
    pub accent_pressed: u32,
    pub danger_pressed: u32,
    pub accent: u32,
    pub accent_soft: u32,
    pub green: u32,
    pub red: u32,
    pub yellow: u32,
    pub diff_added: u32,
    pub diff_removed: u32,
    pub diff_added_word: u32,
    pub diff_removed_word: u32,
}
pub const DARK_PALETTE: Palette = Palette {
    bg_base: 0x1e1e1e,
    bg_surface: 0x252526,
    bg_elevated: 0x2d2d30,
    bg_overlay: 0x383838,
    border: 0x3e3e42,
    text_primary: 0xeeeeee,
    text_subtle: 0xc0c0c0,
    text_muted: 0x909090,
    accent_fill: 0x006fc9,
    danger_fill: 0xbe4c48,
    accent_pressed: 0x005da8,
    danger_pressed: 0xa83c38,
    accent: 0x72aee6,
    accent_soft: 0x303e4d,
    green: 0x80bc88,
    red: 0xe88b88,
    yellow: 0xdbb56c,
    diff_added: 0x20382a,
    diff_removed: 0x402a2b,
    diff_added_word: 0x31573e,
    diff_removed_word: 0x693b3d,
};
pub const LIGHT_PALETTE: Palette = Palette {
    bg_base: 0xfafafa,
    bg_surface: 0xf2f2f2,
    bg_elevated: 0xededed,
    bg_overlay: 0xe3e3e3,
    border: 0xdedede,
    text_primary: 0x242424,
    text_subtle: 0x626262,
    text_muted: 0x707070,
    accent_fill: 0x006fc9,
    danger_fill: 0xbe4c48,
    accent_pressed: 0x005da8,
    danger_pressed: 0xa83c38,
    accent: 0x2867a8,
    accent_soft: 0xdce8f3,
    green: 0x237a3b,
    red: 0xbc3434,
    yellow: 0x93630b,
    diff_added: 0xe4f2e7,
    diff_removed: 0xfbe8e7,
    diff_added_word: 0xb9dfc2,
    diff_removed_word: 0xf1bfbd,
};
pub fn sync_color(counts: Option<(usize, usize)>) -> u32 {
    match counts {
        None => palette().text_subtle,
        Some((0, 0)) => palette().green,
        Some((_, 0)) => palette().accent,
        Some((0, _)) => palette().yellow,
        Some(_) => palette().red,
    }
}

pub fn graph_color(column: usize) -> u32 {
    [
        palette().accent,
        palette().green,
        palette().yellow,
        palette().red,
    ][column % 4]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primary_action_text_retains_contrast_in_each_interaction_state() {
        fn white_contrast(color: u32) -> f64 {
            let linear = |shift: u32| {
                let channel = ((color >> shift) & 255u32) as f64 / 255.0;
                if channel <= 0.04045 {
                    channel / 12.92
                } else {
                    ((channel + 0.055) / 1.055).powf(2.4)
                }
            };
            1.05 / (0.05 + 0.2126 * linear(16) + 0.7152 * linear(8) + 0.0722 * linear(0))
        }
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let p = palette_for(mode);
            for fill in [
                p.accent_fill,
                p.danger_fill,
                p.accent_pressed,
                p.danger_pressed,
            ] {
                assert!(
                    white_contrast(fill) >= 4.5,
                    "insufficient action text contrast for {fill:06x}"
                );
            }
        }
    }
    #[test]
    fn palettes_keep_status_colors_distinct() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let p = palette_for(mode);
            let colors = [p.text_subtle, p.green, p.accent, p.yellow, p.red];
            assert_eq!(
                colors
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                5
            );
        }
    }
}
