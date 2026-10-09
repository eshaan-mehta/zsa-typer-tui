//! QMK keycodes as they appear in Oryx data, interpreted for a host OS set to US QWERTY.

/// Strip the `KC_` prefix and fold common aliases onto one canonical name.
pub fn canonical(code: &str) -> &str {
    let c = code.strip_prefix("KC_").unwrap_or(code);
    match c {
        "GRV" => "GRAVE",
        "ESC" => "ESCAPE",
        "ENT" | "RETURN" => "ENTER",
        "SPC" => "SPACE",
        "BACKSPACE" | "BSPACE" => "BSPC",
        "SEMICOLON" => "SCLN",
        "QUOT" => "QUOTE",
        "COMM" => "COMMA",
        "SLSH" => "SLASH",
        "EQL" => "EQUAL",
        "MINS" => "MINUS",
        "LBRACKET" | "LEFT_BRACKET" => "LBRC",
        "RBRACKET" | "RIGHT_BRACKET" => "RBRC",
        "BACKSLASH" => "BSLS",
        "DELETE" => "DEL",
        "TRNS" | "TRANSPARENT" | "_______" => "TRANSPARENT",
        "LSFT" | "LSHIFT" => "LEFT_SHIFT",
        "RSFT" | "RSHIFT" => "RIGHT_SHIFT",
        "LCTL" | "LCTRL" => "LEFT_CTRL",
        "RCTL" | "RCTRL" => "RIGHT_CTRL",
        "LALT" | "LOPT" => "LEFT_ALT",
        "RALT" | "ROPT" | "ALGR" => "RIGHT_ALT",
        "LGUI" | "LCMD" | "LWIN" => "LEFT_GUI",
        "RGUI" | "RCMD" | "RWIN" => "RIGHT_GUI",
        "PAGE_UP" => "PGUP",
        "PAGE_DOWN" => "PGDN",
        "EXCLAIM" => "EXLM",
        "QUESTION" => "QUES",
        "COLON" => "COLN",
        "UNDERSCORE" => "UNDS",
        "PERCENT" => "PERC",
        "DOLLAR" => "DLR",
        "CIRCUMFLEX" => "CIRC",
        "AMPERSAND" => "AMPR",
        "ASTERISK" => "ASTR",
        "LEFT_PAREN" => "LPRN",
        "RIGHT_PAREN" => "RPRN",
        "LEFT_CURLY_BRACE" => "LCBR",
        "RIGHT_CURLY_BRACE" => "RCBR",
        "LEFT_ANGLE_BRACKET" | "LT" => "LABK",
        "RIGHT_ANGLE_BRACKET" | "GT" => "RABK",
        "DQT" | "DOUBLE_QUOTE" => "DQUO",
        "TILDE" => "TILD",
        other => other,
    }
}

/// Unshifted and shifted characters for keys that type something.
fn base_chars(c: &str) -> Option<(char, char)> {
    if c.len() == 1 {
        let ch = c.chars().next()?;
        if ch.is_ascii_uppercase() {
            return Some((ch.to_ascii_lowercase(), ch));
        }
        let shifted = match ch {
            '1' => '!',
            '2' => '@',
            '3' => '#',
            '4' => '$',
            '5' => '%',
            '6' => '^',
            '7' => '&',
            '8' => '*',
            '9' => '(',
            '0' => ')',
            _ => return None,
        };
        return Some((ch, shifted));
    }
    Some(match c {
        "GRAVE" => ('`', '~'),
        "MINUS" => ('-', '_'),
        "EQUAL" => ('=', '+'),
        "LBRC" => ('[', '{'),
        "RBRC" => (']', '}'),
        "BSLS" => ('\\', '|'),
        "SCLN" => (';', ':'),
        "QUOTE" => ('\'', '"'),
        "COMMA" => (',', '<'),
        "DOT" => ('.', '>'),
        "SLASH" => ('/', '?'),
        "SPACE" => (' ', ' '),
        _ => return None,
    })
}

/// Keycodes that are shorthand for "shift + base key".
fn shifted_alias(c: &str) -> Option<&'static str> {
    Some(match c {
        "EXLM" => "1",
        "AT" => "2",
        "HASH" => "3",
        "DLR" => "4",
        "PERC" => "5",
        "CIRC" => "6",
        "AMPR" => "7",
        "ASTR" => "8",
        "LPRN" => "9",
        "RPRN" => "0",
        "UNDS" => "MINUS",
        "PLUS" => "EQUAL",
        "LCBR" => "LBRC",
        "RCBR" => "RBRC",
        "PIPE" => "BSLS",
        "COLN" => "SCLN",
        "DQUO" => "QUOTE",
        "TILD" => "GRAVE",
        "LABK" => "COMMA",
        "RABK" => "DOT",
        "QUES" => "SLASH",
        _ => return None,
    })
}

/// The character `code` types, given whether shift is held.
pub fn typed_char(code: &str, shift: bool) -> Option<char> {
    let c = canonical(code);
    if let Some(base) = shifted_alias(c) {
        return base_chars(base).map(|(_, s)| s);
    }
    base_chars(c).map(|(u, s)| if shift { s } else { u })
}

/// The keycode that types `ch` (used by the layout editor).
pub fn code_for_char(ch: char) -> Option<String> {
    if ch.is_ascii_alphabetic() {
        return Some(format!("KC_{}", ch.to_ascii_uppercase()));
    }
    const CANDIDATES: &[&str] = &[
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "GRAVE", "MINUS", "EQUAL", "LBRC", "RBRC",
        "BSLS", "SCLN", "QUOTE", "COMMA", "DOT", "SLASH", "SPACE", "EXLM", "AT", "HASH", "DLR",
        "PERC", "CIRC", "AMPR", "ASTR", "LPRN", "RPRN", "UNDS", "PLUS", "LCBR", "RCBR", "PIPE",
        "COLN", "DQUO", "TILD", "LABK", "RABK", "QUES",
    ];
    CANDIDATES
        .iter()
        .find(|c| typed_char(c, false) == Some(ch))
        .map(|c| format!("KC_{c}"))
}

pub fn is_transparent(code: &str) -> bool {
    canonical(code) == "TRANSPARENT"
}

pub fn is_shift(code: &str) -> bool {
    matches!(canonical(code), "LEFT_SHIFT" | "RIGHT_SHIFT")
}

/// Short label (ideally ≤ 4 columns) for drawing on a key.
pub fn label(code: &str) -> String {
    let c = canonical(code);
    if let Some(ch) = typed_char(c, false) {
        return match ch {
            ' ' => "Spc".into(),
            ch if ch.is_ascii_lowercase() => ch.to_ascii_uppercase().to_string(),
            ch => ch.to_string(),
        };
    }
    let s = match c {
        "TRANSPARENT" | "NO" | "XXXXXXX" => "",
        "ESCAPE" => "Esc",
        "TAB" => "Tab",
        "ENTER" => "Ent",
        "BSPC" => "Bsp",
        "DEL" => "Del",
        "CAPS_LOCK" | "CAPS" => "Caps",
        "LEFT_SHIFT" | "RIGHT_SHIFT" => "Sft",
        "LEFT_CTRL" | "RIGHT_CTRL" => "Ctl",
        "LEFT_ALT" | "RIGHT_ALT" => {
            if cfg!(target_os = "macos") {
                "Opt"
            } else {
                "Alt"
            }
        }
        "LEFT_GUI" | "RIGHT_GUI" => {
            if cfg!(target_os = "macos") {
                "Cmd"
            } else {
                "Gui"
            }
        }
        "LEFT" => "←",
        "RIGHT" => "→",
        "UP" => "↑",
        "DOWN" => "↓",
        "HOME" => "Home",
        "END" => "End",
        "PGUP" => "PgUp",
        "PGDN" => "PgDn",
        "INSERT" | "INS" => "Ins",
        "AUDIO_VOL_UP" | "VOLU" => "Vol+",
        "AUDIO_VOL_DOWN" | "VOLD" => "Vol-",
        "AUDIO_MUTE" | "MUTE" => "Mute",
        "MEDIA_PLAY_PAUSE" | "MPLY" => "Play",
        "MEDIA_NEXT_TRACK" | "MNXT" => "Next",
        "MEDIA_PREV_TRACK" | "MPRV" => "Prev",
        "MEDIA_STOP" | "MSTP" => "Stop",
        "QK_BOOT" | "RESET" => "Boot",
        "RGB_TOG" => "LED",
        "RGB_VAI" => "Bri+",
        "RGB_VAD" => "Bri-",
        "RGB_MODE_FORWARD" | "RGB_MOD" => "Mode",
        "RGB_SLD" => "Stat",
        "RGB" => "Col",
        "TOGGLE_LAYER_COLOR" => "LCol",
        "CAPS_WORD" | "CW_TOGG" => "CapW",
        other => {
            // Function keys and anything unknown: show as much of the name as fits.
            return other.chars().take(4).collect();
        }
    };
    s.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chars() {
        assert_eq!(typed_char("KC_B", false), Some('b'));
        assert_eq!(typed_char("KC_B", true), Some('B'));
        assert_eq!(typed_char("KC_EXLM", false), Some('!'));
        assert_eq!(typed_char("KC_COMMA", true), Some('<'));
        assert_eq!(typed_char("KC_ENTER", false), None);
    }

    #[test]
    fn reverse() {
        for ch in "abcXYZ019!@#,./;'[]-=`\\ ~_+{}|:\"<>?".chars() {
            let code = code_for_char(ch).unwrap();
            let shift = ch.is_ascii_uppercase();
            assert_eq!(typed_char(&code, shift), Some(ch), "{ch} -> {code}");
        }
    }

    #[test]
    fn labels() {
        assert_eq!(label("KC_B"), "B");
        assert_eq!(label("KC_SPACE"), "Spc");
        assert_eq!(label("KC_SCLN"), ";");
        assert_eq!(label("KC_F12"), "F12");
        assert_eq!(label("KC_TRANSPARENT"), "");
    }
}
