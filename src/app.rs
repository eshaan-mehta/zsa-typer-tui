//! Application state and input handling.

use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::DefaultTerminal;

use crate::board::Flash;
use crate::device::{self, DeviceEvent};
use crate::generator::Generator;
use crate::geometry::{self, KEY_COUNT};
use crate::keycode;
use crate::layout::{Action, Edit, Key, Layout, Press};
use crate::oryx;
use crate::stats::{Progression, Stats, Targets};
use crate::store::{CursorStyle, LayoutRef, Mode, Store, TARGET_ACCURACY_RANGE, TARGET_WPM_RANGE, WORD_COUNT_RANGE};
use crate::typing::{Second, TypingTest};
use crate::ui;

pub const SETTINGS_ITEMS: [&str; 12] = [
    "Mode",
    "Hints",
    "Cursor",
    "Instant death",
    "Words",
    "Word list",
    "Target speed",
    "Target accuracy",
    "Edit layout",
    "Refetch layout from board",
    "Reset progress",
    "Quit",
];

const FLASH_CORRECT: Duration = Duration::from_millis(70);
const FLASH_MISTAKE: Duration = Duration::from_millis(150);
/// How recent a board key press must be to count as the source of a typed character.
const PRESS_MATCH_WINDOW: Duration = Duration::from_millis(250);
/// After a board press selects a key in the editor, ignore the terminal echo of that press.
const ECHO_SUPPRESS: Duration = Duration::from_millis(150);
/// In settings, a pause this long starts a new type-to-jump search.
const SEARCH_RESET: Duration = Duration::from_millis(1000);

/// Settings that are typed in as numbers: (allowed range, step for ←/→).
pub fn number_field(item: &str) -> Option<(std::ops::RangeInclusive<u32>, u32)> {
    match item {
        "Words" => Some((WORD_COUNT_RANGE, 5)),
        "Target speed" => Some((TARGET_WPM_RANGE, 5)),
        "Target accuracy" => Some((TARGET_ACCURACY_RANGE, 1)),
        _ => None,
    }
}

/// The value after (or before) `current` in `all`, wrapping around.
fn cycle<T: Copy + PartialEq>(all: &[T], current: T, forward: bool) -> T {
    let n = all.len();
    let i = all.iter().position(|v| *v == current).unwrap_or(0);
    all[if forward { (i + 1) % n } else { (i + n - 1) % n }]
}

/// The terminal cursor shape for a cursor setting. Steady, since blinking while typing distracts.
fn cursor_shape(style: CursorStyle) -> SetCursorStyle {
    match style {
        CursorStyle::Line => SetCursorStyle::SteadyBar,
        CursorStyle::Block => SetCursorStyle::SteadyBlock,
        CursorStyle::Underscore => SetCursorStyle::SteadyUnderScore,
    }
}

/// The setting a type-to-jump search lands on: name prefix, then word prefix, then anywhere.
pub fn find_setting(search: &str) -> Option<usize> {
    let names: Vec<String> = SETTINGS_ITEMS.iter().map(|s| s.to_lowercase()).collect();
    names
        .iter()
        .position(|n| n.starts_with(search))
        .or_else(|| names.iter().position(|n| n.split(' ').any(|w| w.starts_with(search))))
        .or_else(|| names.iter().position(|n| n.contains(search)))
}

#[derive(Default)]
pub struct SettingsMenu {
    pub selected: usize,
    /// What's been typed to jump to a setting.
    pub search: String,
    last_typed: Option<Instant>,
    /// A value being typed into the selected setting.
    pub editing: Option<Entry>,
}

/// Text being typed into a setting: a number, or the word list's file path.
pub struct Entry {
    pub text: String,
    /// True until the user types: the first character replaces the current value.
    fresh: bool,
    pub error: Option<String>,
    pub is_path: bool,
}

/// The loaded custom word list, for display.
pub struct WordList {
    pub name: String,
    pub count: usize,
}

/// Turn what the user typed into an absolute path ("~" means the home directory).
fn expand_path(text: &str) -> std::path::PathBuf {
    let text = text.trim();
    let path = match text.strip_prefix("~/").or((text == "~").then_some("")) {
        Some(rest) => dirs::home_dir().unwrap_or_default().join(rest),
        None => std::path::PathBuf::from(text),
    };
    std::fs::canonicalize(&path).unwrap_or(path)
}

/// Load the word list at `path`, describing it for the settings menu.
fn load_words(path: &std::path::Path) -> Result<(Vec<String>, WordList), String> {
    let words = crate::words::load_word_file(path)?;
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let count = words.len();
    Ok((words, WordList { name, count }))
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceState {
    Searching,
    Missing,
    WrongBoard(String),
    Connected { firmware_id: Option<String>, live: bool },
}

impl DeviceState {
    pub fn is_connected(&self) -> bool {
        matches!(self, DeviceState::Connected { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditorPurpose {
    /// First run: confirm the fetched layout.
    Onboarding,
    /// The board has a revision we haven't confirmed yet.
    NewRevision,
    /// Opened from settings.
    Edit,
    /// No layout could be fetched; enter keys by hand.
    Manual,
}

pub struct Editor {
    pub purpose: EditorPurpose,
    /// The layout as fetched, without edits.
    pub base: Layout,
    /// Edits being worked on (saved on confirm).
    pub edits: Vec<Edit>,
    /// The previously confirmed revision, to highlight what changed.
    pub previous: Option<Layout>,
    pub layer: usize,
    pub selected: usize,
    /// Text being typed for the selected key, while editing it.
    pub entry: Option<String>,
    /// Which of the key's actions the entry changes (tab cycles while editing).
    pub slot: Press,
    pub message: Option<String>,
}

impl Editor {
    pub fn current(&self) -> Layout {
        self.base.clone().with_edits(&self.edits)
    }

    pub fn is_edited(&self, layer: usize, key: usize) -> bool {
        self.edits.iter().any(|e| e.layer == layer && e.key == key)
    }

    fn set(&mut self, value: Key) {
        let (layer, key) = (self.layer, self.selected);
        self.edits.retain(|e| !(e.layer == layer && e.key == key));
        if self.base.key(layer, key) != Some(&value) {
            self.edits.push(Edit { layer, key, value });
        }
    }

    /// Change one of the selected key's actions (None clears it).
    fn set_action(&mut self, slot: Press, value: Option<Action>) {
        let mut key = self.current().key(self.layer, self.selected).cloned().unwrap_or_default();
        *key.action_mut(slot) = value;
        self.set(key);
    }

    fn restore(&mut self) {
        let (layer, key) = (self.layer, self.selected);
        self.edits.retain(|e| !(e.layer == layer && e.key == key));
    }
}

/// Which graph the results screen shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChartView {
    Wpm,
    Accuracy,
}

/// The results screen: the test's per-second timeline and where the graph cursor is.
pub struct ResultsView {
    pub chart: ChartView,
    /// Index into `timeline` the cursor is on.
    pub cursor: usize,
    pub timeline: Vec<Second>,
    /// The graph fills the screen (no summary or keyboard).
    pub expanded: bool,
}

impl ResultsView {
    fn new(timeline: Vec<Second>) -> Self {
        let cursor = timeline.len().saturating_sub(1);
        ResultsView { chart: ChartView::Wpm, cursor, timeline, expanded: false }
    }
}

pub enum Screen {
    Typing,
    Results(ResultsView),
    Settings(SettingsMenu),
    Editor(Box<Editor>),
}

pub struct FlashState {
    pub key: usize,
    pub kind: Flash,
    pub until: Instant,
}

pub struct App {
    pub store: Store,
    pub screen: Screen,
    pub test: TypingTest,
    pub device: DeviceState,
    /// The confirmed layout with edits applied.
    pub layout: Option<Layout>,
    pub fetching: bool,
    pub notice: Option<(String, Instant)>,
    pub active_layer: usize,
    pub held: [bool; KEY_COUNT],
    pub flashes: Vec<FlashState>,
    pub stats: Stats,
    /// Letter order for progressive mode, derived from the layout.
    pub progression: Option<Progression>,
    /// Letter unlocked by the test that just finished.
    pub just_unlocked: Option<char>,
    /// The custom word list in use, if any.
    pub word_list: Option<WordList>,
    generator: Generator,
    last_press: Option<(usize, Instant)>,
    suppress_until: Option<Instant>,
    device_rx: Receiver<DeviceEvent>,
    fetch_rx: Option<Receiver<Result<Layout>>>,
    /// An editor waiting for the current test to finish.
    pending_editor: Option<Editor>,
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let store = Store::open();
        let layout = store.confirmed_layout();
        let stats = store.load_stats();
        let mut startup_notice = None;
        let (custom, word_list) = match &store.saved.settings.word_file {
            Some(file) => match load_words(std::path::Path::new(file)) {
                Ok((words, list)) => (Some(words), Some(list)),
                Err(e) => {
                    startup_notice = Some(format!("Couldn't load your word list ({e}). Using built-in words."));
                    (None, None)
                }
            },
            None => (None, None),
        };
        let mut app = App {
            store,
            screen: Screen::Typing,
            test: TypingTest::from_text(""),
            device: DeviceState::Searching,
            layout,
            fetching: false,
            notice: None,
            active_layer: 0,
            held: [false; KEY_COUNT],
            flashes: Vec::new(),
            stats,
            progression: None,
            just_unlocked: None,
            word_list,
            generator: Generator::new(custom.as_deref()),
            last_press: None,
            suppress_until: None,
            device_rx: device::spawn(),
            fetch_rx: None,
            pending_editor: None,
            should_quit: false,
        };
        app.on_layout_changed();
        app.test = app.make_test();
        if let Some(n) = startup_notice {
            app.set_notice(n);
        }
        app
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut shape = None;
        while !self.should_quit {
            self.drain_device();
            self.drain_fetch();
            self.tick();
            // The typing cursor is the terminal's own, so a line cursor can sit between letters.
            let cursor = self.store.saved.settings.cursor;
            if shape != Some(cursor) {
                execute!(terminal.backend_mut(), cursor_shape(cursor))?;
                shape = Some(cursor);
            }
            terminal.draw(|f| ui::draw(f, &self))?;
            if event::poll(Duration::from_millis(16))?
                && let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press {
                        self.on_key(key);
                    }
        }
        Ok(())
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(s, _)| s.as_str())
    }

    fn set_notice(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), Instant::now() + Duration::from_secs(6)));
    }

    fn tick(&mut self) {
        let now = Instant::now();
        self.flashes.retain(|f| f.until > now);
        if self.notice.as_ref().is_some_and(|(_, until)| *until <= now) {
            self.notice = None;
        }
        if !self.test.in_progress() && matches!(self.screen, Screen::Typing | Screen::Results(_))
            && let Some(ed) = self.pending_editor.take() {
                self.screen = Screen::Editor(Box::new(ed));
            }
    }

    // ----- device -----

    fn drain_device(&mut self) {
        loop {
            match self.device_rx.try_recv() {
                Ok(ev) => self.on_device(ev),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.device = DeviceState::Missing;
                    break;
                }
            }
        }
    }

    fn on_device(&mut self, ev: DeviceEvent) {
        match ev {
            DeviceEvent::Missing => self.set_disconnected(DeviceState::Missing),
            DeviceEvent::WrongBoard(name) => self.set_disconnected(DeviceState::WrongBoard(name)),
            DeviceEvent::Connected { firmware_id, live, .. } => {
                self.device = DeviceState::Connected { firmware_id: firmware_id.clone(), live };
                self.active_layer = 0;
                match firmware_id.as_deref().and_then(oryx::parse_firmware_id) {
                    Some((layout_id, revision_id)) => self.ensure_layout(&layout_id, &revision_id),
                    None if self.layout.is_none() => {
                        let mut ed = self.editor(EditorPurpose::Manual, Layout::empty("manual", "manual"));
                        ed.message = Some(
                            "Your board didn't report an Oryx layout, so set your keys by hand.".into(),
                        );
                        self.open_editor(ed);
                    }
                    None => {}
                }
            }
            DeviceEvent::KeyDown(k) => {
                self.held[k] = true;
                self.last_press = Some((k, Instant::now()));
                if let Screen::Editor(ed) = &mut self.screen
                    && ed.entry.is_none() {
                        ed.selected = k;
                        self.suppress_until = Some(Instant::now() + ECHO_SUPPRESS);
                    }
            }
            DeviceEvent::KeyUp(k) => self.held[k] = false,
            DeviceEvent::Layer(l) => self.active_layer = l,
        }
    }

    fn set_disconnected(&mut self, state: DeviceState) {
        self.device = state;
        self.held = [false; KEY_COUNT];
        self.active_layer = 0;
    }

    /// Make sure we have the layout the board says it's running.
    fn ensure_layout(&mut self, layout_id: &str, revision_id: &str) {
        let wanted = LayoutRef { layout_id: layout_id.into(), revision_id: revision_id.into() };
        if self.store.saved.confirmed.as_ref() == Some(&wanted) && self.layout.is_some() {
            return;
        }
        match self.store.cached_layout(layout_id, revision_id) {
            Some(layout) => self.offer_layout(layout),
            None => self.start_fetch(layout_id, revision_id),
        }
    }

    fn start_fetch(&mut self, layout_id: &str, revision_id: &str) {
        if self.fetching {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let (l, r) = (layout_id.to_string(), revision_id.to_string());
        thread::spawn(move || {
            let _ = tx.send(oryx::fetch(&l, &r));
        });
        self.fetch_rx = Some(rx);
        self.fetching = true;
    }

    fn drain_fetch(&mut self) {
        let Some(rx) = &self.fetch_rx else { return };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(anyhow::anyhow!("fetch thread stopped")),
        };
        self.fetch_rx = None;
        self.fetching = false;
        match result {
            Ok(layout) => {
                if let Err(e) = self.store.cache_layout(&layout) {
                    self.set_notice(format!("Couldn't cache layout: {e}"));
                }
                self.offer_layout(layout);
            }
            Err(e) if self.layout.is_some() => {
                self.set_notice(format!("Couldn't fetch your board's layout ({e:#}). Using your saved layout."));
            }
            Err(e) => {
                let (lid, rid) = match &self.device {
                    DeviceState::Connected { firmware_id: Some(id), .. } => {
                        oryx::parse_firmware_id(id).unwrap_or(("manual".into(), "manual".into()))
                    }
                    _ => ("manual".into(), "manual".into()),
                };
                let mut ed = self.editor(EditorPurpose::Manual, Layout::empty(&lid, &rid));
                ed.message = Some(format!("Couldn't fetch your layout from Oryx: {e:#}. Set keys by hand, or retry from settings."));
                self.open_editor(ed);
            }
        }
    }

    /// Show a fetched/cached layout for the user to confirm.
    fn offer_layout(&mut self, layout: Layout) {
        let purpose = if self.store.saved.confirmed.is_some() {
            EditorPurpose::NewRevision
        } else {
            EditorPurpose::Onboarding
        };
        let previous = self
            .store
            .saved
            .confirmed
            .as_ref()
            .and_then(|r| self.store.cached_layout(&r.layout_id, &r.revision_id))
            .filter(|p| p.revision_id != layout.revision_id);
        let mut ed = self.editor(purpose, layout);
        ed.previous = previous;
        self.open_editor(ed);
    }

    fn editor(&self, purpose: EditorPurpose, base: Layout) -> Editor {
        Editor {
            purpose,
            base,
            edits: self.store.saved.edits.clone(),
            previous: None,
            layer: 0,
            selected: 0,
            entry: None,
            slot: Press::Tap,
            message: None,
        }
    }

    fn open_editor(&mut self, ed: Editor) {
        if self.test.in_progress() {
            self.pending_editor = Some(ed);
        } else {
            self.screen = Screen::Editor(Box::new(ed));
        }
    }

    // ----- input -----

    fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if self.suppress_until.take_if(|t| Instant::now() < *t).is_some() {
            return;
        }
        match self.screen {
            Screen::Typing => self.on_typing_key(key),
            Screen::Results(_) => self.on_results_key(key),
            Screen::Settings(_) => self.on_settings_key(key),
            Screen::Editor(_) => self.on_editor_key(key),
        }
    }

    fn new_test(&mut self) {
        self.test = self.make_test();
        self.screen = Screen::Typing;
    }

    fn targets(&self) -> Targets {
        self.store.saved.settings.targets()
    }

    /// Whether the chosen mode can't run because there's no layout yet.
    pub fn mode_needs_layout(&self) -> bool {
        self.store.saved.settings.mode != Mode::Words && self.layout.is_none()
    }

    /// Build the next test for the current mode.
    fn make_test(&mut self) -> TypingTest {
        self.just_unlocked = None;
        let count = self.store.saved.settings.word_count;
        let target = self.targets();
        let mut rng = rand::rng();
        match (self.store.saved.settings.mode, &self.layout, &self.progression) {
            (Mode::Progressive, Some(_), Some(p)) => {
                self.stats.ensure_started(p);
                let allowed: BTreeSet<char> = self.stats.unlocked.iter().copied().collect();
                let focus = self.stats.focus(target);
                let stats = &self.stats;
                let weight = |c: char| {
                    let w = 1.0 + 2.0 * stats.weakness(c, target);
                    if Some(c) == focus { w * 2.0 } else { w }
                };
                TypingTest::from_text(&self.generator.text(&mut rng, &allowed, &weight, focus, count))
            }
            (Mode::WeakKeys, Some(layout), _) => {
                let allowed: BTreeSet<char> = ('a'..='z').filter(|c| layout.find_char(*c).is_some()).collect();
                let focus = self.stats.weakest(1, target).first().copied();
                let stats = &self.stats;
                let weight = |c: char| stats.weakness(c, target);
                TypingTest::from_text(&self.generator.text(&mut rng, &allowed, &weight, focus, count))
            }
            _ => TypingTest::from_text(&self.generator.random_words(&mut rng, count)),
        }
    }

    fn finish_test(&mut self) {
        self.screen = Screen::Results(ResultsView::new(self.test.timeline()));
        let t = self.targets();
        let (Some(layout), Some(p)) = (&self.layout, &self.progression) else { return };
        self.stats.record(&self.test.keystrokes_log, layout);
        self.stats.update_learned(p, t);
        // A failed (instant death) test still counts toward letter stats, but never unlocks.
        if self.store.saved.settings.mode == Mode::Progressive && !self.test.is_failed() {
            self.just_unlocked = self.stats.maybe_unlock(p);
        }
        self.save_stats();
    }

    fn save_stats(&mut self) {
        if let Err(e) = self.store.save_stats(&self.stats) {
            self.set_notice(format!("Couldn't save stats: {e}"));
        }
    }

    /// Recompute everything derived from the layout.
    fn on_layout_changed(&mut self) {
        let Some(layout) = &self.layout else {
            self.progression = None;
            return;
        };
        self.stats.sync_layout(layout);
        let p = Progression::for_layout(layout);
        self.stats.ensure_started(&p);
        self.stats.update_learned(&p, self.store.saved.settings.targets());
        self.progression = Some(p);
        self.save_stats();
    }

    fn on_typing_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => self.screen = Screen::Settings(SettingsMenu::default()),
            KeyCode::Tab => self.new_test(),
            KeyCode::Backspace if ctrl || alt => self.test.delete_word(),
            KeyCode::Char('w') | KeyCode::Char('h') if ctrl => self.test.delete_word(),
            KeyCode::Backspace => self.test.backspace(),
            KeyCode::Char(c) if !ctrl && !alt => {
                if let Some(correct) = self.test.type_char(c) {
                    self.flash_for(c, correct);
                    if !correct && self.store.saved.settings.instant_death {
                        self.test.fail();
                    }
                    if self.test.is_finished() {
                        self.finish_test();
                    }
                }
            }
            _ => {}
        }
    }

    /// Flash the key that produced `c`: the board's most recent press if we have one,
    /// otherwise wherever `c` lives on the layout.
    fn flash_for(&mut self, c: char, correct: bool) {
        let now = Instant::now();
        let from_board = self
            .last_press
            .take()
            .filter(|(_, t)| now.duration_since(*t) <= PRESS_MATCH_WINDOW)
            .map(|(k, _)| k);
        let key = from_board.or_else(|| self.layout.as_ref()?.find_char(c).map(|h| h.key));
        if let Some(key) = key {
            let (kind, dur) = if correct {
                (Flash::Correct, FLASH_CORRECT)
            } else {
                (Flash::Mistake, FLASH_MISTAKE)
            };
            self.flashes.retain(|f| f.key != key);
            self.flashes.push(FlashState { key, kind, until: now + dur });
        }
    }

    fn on_results_key(&mut self, key: KeyEvent) {
        let Screen::Results(view) = &mut self.screen else { return };
        let last = view.timeline.len().saturating_sub(1);
        match key.code {
            // Graph: move the cursor through the test, switch between wpm and accuracy.
            KeyCode::Left | KeyCode::Char('h') => view.cursor = view.cursor.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => view.cursor = (view.cursor + 1).min(last),
            KeyCode::Home => view.cursor = 0,
            KeyCode::End => view.cursor = last,
            KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') | KeyCode::Char('v') => {
                view.chart = match view.chart {
                    ChartView::Wpm => ChartView::Accuracy,
                    ChartView::Accuracy => ChartView::Wpm,
                };
            }
            KeyCode::Char('e') => view.expanded = !view.expanded,
            KeyCode::Tab | KeyCode::Enter => self.new_test(),
            KeyCode::Char('r') => {
                self.test = self.test.restart();
                self.screen = Screen::Typing;
            }
            KeyCode::Esc if view.expanded => view.expanded = false,
            KeyCode::Esc => self.screen = Screen::Settings(SettingsMenu::default()),
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }

    fn on_settings_key(&mut self, key: KeyEvent) {
        let Screen::Settings(menu) = &mut self.screen else { return };
        let item = SETTINGS_ITEMS[menu.selected];
        if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return;
        }

        if let Some(entry) = &mut menu.editing {
            match key.code {
                KeyCode::Esc => menu.editing = None,
                KeyCode::Backspace => {
                    entry.fresh = false;
                    entry.text.pop();
                    entry.error = None;
                }
                KeyCode::Char(c) if c.is_ascii_digit() || entry.is_path => {
                    if entry.fresh {
                        entry.text.clear();
                        entry.fresh = false;
                    }
                    if entry.is_path || entry.text.len() < 3 {
                        entry.text.push(c);
                    }
                    entry.error = None;
                }
                KeyCode::Enter if entry.is_path => {
                    let text = entry.text.clone();
                    if let Err(e) = self.set_word_file(&text) {
                        if let Screen::Settings(SettingsMenu { editing: Some(entry), .. }) = &mut self.screen {
                            entry.error = Some(e);
                        }
                    } else if let Screen::Settings(menu) = &mut self.screen {
                        menu.editing = None;
                    }
                }
                KeyCode::Enter => {
                    let Some((range, _)) = number_field(item) else { return };
                    match entry.text.parse::<u32>() {
                        Ok(v) if range.contains(&v) => {
                            menu.editing = None;
                            self.set_number(item, v);
                        }
                        _ => entry.error = Some(format!("enter a number from {} to {}", range.start(), range.end())),
                    }
                }
                _ => {}
            }
            return;
        }

        let n = SETTINGS_ITEMS.len();
        match key.code {
            KeyCode::Esc if !menu.search.is_empty() => menu.search.clear(),
            KeyCode::Esc => self.screen = Screen::Typing,
            KeyCode::Up | KeyCode::Char('k') => {
                menu.selected = (menu.selected + n - 1) % n;
                menu.search.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                menu.selected = (menu.selected + 1) % n;
                menu.search.clear();
            }
            KeyCode::Enter => {
                menu.search.clear();
                self.activate_setting(item);
            }
            KeyCode::Char(' ') if menu.search.is_empty() => self.activate_setting(item),
            KeyCode::Left => self.adjust_setting(item, false),
            KeyCode::Right => self.adjust_setting(item, true),
            KeyCode::Backspace => {
                menu.search.pop();
                if let Some(i) = find_setting(&menu.search).filter(|_| !menu.search.is_empty()) {
                    menu.selected = i;
                }
            }
            // Digits on a number setting start typing a new value.
            KeyCode::Char(c) if c.is_ascii_digit() && number_field(item).is_some() => {
                menu.search.clear();
                menu.editing = Some(Entry { text: c.to_string(), fresh: false, error: None, is_path: false });
            }
            KeyCode::Char(c) => {
                let now = Instant::now();
                if menu.last_typed.is_some_and(|t| now.duration_since(t) > SEARCH_RESET) {
                    menu.search.clear();
                }
                menu.last_typed = Some(now);
                menu.search.push(c.to_ascii_lowercase());
                if let Some(i) = find_setting(&menu.search) {
                    menu.selected = i;
                }
            }
            _ => {}
        }
    }

    pub fn number_value(&self, item: &str) -> u32 {
        let s = &self.store.saved.settings;
        match item {
            "Words" => s.word_count as u32,
            "Target speed" => s.target_wpm,
            "Target accuracy" => s.target_accuracy,
            _ => 0,
        }
    }

    fn set_number(&mut self, item: &str, v: u32) {
        let s = &mut self.store.saved.settings;
        match item {
            "Words" => s.word_count = v as usize,
            "Target speed" => s.target_wpm = v,
            "Target accuracy" => s.target_accuracy = v,
            _ => return,
        }
        self.persist();
        if item == "Words" {
            self.test = self.make_test();
        }
    }

    /// Load a word list from `text` (a path), or go back to the built-in words if it's empty.
    fn set_word_file(&mut self, text: &str) -> Result<(), String> {
        let (custom, list, path) = if text.trim().is_empty() {
            (None, None, None)
        } else {
            let path = expand_path(text);
            let (words, list) = load_words(&path)?;
            (Some(words), Some(list), Some(path.display().to_string()))
        };
        self.set_notice(match &list {
            Some(l) => format!("Loaded {} words from {}.", l.count, l.name),
            None => "Using the built-in words.".to_string(),
        });
        self.generator = Generator::new(custom.as_deref());
        self.word_list = list;
        self.store.saved.settings.word_file = path;
        self.persist();
        self.test = self.make_test();
        Ok(())
    }

    /// ←/→: step through a setting's values. Actions (reset, quit, ...) only run on Enter.
    fn adjust_setting(&mut self, item: &str, forward: bool) {
        if let Some((range, step)) = number_field(item) {
            let v = self.number_value(item);
            let next = if forward { v.saturating_add(step) } else { v.saturating_sub(step) };
            self.set_number(item, next.clamp(*range.start(), *range.end()));
            return;
        }
        match item {
            "Mode" => {
                let s = &mut self.store.saved.settings;
                s.mode = cycle(&Mode::ALL, s.mode, forward);
                self.persist();
                self.test = self.make_test();
            }
            "Hints" => {
                let s = &mut self.store.saved.settings;
                s.hints = !s.hints;
                self.persist();
            }
            "Cursor" => {
                let s = &mut self.store.saved.settings;
                s.cursor = cycle(&CursorStyle::ALL, s.cursor, forward);
                self.persist();
            }
            "Instant death" => {
                let s = &mut self.store.saved.settings;
                s.instant_death = !s.instant_death;
                self.persist();
            }
            _ => {}
        }
    }

    /// Enter: change a value setting, start typing a number, or run an action.
    fn activate_setting(&mut self, item: &str) {
        if number_field(item).is_some() || item == "Word list" {
            let is_path = item == "Word list";
            let text = if is_path {
                self.store.saved.settings.word_file.clone().unwrap_or_default()
            } else {
                self.number_value(item).to_string()
            };
            if let Screen::Settings(menu) = &mut self.screen {
                menu.editing = Some(Entry { text, fresh: true, error: None, is_path });
            }
            return;
        }
        match item {
            "Mode" | "Hints" | "Cursor" | "Instant death" => self.adjust_setting(item, true),
            "Reset progress" => {
                self.stats = Stats::default();
                self.on_layout_changed();
                self.test = self.make_test();
                self.set_notice("Progress reset.");
            }
            "Edit layout" => {
                let base = self
                    .store
                    .saved
                    .confirmed
                    .as_ref()
                    .and_then(|r| self.store.cached_layout(&r.layout_id, &r.revision_id))
                    .unwrap_or_else(|| Layout::empty("manual", "manual"));
                let purpose = if self.store.saved.confirmed.is_some() { EditorPurpose::Edit } else { EditorPurpose::Manual };
                self.screen = Screen::Editor(Box::new(self.editor(purpose, base)));
            }
            "Refetch layout from board" => match self.device.clone() {
                DeviceState::Connected { firmware_id: Some(id), .. } => match oryx::parse_firmware_id(&id) {
                    Some((l, r)) => {
                        self.start_fetch(&l, &r);
                        self.screen = Screen::Typing;
                        self.set_notice("Fetching your layout from Oryx…");
                    }
                    None => self.set_notice("Your board's firmware isn't an Oryx layout, so there's nothing to fetch."),
                },
                DeviceState::Connected { .. } => self.set_notice("Your board didn't report a layout ID."),
                _ => self.set_notice("Your voyager is missing"),
            },
            "Quit" => self.should_quit = true,
            _ => {}
        }
    }

    fn persist(&mut self) {
        if let Err(e) = self.store.save() {
            self.set_notice(format!("Couldn't save settings: {e}"));
        }
    }

    fn on_editor_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Screen::Editor(ed) = &mut self.screen else { return };

        if let Some(entry) = &mut ed.entry {
            match key.code {
                KeyCode::Esc => ed.entry = None,
                KeyCode::Tab | KeyCode::BackTab => {
                    let n = Press::ALL.len();
                    let i = Press::ALL.iter().position(|p| *p == ed.slot).unwrap_or(0);
                    let next = if key.code == KeyCode::Tab { (i + 1) % n } else { (i + n - 1) % n };
                    ed.slot = Press::ALL[next];
                }
                KeyCode::Enter => {
                    let text = std::mem::take(entry);
                    ed.entry = None;
                    match parse_action(&text) {
                        Some(value) => {
                            ed.set_action(ed.slot, value);
                            ed.message = None;
                        }
                        None if text.is_empty() => {}
                        None => ed.message = Some(format!("Don't know the key \"{text}\".")),
                    }
                }
                KeyCode::Backspace => {
                    entry.pop();
                }
                KeyCode::Char(c) if !ctrl => entry.push(c),
                _ => {}
            }
            return;
        }

        let layers = ed.base.layers.len();
        match key.code {
            KeyCode::Left => ed.selected = geometry::neighbor(ed.selected, -1, 0),
            KeyCode::Right => ed.selected = geometry::neighbor(ed.selected, 1, 0),
            KeyCode::Up => ed.selected = geometry::neighbor(ed.selected, 0, -1),
            KeyCode::Down => ed.selected = geometry::neighbor(ed.selected, 0, 1),
            KeyCode::Tab => ed.layer = (ed.layer + 1) % layers,
            KeyCode::BackTab => ed.layer = (ed.layer + layers - 1) % layers,
            KeyCode::Enter => {
                ed.entry = Some(String::new());
                ed.slot = Press::Tap;
            }
            KeyCode::Delete | KeyCode::Backspace => ed.restore(),
            KeyCode::Char('s') if ctrl => self.confirm_editor(),
            KeyCode::Esc => self.cancel_editor(),
            _ => {}
        }
    }

    fn confirm_editor(&mut self) {
        let Screen::Editor(ed) = std::mem::replace(&mut self.screen, Screen::Typing) else { return };
        if let Err(e) = self.store.cache_layout(&ed.base) {
            self.set_notice(format!("Couldn't save layout: {e}"));
        }
        self.store.saved.confirmed =
            Some(LayoutRef { layout_id: ed.base.layout_id.clone(), revision_id: ed.base.revision_id.clone() });
        self.store.saved.edits = ed.edits.clone();
        self.layout = Some(ed.current());
        self.persist();
        self.on_layout_changed();
        if !self.test.is_started() {
            self.test = self.make_test();
        }
        self.set_notice("Layout saved.");
    }

    fn cancel_editor(&mut self) {
        let Screen::Editor(ed) = std::mem::replace(&mut self.screen, Screen::Typing) else { return };
        match ed.purpose {
            EditorPurpose::NewRevision => self.set_notice("Kept your previous layout. Refetch from settings to review the new one."),
            EditorPurpose::Onboarding | EditorPurpose::Manual if self.layout.is_none() => {
                self.set_notice("No layout saved. Set one up from settings (esc) any time.")
            }
            _ => {}
        }
    }

    // ----- queries for the UI -----

    /// The layer to draw on the typing screen.
    pub fn shown_layer(&self) -> usize {
        let layers = self.layout.as_ref().map_or(1, |l| l.layers.len());
        self.active_layer.min(layers - 1)
    }

    pub fn flash_on(&self, key: usize) -> Option<Flash> {
        self.flashes.iter().find(|f| f.key == key).map(|f| f.kind)
    }

    pub fn hints_on(&self) -> bool {
        self.store.saved.settings.hints
    }
}

/// Parse what the user typed for one of a key's actions in the editor: a single character it
/// should type, a QMK keycode name ("left_shift", "MO 1", ...), or "none" to clear it.
/// Returns None if it can't be understood.
fn parse_action(text: &str) -> Option<Option<Action>> {
    let mut chars = text.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return keycode::code_for_char(c).map(|code| Some(Action::new(code)));
    }
    let t = text.trim().to_ascii_uppercase();
    if t.is_empty() {
        return None;
    }
    if t == "NONE" || t == "NO" {
        return Some(None);
    }
    for prefix in ["MO", "TG", "TO", "OSL", "TT", "DF"] {
        if let Some(rest) = t.strip_prefix(prefix)
            && let Ok(layer) = rest.trim().parse::<u8>()
        {
            return Some(Some(Action { layer: Some(layer), ..Action::new(prefix) }));
        }
    }
    let code = if t.starts_with("KC_") || t.starts_with("QK_") || t.starts_with("RGB") { t } else { format!("KC_{t}") };
    let known = keycode::label(&code);
    // Unknown names fall back to showing the first few letters; accept anything that isn't empty.
    (!known.is_empty() || keycode::is_transparent(&code)).then(|| Some(Action::new(code)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_to_jump() {
        let name = |s: &str| find_setting(s).map(|i| SETTINGS_ITEMS[i]);
        assert_eq!(name("r"), Some("Refetch layout from board"));
        assert_eq!(name("res"), Some("Reset progress"));
        assert_eq!(name("reset"), Some("Reset progress"));
        assert_eq!(name("acc"), Some("Target accuracy"));
        assert_eq!(name("speed"), Some("Target speed"));
        assert_eq!(name("q"), Some("Quit"));
        assert_eq!(name("inst"), Some("Instant death"));
        assert_eq!(name("death"), Some("Instant death"));
        assert_eq!(name("cur"), Some("Cursor"));
        assert_eq!(name("word l"), Some("Word list"));
        assert_eq!(name("list"), Some("Word list"));
        assert_eq!(name("xyz"), None);
        // j/k move the selection, so no setting may need them to be found.
        assert!(SETTINGS_ITEMS.iter().all(|s| !s.to_lowercase().contains(['j', 'k'])));
    }

    #[test]
    fn entries() {
        let code = |s: &str| parse_action(s).map(|a| a.map(|a| a.code));
        assert_eq!(code("left_shift"), Some(Some("KC_LEFT_SHIFT".into())));
        assert_eq!(parse_action("mo 2").flatten().and_then(|a| a.target_layer()), Some(2));
        assert_eq!(parse_action("none"), Some(None));
        assert_eq!(code("trns"), Some(Some("KC_TRNS".into())));
        assert_eq!(parse_action(""), None);
        assert_eq!(code("q"), Some(Some("KC_Q".into())));
        assert_eq!(code(" "), Some(Some("KC_SPACE".into())));
        assert_eq!(code("!"), Some(Some("KC_EXLM".into())));
    }

    #[test]
    fn editing_one_action_keeps_the_others() {
        let mut base = Layout::empty("a", "b");
        base.layers[0].keys[13] = Key::tap("KC_N");
        let mut ed = Editor {
            purpose: EditorPurpose::Edit,
            base,
            edits: Vec::new(),
            previous: None,
            layer: 0,
            selected: 13,
            entry: None,
            slot: Press::Tap,
            message: None,
        };
        ed.set_action(Press::DoubleTap, parse_action("esc").flatten());
        let k = ed.current().key(0, 13).cloned().unwrap();
        assert_eq!(k.tap.map(|a| a.code), Some("KC_N".into()));
        assert_eq!(k.double_tap.map(|a| a.code), Some("KC_ESC".into()));
        ed.set_action(Press::DoubleTap, None);
        assert!(!ed.is_edited(0, 13), "clearing the only change leaves no edit");
    }
}
