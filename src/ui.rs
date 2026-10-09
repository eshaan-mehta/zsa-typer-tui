//! Rendering. Minimal, ttyper-style: dim untyped text, bright correct text, red mistakes.

use std::ops::Range;

use ratatui::layout::{Alignment, Constraint, Layout as Split, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{
    find_setting, number_field, App, ChartView, DeviceState, Editor, EditorPurpose, ResultsView, Screen, SettingsMenu,
    SETTINGS_ITEMS,
};
use crate::board::{Board, HintRole, KeyVisual, Marker, Scale};
use crate::geometry::KEY_COUNT;
use crate::layout::{Layout, Press};
use crate::stats::MIN_SAMPLES;
use crate::store::Mode;
use crate::theme;
use crate::typing::TypingTest;

const TEXT_WIDTH: u16 = 72;
const TEXT_LINES: usize = 3;

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

/// Text still to type: the dim gray, faded further by the terminal (toward its own
/// background), so it stands well apart from typed text in light and dark themes.
fn untyped() -> Style {
    dim().add_modifier(Modifier::DIM)
}

pub fn draw(f: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Typing => draw_typing(f, app, "tab new test · esc settings"),
        Screen::Results(view) => draw_results(f, app, view),
        Screen::Settings(menu) => {
            draw_typing(f, app, settings_actions(menu));
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

/// Lines at the bottom of every screen: a notice (when there is one), then the actions.
/// Screens with a second line of actions need one more.
const BOTTOM_H: u16 = 2;

/// The bottom of every screen: notices on their own line, and the keys you can press on the
/// last line(s), so actions are always in the same place.
fn draw_bottom(f: &mut Frame, app: &App, area: Rect, actions: &[&str]) {
    let [notice, keys] =
        Split::vertical([Constraint::Length(1), Constraint::Length(actions.len() as u16)]).areas(area);
    if let Some(n) = app.notice() {
        f.render_widget(Paragraph::new(Line::styled(n.to_string(), Style::new().fg(theme::ACCENT))).alignment(Alignment::Center), notice);
    }
    let lines: Vec<Line> = actions.iter().map(|a| Line::styled(a.to_string(), dim())).collect();
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), keys);
}

/// What can be pressed in the settings menu right now.
fn settings_actions(menu: &SettingsMenu) -> &'static str {
    match &menu.editing {
        Some(entry) if entry.is_path => "enter load · esc cancel",
        Some(_) => "enter save · esc cancel",
        None if !menu.search.is_empty() => "↑↓/jk move · enter change · ←→ adjust · esc clear search",
        None => "↑↓/jk move · type to jump · enter change · ←→ adjust · esc close",
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

/// The visible lines of the test, and where the cursor is in them (row, column).
fn text_lines(t: &TypingTest, width: usize) -> (Vec<Line<'static>>, (u16, u16)) {
    let lines = wrap(&t.target, width);
    let cursor = t.input.len();
    let current = lines.iter().position(|r| r.contains(&cursor)).unwrap_or(lines.len().saturating_sub(1));
    let first = current.saturating_sub(1).min(lines.len().saturating_sub(TEXT_LINES));
    let col = lines.get(current).map_or(0, |r| cursor.saturating_sub(r.start));
    let shown = lines
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
                        None => Span::styled(expected.to_string(), untyped()),
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect();
    (shown, ((current - first) as u16, col as u16))
}

/// How far the board sits above an even split of the space under the text.
const BOARD_LIFT: u16 = 6;

/// Space left for the board after the fixed rows on the typing screen.
const TYPING_CHROME: u16 = 1 + 1 + 1 + 1 + TEXT_GAP + TEXT_LINES as u16 + PROGRESS_H + 1 + 1 + 1 + BOTTOM_H;

/// Blank lines between the info lines and the text to type.
const TEXT_GAP: u16 = 3;

/// The progress bar, right under the text's rows (the text rarely fills all of them, so
/// there's usually space above it).
const PROGRESS_H: u16 = 1;

/// How far through the text you are: a thin line under it that fills in as you type.
fn progress_line(t: &TypingTest, width: usize) -> Line<'static> {
    let done = (t.input.len() * width).checked_div(t.target.len()).unwrap_or(0).min(width);
    Line::from(vec![
        Span::styled("━".repeat(done), Style::new().fg(theme::ACCENT)),
        Span::styled("─".repeat(width - done), untyped()),
    ])
}

/// What the current mode is working on: the letter strip in progressive mode, the weakest
/// letters in weak keys mode. (The line under it names the mode, so this doesn't.)
fn mode_line(app: &App) -> Line<'static> {
    let s = &app.store.saved.settings;
    let target = s.targets();
    if app.mode_needs_layout() {
        return Line::styled("needs your layout (connect your Voyager once) · using words for now", dim());
    }
    match s.mode {
        Mode::Words => Line::default(),
        Mode::Progressive => {
            let Some(p) = &app.progression else { return Line::default() };
            let focus = app.stats.focus(target);
            let mut spans = Vec::new();
            for c in &p.order {
                let style = if !app.stats.unlocked.contains(c) {
                    dim()
                } else if app.stats.letter(*c).learned {
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
                Line::styled(format!("all letters meet your targets ({} wpm, {}%)", s.target_wpm, s.target_accuracy), dim())
            } else if weak.is_empty() {
                Line::styled("not enough data yet, keep typing", dim())
            } else {
                let letters = weak.iter().map(char::to_string).collect::<Vec<_>>().join("  ");
                Line::from(vec![Span::styled("weakest  ", dim()), Span::styled(letters, Style::new().fg(theme::MISTAKE).bold())])
            }
        }
    }
}

// ----- screens -----

fn draw_typing(f: &mut Frame, app: &App, actions: &str) {
    let area = f.area();
    let show_board = app.device.is_connected();
    let scale = if show_board { Scale::fit(area.width, area.height.saturating_sub(TYPING_CHROME)) } else { None };
    let board_h = scale.map_or(if show_board { 1 } else { 0 }, |s| s.height());
    let text_w = TEXT_WIDTH.min(area.width.saturating_sub(4)).max(10);

    // Spare height: half above the text; the rest goes between the text and the board and
    // below the board, with the board lifted BOARD_LIFT lines off where a 5:3 split would
    // put it. Keeps the test clearly separated without pinning the keyboard to the bottom.
    let fixed = 1 + 1 + 1 + TEXT_GAP + TEXT_LINES as u16 + PROGRESS_H + 1 + board_h + 1 + BOTTOM_H;
    let spare = area.height.saturating_sub(fixed);
    let above_text = spare / 2;
    // (Rounded, and always leaving a line between the text and the board.)
    let below_board = ((spare * 3 + 8) / 16 + BOARD_LIFT).min((spare - above_text).saturating_sub(1));
    let above_board = spare - above_text - below_board;
    let [status, _, mode, stats, _, text, progress, _, caption, board, _, foot, _] = Split::vertical([
        Constraint::Length(1),
        Constraint::Length(above_text),
        Constraint::Length(1),
        Constraint::Length(1),
        // Gap so the text to type stands on its own below the info lines.
        Constraint::Length(TEXT_GAP),
        Constraint::Length(TEXT_LINES as u16),
        Constraint::Length(PROGRESS_H),
        Constraint::Length(above_board),
        Constraint::Length(1),
        Constraint::Length(board_h),
        // Actions sit just under the keyboard (not at the screen's bottom edge on tall
        // terminals); the leftover space goes below them.
        Constraint::Length(1),
        Constraint::Length(BOTTOM_H),
        Constraint::Length(below_board),
    ])
    .areas(area);

    f.render_widget(Paragraph::new(status_line(app)).alignment(Alignment::Center), status);

    let t = &app.test;
    // Before a test: how it's set up. During: live numbers.
    let stats_text = if t.is_started() {
        format!("{:.0} wpm   {:.0}%   {}/{}", t.wpm(), t.accuracy(), t.words_done(), t.word_count)
    } else {
        let s = &app.store.saved.settings;
        let mut parts = vec![s.mode.name().to_string(), format!("{} words", t.word_count)];
        parts.push(if s.hints { "hints on" } else { "hints off" }.to_string());
        if s.instant_death {
            parts.push("instant death".to_string());
        }
        parts.join(" · ")
    };
    let text_area = centered(text, text_w + 1);
    // What this mode is working on, shown before a test starts; hidden while typing.
    if !t.is_started() {
        f.render_widget(Paragraph::new(mode_line(app)), centered(mode, text_w + 1));
    }
    f.render_widget(Paragraph::new(Line::styled(stats_text, dim())), centered(stats, text_w + 1));
    let (lines, (row, col)) = text_lines(t, text_w as usize);
    f.render_widget(Paragraph::new(lines), text_area);
    // The terminal's cursor, shaped by the Cursor setting. Hidden while settings are open.
    if matches!(app.screen, Screen::Typing) && row < text_area.height && col < text_area.width {
        f.set_cursor_position((text_area.x + col, text_area.y + row));
    }
    // Lined up under the text, as wide as it wraps.
    let bar_area = Rect { y: progress.bottom().saturating_sub(1), height: progress.height.min(1), ..text_area };
    f.render_widget(Paragraph::new(progress_line(t, text_w.min(bar_area.width) as usize)), bar_area);

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

    draw_bottom(f, app, foot, &[actions]);
}

/// Height of the results graph, including its axis labels.
const CHART_H: u16 = 11;
/// Widest the results graph gets.
const CHART_MAX_W: u16 = 100;

fn draw_results(f: &mut Frame, app: &App, view: &ResultsView) {
    let area = f.area();
    let t = &app.test;
    if view.expanded {
        // Just the graph, as big as the screen.
        let [_, chart_head, chart, _, foot] = Split::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(BOTTOM_H + 1),
        ])
        .areas(area);
        let chart_w = area.width.saturating_sub(4);
        draw_chart(f, view, centered(chart_head, chart_w), centered(chart, chart_w));
        draw_bottom(f, app, foot, &["←→/hl inspect · ↑↓/jk wpm/accuracy · e/esc shrink", "tab next test · r retry · q quit"]);
        return;
    }
    // Status, summary, gap, graph header, graph, gap, footer; the board only if it still fits.
    let fixed = 1 + 7 + 1 + 1 + CHART_H + 1 + 1 + BOTTOM_H + 1;
    let show_board = app.device.is_connected() && app.layout.is_some();
    let scale = if show_board { Scale::fit(area.width, area.height.saturating_sub(fixed + 2)) } else { None };

    // After a test the next step matters more than the keyboard, so the actions sit right
    // under the graph (where the eyes are) and the board goes below them.
    let [status, _, summary, _, chart_head, chart, foot, _, board, _] = Split::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(7),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(CHART_H),
        Constraint::Length(BOTTOM_H + 1),
        Constraint::Length(1),
        Constraint::Length(scale.map_or(0, |s| s.height())),
        Constraint::Fill(1),
    ])
    .areas(area);
    let chart_w = CHART_MAX_W.min(area.width.saturating_sub(4));
    draw_chart(f, view, centered(chart_head, chart_w), centered(chart, chart_w));

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

    // The graph's keys first, then everything else.
    draw_bottom(f, app, foot, &["←→/hl inspect · ↑↓/jk wpm/accuracy · e expand", "tab next test · r retry · esc settings · q quit"]);
}

/// The results graph: running wpm and per-second raw (or running accuracy), red dots where
/// mistakes happened, and a cursor with the values at that second.
fn draw_chart(f: &mut Frame, view: &ResultsView, head: Rect, area: Rect) {
    let tl = &view.timeline;
    if tl.len() < 2 {
        f.render_widget(Paragraph::new(Line::styled("too short to graph", dim())).alignment(Alignment::Center), head);
        return;
    }
    let accent = Style::new().fg(theme::ACCENT);
    let raw_style = Style::new().fg(Color::DarkGray);
    let errors_style = Style::new().fg(theme::MISTAKE);
    let cur = &tl[view.cursor.min(tl.len() - 1)];

    // Header: which graph (the other one dim), then the values under the cursor.
    let tab = |name: &'static str, on: bool| {
        Span::styled(name, if on { accent.bold().underlined() } else { dim() })
    };
    let secs = fmt_secs(cur.t);
    let mut spans = vec![
        tab("wpm", view.chart == ChartView::Wpm),
        Span::raw("  "),
        tab("accuracy", view.chart == ChartView::Accuracy),
        Span::raw("      "),
        Span::styled(secs, Style::new().bold()),
        Span::raw("  "),
    ];
    match view.chart {
        ChartView::Wpm => spans.extend([
            Span::styled("wpm ", dim()),
            Span::styled(format!("{:.0}", cur.wpm), accent.bold()),
            Span::styled("  raw ", dim()),
            Span::styled(format!("{:.0}", cur.raw), Style::new().bold()),
        ]),
        ChartView::Accuracy => spans.extend([
            Span::styled("accuracy ", dim()),
            Span::styled(format!("{:.0}%", cur.accuracy), accent.bold()),
        ]),
    }
    spans.push(Span::styled("  errors ", dim()));
    spans.push(Span::styled(cur.errors.to_string(), if cur.errors > 0 { errors_style.bold() } else { Style::new().bold() }));
    f.render_widget(Paragraph::new(Line::from(spans)).alignment(Alignment::Center), head);

    // The plot fills the area down to the x axis and its labels; taller plots get finer ticks.
    let axis_y = area.bottom().saturating_sub(2);
    if axis_y <= area.top() + 2 {
        return;
    }
    let height = axis_y - area.top();
    let wanted_ticks = (height / ROWS_PER_TICK).max(2) as usize;

    // Series for the chosen graph, and its y range, rounded out to whole ticks.
    let main: Vec<(f64, f64)> = tl
        .iter()
        .map(|s| (s.t, if view.chart == ChartView::Wpm { s.wpm } else { s.accuracy }))
        .collect();
    let raw: Vec<(f64, f64)> = tl.iter().map(|s| (s.t, s.raw)).collect();
    let (y0, y1, y_step) = match view.chart {
        ChartView::Wpm => {
            let top = tl.iter().map(|s| s.wpm.max(s.raw)).fold(10.0, f64::max) * 1.05;
            let step = nice_step(top, wanted_ticks);
            (0.0, (top / step).ceil() * step, step)
        }
        ChartView::Accuracy => {
            let low = tl.iter().map(|s| s.accuracy).fold(100.0, f64::min);
            let step = nice_step((100.0 - low).max(10.0), wanted_ticks);
            (((low / step).floor() * step).min(90.0), 100.0, step)
        }
    };
    let y_ticks: Vec<f64> =
        (0..).map(|i| y0 + i as f64 * y_step).take_while(|v| *v <= y1 + y_step / 1000.0).collect();
    let fmt_y = |v: f64| if view.chart == ChartView::Accuracy { format!("{}%", fmt_num(v)) } else { fmt_num(v) };
    let (x0, x1) = (tl[0].t, tl[tl.len() - 1].t);

    // Axes: y labels, then a left axis and a bottom axis with x labels under it.
    let buf = f.buffer_mut();
    let label_w = y_ticks.iter().map(|v| fmt_y(*v).chars().count()).max().unwrap_or(1) as u16;
    let axis_x = area.left() + label_w + 1;
    if axis_x + 4 >= area.right() {
        return;
    }
    let plot = Plot { left: axis_x + 1, top: area.top(), width: area.right() - axis_x - 1, height, x: (x0, x1), y: (y0, y1) };
    let axis = dim();
    for y in plot.top..axis_y {
        buf[(axis_x, y)].set_char('│').set_style(axis);
    }
    buf[(axis_x, axis_y)].set_char('└').set_style(axis);
    for x in plot.left..area.right() {
        buf[(x, axis_y)].set_char('─').set_style(axis);
    }
    // Each y tick: a label, a mark on the axis, and a faint gridline across (not at the bottom,
    // which the x axis already marks).
    for &v in &y_ticks {
        let (row, label) = (plot.row(v), fmt_y(v));
        buf.set_string(axis_x - 1 - label.chars().count() as u16, row, &label, axis);
        buf[(axis_x, row)].set_char('┤');
        if v > y0 {
            for x in plot.left..area.right() {
                buf[(x, row)].set_char('┈').set_style(untyped());
            }
        }
    }
    // X labels: the test's length at the right end, then round times wherever they fit.
    let label_row = axis_y + 1;
    let end = fmt_secs(x1);
    let end_x = area.right() - end.chars().count() as u16;
    buf.set_string(end_x, label_row, &end, axis);
    let x_step = nice_step(x1 - x0, (plot.width / COLS_PER_X_TICK).max(1) as usize);
    let mut free_from = plot.left;
    for t in (0..).map(|i| i as f64 * x_step).skip_while(|t| *t < x0).take_while(|t| *t <= x1) {
        let (col, label) = (plot.col(t), fmt_secs(t));
        let w = label.chars().count() as u16;
        let start = col.saturating_sub(w / 2).max(plot.left);
        if start < free_from || start + w + 1 > end_x {
            continue;
        }
        buf.set_string(start, label_row, &label, axis);
        buf[(col, axis_y)].set_char('┬');
        free_from = start + w + 1;
    }

    // Cursor first so the lines cross over it, then raw under the main line, then mistakes.
    let cursor_col = plot.col(cur.t);
    for y in plot.top..plot.top + plot.height {
        buf[(cursor_col, y)].set_char('│').set_style(Style::new().fg(theme::MISTAKE));
    }
    match view.chart {
        ChartView::Wpm => plot.draw_lines(buf, &[(&raw, raw_style), (&main, accent)]),
        ChartView::Accuracy => plot.draw_lines(buf, &[(&main, accent)]),
    }
    for s in tl.iter().filter(|s| s.errors > 0) {
        let v = if view.chart == ChartView::Wpm { s.wpm } else { s.accuracy };
        buf[(plot.col(s.t), plot.row(v))].set_char('×').set_style(errors_style.bold());
    }
}

/// Rows between y ticks (at least), and columns between x ticks.
const ROWS_PER_TICK: u16 = 3;
const COLS_PER_X_TICK: u16 = 10;

/// A round step for axis ticks (1, 2 or 5 times a power of ten, or 25, 250, ...) that splits
/// `span` into at most about `n` parts.
fn nice_step(span: f64, n: usize) -> f64 {
    let rough = span / n.max(1) as f64;
    if rough.is_nan() || rough <= 0.0 {
        return 1.0;
    }
    let magnitude = 10f64.powf(rough.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .filter(|m| *m != 2.5 || magnitude >= 10.0)
        .map(|m| m * magnitude)
        .find(|step| *step >= rough)
        .unwrap_or(10.0 * magnitude)
}

/// Braille dot bits by (row, column) within a character.
const BRAILLE: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/// Where a graph's data goes on screen. Lines are drawn in braille dots, 2 across and 4 down
/// in each character, so they show four times the detail of a character per row.
struct Plot {
    left: u16,
    top: u16,
    width: u16,
    height: u16,
    x: (f64, f64),
    y: (f64, f64),
}

impl Plot {
    fn dots_w(&self) -> usize {
        self.width as usize * 2
    }

    fn dots_h(&self) -> usize {
        self.height as usize * 4
    }

    /// Dot column for time `t`.
    fn dot_x(&self, t: f64) -> usize {
        let frac = ((t - self.x.0) / (self.x.1 - self.x.0)).clamp(0.0, 1.0);
        (frac * (self.dots_w() - 1) as f64).round() as usize
    }

    /// Dot row for value `v`, counting from the top.
    fn dot_y(&self, v: f64) -> usize {
        let frac = ((v - self.y.0) / (self.y.1 - self.y.0)).clamp(0.0, 1.0);
        let bottom = self.dots_h() - 1;
        bottom - (frac * bottom as f64).round() as usize
    }

    fn col(&self, t: f64) -> u16 {
        self.left + (self.dot_x(t) / 2) as u16
    }

    fn row(&self, v: f64) -> u16 {
        self.top + (self.dot_y(v) / 4) as u16
    }

    /// The dots of a line through `points` (sorted by x), as a braille pattern per character:
    /// one dot per dot column, with steps split between the columns either side.
    fn trace(&self, points: &[(f64, f64)]) -> Vec<u8> {
        let w = self.width as usize;
        let mut cells = vec![0u8; w * self.height as usize];
        let mut set = |x: usize, y: usize| cells[y / 4 * w + x / 2] |= BRAILLE[y % 4][x % 2];
        let mut prev: Option<usize> = None;
        for x in 0..self.dots_w() {
            let t = self.x.0 + x as f64 / (self.dots_w() - 1) as f64 * (self.x.1 - self.x.0);
            let y = self.dot_y(interpolate(points, t));
            match prev {
                Some(p) => {
                    let mid = (p + y) / 2;
                    for yy in p.min(mid)..=p.max(mid) {
                        set(x - 1, yy);
                    }
                    for yy in mid.min(y)..=mid.max(y) {
                        set(x, yy);
                    }
                }
                None => set(x, y),
            }
            prev = Some(y);
        }
        cells
    }

    /// Lines through each series, later ones on top. A character shows only the topmost line
    /// through it, so crossing lines don't smear into each other's color.
    fn draw_lines(&self, buf: &mut ratatui::buffer::Buffer, series: &[(&[(f64, f64)], Style)]) {
        let traced: Vec<(Vec<u8>, Style)> = series.iter().map(|(points, style)| (self.trace(points), *style)).collect();
        let w = self.width as usize;
        for i in 0..w * self.height as usize {
            let Some((dots, style)) = traced.iter().rev().find(|(dots, _)| dots[i] != 0) else { continue };
            let ch = char::from_u32(0x2800 + u32::from(dots[i])).unwrap_or(' ');
            buf[(self.left + (i % w) as u16, self.top + (i / w) as u16)].set_char(ch).set_style(*style);
        }
    }
}

/// Linear interpolation between sorted (x, y) points, clamped at the ends.
fn interpolate(points: &[(f64, f64)], x: f64) -> f64 {
    let Some(&(first_x, first_y)) = points.first() else { return 0.0 };
    if x <= first_x {
        return first_y;
    }
    for w in points.windows(2) {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        if x <= bx {
            return if bx > ax { ay + (by - ay) * (x - ax) / (bx - ax) } else { by };
        }
    }
    points.last().map_or(0.0, |p| p.1)
}

/// A number to one decimal, without a trailing ".0" (so "4", "6.9").
fn fmt_num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 { format!("{r:.0}") } else { format!("{r:.1}") }
}

/// Seconds to one decimal, without a trailing ".0" (so "4s", "6.9s").
fn fmt_secs(v: f64) -> String {
    format!("{}s", fmt_num(v))
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
                (_, Some(entry)) if selected && !entry.is_path => format!("{}▏{unit}", entry.text),
                ("Word list", _) => match &app.word_list {
                    Some(l) => {
                        let count = format!(" ({})", l.count);
                        let room = 20usize.saturating_sub(count.chars().count());
                        let name: String = if l.name.chars().count() > room {
                            l.name.chars().take(room.saturating_sub(1)).chain(['…']).collect()
                        } else {
                            l.name.clone()
                        };
                        name + &count
                    }
                    None => "built-in".to_string(),
                },
                ("Mode", _) => s.mode.name().to_string(),
                ("Hints", _) => if s.hints { "on" } else { "off" }.to_string(),
                ("Cursor", _) => s.cursor.name().to_string(),
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
    // Bottom of the popup: what's being typed or searched, and details/errors. Keys are in the
    // screen's bottom bar like everywhere else.
    let mut detail = Line::default();
    let status = match &menu.editing {
        // The path gets the whole status line; hints and errors take the help line.
        Some(entry) if entry.is_path => {
            let room = inner.width.saturating_sub(10) as usize;
            let chars: Vec<char> = entry.text.chars().collect();
            let shown: String = if chars.len() > room {
                std::iter::once('…').chain(chars[chars.len() - room + 1..].iter().copied()).collect()
            } else {
                entry.text.clone()
            };
            detail = match &entry.error {
                Some(err) => Line::styled(err.clone(), Style::new().fg(theme::MISTAKE)),
                None => Line::styled("words separated by spaces or new lines · empty for built-in", dim()),
            };
            Line::from(vec![Span::styled("file: ", dim()), Span::styled(format!("{shown}▏"), accent)])
        }
        Some(entry) => match &entry.error {
            Some(err) => Line::styled(err.clone(), Style::new().fg(theme::MISTAKE)),
            None => {
                let range = number_field(SETTINGS_ITEMS[menu.selected]).map(|(r, _)| format!("{}–{}", r.start(), r.end()));
                Line::styled(format!("type a number ({})", range.unwrap_or_default()), dim())
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
    lines.push(detail);
    f.render_widget(Paragraph::new(lines), inner.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 }));
}

fn draw_editor(f: &mut Frame, app: &App, ed: &Editor) {
    let area = f.area();
    let current = ed.current();
    let scale = Scale::fit(area.width, area.height.saturating_sub(9 + BOTTOM_H));

    let [_, title, sub, msg, _, board, _, info, entry, _, foot, _] = Split::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(scale.map_or(1, |s| s.height())),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        // Actions just under the content, which is centered with them.
        Constraint::Length(1),
        Constraint::Length(BOTTOM_H),
        Constraint::Fill(1),
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
                Span::styled("   a character or keycode (left_shift, mo 1, none)", dim()),
            ])),
            entry,
        );
    }

    let actions = if ed.entry.is_some() {
        "tab other action · enter apply · esc cancel".to_string()
    } else {
        format!("arrows/press a key: select · enter change · del undo · tab layer · ctrl+s save · esc {cancel}")
    };
    draw_bottom(f, app, foot, &[&actions]);
}

fn centered(area: Rect, width: u16) -> Rect {
    let w = width.min(area.width);
    Rect { x: area.x + (area.width - w) / 2, width: w, ..area }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_lines_are_braille_dots() {
        let area = Rect::new(0, 0, 6, 2);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        // 12 dots across (one per time unit) and 8 down (one per value unit).
        let plot = Plot { left: 0, top: 0, width: 6, height: 2, x: (0.0, 11.0), y: (0.0, 7.0) };
        // Flat at 0, then a step up to 7 halfway: the step is split over the dots either side.
        plot.draw_lines(&mut buf, &[(&[(0.0, 0.0), (5.0, 0.0), (6.0, 7.0), (11.0, 7.0)], Style::new())]);
        let rows: Vec<String> = (0..2).map(|y| (0..6).map(|x| buf[(x, y)].symbol()).collect()).collect();
        assert_eq!(rows[0], "  ⢀⡏⠉⠉");
        assert_eq!(rows[1], "⣀⣀⣸   ");
        assert_eq!(interpolate(&[(0.0, 0.0), (2.0, 10.0)], 1.0), 5.0);
    }

    #[test]
    fn round_tick_steps() {
        assert_eq!(nice_step(73.5, 3), 25.0);
        assert_eq!(nice_step(73.5, 12), 10.0);
        assert_eq!(nice_step(10.0, 4), 5.0, "no 2.5% accuracy ticks");
        assert_eq!(nice_step(23.2, 9), 5.0);
        assert_eq!(nice_step(0.0, 3), 1.0);
    }

    #[test]
    fn seconds_labels() {
        assert_eq!(fmt_secs(3.95), "4s");
        assert_eq!(fmt_secs(6.9), "6.9s");
        assert_eq!(fmt_secs(1.0), "1s");
    }

    #[test]
    fn wraps_at_spaces() {
        let t: Vec<char> = "the quick brown fox".chars().collect();
        let lines: Vec<String> = wrap(&t, 10).into_iter().map(|r| t[r].iter().collect()).collect();
        assert_eq!(lines, vec!["the quick ", "brown fox"]);
        let long: Vec<char> = "abcdefghijkl".chars().collect();
        assert_eq!(wrap(&long, 5), vec![0..5, 5..10, 10..12]);
    }

    #[test]
    fn progress_fills_as_you_type() {
        let mut t = TypingTest::from_text("abcd");
        let filled = |t: &TypingTest| progress_line(t, 8).spans[0].content.chars().count();
        assert_eq!(filled(&t), 0);
        t.type_char('a');
        t.type_char('x');
        assert_eq!(filled(&t), 4, "mistakes still move you along");
        assert_eq!(progress_line(&t, 8).width(), 8);
        assert_eq!(progress_line(&TypingTest::from_text(""), 8).width(), 8);
    }

    #[test]
    fn cursor_follows_the_wrapped_text() {
        let mut t = TypingTest::from_text("the quick brown fox");
        assert_eq!(text_lines(&t, 10).1, (0, 0));
        for c in "the quick b".chars() {
            t.type_char(c);
        }
        // "the quick " fills the first line, so the next letter is the second on line two.
        assert_eq!(text_lines(&t, 10).1, (1, 1));
    }
}
