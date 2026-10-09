//! Physical layout of the ZSA Voyager.
//!
//! Positions and matrix wiring come from QMK's `keyboards/zsa/voyager/keyboard.json`.
//! Key indices (0..52) follow the same order as that file's LAYOUT and as Oryx's
//! per-layer key arrays: left half rows, left thumbs, right half rows, right thumbs.

pub const KEY_COUNT: usize = 52;

/// Width of the board in key units.
pub const BOARD_UNITS_W: f32 = 16.0;

#[derive(Debug, Clone, Copy)]
pub struct KeyGeom {
    /// Left edge, in key units.
    pub x: f32,
    /// Top edge, in key units.
    pub y: f32,
    /// Matrix row reported by the firmware.
    pub row: u8,
    /// Matrix column reported by the firmware.
    pub col: u8,
}

const fn k(x: f32, y: f32, row: u8, col: u8) -> KeyGeom {
    KeyGeom { x, y, row, col }
}

pub const KEYS: [KeyGeom; KEY_COUNT] = [
    // Left half
    k(0.0, 0.5, 0, 1), k(1.0, 0.5, 0, 2), k(2.0, 0.25, 0, 3), k(3.0, 0.0, 0, 4), k(4.0, 0.25, 0, 5), k(5.0, 0.5, 0, 6),
    k(0.0, 1.5, 1, 1), k(1.0, 1.5, 1, 2), k(2.0, 1.25, 1, 3), k(3.0, 1.0, 1, 4), k(4.0, 1.25, 1, 5), k(5.0, 1.5, 1, 6),
    k(0.0, 2.5, 2, 1), k(1.0, 2.5, 2, 2), k(2.0, 2.25, 2, 3), k(3.0, 2.0, 2, 4), k(4.0, 2.25, 2, 5), k(5.0, 2.5, 2, 6),
    k(0.0, 3.5, 3, 1), k(1.0, 3.5, 3, 2), k(2.0, 3.25, 3, 3), k(3.0, 3.0, 3, 4), k(4.0, 3.25, 3, 5), k(5.0, 3.5, 4, 4),
    // Left thumbs
    k(5.0, 4.5, 5, 0), k(6.0, 4.75, 5, 1),
    // Right half
    k(10.0, 0.5, 6, 0), k(11.0, 0.25, 6, 1), k(12.0, 0.0, 6, 2), k(13.0, 0.25, 6, 3), k(14.0, 0.5, 6, 4), k(15.0, 0.5, 6, 5),
    k(10.0, 1.5, 7, 0), k(11.0, 1.25, 7, 1), k(12.0, 1.0, 7, 2), k(13.0, 1.25, 7, 3), k(14.0, 1.5, 7, 4), k(15.0, 1.5, 7, 5),
    k(10.0, 2.5, 8, 0), k(11.0, 2.25, 8, 1), k(12.0, 2.0, 8, 2), k(13.0, 2.25, 8, 3), k(14.0, 2.5, 8, 4), k(15.0, 2.5, 8, 5),
    k(10.0, 3.5, 10, 2), k(11.0, 3.25, 9, 1), k(12.0, 3.0, 9, 2), k(13.0, 3.25, 9, 3), k(14.0, 3.5, 9, 4), k(15.0, 3.5, 9, 5),
    // Right thumbs
    k(9.0, 4.75, 11, 5), k(10.0, 4.5, 11, 6),
];

/// Map a firmware-reported matrix position to a key index.
pub fn key_at_matrix(row: u8, col: u8) -> Option<usize> {
    KEYS.iter().position(|g| g.row == row && g.col == col)
}

/// Key index to the left/right/up/down of `from`, for keyboard navigation in the editor.
pub fn neighbor(from: usize, dx: i32, dy: i32) -> usize {
    let a = KEYS[from];
    let (ax, ay) = (a.x + 0.5, a.y + 0.5);
    KEYS.iter()
        .enumerate()
        .filter(|(i, _)| *i != from)
        .filter_map(|(i, b)| {
            let (bx, by) = (b.x + 0.5 - ax, b.y + 0.5 - ay);
            // Distance along the requested direction must be positive.
            let along = bx * dx as f32 + by * dy as f32;
            if along <= 0.2 {
                return None;
            }
            let across = (bx * dy as f32).abs() + (by * dx as f32).abs();
            Some((i, along + across * 2.0))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
        .unwrap_or(from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_positions_are_unique() {
        for (i, g) in KEYS.iter().enumerate() {
            assert_eq!(key_at_matrix(g.row, g.col), Some(i));
        }
    }

    #[test]
    fn neighbors_move_sensibly() {
        assert_eq!(neighbor(13, 1, 0), 14);
        assert_eq!(neighbor(14, 0, 1), 20);
        assert_eq!(neighbor(14, 0, -1), 8);
    }
}
