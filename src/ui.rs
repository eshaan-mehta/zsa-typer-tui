//! Rendering. Minimal, ttyper-style: dim untyped text, bright correct text, red mistakes.

use std::ops::Range;

use ratatui::layout::{Alignment, Constraint, Layout as Split, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{find_setting, number_field, App, DeviceState, Editor, EditorPurpose, Screen, SettingsMenu, SETTINGS_ITEMS};
use crate::board::{Board, HintRole, KeyVisual, Marker, Scale};
use crate::geometry::KEY_COUNT;
use crate::layout::{Layout, Press};
use crate::stats::MIN_SAMPLES;
use crate::store::Mode;
use crate::theme;

const TEXT_WIDTH: u16 = 72;
const TEXT_LINES: usize = 3;

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub fn draw(f: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Typing => draw_typing(f, app),
        Screen::Results => draw_results(f, app),
        Screen::Settings(menu) => {
            draw_typing(f, app);
            draw_settings(f, app, menu);
        }
        Screen::Editor(ed) => draw_editor(f, app, ed),
    }
}

// ----- shared pieces -----

fn status_line(app: &App) -> Line<'static> {
    match &app.device {
        DeviceState::Searching => Line::styled("looking for your voyager…", dim()),
        DeviceState::Missing => Line::styled("Your voyager is missing", Style::new().fg(theme::WARNING)),
        DeviceState::WrongBoard(name) => Line::styled(
            format!("Your voyager is missing (found a {name}; this app is built for the Voyager)"),
            Style::new().fg(theme::WARNING),
        ),
        DeviceState::Connected { live, .. } => {
            let mut parts = vec!["ZSA Voyager".to_string()];
            match &app.layout {
                Some(l) => {
                    parts.push(l.title.clone());
                    if let Some(layer) = l.layers.get(app.shown_layer()) {
                        parts.push(layer.title.clone());
                    }
                }
                None => parts.push("no layout".into()),
            }
            if app.fetching {
                parts.push("fetching layout…".into());
            }
            if !live {
                parts.push("no live key data".into());
            }
            Line::styled(parts.join(" · "), dim())
        }
    }
}

fn footer(app: &App, help: &str) -> Line<'static> {
    match app.notice() {
        Some(n) => Line::styled(n.to_string(), Style::new().fg(theme::ACCENT)),
        None => Line::styled(help.to_string(), dim()),
    }
}

/// Key visuals for the live board: labels for the active layer, presses, and flashes.
fn live_keys(app: &App, layout: Option<&Layout>, layer: usize) -> [KeyVisual; KEY_COUNT] {
    std::array::from_fn(|i| {
        let (label, hold, dim, more) = match layout {
            Some(l) => {
                let (from, k) = l.resolve(layer, i);
                let label = k.label();
                let dim = from != layer || (label.is_empty() && k.hold.is_none() && !k.has_extra());
                (label, k.hold_label(), dim, k.has_extra())
            }
            None => (String::new(), None, true, false),
        };
        KeyVisual { label, hold, dim, more, pressed: app.held[i], flash: app.flash_on(i), ..Default::default() }
    })
}

fn display_char(c: char) -> String {
    match c {
        ' ' => "space".into(),
        c => c.to_string(),
    }
}

/// Mark the keys for the next character: the key itself, plus any shift or layer key to hold.
fn apply_hint(app: &App, layout: &Layout, layer: usize, keys: &mut [KeyVisual; KEY_COUNT]) {
    let Some(h) = app.test.next_char().and_then(|c| layout.find_char(c)) else { return };
    if h.layer == layer {
        keys[h.key].hint = Some(HintRole::Target);
        // Not a plain tap: say how to press it where the hold label normally goes.
        if h.press != Press::Tap {
            keys[h.key].hold = Some(h.press.short().to_string());
        }
    } else if let (0, Some(lk)) = (layer, h.layer_key) {
        keys[lk].hint = Some(HintRole::Hold);
    }
    if let Some(sk) = h.shift_key
        && keys[sk].hint.is_none()
    {
        keys[sk].hint = Some(HintRole::Hold);
    }
}

/// Split `target` into lines of at most `width` columns, breaking after spaces.
fn wrap(target: &[char], width: usize) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let len = target.len();
    while start < len {
        if len - start <= width {
            lines.push(start..len);
            break;
        }
        let limit = start + width;
        match (start..=limit).rev().find(|&i| target[i] == ' ') {
            Some(i) if i > start => {
                lines.push(start..i + 1);
                start = i + 1;
            }
            _ => {
                lines.push(start..limit);
                start = limit;
            }
        }
    }
    lines
}

fn text_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let t = &app.test;
    let lines = wrap(&t.target, width);
    let cursor = t.input.len();
    let current = lines.iter().position(|r| r.contains(&cursor)).unwrap_or(lines.len().saturating_sub(1));
    let first = current.saturating_sub(1).min(lines.len().saturating_sub(TEXT_LINES));
    lines
        .iter()
        .skip(first)
        .take(TEXT_LINES)
        .map(|r| {
            let spans: Vec<Span> = r
                .clone()
                .map(|i| {
                    let expected = t.target[i];
                    match t.input.get(i) {
                        Some(&typed) if typed == expected => Span::styled(expected.to_string(), Style::new().fg(Color::Reset)),
                        Some(_) => {
                            let shown = if expected == ' ' { '·' } else { expected };
                            Span::styled(shown.to_string(), Style::new().fg(theme::MISTAKE))
                        }
                        None if i == cursor => {
                            Span::styled(expected.to_string(), dim().add_modifier(Modifier::UNDERLINED))
                        }
                        None => Span::styled(expected.to_string(), dim()),
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

/// How far the board sits above an even split of the space under the text.
const BOARD_LIFT: u16 = 6;

/// Space left for the board after the fixed rows on the typing screen.
const TYPING_CHROME: u16 = 1 + 1 + 1 + 1 + 2 + TEXT_LINES as u16 + 1 + 1 + 1 + 1;

/// What the current mode is working on: the letter strip in progressive mode, weak letters otherwise.
fn mode_line(app: &App) -> Line<'static> {
    let s = &app.store.saved.settings;
    let target = s.targets();
    if app.mode_needs_layout() {
        return Line::styled(format!("{} mode needs your layout (connect your Voyager once) · using words", s.mode.name()), dim());
    }
    match s.mode {
        Mode::Words => Line::styled("words", dim()),
        Mode::Progressive => {
            let Some(p) = &app.progression else { return Line::default() };
            let focus = app.stats.focus(target);
            let mut spans = Vec::new();
            for c in &p.order {
                let style = if !app.stats.unlocked.contains(c) {
                    dim()
                } else if app.stats.letter(*c).meets(target) {
                    Style::new().fg(theme::CORRECT)
                } else {
                    Style::new().fg(Color::Reset)
                };
                let style = if Some(*c) == focus { style.fg(theme::ACCENT).bold().underlined() } else { style };
                spans.push(Span::styled(c.to_string(), style));
                spans.push(Span::raw(" "));
            }
            if let Some(c) = focus {
                spans.push(Span::styled(format!("  focus {c}"), dim()));
            }
            Line::from(spans)
        }
        Mode::WeakKeys => {
            let weak = app.stats.weakest(3, target);
            let has_data = app.stats.letters.values().any(|l| l.samples >= MIN_SAMPLES);
            if weak.is_empty() && has_data {
                Line::styled(
                    format!("weak keys · all letters meet your targets ({} wpm, {}%)", s.target_wpm, s.target_accuracy),
                    dim(),
                )
            } else if weak.is_empty() {
                Line::styled("weak keys · not enough data yet, keep typing", dim())
            } else {
                let letters = weak.iter().map(char::to_string).collect::<Vec<_>>().join("  ");
                Line::from(vec![Span::styled("weak keys  ", dim()), Span::styled(letters, Style::new().fg(theme::MISTAKE).bold())])
            }
        }
    }
}

// ----- screens -----

fn draw_typing(f: &mut Frame, app: &App) {
    let area = f.area();
    let show_board = app.device.is_connected();
    let scale = if show_board { Scale::fit(area.width, area.height.saturating_sub(TYPING_CHROME)) } else { None };
    let board_h = scale.map_or(if show_board { 1 } else { 0 }, |s| s.height());
    let text_w = TEXT_WIDTH.min(area.width.saturating_sub(4)).max(10);

    // Spare height: half above the text; the rest goes between the text and the board and
    // below the board, with the board lifted BOARD_LIFT lines off where a 5:3 split would
    // put it. Keeps the test clearly separated without pinning the keyboard to the bottom.
    let fixed = 1 + 1 + 1 + 2 + TEXT_LINES as u16 + 1 + board_h + 1;
    let spare = area.height.saturating_sub(fixed);
    let above_text = spare / 2;
    // (Rounded, and always leaving a line between the text and the board.)
    let below_board = ((spare * 3 + 8) / 16 + BOARD_LIFT).min((spare - above_text).saturating_sub(1));
    let above_board = spare - above_text - below_board;
    let [status, _, mode, stats, _, text, _, caption, board, _, foot] = Split::vertical([
        Constraint::Length(1),
        Constraint::Length(above_text),
        Constraint::Length(1),
        Constraint::Length(1),
        // Gap so the text to type stands on its own below the info lines.
        Constraint::Length(2),
        Constraint::Length(TEXT_LINES as u16),
        Constraint::Length(above_board),
        Constraint::Length(1),
        Constraint::Length(board_h),
        Constraint::Length(below_board),
        Constraint::Length(1),
    ])
    .areas(area);

    f.render_widget(Paragraph::new(status_line(app)).alignment(Alignment::Center), status);

    let t = &app.test;
    let stats_text = if t.is_started() {
        format!("{:.0} wpm   {:.0}%   {}/{}", t.wpm(), t.accuracy(), t.words_done(), t.word_count)
    } else {
        format!("{} words · start typing", t.word_count)
    };
    let text_area = centered(text, text_w + 1);
    // What this mode is working on, shown before a test starts; hidden while typing.
    if !t.is_started() {
        f.render_widget(Paragraph::new(mode_line(app)), centered(mode, text_w + 1));
    }
    f.render_widget(Paragraph::new(Line::styled(stats_text, dim())), centered(stats, text_w + 1));
    f.render_widget(Paragraph::new(text_lines(app, text_w as usize)), text_area);

    if show_board {
        let layer = app.shown_layer();
        let layout = app.layout.as_ref();
        let mut keys = live_keys(app, layout, layer);
        let caption_line = match layout {
            None if app.fetching => Some(Line::styled("fetching your layout…", dim())),
            None => Some(Line::styled("no layout set · esc → Edit layout", dim())),
            Some(l) => {
                if app.hints_on() {
                    apply_hint(app, l, layer, &mut keys);
                }
                None
            }
        };
        if let Some(line) = caption_line {
            f.render_widget(Paragraph::new(line).alignment(Alignment::Center), caption);
        }
        match scale {
            Some(scale) => f.render_widget(Board { keys: &keys, scale }, board),
            None => f.render_widget(
                Paragraph::new(Line::styled("make the terminal bigger to see your Voyager", dim())).alignment(Alignment::Center),
                board,
            ),
        }
    }

    let hints = if app.hints_on() { "on" } else { "off" };
    let settings = &app.store.saved.settings;
    let death = if settings.instant_death { " · instant death" } else { "" };
    let help = format!("{}{death} · tab new test · esc settings · hints {hints}", settings.mode.name());
    f.render_widget(Paragraph::new(footer(app, &help)).alignment(Alignment::Center), foot);
}

fn draw_results(f: &mut Frame, app: &App) {
    let area = f.area();
    let t = &app.test;
    let show_board = app.device.is_connected() && app.layout.is_some();
    let scale = if show_board { Scale::fit(area.width, area.height.saturating_sub(14)) } else { None };

    let [status, _, summary, _, board, _, foot] = Split::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(7),
        Constraint::Length(1),
        Constraint::Length(scale.map_or(0, |s| s.height())),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    f.render_widget(Paragraph::new(status_line(app)).alignment(Alignment::Center), status);

    let missed = t.most_missed(5);
    let missed_line = if let Some((typed, expected)) = t.last_mistake().filter(|_| t.is_failed()) {
        Line::from(vec![
            Span::styled("instant death  ", Style::new().fg(theme::MISTAKE).bold()),
            Span::styled(
                format!("typed {} instead of {}", display_char(typed), display_char(expected)),
                Style::new().fg(theme::MISTAKE),
            ),
        ])
    } else if missed.is_empty() {
        Line::styled("no mistakes 🎉", Style::new().fg(theme::CORRECT))
    } else {
        let list = missed.iter().map(|(c, n)| format!("{} ×{n}", display_char(*c))).collect::<Vec<_>>().join("   ");
        Line::from(vec![Span::styled("missed  ", dim()), Span::styled(list, Style::new().fg(theme::MISTAKE))])
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled(format!("{:.0}", t.wpm()), Style::new().fg(theme::ACCENT).bold()),
            Span::styled(" wpm     ", dim()),
            Span::styled(format!("{:.0}%", t.accuracy()), Style::new().fg(theme::ACCENT).bold()),
            Span::styled(" acc", dim()),
        ]),
        Line::default(),
        Line::styled(
            format!(
                "raw {:.0} · {} words · {:.1}s",
                t.raw_wpm(),
                if t.is_failed() { format!("{}/{}", t.words_done(), t.word_count) } else { t.word_count.to_string() },
                t.elapsed().as_secs_f64()
            ),
            dim(),
        ),
        Line::default(),
        missed_line,
        Line::default(),
    ];
    lines.push(match app.just_unlocked {
        Some(c) => Line::from(vec![
            Span::styled("new letter unlocked: ", Style::new().fg(theme::CORRECT)),
            Span::styled(c.to_string(), Style::new().fg(theme::CORRECT).bold()),
        ]),
        None => mode_line(app),
    });
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), summary);

    if let (Some(scale), Some(layout)) = (scale, app.layout.as_ref()) {
        let layer = app.shown_layer();
        let mut keys = live_keys(app, Some(layout), layer);
        if let Some(h) = app.just_unlocked.and_then(|c| layout.find_char(c)).filter(|h| h.layer == layer) {
            keys[h.key].hint = Some(HintRole::Target);
        }
        for (c, _) in t.most_missed(usize::MAX) {
            if let Some(h) = layout.find_char(c).filter(|h| h.layer == layer) {
                keys[h.key].marker = Some(Marker::Missed);
            }
        }
        f.render_widget(Board { keys: &keys, scale }, board);
    }

    f.render_widget(
        Paragraph::new(footer(app, "tab next test · r retry · esc settings · q quit")).alignment(Alignment::Center),
        foot,
    );
}

fn draw_settings(f: &mut Frame, app: &App, menu: &SettingsMenu) {
    let s = &app.store.saved.settings;
    let w = 70;
    let h = SETTINGS_ITEMS.len() as u16 + 5;
    let area = f.area();
    let popup = Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w.min(area.width),
        height: h.min(area.height),
    };
    f.render_widget(Clear, popup);
    let block = Block::bordered().border_type(BorderType::Rounded).border_style(dim()).title(" settings ");
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let accent = Style::new().fg(theme::ACCENT);

    let mut lines: Vec<Line> = SETTINGS_ITEMS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let selected = i == menu.selected;
            let unit = match *name {
                "Target speed" => " wpm",
                "Target accuracy" => "%",
                _ => "",
            };
            let value = match (*name, &menu.editing) {
                (_, Some(entry)) if selected => format!("{}▏{unit}", entry.text),
                ("Mode", _) => s.mode.name().to_string(),
                ("Hints", _) => if s.hints { "on" } else { "off" }.to_string(),
                ("Instant death", _) => if s.instant_death { "on" } else { "off" }.to_string(),
                ("Words" | "Target speed" | "Target accuracy", _) => format!("{}{unit}", app.number_value(name)),
                _ => String::new(),
            };
            let marker = if selected { "› " } else { "  " };
            let style = if selected { accent } else { Style::new() };
            Line::styled(format!("{marker}{name:<42}{value:>20}"), style)
        })
        .collect();
    lines.push(Line::default());

    // Status line: number entry help/error, or the type-to-jump search.
    let status = match &menu.editing {
        Some(entry) => match &entry.error {
            Some(err) => Line::styled(err.clone(), Style::new().fg(theme::MISTAKE)),
            None => {
                let range = number_field(SETTINGS_ITEMS[menu.selected]).map(|(r, _)| format!("{}–{}", r.start(), r.end()));
                Line::styled(format!("type a number ({}) · enter save · esc cancel", range.unwrap_or_default()), dim())
            }
        },
        None if !menu.search.is_empty() => {
            let found = find_setting(&menu.search).is_some();
            Line::from(vec![
                Span::styled("jump: ", dim()),
                Span::styled(menu.search.clone(), if found { accent } else { Style::new().fg(theme::MISTAKE) }),
                Span::styled(if found { "" } else { "  (no match)" }, dim()),
            ])
        }
        None => Line::default(),
    };
    lines.push(status);
    lines.push(Line::styled("↑↓/jk move · type to jump · enter change · ←→ adjust · esc close", dim()));
    f.render_widget(Paragraph::new(lines), inner.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 }));
}

fn draw_editor(f: &mut Frame, app: &App, ed: &Editor) {
    let area = f.area();
    let current = ed.current();
    let scale = Scale::fit(area.width, area.height.saturating_sub(10));

    let [_, title, sub, msg, _, board, _, info, entry, _, foot] = Split::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(scale.map_or(1, |s| s.height())),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let (heading, cancel) = match ed.purpose {
        EditorPurpose::Onboarding => ("Verify your layout", "skip for now"),
        EditorPurpose::NewRevision => ("Your Voyager has a new layout revision", "keep old layout"),
        EditorPurpose::Edit => ("Edit layout", "discard changes"),
        EditorPurpose::Manual => ("Set up your layout", "skip for now"),
    };
    let center = |line: Line<'static>| Paragraph::new(line).alignment(Alignment::Center);
    f.render_widget(center(Line::styled(heading, Style::new().bold())), title);

    let layer_title = current.layers.get(ed.layer).map(|l| l.title.clone()).unwrap_or_default();
    f.render_widget(
        center(Line::styled(
            format!("{} · layer {}/{}: {}", current.title, ed.layer + 1, current.layers.len(), layer_title),
            dim(),
        )),
        sub,
    );

    let message = ed.message.clone().or_else(|| {
        let mut notes = Vec::new();
        if ed.previous.is_some() {
            notes.push("pink keys changed since your last revision");
        }
        if !ed.edits.is_empty() {
            notes.push("teal keys are your manual edits");
        }
        (!notes.is_empty()).then(|| notes.join(" · "))
    });
    if let Some(m) = message {
        f.render_widget(center(Line::styled(m, Style::new().fg(theme::ACCENT))), msg);
    }

    let mut keys = live_keys(app, Some(&current), ed.layer);
    for (i, k) in keys.iter_mut().enumerate() {
        k.flash = None;
        k.selected = i == ed.selected;
        k.marker = if ed.is_edited(ed.layer, i) {
            Some(Marker::Edited)
        } else if ed.previous.as_ref().is_some_and(|p| ed.base.differs(p, ed.layer, i)) {
            Some(Marker::Changed)
        } else {
            None
        };
    }
    match scale {
        Some(scale) => f.render_widget(Board { keys: &keys, scale }, board),
        None => f.render_widget(center(Line::styled("make the terminal bigger to see your Voyager", dim())), board),
    }

    // Every action on the selected key; while editing, the one being changed is highlighted.
    let sel = current.key(ed.layer, ed.selected).cloned().unwrap_or_default();
    let mut info_spans = vec![Span::styled("selected  ", dim())];
    let mut first = true;
    for press in Press::ALL {
        let editing = ed.entry.is_some() && ed.slot == press;
        let value = sel.action(press).map(|a| a.label()).filter(|l| !l.is_empty());
        // Tap is always listed; the others only when set (or being edited).
        if press != Press::Tap && value.is_none() && !editing {
            continue;
        }
        if !first {
            info_spans.push(Span::styled(" · ", dim()));
        }
        first = false;
        let name_style = if editing { Style::new().fg(theme::ACCENT) } else { dim() };
        info_spans.push(Span::styled(format!("{} ", press.name()), name_style));
        let value_style = if editing { Style::new().fg(theme::ACCENT).bold() } else { Style::new().bold() };
        info_spans.push(Span::styled(value.unwrap_or_else(|| "—".into()), value_style));
    }
    if ed.is_edited(ed.layer, ed.selected) {
        info_spans.push(Span::styled("   (edited)", Style::new().fg(theme::EDITED)));
    }
    f.render_widget(center(Line::from(info_spans)), info);

    if let Some(text) = &ed.entry {
        f.render_widget(
            center(Line::from(vec![
                Span::styled(format!("new {}: ", ed.slot.name()), dim()),
                Span::styled(format!("{text}▏"), Style::new().fg(theme::ACCENT)),
                Span::styled("   a character or keycode (left_shift, mo 1, none) · tab other action · enter apply", dim()),
            ])),
            entry,
        );
    }

    let help = format!("arrows/press a key: select · enter change · del undo · tab layer · ctrl+s save · esc {cancel}");
    f.render_widget(Paragraph::new(footer(app, &help)).alignment(Alignment::Center), foot);
}

fn centered(area: Rect, width: u16) -> Rect {
    let w = width.min(area.width);
    Rect { x: area.x + (area.width - w) / 2, width: w, ..area }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_spaces() {
        let t: Vec<char> = "the quick brown fox".chars().collect();
        let lines: Vec<String> = wrap(&t, 10).into_iter().map(|r| t[r].iter().collect()).collect();
        assert_eq!(lines, vec!["the quick ", "brown fox"]);
        let long: Vec<char> = "abcdefghijkl".chars().collect();
        assert_eq!(wrap(&long, 5), vec![0..5, 5..10, 10..12]);
    }
}
