//! One typing test: the target text, what's been typed, and the stats.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// One typed character, for per-letter stats.
#[derive(Debug, Clone, PartialEq)]
pub struct Keystroke {
    pub expected: char,
    pub correct: bool,
    /// Time since the previous keystroke (None for the first one).
    pub interval: Option<Duration>,
}

pub struct TypingTest {
    pub target: Vec<char>,
    pub input: Vec<char>,
    pub word_count: usize,
    started: Option<Instant>,
    finished: Option<Instant>,
    /// Characters typed (backspaces don't undo these).
    pub keystrokes: u32,
    pub mistakes: u32,
    /// Expected character → times it was mistyped.
    pub missed: HashMap<char, u32>,
    pub keystrokes_log: Vec<Keystroke>,
    last_key_at: Option<Instant>,
    /// Ended early by a mistake in instant death.
    failed: bool,
}

impl TypingTest {
    pub fn from_text(text: &str) -> Self {
        TypingTest {
            target: text.chars().collect(),
            input: Vec::new(),
            word_count: text.split_whitespace().count(),
            started: None,
            finished: None,
            keystrokes: 0,
            mistakes: 0,
            missed: HashMap::new(),
            keystrokes_log: Vec::new(),
            last_key_at: None,
            failed: false,
        }
    }

    /// Same text, fresh attempt.
    pub fn restart(&self) -> Self {
        Self::from_text(&self.target.iter().collect::<String>())
    }

    pub fn is_started(&self) -> bool {
        self.started.is_some()
    }

    pub fn is_finished(&self) -> bool {
        self.finished.is_some()
    }

    /// End the test now, as a failure (instant death).
    pub fn fail(&mut self) {
        self.failed = true;
        self.finished.get_or_insert_with(Instant::now);
    }

    pub fn is_failed(&self) -> bool {
        self.failed
    }

    /// The last character typed and the one that was expected, if it was a mistake.
    pub fn last_mistake(&self) -> Option<(char, char)> {
        let k = self.keystrokes_log.last().filter(|k| !k.correct)?;
        Some((*self.input.last()?, k.expected))
    }

    pub fn in_progress(&self) -> bool {
        self.is_started() && !self.is_finished()
    }

    pub fn next_char(&self) -> Option<char> {
        if self.is_finished() {
            return None;
        }
        self.target.get(self.input.len()).copied()
    }

    /// Returns whether the character was correct, or None if the test is already over.
    pub fn type_char(&mut self, c: char) -> Option<bool> {
        let expected = self.next_char()?;
        let now = Instant::now();
        self.started.get_or_insert(now);
        self.input.push(c);
        self.keystrokes += 1;
        let correct = c == expected;
        let interval = self.last_key_at.map(|t| now - t);
        self.last_key_at = Some(now);
        self.keystrokes_log.push(Keystroke { expected, correct, interval });
        if !correct {
            self.mistakes += 1;
            *self.missed.entry(expected).or_default() += 1;
        }
        if self.input.len() == self.target.len() {
            self.finished = Some(now);
        }
        Some(correct)
    }

    pub fn backspace(&mut self) {
        if !self.is_finished() {
            self.input.pop();
            self.last_key_at = Some(Instant::now());
        }
    }

    /// Delete back to the start of the current (or previous) word.
    pub fn delete_word(&mut self) {
        if self.is_finished() {
            return;
        }
        while self.input.last() == Some(&' ') {
            self.input.pop();
        }
        while self.input.last().is_some_and(|c| *c != ' ') {
            self.input.pop();
        }
        self.last_key_at = Some(Instant::now());
    }

    pub fn elapsed(&self) -> Duration {
        match (self.started, self.finished) {
            (Some(s), Some(f)) => f - s,
            (Some(s), None) => s.elapsed(),
            _ => Duration::ZERO,
        }
    }

    fn per_minute(&self, chars: usize) -> f64 {
        let mins = self.elapsed().as_secs_f64() / 60.0;
        if mins < 1.0 / 600.0 {
            return 0.0;
        }
        chars as f64 / 5.0 / mins
    }

    /// Words per minute, counting only characters that are currently correct.
    pub fn wpm(&self) -> f64 {
        let correct = self.input.iter().zip(&self.target).filter(|(a, b)| a == b).count();
        self.per_minute(correct)
    }

    pub fn raw_wpm(&self) -> f64 {
        self.per_minute(self.input.len())
    }

    pub fn accuracy(&self) -> f64 {
        if self.keystrokes == 0 {
            return 100.0;
        }
        100.0 * (self.keystrokes - self.mistakes) as f64 / self.keystrokes as f64
    }

    /// Words fully typed so far.
    pub fn words_done(&self) -> usize {
        // Finishing the text completes the last word; dying mid-word doesn't.
        self.input.iter().filter(|c| **c == ' ').count() + usize::from(self.is_finished() && !self.failed)
    }

    /// The most-missed characters, worst first.
    pub fn most_missed(&self, n: usize) -> Vec<(char, u32)> {
        let mut v: Vec<(char, u32)> = self.missed.iter().map(|(c, n)| (*c, *n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_stats() {
        let mut t = TypingTest::from_text("ab cd");
        assert_eq!(t.type_char('a'), Some(true));
        assert_eq!(t.type_char('x'), Some(false));
        t.backspace();
        assert_eq!(t.type_char('b'), Some(true));
        assert_eq!(t.type_char(' '), Some(true));
        assert_eq!(t.type_char('c'), Some(true));
        assert!(!t.is_finished());
        assert_eq!(t.type_char('d'), Some(true));
        assert!(t.is_finished());
        assert_eq!(t.type_char('z'), None);
        assert_eq!(t.keystrokes, 6);
        assert_eq!(t.mistakes, 1);
        assert_eq!(t.most_missed(3), vec![('b', 1)]);
        assert_eq!(t.words_done(), 2);
        let log: Vec<(char, bool)> = t.keystrokes_log.iter().map(|k| (k.expected, k.correct)).collect();
        assert_eq!(log, vec![('a', true), ('b', false), ('b', true), (' ', true), ('c', true), ('d', true)]);
        assert!(t.keystrokes_log[0].interval.is_none() && t.keystrokes_log[1].interval.is_some());
    }

    #[test]
    fn failing_ends_the_test() {
        let mut t = TypingTest::from_text("abc");
        t.type_char('a');
        assert_eq!(t.type_char('x'), Some(false));
        assert_eq!(t.last_mistake(), Some(('x', 'b')));
        t.fail();
        assert!(t.is_finished() && t.is_failed());
        assert_eq!(t.type_char('c'), None);
        assert_eq!(t.words_done(), 0);
    }

    #[test]
    fn delete_word() {
        let mut t = TypingTest::from_text("hello there");
        for c in "hello th".chars() {
            t.type_char(c);
        }
        t.delete_word();
        assert_eq!(t.input.iter().collect::<String>(), "hello ");
        t.delete_word();
        assert!(t.input.is_empty());
    }
}
