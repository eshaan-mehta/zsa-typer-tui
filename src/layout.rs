//! The keymap: what each of the 52 keys does on each layer.

use serde::{Deserialize, Serialize};

use crate::geometry::KEY_COUNT;
use crate::keycode;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<u8>,
    /// Modifiers sent along with the key: any of "ctrl", "shift", "alt", "gui".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<String>,
}

impl Action {
    pub fn new(code: impl Into<String>) -> Self {
        Action { code: code.into(), layer: None, mods: Vec::new() }
    }

    fn has_mod(&self, m: &str) -> bool {
        self.mods.iter().any(|x| x == m)
    }

    /// Character typed by this action, given whether shift is physically held.
    pub fn typed_char(&self, shift_held: bool) -> Option<char> {
        if self.has_mod("ctrl") || self.has_mod("alt") || self.has_mod("gui") {
            return None;
        }
        keycode::typed_char(&self.code, shift_held || self.has_mod("shift"))
    }

    /// Layer this action activates, if it is a layer key.
    pub fn target_layer(&self) -> Option<usize> {
        match self.code.as_str() {
            "MO" | "LT" | "TG" | "TT" | "OSL" | "TO" | "DF" => self.layer.map(usize::from),
            _ => None,
        }
    }

    pub fn label(&self) -> String {
        if let Some(l) = self.layer {
            let prefix = match self.code.as_str() {
                "MO" | "LT" => "L",
                "TO" => "→L",
                "TG" => "TG",
                "TT" => "TT",
                "OSL" => "OS",
                "DF" => "DF",
                _ => "",
            };
            if !prefix.is_empty() {
                return format!("{prefix}{l}");
            }
        }
        if let Some(ch) = self.typed_char(false) {
            return keycode::label(&keycode::code_for_char(ch).unwrap_or_default());
        }
        let mut s = String::new();
        for (m, sym) in [("ctrl", "^"), ("alt", "⌥"), ("gui", "⌘"), ("shift", "⇧")] {
            if self.has_mod(m) {
                s.push_str(sym);
            }
        }
        s + &keycode::label(&self.code)
    }

    pub fn is_shift(&self) -> bool {
        keycode::is_shift(&self.code)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Key {
    #[serde(default)]
    pub tap: Option<Action>,
    #[serde(default)]
    pub hold: Option<Action>,
    #[serde(default)]
    pub custom_label: Option<String>,
}

impl Key {
    pub fn tap(code: impl Into<String>) -> Self {
        Key { tap: Some(Action::new(code)), ..Default::default() }
    }

    pub fn is_transparent(&self) -> bool {
        self.hold.is_none() && self.tap.as_ref().is_some_and(|t| keycode::is_transparent(&t.code))
    }

    pub fn label(&self) -> String {
        if let Some(l) = &self.custom_label {
            return l.clone();
        }
        self.tap.as_ref().map(Action::label).unwrap_or_default()
    }

    pub fn hold_label(&self) -> Option<String> {
        self.hold.as_ref().map(Action::label).filter(|s| !s.is_empty())
    }

    fn actions(&self) -> impl Iterator<Item = &Action> {
        self.tap.iter().chain(self.hold.iter())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub title: String,
    pub keys: Vec<Key>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub title: String,
    pub layout_id: String,
    pub revision_id: String,
    pub layers: Vec<Layer>,
}

/// A user correction to one key, stored separately from the fetched layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edit {
    pub layer: usize,
    pub key: usize,
    pub value: Key,
}

/// How to type one character on this layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub layer: usize,
    pub key: usize,
    /// Key to hold to reach `layer` (None when `layer` is 0, or no layer key was found).
    pub layer_key: Option<usize>,
    /// Shift key to hold, when the character needs shift.
    pub shift_key: Option<usize>,
    pub needs_shift: bool,
}

impl Layout {
    /// A single empty layer, for entering a layout by hand.
    pub fn empty(layout_id: &str, revision_id: &str) -> Self {
        Layout {
            title: "Manual layout".into(),
            layout_id: layout_id.into(),
            revision_id: revision_id.into(),
            layers: vec![Layer { title: "Main".into(), keys: vec![Key::default(); KEY_COUNT] }],
        }
    }

    pub fn with_edits(mut self, edits: &[Edit]) -> Self {
        for e in edits {
            if let Some(slot) = self.layers.get_mut(e.layer).and_then(|l| l.keys.get_mut(e.key)) {
                *slot = e.value.clone();
            }
        }
        self
    }

    /// The key that actually acts at (layer, idx), falling through transparent keys to the base layer.
    pub fn resolve(&self, layer: usize, idx: usize) -> (usize, &Key) {
        let key = &self.layers[layer].keys[idx];
        if layer > 0 && key.is_transparent() {
            (0, &self.layers[0].keys[idx])
        } else {
            (layer, key)
        }
    }

    pub fn key(&self, layer: usize, idx: usize) -> Option<&Key> {
        self.layers.get(layer)?.keys.get(idx)
    }

    /// A base-layer key that switches to `target`, preferring momentary (hold) keys.
    pub fn layer_key(&self, target: usize) -> Option<usize> {
        let base = &self.layers.first()?.keys;
        let momentary = |a: &Action| a.target_layer() == Some(target) && matches!(a.code.as_str(), "MO" | "LT");
        base.iter()
            .position(|k| k.actions().any(momentary))
            .or_else(|| base.iter().position(|k| k.actions().any(|a| a.target_layer() == Some(target))))
    }

    /// A shift key reachable while `layer` is active.
    pub fn shift_key(&self, layer: usize) -> Option<usize> {
        (0..KEY_COUNT).find(|&i| self.resolve(layer, i).1.actions().any(Action::is_shift))
    }

    /// Where `ch` lives: base layer first, unshifted before shifted.
    pub fn find_char(&self, ch: char) -> Option<Hint> {
        for (layer, l) in self.layers.iter().enumerate() {
            for needs_shift in [false, true] {
                for (key, k) in l.keys.iter().enumerate() {
                    if layer > 0 && k.is_transparent() {
                        continue;
                    }
                    let Some(tap) = &k.tap else { continue };
                    if tap.typed_char(needs_shift) != Some(ch) {
                        continue;
                    }
                    // A shift-modified keycode (e.g. KC_EXLM) types the same thing either way.
                    let needs_shift = needs_shift && tap.typed_char(false) != Some(ch);
                    return Some(Hint {
                        layer,
                        key,
                        layer_key: if layer > 0 { self.layer_key(layer) } else { None },
                        shift_key: if needs_shift { self.shift_key(layer) } else { None },
                        needs_shift,
                    });
                }
            }
        }
        None
    }

    /// Keys whose definition differs between two layouts (for highlighting a new revision).
    pub fn differs(&self, other: &Layout, layer: usize, idx: usize) -> bool {
        self.key(layer, idx) != other.key(layer, idx)
    }

    /// Basic sanity checks on data from outside (Oryx or disk).
    pub fn validate(&self) -> Result<(), String> {
        if self.layers.is_empty() {
            return Err("layout has no layers".into());
        }
        for l in &self.layers {
            if l.keys.len() != KEY_COUNT {
                return Err(format!("layer \"{}\" has {} keys, expected {KEY_COUNT}", l.title, l.keys.len()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Layout {
        let mut base = vec![Key::default(); KEY_COUNT];
        base[13] = Key::tap("KC_N");
        base[19] = Key { tap: Some(Action::new("KC_X")), hold: Some(Action { layer: Some(1), ..Action::new("MO") }), custom_label: None };
        base[50] = Key { tap: Some(Action::new("KC_BSPC")), hold: Some(Action::new("KC_LEFT_SHIFT")), custom_label: None };
        base[51] = Key::tap("KC_SPACE");
        let mut sym = vec![Key::tap("KC_TRANSPARENT"); KEY_COUNT];
        sym[7] = Key::tap("KC_EXLM");
        sym[8] = Key::tap("KC_2");
        Layout {
            title: "t".into(),
            layout_id: "a".into(),
            revision_id: "b".into(),
            layers: vec![Layer { title: "Main".into(), keys: base }, Layer { title: "Sym".into(), keys: sym }],
        }
    }

    #[test]
    fn hints() {
        let l = sample();
        assert_eq!(l.find_char('n').map(|h| (h.layer, h.key, h.needs_shift)), Some((0, 13, false)));
        let cap = l.find_char('N').unwrap();
        assert_eq!((cap.key, cap.needs_shift, cap.shift_key), (13, true, Some(50)));
        let bang = l.find_char('!').unwrap();
        assert_eq!((bang.layer, bang.key, bang.layer_key, bang.needs_shift), (1, 7, Some(19), false));
        // '@' is shift+2 on the symbol layer; shift comes from the base layer through a transparent key.
        let at = l.find_char('@').unwrap();
        assert_eq!((at.layer, at.key, at.shift_key), (1, 8, Some(50)));
        assert!(l.find_char('q').is_none());
    }

    #[test]
    fn edits_override_keys() {
        let l = sample().with_edits(&[Edit { layer: 0, key: 13, value: Key::tap("KC_Q") }]);
        assert_eq!(l.find_char('q').map(|h| h.key), Some(13));
    }
}
