//! Colors. These come from the fixed part of the 256-color palette, so they look the same
//! in light and dark terminal themes (themes remap the 16 basic colors, which is how
//! "yellow" ends up mustard on some of them).

use ratatui::style::Color;

/// Next key to press, focus letter, highlighted numbers. #0087ff
pub const ACCENT: Color = Color::Indexed(33);
/// Keys to hold for the next character (shift, layer keys). #af5fff
pub const HOLD: Color = Color::Indexed(135);
/// Correct keypress flash, learned letters. #00af5f
pub const CORRECT: Color = Color::Indexed(35);
/// Mistake flash, mistyped text. #d70000
pub const MISTAKE: Color = Color::Indexed(160);
/// A key being held down. #a8a8a8
pub const PRESSED: Color = Color::Indexed(248);
/// "Your voyager is missing" and other warnings. #d75f00
pub const WARNING: Color = Color::Indexed(166);
/// Keys that changed in a new layout revision. #d75fd7
pub const CHANGED: Color = Color::Indexed(170);
/// Keys the user edited by hand. #00afaf
pub const EDITED: Color = Color::Indexed(37);
/// Text on colored fills.
pub const ON_FILL: Color = Color::Indexed(231);
/// Text on the light gray "pressed" fill.
pub const ON_PRESSED: Color = Color::Indexed(16);
