//! Builds practice text from a restricted set of letters.
//!
//! Real words are used when enough of them can be spelled with the allowed letters.
//! Otherwise the gaps are filled with pronounceable pseudo-words from a letter model
//! trained on English words ("tesh", "rean", ...), like keybr does.

use std::collections::{BTreeSet, HashMap};

use rand::{Rng, RngExt};

use crate::words;

const START: char = '^';
const END: char = '$';
/// With at least this many real words available, text is all real words.
const REAL_WORDS_FOR_FULL_TEXT: f64 = 40.0;
const MIN_LEN: usize = 2;
const MAX_LEN: usize = 7;

/// Letter-transition counts: what follows a pair of letters, and what follows one letter.
pub struct LetterModel {
    trigrams: HashMap<(char, char), Vec<(char, u32)>>,
    bigrams: HashMap<char, Vec<(char, u32)>>,
}

impl LetterModel {
    pub fn train<'a>(corpus: impl IntoIterator<Item = &'a str>) -> Self {
        let mut tri: HashMap<(char, char), HashMap<char, u32>> = HashMap::new();
        let mut bi: HashMap<char, HashMap<char, u32>> = HashMap::new();
        for w in corpus {
            let chars: Vec<char> = [START, START].into_iter().chain(w.chars()).chain([END]).collect();
            for win in chars.windows(3) {
                *tri.entry((win[0], win[1])).or_default().entry(win[2]).or_default() += 1;
                *bi.entry(win[1]).or_default().entry(win[2]).or_default() += 1;
            }
        }
        let flatten = |m: HashMap<char, u32>| {
            let mut v: Vec<(char, u32)> = m.into_iter().collect();
            v.sort(); // deterministic order for a given corpus
            v
        };
        LetterModel {
            trigrams: tri.into_iter().map(|(k, v)| (k, flatten(v))).collect(),
            bigrams: bi.into_iter().map(|(k, v)| (k, flatten(v))).collect(),
        }
    }

    /// A pronounceable pseudo-word using only `allowed` letters, favoring letters by `weight`.
    pub fn pseudo_word(&self, rng: &mut impl Rng, allowed: &BTreeSet<char>, weight: &dyn Fn(char) -> f64) -> Option<String> {
        let mut word = String::new();
        let (mut a, mut b) = (START, START);
        loop {
            let len = word.len();
            let usable = |c: char| {
                if c == END {
                    len >= MIN_LEN
                } else {
                    len < MAX_LEN && allowed.contains(&c)
                }
            };
            let pick = |options: Option<&Vec<(char, u32)>>| -> Vec<(char, f64)> {
                options
                    .into_iter()
                    .flatten()
                    .filter(|(c, _)| usable(*c))
                    .map(|(c, n)| (*c, *n as f64 * if *c == END { 1.0 } else { weight(*c) }))
                    .collect()
            };
            let mut options = pick(self.trigrams.get(&(a, b)));
            if options.is_empty() {
                options = pick(self.bigrams.get(&b));
            }
            let next = weighted(rng, &options)?;
            if next == END {
                return Some(word);
            }
            word.push(next);
            (a, b) = (b, next);
        }
    }
}

/// Pick from (item, weight) pairs.
fn weighted<T: Copy>(rng: &mut impl Rng, options: &[(T, f64)]) -> Option<T> {
    let total: f64 = options.iter().map(|(_, w)| w).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = rng.random_range(0.0..total);
    for (item, w) in options {
        if x < *w {
            return Some(*item);
        }
        x -= w;
    }
    options.last().map(|(t, _)| *t)
}

pub struct Generator {
    /// Words for words mode, as written.
    plain: Vec<String>,
    /// Words the letter-filtered modes pick from.
    corpus: Vec<String>,
    model: LetterModel,
}

impl Generator {
    /// Uses `custom` words if given, otherwise the built-in lists.
    pub fn new(custom: Option<&[String]>) -> Self {
        let builtin: Vec<String> = words::corpus().into_iter().map(String::from).collect();
        let (plain, corpus) = match custom {
            Some(words) => (words.to_vec(), words.to_vec()),
            None => (words::ENGLISH_200.iter().map(|w| w.to_string()).collect(), builtin.clone()),
        };
        // Letter patterns come from the built-in words plus any plain lowercase custom words,
        // so pseudo-words stay pronounceable even with a small or unusual list.
        let lowercase_custom = corpus.iter().filter(|w| w.chars().all(|c| c.is_ascii_lowercase()));
        let model = LetterModel::train(builtin.iter().chain(lowercase_custom).map(String::as_str));
        Generator { plain, corpus, model }
    }

    /// `count` random words for words mode.
    pub fn random_words(&self, rng: &mut impl Rng, count: usize) -> String {
        (0..count).map(|_| self.plain[rng.random_range(0..self.plain.len())].as_str()).collect::<Vec<_>>().join(" ")
    }

    /// Real words spellable with `allowed`.
    pub fn real_words(&self, allowed: &BTreeSet<char>) -> Vec<&str> {
        self.corpus
            .iter()
            .map(String::as_str)
            .filter(|w| w.len() >= MIN_LEN && w.chars().all(|c| allowed.contains(&c)))
            .collect()
    }

    /// `count` words using only `allowed` letters. Letters with a higher `weight` show up more,
    /// and about half the words contain `focus`.
    pub fn text(
        &self,
        rng: &mut impl Rng,
        allowed: &BTreeSet<char>,
        weight: &dyn Fn(char) -> f64,
        focus: Option<char>,
        count: usize,
    ) -> String {
        let real = self.real_words(allowed);
        let real_share = (real.len() as f64 / REAL_WORDS_FOR_FULL_TEXT).min(1.0);
        // Weight real words by their letters, sharpened so weak letters really stand out.
        let real_weighted: Vec<(&str, f64)> = real
            .iter()
            .map(|w| {
                let mean = w.chars().map(weight).sum::<f64>() / w.len() as f64;
                (*w, mean.powi(3))
            })
            .collect();

        let mut out: Vec<String> = Vec::with_capacity(count);
        for _ in 0..count {
            let want_focus = focus.is_some() && rng.random_bool(0.5);
            let mut chosen = None;
            for _ in 0..40 {
                let candidate = if !real.is_empty() && rng.random_bool(real_share) {
                    weighted(rng, &real_weighted).map(String::from)
                } else {
                    self.model.pseudo_word(rng, allowed, weight)
                };
                let Some(c) = candidate else { continue };
                let repeat = out.last() == Some(&c);
                let has_focus = focus.is_none_or(|f| c.contains(f));
                chosen = Some(c);
                if !repeat && (!want_focus || has_focus) {
                    break;
                }
            }
            match chosen {
                Some(w) => out.push(w),
                // Nothing works with these letters; fall back to single letters.
                None => out.push(allowed.iter().next().map_or("a".into(), |c| c.to_string())),
            }
        }
        out.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn set(s: &str) -> BTreeSet<char> {
        s.chars().collect()
    }

    #[test]
    fn only_allowed_letters() {
        let g = Generator::new(None);
        let mut rng = StdRng::seed_from_u64(1);
        for letters in ["etainshr", "asdfjkl", "asdfjklei"] {
            let allowed = set(letters);
            let text = g.text(&mut rng, &allowed, &|_| 1.0, None, 50);
            assert_eq!(text.split(' ').count(), 50);
            for c in text.chars().filter(|c| *c != ' ') {
                assert!(allowed.contains(&c), "{c} in {text}");
            }
        }
    }

    #[test]
    fn small_sets_use_pseudo_words() {
        let g = Generator::new(None);
        let mut rng = StdRng::seed_from_u64(2);
        let allowed = set("asdfjkl");
        let real = g.real_words(&allowed);
        assert!(real.len() < 10);
        let text = g.text(&mut rng, &allowed, &|_| 1.0, None, 30);
        assert!(text.split(' ').any(|w| !real.contains(&w)), "{text}");
    }

    #[test]
    fn custom_words() {
        let custom: Vec<String> = ["rust", "tusk", "Struct!", "trust"].iter().map(|w| w.to_string()).collect();
        let g = Generator::new(Some(&custom));
        let mut rng = StdRng::seed_from_u64(4);
        // Words mode uses the custom list as written, punctuation and all.
        let words = g.random_words(&mut rng, 30);
        assert!(words.split(' ').all(|w| custom.iter().any(|c| c == w)), "{words}");
        // Letter modes only take custom words that fit the allowed letters.
        assert_eq!(g.real_words(&set("rustk")), vec!["rust", "tusk", "trust"]);
    }

    #[test]
    fn focus_letter_shows_up() {
        let g = Generator::new(None);
        let mut rng = StdRng::seed_from_u64(3);
        let text = g.text(&mut rng, &set("etainshro"), &|_| 1.0, Some('o'), 40);
        let with_o = text.split(' ').filter(|w| w.contains('o')).count();
        assert!(with_o >= 15, "{with_o}: {text}");
    }
}

#[cfg(test)]
mod samples {
    use super::*;

    /// `cargo test print_samples -- --ignored --nocapture` to eyeball generated text.
    #[test]
    #[ignore]
    fn print_samples() {
        let g = Generator::new(None);
        let mut rng = rand::rng();
        for letters in ["etainshr", "etainshro", "etainshrodlcu", "asdfjkl", "asdfjklei"] {
            let allowed: BTreeSet<char> = letters.chars().collect();
            println!("[{letters}] ({} real words)", g.real_words(&allowed).len());
            println!("  {}\n", g.text(&mut rng, &allowed, &|_| 1.0, letters.chars().last(), 20));
        }
    }
}
