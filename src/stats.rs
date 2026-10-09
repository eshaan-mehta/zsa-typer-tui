//! Per-letter speed and accuracy, and progressive-mode unlocking.
//!
//! Everything is derived from the user's own layout: which letters start unlocked
//! depends on what sits under their resting fingers, and the unlock order depends on
//! how easy each letter's key is to reach on the Voyager.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::layout::Layout;
use crate::typing::Keystroke;

/// Smoothing for running averages; higher follows recent typing more closely.
const ALPHA: f64 = 0.1;
/// Samples a letter needs before its numbers are trusted.
pub const MIN_SAMPLES: u32 = 10;
/// What a letter needs to count as learned (both set in settings).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Targets {
    pub wpm: f64,
    /// 0..1
    pub accuracy: f64,
}
/// Pauses longer than this aren't counted as typing speed.
const MAX_INTERVAL_MS: f64 = 2000.0;
/// Progressive mode starts with at least this many letters, so text can vary.
const MIN_START_LETTERS: usize = 6;

/// Where the fingers rest: pinky, ring, middle, index on the home row of each half.
pub const RESTING_KEYS: [usize; 8] = [13, 14, 15, 16, 39, 40, 41, 42];

/// Approximate English letter frequency (%), used to decide which letters are most useful.
fn frequency(c: char) -> f64 {
    match c {
        'e' => 12.7, 't' => 9.1, 'a' => 8.2, 'o' => 7.5, 'i' => 7.0, 'n' => 6.7, 's' => 6.3,
        'h' => 6.1, 'r' => 6.0, 'd' => 4.3, 'l' => 4.0, 'c' => 2.8, 'u' => 2.8, 'm' => 2.4,
        'w' => 2.4, 'f' => 2.2, 'g' => 2.0, 'y' => 2.0, 'p' => 1.9, 'b' => 1.5, 'v' => 1.0,
        'k' => 0.8, 'j' => 0.15, 'x' => 0.15, 'q' => 0.1, 'z' => 0.07,
        _ => 0.0,
    }
}

/// How hard a key is to reach; 0 is a resting position.
fn reach_cost(key: usize) -> f64 {
    if matches!(key, 24 | 25 | 50 | 51) {
        return 1.0; // thumbs
    }
    // Columns from the outside in: 0 outer pinky, 1 pinky, 2 ring, 3 middle, 4 index, 5 inner index.
    let (row, col) = if key < 24 { (key / 6, key % 6) } else { ((key - 26) / 6, 5 - (key - 26) % 6) };
    let row_cost = [3.0, 1.0, 0.0, 1.2][row];
    let col_cost = [2.0, 1.0, 0.5, 0.0, 0.0, 0.8][col];
    row_cost + col_cost
}

/// The order letters are learned in on a given layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Progression {
    pub order: Vec<char>,
    /// How many letters of `order` are unlocked from the start.
    pub start_len: usize,
}

impl Progression {
    pub fn for_layout(layout: &Layout) -> Self {
        let mut start = Vec::new();
        let mut rest = Vec::new();
        for c in 'a'..='z' {
            let Some(h) = layout.find_char(c) else { continue };
            if h.layer == 0 && RESTING_KEYS.contains(&h.key) {
                start.push(c);
            } else {
                let layer_cost = if h.layer > 0 { 3.0 } else { 0.0 };
                rest.push((c, frequency(c) / (1.0 + reach_cost(h.key) + layer_cost)));
            }
        }
        start.sort_by(|a, b| frequency(*b).total_cmp(&frequency(*a)));
        rest.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut rest: Vec<char> = rest.into_iter().map(|(c, _)| c).collect();

        // Text needs a vowel, and a handful of letters to be varied.
        let is_vowel = |c: &char| "aeiou".contains(*c);
        if !start.iter().any(is_vowel)
            && let Some(i) = rest.iter().position(is_vowel) {
                start.push(rest.remove(i));
            }
        while start.len() < MIN_START_LETTERS && !rest.is_empty() {
            start.push(rest.remove(0));
        }
        let start_len = start.len();
        start.extend(rest);
        Progression { order: start, start_len }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LetterStats {
    pub samples: u32,
    /// Running average time to type it, in ms since the previous keystroke.
    pub avg_ms: Option<f64>,
    /// Running accuracy, 0..1.
    pub accuracy: Option<f64>,
    /// (layer, key) the letter was on when these stats were collected.
    pub key: Option<(usize, usize)>,
}

impl LetterStats {
    pub fn wpm(&self) -> Option<f64> {
        self.avg_ms.map(|ms| 12_000.0 / ms.max(1.0))
    }

    fn record(&mut self, correct: bool, interval_ms: Option<f64>) {
        self.samples += 1;
        let hit = if correct { 1.0 } else { 0.0 };
        self.accuracy = Some(self.accuracy.map_or(hit, |a| a + ALPHA * (hit - a)));
        if let Some(ms) = interval_ms.filter(|ms| correct && *ms <= MAX_INTERVAL_MS) {
            self.avg_ms = Some(self.avg_ms.map_or(ms, |a| a + ALPHA * (ms - a)));
        }
    }

    pub fn meets(&self, t: Targets) -> bool {
        self.samples >= MIN_SAMPLES
            && self.wpm().is_some_and(|w| w >= t.wpm)
            && self.accuracy.is_some_and(|a| a >= t.accuracy)
    }

    /// 0 (needs work) to 1 (at target), counting low sample counts as unproven.
    pub fn skill(&self, t: Targets) -> f64 {
        let speed = self.wpm().map_or(0.0, |w| (w / t.wpm).min(1.0));
        // Accuracy counts from 15 points under the target up to the target.
        let floor = t.accuracy - 0.15;
        let acc = self.accuracy.map_or(0.0, |a| ((a - floor) / (t.accuracy - floor)).clamp(0.0, 1.0));
        let confidence = (self.samples as f64 / MIN_SAMPLES as f64).min(1.0);
        speed * acc * confidence
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    /// The layout these stats belong to.
    pub layout_id: Option<String>,
    pub letters: BTreeMap<char, LetterStats>,
    /// Letters unlocked in progressive mode, in unlock order. Only ever grows.
    pub unlocked: Vec<char>,
}

impl Stats {
    /// Keep stats honest when the layout changes: a different layout starts over, and a
    /// letter that moved to another key loses its stats (it's a new key to learn).
    pub fn sync_layout(&mut self, layout: &Layout) {
        if self.layout_id.as_deref() != Some(layout.layout_id.as_str()) {
            *self = Stats { layout_id: Some(layout.layout_id.clone()), ..Default::default() };
        }
        for (c, s) in self.letters.iter_mut() {
            let now = layout.find_char(*c).map(|h| (h.layer, h.key));
            if s.key.is_some() && s.key != now {
                *s = LetterStats { key: now, ..Default::default() };
            }
        }
    }

    pub fn record(&mut self, keystrokes: &[Keystroke], layout: &Layout) {
        for k in keystrokes {
            if !k.expected.is_ascii_alphabetic() {
                continue;
            }
            let c = k.expected.to_ascii_lowercase();
            let s = self.letters.entry(c).or_default();
            if s.key.is_none() {
                s.key = layout.find_char(c).map(|h| (h.layer, h.key));
            }
            s.record(k.correct, k.interval.map(|d| d.as_secs_f64() * 1000.0));
        }
    }

    pub fn letter(&self, c: char) -> LetterStats {
        self.letters.get(&c).cloned().unwrap_or_default()
    }

    /// Make sure progressive mode has its starting letters.
    pub fn ensure_started(&mut self, p: &Progression) {
        if self.unlocked.is_empty() {
            self.unlocked = p.order[..p.start_len].to_vec();
        }
    }

    /// Unlock the next letter if every unlocked letter is at target. Returns it.
    pub fn maybe_unlock(&mut self, p: &Progression, t: Targets) -> Option<char> {
        if !self.unlocked.iter().all(|c| self.letter(*c).meets(t)) {
            return None;
        }
        let next = *p.order.iter().find(|c| !self.unlocked.contains(c))?;
        self.unlocked.push(next);
        Some(next)
    }

    /// The unlocked letter that most needs practice.
    pub fn focus(&self, t: Targets) -> Option<char> {
        self.unlocked
            .iter()
            .copied()
            .min_by(|a, b| self.letter(*a).skill(t).total_cmp(&self.letter(*b).skill(t)))
    }

    /// The `n` weakest letters with enough data to judge.
    pub fn weakest(&self, n: usize, t: Targets) -> Vec<char> {
        let mut v: Vec<(char, f64)> = self
            .letters
            .iter()
            .filter(|(_, s)| s.samples >= MIN_SAMPLES)
            .map(|(c, s)| (*c, s.skill(t)))
            .filter(|(_, skill)| *skill < 1.0)
            .collect();
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        v.into_iter().take(n).map(|(c, _)| c).collect()
    }

    /// How much a letter needs practice, 0.05..1. Unseen letters count as fairly weak.
    pub fn weakness(&self, c: char, t: Targets) -> f64 {
        let s = self.letter(c);
        if s.samples < MIN_SAMPLES {
            return 0.6;
        }
        (1.0 - s.skill(t)).clamp(0.05, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn gallium() -> Layout {
        let v: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/oryx_layout.json")).unwrap();
        crate::oryx::parse(&v, "nvKL0", "yoAjor").unwrap()
    }

    #[test]
    fn progression_follows_the_layout() {
        let p = Progression::for_layout(&gallium());
        let start: String = p.order[..p.start_len].iter().collect();
        assert_eq!(start, "etainshr");
        assert_eq!(p.order.len(), 26);
        // Top-row middle finger 'o' is the first letter to unlock after the resting keys.
        assert_eq!(p.order[p.start_len], 'o');
    }

    #[test]
    fn qwerty_like_start_gets_a_vowel() {
        // Resting keys hold a s d f j k l ; (the ';' isn't a letter).
        let mut l = gallium();
        for (key, code) in [(13, "A"), (14, "S"), (15, "D"), (16, "F"), (39, "J"), (40, "K"), (41, "L"), (42, "SCLN")] {
            l.layers[0].keys[key] = crate::layout::Key::tap(format!("KC_{code}"));
        }
        // Put the displaced letters somewhere harmless.
        for (key, code) in [(1, "N"), (2, "R"), (3, "T"), (4, "S"), (27, "H"), (28, "E"), (29, "I")] {
            l.layers[0].keys[key] = crate::layout::Key::tap(format!("KC_{code}"));
        }
        let p = Progression::for_layout(&l);
        let start = &p.order[..p.start_len];
        assert!(start.iter().any(|c| "aeiou".contains(*c)));
        assert!(start.len() >= MIN_START_LETTERS);
    }

    #[test]
    fn unlocking_needs_every_letter_at_target() {
        let layout = gallium();
        let p = Progression::for_layout(&layout);
        let mut s = Stats::default();
        s.sync_layout(&layout);
        s.ensure_started(&p);
        let t = Targets { wpm: 30.0, accuracy: 0.95 };
        assert_eq!(s.maybe_unlock(&p, t), None);
        let fast = Keystroke { expected: 'x', correct: true, interval: Some(Duration::from_millis(200)), at: Duration::ZERO };
        let ks: Vec<Keystroke> = s
            .unlocked
            .iter()
            .flat_map(|c| std::iter::repeat_n(Keystroke { expected: *c, ..fast.clone() }, MIN_SAMPLES as usize))
            .collect();
        s.record(&ks, &layout);
        assert_eq!(s.maybe_unlock(&p, t), Some('o'));
        assert_eq!(s.maybe_unlock(&p, t), None, "'o' has no practice yet");
    }

    #[test]
    fn moved_letters_lose_their_stats() {
        let layout = gallium();
        let mut s = Stats::default();
        s.sync_layout(&layout);
        let k = Keystroke { expected: 'n', correct: true, interval: Some(Duration::from_millis(300)), at: Duration::ZERO };
        s.record(&[k.clone(), k], &layout);
        assert_eq!(s.letter('n').samples, 2);
        let moved = layout.clone().with_edits(&[crate::layout::Edit { layer: 0, key: 13, value: crate::layout::Key::tap("KC_Q") }, crate::layout::Edit { layer: 0, key: 20, value: crate::layout::Key::tap("KC_N") }]);
        s.sync_layout(&moved);
        assert_eq!(s.letter('n').samples, 0);
    }
}
