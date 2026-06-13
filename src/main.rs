//! Keystroke — a typing tutor built with Slint.

mod finger;
mod keyboard_layout;
mod lessons;
mod texts;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ModelRc, Timer, TimerMode, VecModel};

use finger::Finger;
use keyboard_layout::KeyboardLayout;

slint::include_modules!();

// ---------------------------------------------------------------------------
//  Domain model
// ---------------------------------------------------------------------------

/// Currently displayed page / mode of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Home,
    Lesson(usize),
    Practice(usize),
}

/// An active typing session.
struct Session {
    target: Vec<char>,
    typed: Vec<char>,
    /// Total number of incorrect keystrokes (including ones later corrected).
    errors: usize,
    start_time: Option<Instant>,
    finished_at: Option<Instant>,
}

impl Session {
    fn new(text: &str) -> Self {
        Self {
            target: text.chars().collect(),
            typed: Vec::new(),
            errors: 0,
            start_time: None,
            finished_at: None,
        }
    }

    fn position(&self) -> usize {
        self.typed.len()
    }

    fn is_finished(&self) -> bool {
        self.finished_at.is_some()
    }

    fn elapsed_secs(&self, now: Instant) -> f64 {
        match (self.start_time, self.finished_at) {
            (Some(s), Some(f)) => (f - s).as_secs_f64(),
            (Some(s), None) => (now - s).as_secs_f64(),
            _ => 0.0,
        }
    }

    fn correct_chars(&self) -> usize {
        self.typed
            .iter()
            .zip(self.target.iter())
            .filter(|(a, b)| a == b)
            .count()
    }

    fn wpm(&self, now: Instant) -> f64 {
        let secs = self.elapsed_secs(now);
        if secs < 0.05 {
            return 0.0;
        }
        // Standard "net WPM" using correctly-typed characters / 5.
        (self.correct_chars() as f64 / 5.0) / (secs / 60.0)
    }

    fn accuracy(&self) -> f64 {
        // Net accuracy: correctly typed characters divided by the total number
        // of keystrokes (including ones that were wrong and later corrected).
        if self.typed.is_empty() {
            return 100.0;
        }
        let correct = self.correct_chars();
        let denominator = correct + self.errors;
        if denominator == 0 {
            return 100.0;
        }
        (correct as f64 / denominator as f64) * 100.0
    }
}

struct AppState {
    mode: Mode,
    session: Option<Session>,
    lessons_completed: HashSet<usize>,
    best_wpm: Option<f64>,
    last_lesson_index: usize,
    last_text_index: usize,
    layout: &'static KeyboardLayout,
}

impl AppState {
    fn new(layout: &'static KeyboardLayout) -> Self {
        Self {
            mode: Mode::Home,
            session: None,
            lessons_completed: HashSet::new(),
            best_wpm: None,
            last_lesson_index: 0,
            last_text_index: 0,
            layout,
        }
    }
}

// ---------------------------------------------------------------------------
//  Helpers — text wrapping & UI building
// ---------------------------------------------------------------------------

/// Wraps a text into rows that each contain no more than `max_chars`
/// characters, breaking at word boundaries when possible.
/// Returns a list of (start_index, line_chars) pairs so the caller knows
/// where each row begins in the original character index.
fn wrap_indices(target: &[char], max_chars: usize) -> Vec<std::ops::Range<usize>> {
    let mut rows = Vec::new();
    let mut row_start = 0usize;
    let mut last_space: Option<usize> = None;
    let mut i = 0usize;
    while i < target.len() {
        let len_so_far = i - row_start + 1;
        if target[i] == ' ' {
            last_space = Some(i);
        }
        if len_so_far > max_chars {
            // Break at the last space that lies strictly before `i` (so we
            // include the space on the previous row and still make progress).
            // If the overflowing character is itself a space, or no usable
            // space exists in this row, hard-break at `i` instead.
            let break_at = match last_space {
                Some(s) if s >= row_start && s < i => s + 1,
                _ => i,
            };
            rows.push(row_start..break_at);
            row_start = break_at;
            last_space = None;
            // do not advance i; reprocess in new row
            continue;
        }
        i += 1;
    }
    if row_start < target.len() {
        rows.push(row_start..target.len());
    }
    if rows.is_empty() {
        rows.push(0..0);
    }
    rows
}

fn build_rows_model(session: &Session, layout: &KeyboardLayout) -> ModelRc<CharRow> {
    let ranges = wrap_indices(&session.target, 56);
    let pos = session.position();
    let mut rows: Vec<CharRow> = Vec::with_capacity(ranges.len());

    for range in ranges {
        let mut cells: Vec<CharCell> = Vec::with_capacity(range.end - range.start);
        for i in range {
            let ch = session.target[i];
            let state: i32 = if i < pos {
                // Already typed: green if it matched, red otherwise.
                if session.typed[i] == ch {
                    1
                } else {
                    2
                }
            } else if i == pos {
                3 // cursor
            } else {
                0 // untyped
            };
            let fg = layout.finger_for_char(ch).as_i32();
            cells.push(CharCell {
                character: ch.to_string().into(),
                state,
                finger: fg,
            });
        }
        rows.push(CharRow {
            cells: ModelRc::new(VecModel::from(cells)),
        });
    }
    ModelRc::new(VecModel::from(rows))
}

/// Build a [`KeyRow`] model from a static [`KeyboardLayout`] for the Slint UI.
fn build_keyboard_rows(layout: &KeyboardLayout) -> ModelRc<KeyRow> {
    let rows: Vec<KeyRow> = layout
        .rows
        .iter()
        .map(|row| {
            let keys: Vec<KeyDef> = row
                .iter()
                .map(|k| KeyDef {
                    label: k.label.into(),
                    unshifted: char_to_string(k.unshifted).into(),
                    shifted: char_to_string(k.shifted).into(),
                    width: k.width,
                    finger: k.finger.as_i32(),
                })
                .collect();
            KeyRow {
                keys: ModelRc::new(VecModel::from(keys)),
            }
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

fn char_to_string(c: char) -> String {
    if c == '\0' {
        String::new()
    } else {
        c.to_string()
    }
}

fn finger_label(f: Finger) -> &'static str {
    match f {
        Finger::None => "any",
        Finger::LeftPinky => "left pinky",
        Finger::LeftRing => "left ring",
        Finger::LeftMiddle => "left middle",
        Finger::LeftIndex => "left index",
        Finger::LeftThumb => "left thumb",
        Finger::RightThumb => "right thumb",
        Finger::RightIndex => "right index",
        Finger::RightMiddle => "right middle",
        Finger::RightRing => "right ring",
        Finger::RightPinky => "right pinky",
    }
}

// ---------------------------------------------------------------------------
//  Updating the UI from the current state
// ---------------------------------------------------------------------------

fn update_session_view(ui: &AppWindow, state: &AppState) {
    let session = match &state.session {
        Some(s) => s,
        None => return,
    };

    // Title & description per mode
    match state.mode {
        Mode::Lesson(i) => {
            let lesson = &lessons::LESSONS[i];
            ui.set_session_title(lesson.title.into());
            ui.set_session_description(lesson.description.into());
        }
        Mode::Practice(i) => {
            let text = &texts::TEXTS[i];
            ui.set_session_title(text.title.into());
            ui.set_session_description(
                format!(
                    "From {} \u{2014} type the passage below as quickly and accurately as you can.",
                    text.source
                )
                .into(),
            );
        }
        Mode::Home => return,
    }

    ui.set_rows(build_rows_model(session, state.layout));

    // Highlight next key + finger
    if let Some(&c) = session.target.get(session.position()) {
        // Send the raw character; the Slint Key element matches against
        // either the unshifted or shifted character of each key, so this works
        // for both lowercase and shifted/non-ASCII characters.
        ui.set_next_key(c.to_string().into());
        let f = state.layout.finger_for_char(c);
        ui.set_active_finger(f.as_i32());
        ui.set_finger_hint_index(f.as_i32());
        let display_char = if c == ' ' {
            "space".to_string()
        } else {
            format!("'{}'", c)
        };
        if matches!(f, Finger::None) {
            ui.set_finger_hint(format!("Next: {}", display_char).into());
        } else {
            ui.set_finger_hint(
                format!("Next: {}  —  use your {}", display_char, finger_label(f)).into(),
            );
        }
    } else {
        ui.set_next_key("".into());
        ui.set_active_finger(-1);
        ui.set_finger_hint_index(-1);
        ui.set_finger_hint("Lesson complete! Press Retry or move to the next one.".into());
    }

    // Live stats
    let now = Instant::now();
    let elapsed = session.elapsed_secs(now);
    ui.set_time_text(format!("{:.1}s", elapsed).into());
    ui.set_wpm_text(format!("{:.0}", session.wpm(now)).into());
    ui.set_accuracy_text(format!("{:.0}%", session.accuracy()).into());
    ui.set_errors_text(format!("{}", session.errors).into());
    let progress = if session.target.is_empty() {
        0.0
    } else {
        session.position() as f32 / session.target.len() as f32
    };
    ui.set_progress(progress);

    // Finished state
    ui.set_finished(session.is_finished());
    if session.is_finished() {
        ui.set_result_summary(
            format!(
                "WPM: {:.0}     Accuracy: {:.0}%     Time: {:.1}s     Errors: {}",
                session.wpm(now),
                session.accuracy(),
                elapsed,
                session.errors,
            )
            .into(),
        );
    }
}

fn refresh_picker_lists(ui: &AppWindow, state: &AppState) {
    // Lessons sidebar
    let items: Vec<PickerItem> = lessons::LESSONS
        .iter()
        .enumerate()
        .map(|(i, l)| PickerItem {
            title: l.title.into(),
            subtitle: format!("{} characters", l.text.chars().count()).into(),
            completed: state.lessons_completed.contains(&i),
        })
        .collect();
    ui.set_lessons_list(ModelRc::new(VecModel::from(items)));

    // Texts sidebar — subtitle is the source attribution.
    let items: Vec<PickerItem> = texts::TEXTS
        .iter()
        .map(|t| PickerItem {
            title: t.title.into(),
            subtitle: t.source.into(),
            completed: false,
        })
        .collect();
    ui.set_texts_list(ModelRc::new(VecModel::from(items)));

    // Selected indices
    if let Mode::Lesson(i) = state.mode {
        ui.set_lessons_selected_index(i as i32);
    }
    if let Mode::Practice(i) = state.mode {
        ui.set_texts_selected_index(i as i32);
    }
}

fn refresh_home(ui: &AppWindow, state: &AppState) {
    let text = match state.best_wpm {
        Some(wpm) => format!(
            "Best so far: {:.0} WPM\nLessons completed: {} / {}",
            wpm,
            state.lessons_completed.len(),
            lessons::LESSONS.len()
        ),
        None => "Complete a typing test to see your\nbest result here.".to_string(),
    };
    ui.set_last_result_text(text.into());
}

// ---------------------------------------------------------------------------
//  Session helpers
// ---------------------------------------------------------------------------

fn start_lesson(state: &mut AppState, idx: usize) {
    let idx = idx.min(lessons::LESSONS.len() - 1);
    state.mode = Mode::Lesson(idx);
    state.last_lesson_index = idx;
    state.session = Some(Session::new(lessons::LESSONS[idx].text));
}

fn start_practice(state: &mut AppState, idx: usize) {
    let idx = idx.min(texts::TEXTS.len() - 1);
    state.mode = Mode::Practice(idx);
    state.last_text_index = idx;
    state.session = Some(Session::new(texts::TEXTS[idx].content));
}

fn restart_current(state: &mut AppState) {
    match state.mode {
        Mode::Lesson(i) => state.session = Some(Session::new(lessons::LESSONS[i].text)),
        Mode::Practice(i) => state.session = Some(Session::new(texts::TEXTS[i].content)),
        Mode::Home => {}
    }
}

fn advance_to_next(state: &mut AppState) {
    match state.mode {
        Mode::Lesson(i) => {
            let next = (i + 1).min(lessons::LESSONS.len() - 1);
            start_lesson(state, next);
        }
        Mode::Practice(i) => {
            let next = (i + 1) % texts::TEXTS.len();
            start_practice(state, next);
        }
        Mode::Home => {}
    }
}

// ---------------------------------------------------------------------------
//  Wiring
// ---------------------------------------------------------------------------

fn schedule_clear_pressed(ui: &AppWindow) {
    let ui_handle = ui.as_weak();
    Timer::single_shot(Duration::from_millis(140), move || {
        if let Some(ui) = ui_handle.upgrade() {
            ui.set_pressed_key("".into());
            ui.set_pressed_finger(-1);
        }
    });
}

fn handle_key_typed(ui: &AppWindow, state: &Rc<RefCell<AppState>>, text: &str) {
    let mut s = state.borrow_mut();

    // Auto-start the session/page if user starts typing on the home page
    // (a nice shortcut).
    if matches!(s.mode, Mode::Home) {
        return;
    }

    // Capture layout up-front to avoid borrowing `s` again while a mutable
    // borrow of `s.session` is active.
    let layout = s.layout;

    let session = match s.session.as_mut() {
        Some(x) => x,
        None => return,
    };
    if session.is_finished() {
        return;
    }
    // Take the first usable character from the input string.
    let typed_char = match text.chars().next() {
        Some(c) if !c.is_control() => c,
        _ => return,
    };

    if session.start_time.is_none() {
        session.start_time = Some(Instant::now());
    }

    let pos = session.position();
    if pos >= session.target.len() {
        return;
    }
    let expected = session.target[pos];
    if typed_char != expected {
        session.errors += 1;
    }
    session.typed.push(typed_char);

    // Visual feedback: pressed key + pressed finger.
    // Send the actual character typed; the Slint Key element matches both
    // unshifted and shifted forms so this works for capitals and symbols.
    ui.set_pressed_key(typed_char.to_string().into());
    let pf = layout.finger_for_char(typed_char).as_i32();
    ui.set_pressed_finger(pf);
    schedule_clear_pressed(ui);

    // Did we just finish?
    if session.position() >= session.target.len() {
        session.finished_at = Some(Instant::now());
        let final_wpm = session.wpm(session.finished_at.unwrap());
        if let Mode::Lesson(i) = s.mode {
            s.lessons_completed.insert(i);
        }
        if s.best_wpm.map_or(true, |b| final_wpm > b) {
            s.best_wpm = Some(final_wpm);
        }
        refresh_home(ui, &s);
        refresh_picker_lists(ui, &s);
    }

    update_session_view(ui, &s);
}

fn handle_backspace(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let mut s = state.borrow_mut();
    let session = match s.session.as_mut() {
        Some(x) => x,
        None => return,
    };
    if session.is_finished() {
        return;
    }
    if session.position() > 0 {
        session.typed.pop();
    }
    update_session_view(ui, &s);
}

fn main() -> Result<(), slint::PlatformError> {
    // Detect the active keyboard layout reported by the OS. Falls back to
    // US QWERTY when detection fails or the layout isn't in our table.
    let detected = keyboard_layout::detect();
    let layout = detected.layout;

    let ui = AppWindow::new()?;
    let state = Rc::new(RefCell::new(AppState::new(layout)));

    // Configure the virtual keyboard widget and the layout label in the top bar.
    ui.set_keyboard_rows(build_keyboard_rows(layout));
    let layout_label = match detected.detected_klid.as_deref() {
        None => format!("{}  (default \u{2014} detection unavailable)", layout.name),
        Some(klid) if !detected.is_exact_match => {
            format!("{}  (KLID {} \u{2014} using fallback)", layout.name, klid)
        }
        Some(klid) => format!("{}  (KLID {})", layout.name, klid),
    };
    ui.set_layout_name(layout_label.into());

    refresh_picker_lists(&ui, &state.borrow());
    refresh_home(&ui, &state.borrow());

    // ----- Navigation -------------------------------------------------------
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_nav_home(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                s.mode = Mode::Home;
                s.session = None;
                ui.set_page(AppPage::Home);
                refresh_home(&ui, &s);
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_nav_lessons(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                let idx = match s.mode {
                    Mode::Lesson(i) => i,
                    _ => s.last_lesson_index,
                };
                start_lesson(&mut s, idx);
                ui.set_page(AppPage::Lessons);
                refresh_picker_lists(&ui, &s);
                update_session_view(&ui, &s);
                ui.invoke_focus_typing();
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_nav_practice(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                let idx = match s.mode {
                    Mode::Practice(i) => i,
                    _ => s.last_text_index,
                };
                start_practice(&mut s, idx);
                ui.set_page(AppPage::Practice);
                refresh_picker_lists(&ui, &s);
                update_session_view(&ui, &s);
                ui.invoke_focus_typing();
            }
        });
    }

    // ----- Picker callbacks -------------------------------------------------
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_lesson_clicked(move |i| {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                start_lesson(&mut s, i as usize);
                refresh_picker_lists(&ui, &s);
                update_session_view(&ui, &s);
                ui.invoke_focus_typing();
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_text_clicked(move |i| {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                start_practice(&mut s, i as usize);
                refresh_picker_lists(&ui, &s);
                update_session_view(&ui, &s);
                ui.invoke_focus_typing();
            }
        });
    }

    // ----- Restart / next item ---------------------------------------------
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_restart(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                restart_current(&mut s);
                update_session_view(&ui, &s);
                ui.invoke_focus_typing();
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_next_item(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                advance_to_next(&mut s);
                refresh_picker_lists(&ui, &s);
                update_session_view(&ui, &s);
                ui.invoke_focus_typing();
            }
        });
    }

    // ----- Typing -----------------------------------------------------------
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_key_typed(move |text| {
            if let Some(ui) = ui_handle.upgrade() {
                let text: String = text.into();
                handle_key_typed(&ui, &state, &text);
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_backspace(move || {
            if let Some(ui) = ui_handle.upgrade() {
                handle_backspace(&ui, &state);
            }
        });
    }

    // ----- Live timer to drive WPM / elapsed time --------------------------
    let live_timer = Timer::default();
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        live_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
            let s = state.borrow();
            if let Some(session) = &s.session {
                if session.start_time.is_some() && !session.is_finished() {
                    if let Some(ui) = ui_handle.upgrade() {
                        update_session_view(&ui, &s);
                    }
                }
            }
        });
    }

    ui.run()?;
    drop(live_timer);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::wrap_indices;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn wrap_short_text_returns_single_row() {
        let t = chars("hello world");
        let rows = wrap_indices(&t, 56);
        assert_eq!(rows, vec![0..11]);
    }

    #[test]
    fn wrap_empty_text_returns_one_empty_row() {
        let rows = wrap_indices(&[], 56);
        assert_eq!(rows, vec![0..0]);
    }

    #[test]
    fn wrap_breaks_at_previous_space() {
        let t = chars("aa bb cc dd ee");
        let rows = wrap_indices(&t, 5);
        // First row "aa bb" fills exactly 5 chars, then the space at index 5
        // overflows and forces a hard break (no usable earlier space is
        // strictly before i), so the leading space appears on row 2.
        assert_eq!(rows, vec![0..5, 5..9, 9..14]);
    }

    // Regression test: previously, when the character at position `i` was a
    // space that itself pushed the row past `max_chars`, the algorithm broke
    // at `i + 1` without advancing `i`, causing `i - row_start` to underflow
    // on the next iteration. Reproduced when clicking Next from "Peer Gynt"
    // to "Sult (åpning)" in the Norwegian sample texts.
    #[test]
    fn wrap_does_not_panic_when_overflowing_char_is_space() {
        let t = chars(
            "Det var i den tid jeg gikk omkring og sultet i Kristiania, denne \
             forunderlige by som ingen forlater før han har fått merker av den.",
        );
        let rows = wrap_indices(&t, 56);
        assert!(!rows.is_empty());
        // Every range must be non-empty and sequential.
        let mut last_end = 0usize;
        for r in &rows {
            assert!(
                r.start == last_end,
                "row {:?} does not start at {}",
                r,
                last_end
            );
            assert!(r.end > r.start, "row {:?} is empty", r);
            last_end = r.end;
        }
        assert_eq!(last_end, t.len());
    }

    #[test]
    fn wrap_hard_breaks_when_no_space_fits() {
        let t = chars("abcdefghij");
        let rows = wrap_indices(&t, 3);
        assert_eq!(rows, vec![0..3, 3..6, 6..9, 9..10]);
    }
}
