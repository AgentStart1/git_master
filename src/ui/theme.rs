pub const BG_BASE: u32 = 0x0a0e16;
pub const BG_SURFACE: u32 = 0x101725;
pub const BG_ELEVATED: u32 = 0x172033;
pub const BG_OVERLAY: u32 = 0x263149;
pub const BORDER: u32 = 0x263248;
pub const TEXT_PRIMARY: u32 = 0xe8eef8;
pub const TEXT_SUBTLE: u32 = 0x91a0b8;
pub const TEXT_MUTED: u32 = 0x65738b;
pub const ACCENT: u32 = 0x6ea8fe;
pub const ACCENT_SOFT: u32 = 0x172b49;
pub const GREEN: u32 = 0x61d6a3;
pub const RED: u32 = 0xff748d;
pub const YELLOW: u32 = 0xf2c76e;

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
