pub const BG_BASE: u32 = 0x1e1e2e;
pub const BG_SURFACE: u32 = 0x181825;
pub const BG_OVERLAY: u32 = 0x313244;
pub const TEXT_PRIMARY: u32 = 0xcdd6f4;
pub const TEXT_SUBTLE: u32 = 0xa6adc8;
pub const ACCENT: u32 = 0x89b4fa;
pub const GREEN: u32 = 0xa6e3a1;
pub const RED: u32 = 0xf38ba8;
pub const YELLOW: u32 = 0xf9e2af;

pub fn sync_color(counts: Option<(usize, usize)>) -> u32 {
    match counts {
        None => TEXT_SUBTLE,
        Some((0, 0)) => GREEN,
        Some((_, 0)) => ACCENT,
        Some((0, _)) => YELLOW,
        Some(_) => RED,
    }
}

pub fn graph_color(column: usize) -> u32 {
    [ACCENT, GREEN, YELLOW, RED][column % 4]
}

#[cfg(test)]
mod tests {
    #[test]
    fn sync_states_have_distinct_colors() {
        let colors =
            [None, Some((0, 0)), Some((1, 0)), Some((0, 1)), Some((1, 1))].map(super::sync_color);
        assert_eq!(
            colors
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            5
        );
    }
}
