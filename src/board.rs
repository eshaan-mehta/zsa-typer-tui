//! Draws the Voyager at its true shape: column stagger, split halves, and thumb keys.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::geometry::{BOARD_UNITS_W, KEYS, KEY_COUNT};
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Flash {
    Correct,
    Mistake,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HintRole {
    /// The key that types the next character.
    Target,
    /// A key to hold for it (shift or a layer key).
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Marker {
    /// Differs from the previously confirmed revision.
    Changed,
    /// The user changed this key by hand.
    Edited,
    /// Mistyped during the last test.
    Missed,
}

#[derive(Debug, Clone, Default)]
pub struct KeyVisual {
    pub label: String,
    pub hold: Option<String>,
    /// Falls through from a lower layer, or empty.
    pub dim: bool,
    pub pressed: bool,
    pub flash: Option<Flash>,
    pub hint: Option<HintRole>,
    pub selected: bool,
    pub marker: Option<Marker>,
    /// The key also has a double-tap or tap-then-hold action; shown as a small dot.
    pub more: bool,
}

/// Key size in terminal cells. Keys in a column share their horizontal borders, so each
/// row adds 2 lines; the column stagger (in quarter-key steps) is one line per step.
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    pub key_w: u16,
}

/// A lone key is 3 lines: top border, label, bottom border.
const KEY_H: u16 = 3;
const ROW_H: u16 = 2;
/// Key widths to try, widest first. 5 is the narrowest that fits a 3-letter label ("Esc").
const KEY_WIDTHS: [u16; 3] = [7, 6, 5];

impl Scale {
    pub fn width(self) -> u16 {
        (BOARD_UNITS_W * self.key_w as f32) as u16
    }

    pub fn height(self) -> u16 {
        KEYS.iter().map(|g| self.y(g.y)).max().unwrap_or(0) + KEY_H
    }

    /// Line of a key's top border, relative to the board.
    fn y(self, units: f32) -> u16 {
        let row = units.floor();
        let stagger_steps = ((units - row) * 4.0).round();
        // Thumb keys get their own top border instead of sharing the column's bottom one.
        let thumb_gap = u16::from(row >= THUMB_ROW);
        row as u16 * ROW_H + stagger_steps as u16 + thumb_gap
    }

    /// Widest keys that fit, if any.
    pub fn fit(width: u16, height: u16) -> Option<Scale> {
        KEY_WIDTHS
            .into_iter()
            .map(|key_w| Scale { key_w })
            .find(|s| s.width() <= width && s.height() <= height)
    }
}

/// Thumb keys are on key-unit row 4; they're drawn as separate boxes, never stacked.
const THUMB_ROW: f32 = 4.0;

/// Whether another key sits directly above (`dir` = -1) or below (+1) key `i` in its column.
fn stacked(i: usize, dir: f32) -> bool {
    let g = KEYS[i];
    let row = g.y.floor();
    row < THUMB_ROW
        && KEYS.iter().any(|o| o.x == g.x && o.y.floor() == row + dir && o.y.floor() < THUMB_ROW)
}

fn is_box_char(ch: char) -> bool {
    matches!(ch, '─' | '│' | '╭' | '╮' | '╰' | '╯' | '├' | '┤')
}

/// How a key is colored.
struct Look {
    /// Outline color, when the key is highlighted.
    line: Option<Color>,
    /// Style of the label row's inside (may carry a background fill).
    label: Style,
}

fn look(k: &KeyVisual) -> Look {
    let fill = |bg: Color, fg: Color| Look {
        line: Some(bg),
        label: Style::new().bg(bg).fg(fg).add_modifier(Modifier::BOLD),
    };
    let outline = |c: Color, label: Style| Look { line: Some(c), label };
    if let Some(f) = k.flash {
        return match f {
            Flash::Correct => fill(theme::CORRECT, theme::ON_FILL),
            Flash::Mistake => fill(theme::MISTAKE, theme::ON_FILL),
        };
    }
    if k.pressed {
        return fill(theme::PRESSED, theme::ON_PRESSED);
    }
    if k.selected {
        return fill(theme::ACCENT, theme::ON_FILL);
    }
    match k.hint {
        Some(HintRole::Target) => return outline(theme::ACCENT, Style::new().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Some(HintRole::Hold) => return outline(theme::HOLD, Style::new().fg(theme::HOLD).add_modifier(Modifier::BOLD)),
        None => {}
    }
    let label = if k.dim { Style::new().fg(Color::DarkGray) } else { Style::new().fg(Color::Reset) };
    let line = match k.marker {
        Some(Marker::Changed) => Some(theme::CHANGED),
        Some(Marker::Edited) => Some(theme::EDITED),
        Some(Marker::Missed) => Some(theme::MISTAKE),
        None => None,
    };
    Look { line, label }
}

pub struct Board<'a> {
    pub keys: &'a [KeyVisual; KEY_COUNT],
    pub scale: Scale,
}

impl Widget for Board<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let s = self.scale;
        let ox = area.x + area.width.saturating_sub(s.width()) / 2;
        let inner = (s.key_w - 2) as usize;
        let right = s.key_w - 1;
        let origin = |i: usize| (ox + (KEYS[i].x * s.key_w as f32) as u16, area.y + s.y(KEYS[i].y));
        let fits = |(x, y): (u16, u16)| x + s.key_w <= area.right() && y + KEY_H <= area.bottom();
        let base = Style::new().fg(Color::DarkGray);
        let hold_text = base.add_modifier(Modifier::DIM);

        // Draw `text` at (x, y): box characters in `border`, other characters in `text_style`.
        let put = |buf: &mut Buffer, x: u16, y: u16, text: &str, border: Style, text_style: Style| {
            for (dx, ch) in text.chars().enumerate() {
                let style = if is_box_char(ch) { border } else { text_style };
                buf[(x + dx as u16, y)].set_char(ch).set_style(style);
            }
        };

        // Outlines in gray. Each key draws its label row and bottom border; only keys with
        // nothing above draw a top border (otherwise it's the bottom border of the key above).
        for (i, key) in self.keys.iter().enumerate() {
            let (x, y) = origin(i);
            if !fits((x, y)) {
                continue;
            }
            if !stacked(i, -1.0) {
                put(buf, x, y, &format!("╭{}╮", "─".repeat(inner)), base, base);
            }
            put(buf, x, y + 1, &format!("│{}│", center(&key.label, inner, ' ')), base, look(key).label);
            if key.more {
                let cell = &mut buf[(x + inner as u16, y + 1)];
                if cell.symbol() == " " {
                    cell.set_char('•').set_fg(Color::DarkGray);
                }
            }
            let (l, r) = if stacked(i, 1.0) { ('├', '┤') } else { ('╰', '╯') };
            let hold = key.hold.as_deref().unwrap_or("");
            put(buf, x, y + 2, &format!("{l}{}{r}", center(hold, inner, '─')), base, hold_text);
        }

        // Highlighted keys recolor only what is theirs: their side bars and the horizontal
        // runs of their top/bottom borders. Shared junctions (├ ┤) stay gray, because their
        // vertical stroke runs into the neighbouring key. Fills never touch border rows.
        for (i, key) in self.keys.iter().enumerate() {
            let (x, y) = origin(i);
            let Some(color) = look(key).line else { continue };
            if !fits((x, y)) {
                continue;
            }
            let line = Style::new().fg(color);
            for row in [y, y + 2] {
                for dx in 0..s.key_w {
                    let cell = &mut buf[(x + dx, row)];
                    let ch = cell.symbol().chars().next().unwrap_or(' ');
                    if (dx == 0 || dx == right) && matches!(ch, '├' | '┤') {
                        continue;
                    }
                    cell.set_style(if is_box_char(ch) { line } else { line.add_modifier(Modifier::DIM) });
                }
            }
            buf[(x, y + 1)].set_style(line);
            buf[(x + right, y + 1)].set_style(line);
        }
    }
}

/// Center `text` in `width` columns, truncating if needed.
fn center(text: &str, width: usize, fill: char) -> String {
    let chars: Vec<char> = text.chars().take(width).collect();
    let pad = width - chars.len();
    let left = pad / 2;
    let mut s: String = std::iter::repeat_n(fill, left).collect();
    s.extend(chars);
    s.extend(std::iter::repeat_n(fill, pad - left));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales() {
        assert_eq!(Scale { key_w: 7 }.width(), 112);
        assert_eq!(Scale { key_w: 7 }.height(), 15);
        assert_eq!(Scale::fit(120, 30).map(|s| s.key_w), Some(7));
        assert_eq!(Scale::fit(100, 30).map(|s| s.key_w), Some(6));
        assert_eq!(Scale::fit(80, 15).map(|s| s.key_w), Some(5));
        assert!(Scale::fit(79, 30).is_none());
        assert!(Scale::fit(120, 14).is_none());
    }

    #[test]
    fn keys_in_a_column_share_borders() {
        let s = Scale { key_w: 7 };
        for (a, b) in KEYS.iter().zip(KEYS.iter().skip(6)).take(18) {
            // Same column, next row down (left half rows are 6 keys apart).
            assert_eq!(s.y(b.y), s.y(a.y) + ROW_H);
        }
        // Thumb keys are separate boxes: the outer one starts right under its column's bottom
        // border (not sharing it), and the inner one sits a step lower.
        for t in [24, 25, 50, 51] {
            assert!(!stacked(t, -1.0) && !stacked(t, 1.0));
        }
        assert!(!stacked(23, 1.0) && !stacked(44, 1.0));
        assert_eq!(s.y(KEYS[24].y), s.y(KEYS[23].y) + KEY_H);
        assert_eq!(s.y(KEYS[25].y), s.y(KEYS[24].y) + 1);
    }

    #[test]
    fn highlights_stay_inside_their_key() {
        let mut keys: [KeyVisual; KEY_COUNT] = std::array::from_fn(|_| KeyVisual { label: "x".into(), ..Default::default() });
        // N: home row, left pinky column, with keys above (B) and below (X).
        keys[13].flash = Some(Flash::Mistake);
        let scale = Scale { key_w: 7 };
        let area = Rect::new(0, 0, scale.width(), scale.height());
        let mut buf = Buffer::empty(area);
        Board { keys: &keys, scale }.render(area, &mut buf);

        let x = (KEYS[13].x * 7.0) as u16;
        let y = scale.y(KEYS[13].y);
        let gray = Color::DarkGray;
        for row in [y, y + 2] {
            // Shared corners keep the neutral color.
            assert_eq!(buf[(x, row)].symbol(), "├");
            assert_eq!(buf[(x, row)].fg, gray);
            assert_eq!(buf[(x + 6, row)].fg, gray);
            // The border runs between them are the key's color, with no background fill.
            for dx in 1..6 {
                assert_eq!(buf[(x + dx, row)].fg, theme::MISTAKE);
                assert_eq!(buf[(x + dx, row)].bg, Color::Reset);
            }
        }
        // The fill covers only the inside of the label row.
        for dx in 1..6 {
            assert_eq!(buf[(x + dx, y + 1)].bg, theme::MISTAKE);
        }
        assert_eq!(buf[(x, y + 1)].bg, Color::Reset);
        // The keys above and below are untouched.
        assert_eq!(buf[(x, y - 1)].fg, gray);
        assert_eq!(buf[(x, y + 3)].fg, gray);
    }

    #[test]
    fn centering() {
        assert_eq!(center("ab", 4, ' '), " ab ");
        assert_eq!(center("Home", 3, ' '), "Hom");
        assert_eq!(center("L2", 5, '─'), "─L2──");
    }
}
