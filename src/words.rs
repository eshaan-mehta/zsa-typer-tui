//! Word lists. `ENGLISH_200` is for the plain word test; `corpus()` adds `MORE` for the
//! letter-filtered modes (progressive, weak keys) and for training the pseudo-word model.

pub const ENGLISH_200: &[&str] = &[
    "the", "be", "of", "and", "a", "to", "in", "he", "have", "it", "that", "for", "they", "i", "with", "as",
    "not", "on", "she", "at", "by", "this", "we", "you", "do", "but", "from", "or", "which", "one", "would",
    "all", "will", "there", "say", "who", "make", "when", "can", "more", "if", "no", "man", "out", "other",
    "so", "what", "time", "up", "go", "about", "than", "into", "could", "state", "only", "new", "year", "some",
    "take", "come", "these", "know", "see", "use", "get", "like", "then", "first", "any", "work", "now", "may",
    "such", "give", "over", "think", "most", "even", "find", "day", "also", "after", "way", "many", "must",
    "look", "before", "great", "back", "through", "long", "where", "much", "should", "well", "people", "down",
    "own", "just", "because", "good", "each", "those", "feel", "seem", "how", "high", "too", "place", "little",
    "world", "very", "still", "nation", "hand", "old", "life", "tell", "write", "become", "here", "show",
    "house", "both", "between", "need", "mean", "call", "develop", "under", "last", "right", "move", "thing",
    "general", "school", "never", "same", "another", "begin", "while", "number", "part", "turn", "real",
    "leave", "might", "want", "point", "form", "off", "child", "few", "small", "since", "against", "ask",
    "late", "home", "interest", "large", "person", "end", "open", "public", "follow", "during", "present",
    "without", "again", "hold", "govern", "around", "possible", "head", "consider", "word", "program",
    "problem", "however", "lead", "system", "set", "order", "eye", "plan", "run", "keep", "face", "fact",
    "group", "play", "stand", "increase", "early", "course", "change", "help", "line",
];

/// More everyday words, so filtered tests still have variety (and every letter has words).
pub const MORE: &[&str] = &[
    "able", "above", "accept", "across", "act", "add", "admit", "afraid", "age", "ago", "agree", "ahead", "air",
    "allow", "almost", "alone", "along", "already", "always", "among", "amount", "angle", "angry", "animal",
    "answer", "anyone", "apart", "apple", "area", "argue", "arm", "army", "art", "aside", "asleep", "attack",
    "avoid", "awake", "away", "baby", "bad", "bag", "ball", "band", "bank", "bar", "base", "basic", "basket",
    "bath", "bear", "beat", "bed", "beer", "began", "behind", "being", "belief", "bell", "below", "belt",
    "bench", "best", "better", "beyond", "big", "bird", "birth", "bit", "bite", "black", "blade", "blank",
    "blind", "block", "blood", "blow", "blue", "board", "boat", "body", "bone", "book", "border", "born",
    "borrow", "boss", "bottle", "bottom", "bowl", "box", "boy", "brain", "branch", "brave", "bread", "break",
    "brick", "bridge", "brief", "bright", "bring", "broad", "broken", "brother", "brown", "brush", "build",
    "burn", "bus", "busy", "buy", "cake", "calm", "camera", "camp", "card", "care", "careful", "carry",
    "case", "cast", "cat", "catch", "cause", "cell", "center", "chair", "chance", "charge", "cheap", "check",
    "cheese", "chest", "chicken", "chief", "choice", "choose", "church", "circle", "city", "claim", "class",
    "clean", "clear", "climb", "clock", "close", "cloud", "club", "coast", "coat", "coffee", "cold", "collect",
    "color", "comfort", "common", "copy", "corner", "correct", "cost", "cotton", "count", "country", "couple",
    "cover", "cow", "crazy", "cream", "create", "crew", "crop", "cross", "crowd", "cry", "cup", "current",
    "cut", "cycle", "damage", "dance", "danger", "dark", "data", "date", "daughter", "dead", "deal", "dear",
    "death", "debate", "decide", "deep", "degree", "deliver", "depend", "desk", "detail", "device", "die",
    "diet", "differ", "dinner", "direct", "dirty", "discover", "dish", "doctor", "dog", "dollar", "door",
    "double", "doubt", "draw", "dream", "dress", "drink", "drive", "drop", "dry", "duck", "dust", "duty",
    "each", "ear", "earth", "east", "easy", "eat", "edge", "effect", "effort", "egg", "eight", "either",
    "else", "empty", "energy", "engine", "enjoy", "enough", "enter", "entire", "equal", "error", "escape",
    "evening", "event", "ever", "every", "exact", "exam", "example", "except", "excite", "exercise", "exist",
    "expect", "expert", "explain", "express", "extra", "fail", "fair", "fall", "false", "family", "famous",
    "far", "farm", "fast", "fat", "father", "fault", "fear", "feed", "feet", "fell", "felt", "field",
    "fight", "figure", "fill", "film", "final", "fine", "finger", "finish", "fire", "firm", "fish", "fit",
    "five", "fix", "flag", "flat", "flight", "floor", "flow", "flower", "fly", "focus", "fold", "food",
    "foot", "force", "forest", "forget", "fork", "forward", "four", "frame", "free", "fresh", "friend",
    "front", "fruit", "full", "fun", "funny", "future", "game", "garden", "gate", "gather", "gave", "gentle",
    "gift", "girl", "glad", "glass", "goal", "gold", "gone", "grab", "grass", "gray", "green", "grew",
    "ground", "grow", "guard", "guess", "guest", "guide", "habit", "hair", "half", "hall", "happen", "happy",
    "hard", "hat", "hate", "heart", "heat", "heavy", "height", "hello", "her", "hero", "hide", "hill",
    "him", "hire", "history", "hit", "hole", "holiday", "hollow", "honest", "hope", "horse", "hospital",
    "hot", "hotel", "hour", "huge", "human", "hungry", "hunt", "hurry", "hurt", "husband", "ice", "idea",
    "image", "imagine", "inch", "include", "income", "indeed", "inside", "instead", "iron", "island", "issue",
    "item", "jacket", "jam", "jar", "jaw", "jazz", "job", "join", "joke", "journey", "joy", "judge", "juice",
    "jump", "jungle", "junior", "just", "keen", "key", "kick", "kid", "kind", "king", "kiss", "kitchen",
    "kite", "knee", "knife", "knock", "knot", "known", "label", "lady", "lake", "lamp", "land", "language",
    "large", "laugh", "law", "lay", "layer", "lazy", "learn", "least", "led", "left", "leg", "lemon",
    "lend", "length", "less", "lesson", "let", "letter", "level", "lie", "lift", "light", "limit", "list",
    "listen", "load", "local", "lock", "lone", "lose", "lost", "lot", "loud", "love", "low", "luck", "lunch",
    "machine", "mad", "main", "major", "map", "mark", "market", "match", "matter", "maybe", "meal", "measure",
    "meat", "meet", "member", "memory", "mention", "metal", "method", "middle", "milk", "million", "mind",
    "minute", "mirror", "miss", "mistake", "mix", "model", "modern", "moment", "money", "month", "moon",
    "morning", "mother", "motion", "mountain", "mouse", "mouth", "movie", "much", "music", "myself", "name",
    "narrow", "nature", "near", "nearly", "neck", "needle", "neither", "nerve", "nest", "net", "next", "nice",
    "night", "nine", "noble", "noise", "none", "noon", "nor", "north", "nose", "note", "nothing", "notice",
    "novel", "nurse", "nut", "object", "ocean", "offer", "office", "often", "oil", "okay", "once", "onion",
    "opinion", "orange", "organ", "outside", "oven", "owner", "page", "pain", "paint", "pair", "paper",
    "parent", "park", "party", "pass", "past", "path", "pay", "peace", "pen", "pencil", "pepper", "perfect",
    "perhaps", "period", "pet", "phone", "photo", "piano", "pick", "picture", "piece", "pilot", "pink",
    "pipe", "pity", "plain", "planet", "plant", "plate", "please", "plenty", "pocket", "poem", "police",
    "polite", "pool", "poor", "popular", "port", "pour", "power", "practice", "praise", "prepare", "press",
    "pretty", "price", "pride", "print", "prize", "proud", "prove", "pull", "pupil", "push", "puzzle",
    "quality", "quarter", "queen", "question", "quick", "quiet", "quite", "quote", "race", "radio", "rain",
    "raise", "range", "rare", "rate", "rather", "reach", "read", "ready", "reason", "record", "red", "relax",
    "remain", "remember", "repeat", "reply", "report", "rest", "result", "return", "rice", "rich", "ride",
    "ring", "rise", "river", "road", "rock", "role", "roof", "room", "root", "rope", "rose", "rough",
    "round", "route", "row", "rule", "rush", "sad", "safe", "sail", "salt", "sand", "save", "scale", "scene",
    "science", "score", "sea", "season", "seat", "second", "secret", "seed", "sell", "send", "sense",
    "serve", "seven", "shade", "shadow", "shake", "shape", "share", "sharp", "sheep", "sheet", "shelf",
    "shell", "shine", "ship", "shirt", "shoe", "shop", "short", "shot", "shout", "shut", "shy", "sick",
    "side", "sight", "sign", "silent", "silver", "simple", "sing", "single", "sister", "sit", "six", "size",
    "skill", "skin", "sky", "sleep", "slide", "slow", "smart", "smell", "smile", "smoke", "smooth", "snake",
    "snow", "soap", "soft", "soil", "soldier", "solid", "solve", "son", "song", "soon", "sorry", "sort",
    "soul", "sound", "soup", "south", "space", "speak", "special", "speed", "spell", "spend", "spirit",
    "split", "sport", "spot", "spread", "spring", "square", "staff", "stage", "stair", "stamp", "star",
    "start", "station", "stay", "steal", "steam", "steel", "step", "stick", "stone", "stop", "store",
    "storm", "story", "straight", "strange", "stream", "street", "strong", "student", "study", "stuff",
    "style", "subject", "sudden", "sugar", "suit", "summer", "sun", "supply", "support", "sure", "surface",
    "surprise", "sweet", "swim", "table", "tail", "talk", "tall", "task", "taste", "tax", "tea", "teach",
    "team", "tear", "teeth", "ten", "tend", "term", "test", "text", "thank", "theory", "thick", "thin",
    "third", "thought", "thread", "three", "throw", "thumb", "ticket", "tidy", "tie", "tiger", "tight",
    "tiny", "tired", "title", "today", "toe", "together", "tomorrow", "tone", "tongue", "tonight", "tool",
    "tooth", "top", "topic", "total", "touch", "tour", "toward", "towel", "tower", "town", "toy", "track",
    "trade", "train", "travel", "tree", "trip", "trouble", "truck", "true", "trust", "truth", "try", "tube",
    "type", "uncle", "unit", "until", "upon", "upper", "urge", "useful", "usual", "valley", "value", "van",
    "vast", "verb", "very", "view", "village", "visit", "voice", "vote", "wait", "wake", "walk", "wall",
    "warm", "warn", "wash", "waste", "watch", "water", "wave", "weak", "wealth", "wear", "weather", "week",
    "weight", "west", "wet", "wheel", "whole", "wide", "wife", "wild", "win", "wind", "window", "wine",
    "wing", "winter", "wire", "wise", "wish", "woman", "wonder", "wood", "wool", "worry", "worth", "wrap",
    "wrong", "yard", "yellow", "yes", "yet", "young", "youth", "zero", "zone", "zoo",
];

/// Largest word file we'll read, so a wrong path (a video, say) fails fast.
const MAX_WORD_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// Read a user's word list: words separated by whitespace (usually one per line).
/// Keeps them as written (case, punctuation) and drops duplicates.
pub fn load_word_file(path: &std::path::Path) -> Result<Vec<String>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("can't open {}: {e}", path.display()))?;
    if meta.is_dir() {
        return Err(format!("{} is a folder, not a file", path.display()));
    }
    if meta.len() > MAX_WORD_FILE_BYTES {
        return Err(format!("{} is too big for a word list (over 5 MB)", path.display()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let mut seen = std::collections::HashSet::new();
    let words: Vec<String> = text.split_whitespace().filter(|w| seen.insert(*w)).map(String::from).collect();
    if words.is_empty() {
        return Err(format!("{} has no words in it", path.display()));
    }
    Ok(words)
}

/// All words, without duplicates.
pub fn corpus() -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    ENGLISH_200.iter().chain(MORE).copied().filter(|w| seen.insert(*w)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_plain_lowercase() {
        for w in ENGLISH_200.iter().chain(MORE) {
            assert!(w.chars().all(|c| c.is_ascii_lowercase()), "{w}");
        }
    }

    #[test]
    fn loads_word_files() {
        let dir = std::env::temp_dir().join(format!("zsa-typer-words-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("words.txt");
        std::fs::write(&file, "alpha\nbeta  gamma\n\nalpha\nDelta!\n").unwrap();
        assert_eq!(load_word_file(&file).unwrap(), vec!["alpha", "beta", "gamma", "Delta!"]);
        std::fs::write(&file, "  \n").unwrap();
        assert!(load_word_file(&file).unwrap_err().contains("no words"));
        assert!(load_word_file(&dir).unwrap_err().contains("folder"));
        assert!(load_word_file(&dir.join("missing.txt")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_letter_has_words() {
        let c = corpus();
        for l in 'a'..='z' {
            assert!(c.iter().filter(|w| w.contains(l)).count() >= 3, "{l}");
        }
    }
}
