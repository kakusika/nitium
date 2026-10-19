//! Page zoom steps (Ctrl +, Ctrl -, Ctrl 0).

/// The zoom levels offered, as multipliers of the normal size.
pub const LEVELS: &[f32] = &[
    0.3, 0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0,
];

pub const DEFAULT: f32 = 1.0;

/// Levels closer than this are considered equal (zoom values round-trip through floats).
const EPSILON: f32 = 0.001;

/// The next larger level, or the largest if already there.
pub fn zoom_in(current: f32) -> f32 {
    LEVELS
        .iter()
        .copied()
        .find(|&level| level > current + EPSILON)
        .unwrap_or(LEVELS[LEVELS.len() - 1])
}

/// The next smaller level, or the smallest if already there.
pub fn zoom_out(current: f32) -> f32 {
    LEVELS
        .iter()
        .rev()
        .copied()
        .find(|&level| level < current - EPSILON)
        .unwrap_or(LEVELS[0])
}

/// `1.25` -> `125`.
pub fn percent(zoom: f32) -> i32 {
    (zoom * 100.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_up_and_down() {
        assert_eq!(zoom_in(1.0), 1.1);
        assert_eq!(zoom_out(1.0), 0.9);
        assert_eq!(zoom_in(1.1), 1.25);
        assert_eq!(zoom_out(1.25), 1.1);
    }

    #[test]
    fn clamps_at_the_ends() {
        assert_eq!(zoom_in(5.0), 5.0);
        assert_eq!(zoom_in(9.0), 5.0);
        assert_eq!(zoom_out(0.3), 0.3);
        assert_eq!(zoom_out(0.01), 0.3);
    }

    #[test]
    fn off_grid_values_snap_to_the_next_level() {
        assert_eq!(zoom_in(1.05), 1.1);
        assert_eq!(zoom_out(1.05), 1.0);
    }

    #[test]
    fn tolerates_float_noise() {
        assert_eq!(zoom_in(1.0000001), 1.1);
        assert_eq!(zoom_out(0.9999999), 0.9);
    }

    #[test]
    fn levels_are_sorted_and_include_default() {
        assert!(LEVELS.windows(2).all(|w| w[0] < w[1]));
        assert!(LEVELS.contains(&DEFAULT));
    }

    #[test]
    fn percent_rounds() {
        assert_eq!(percent(1.0), 100);
        assert_eq!(percent(1.25), 125);
        assert_eq!(percent(0.67), 67);
    }
}
