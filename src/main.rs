//! Keystroke — a typing tutor built with Slint.

mod finger;
mod keyboard_layout;
mod lessons;
mod net;
mod texts;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ModelRc, Timer, TimerMode, VecModel};

use finger::Finger;
use keyboard_layout::KeyboardLayout;
use net::{NetEvent, NetService, Peer};

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
    /// LAN multiplayer race. The detailed sub-state is in `AppState::game`.
    Game,
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
    /// Display name used in the LAN lobby. Survives navigating away from the
    /// Games tab so the user doesn't have to re-enter it.
    player_name: String,
    /// Networking and race state. Only `Some` while the Games tab is active.
    net: Option<NetService>,
    game: GameSession,
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
            player_name: default_player_name(),
            net: None,
            game: GameSession::default(),
        }
    }
}

/// Multiplayer game state, valid whenever `mode == Mode::Game`.
#[derive(Default)]
struct GameSession {
    sub: GameSubState,
    peers: Vec<Peer>,
    network_status: String,
    message: String,
    opponent: Option<Peer>,
    /// Last opponent we raced — retained after the race ends so the
    /// "Rematch" button knows who to re-invite.
    last_opponent_id: Option<String>,
    /// Set when another peer is asking us to race. Cleared on accept/decline
    /// or when the request times out.
    incoming_request: Option<Peer>,
    text_len: usize,
    countdown_started_at: Option<Instant>,
    countdown_secs: i32,
    opponent_pos: usize,
    opponent_errors: usize,
    opponent_finished: bool,
    opponent_wpm: f64,
    opponent_accuracy: f64,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum GameSubState {
    #[default]
    Lobby,
    Countdown,
    Racing,
    Finished,
}

impl From<GameSubState> for GameState {
    fn from(s: GameSubState) -> Self {
        match s {
            GameSubState::Lobby => GameState::Lobby,
            GameSubState::Countdown => GameState::Countdown,
            GameSubState::Racing => GameState::Racing,
            GameSubState::Finished => GameState::Finished,
        }
    }
}

const COUNTDOWN_SECS: i32 = 3;

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
        // The race view has its own header; nothing to set here.
        Mode::Game => {}
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
//  Games — UI mirroring
// ---------------------------------------------------------------------------

fn refresh_game_view(ui: &AppWindow, state: &AppState) {
    ui.set_game_state(state.game.sub.into());
    ui.set_my_player_name(state.player_name.as_str().into());
    ui.set_network_status(state.game.network_status.as_str().into());
    ui.set_game_message(state.game.message.as_str().into());
    ui.set_peers(build_peers_model(&state.game.peers));
    ui.set_opponent_name(
        state
            .game
            .opponent
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("Opponent")
            .into(),
    );
    ui.set_countdown_secs(state.game.countdown_secs);
    ui.set_incoming_request_name(
        state
            .game
            .incoming_request
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("")
            .into(),
    );
    ui.set_has_last_opponent(state.game.last_opponent_id.is_some());

    // Progress bars + per-side stats
    let (my_progress, my_stats) = if let Some(session) = &state.session {
        let p = if session.target.is_empty() {
            0.0
        } else {
            session.position() as f32 / session.target.len() as f32
        };
        let now = session.finished_at.unwrap_or_else(Instant::now);
        let stats = if session.start_time.is_some() {
            format!(
                "{:.0} WPM / {:.0}%",
                session.wpm(now),
                session.accuracy()
            )
        } else {
            String::new()
        };
        (p, stats)
    } else {
        (0.0, String::new())
    };
    ui.set_my_progress(my_progress);
    ui.set_my_stats_text(my_stats.into());

    let opp_progress = if state.game.text_len == 0 {
        0.0
    } else {
        (state.game.opponent_pos as f32 / state.game.text_len as f32).min(1.0)
    };
    ui.set_opponent_progress(opp_progress);
    ui.set_opponent_finished(state.game.opponent_finished);
    let opp_stats = if state.game.opponent_finished {
        format!(
            "{:.0} WPM / {:.0}%",
            state.game.opponent_wpm, state.game.opponent_accuracy
        )
    } else if state.game.text_len > 0 {
        format!(
            "{} / {} chars",
            state.game.opponent_pos.min(state.game.text_len),
            state.game.text_len
        )
    } else {
        String::new()
    };
    ui.set_opponent_stats_text(opp_stats.into());

    ui.set_race_result_summary(race_result_summary(state).into());
}

fn build_peers_model(peers: &[Peer]) -> ModelRc<LanPeer> {
    let mut items: Vec<LanPeer> = peers
        .iter()
        .map(|p| LanPeer {
            id: p.id.as_str().into(),
            name: p.name.as_str().into(),
            address: p.tcp_addr.to_string().into(),
        })
        .collect();
    // Stable display order: by name, then address.
    items.sort_by(|a, b| {
        let an: &str = a.name.as_str();
        let bn: &str = b.name.as_str();
        let aa: &str = a.address.as_str();
        let ba: &str = b.address.as_str();
        an.cmp(bn).then(aa.cmp(ba))
    });
    ModelRc::new(VecModel::from(items))
}

/// Human-readable result string shown in the race "finished" panel.
///
/// It updates progressively: "You finished first", then "... and you won!" or
/// "...but {opponent} caught up" once they also finish.
fn race_result_summary(state: &AppState) -> String {
    let opp_name = state
        .game
        .opponent
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("Opponent");
    let me_done = state
        .session
        .as_ref()
        .map(|s| s.is_finished())
        .unwrap_or(false);
    let opp_done = state.game.opponent_finished;

    if !me_done && !opp_done {
        return String::new();
    }

    let (my_wpm, my_acc) = match state.session.as_ref() {
        Some(s) if s.is_finished() => {
            let now = s.finished_at.unwrap_or_else(Instant::now);
            (s.wpm(now), s.accuracy())
        }
        _ => (0.0, 0.0),
    };

    match (me_done, opp_done) {
        (true, true) => {
            let winner = if my_wpm > state.game.opponent_wpm + 0.5 {
                "\u{1F3C6} You won!".to_string()
            } else if state.game.opponent_wpm > my_wpm + 0.5 {
                format!("\u{1F948} {} won.", opp_name)
            } else {
                "\u{1F91D} It's a tie!".to_string()
            };
            format!(
                "{}  \u{2014}  You: {:.0} WPM / {:.0}%   |   {}: {:.0} WPM / {:.0}%",
                winner,
                my_wpm,
                my_acc,
                opp_name,
                state.game.opponent_wpm,
                state.game.opponent_accuracy,
            )
        }
        (true, false) => format!(
            "\u{1F3C1} You finished first \u{2014} {:.0} WPM / {:.0}%. Waiting for {}\u{2026}",
            my_wpm, my_acc, opp_name
        ),
        (false, true) => format!(
            "{} finished at {:.0} WPM / {:.0}% \u{2014} keep going!",
            opp_name, state.game.opponent_wpm, state.game.opponent_accuracy
        ),
        (false, false) => String::new(),
    }
}

fn default_player_name() -> String {
    std::env::var("USERNAME")
        .ok()
        .or_else(|| std::env::var("USER").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Player".to_string())
}

fn start_game(state: &mut AppState) -> Result<(), String> {
    if state.net.is_none() {
        match NetService::start(state.player_name.clone()) {
            Ok(net) => {
                state.net = Some(net);
            }
            Err(e) => {
                state.game.message = format!("Failed to start LAN networking: {}", e);
                return Err(state.game.message.clone());
            }
        }
    }
    state.mode = Mode::Game;
    state.session = None;
    let net_ref = state.net.as_ref();
    state.game = GameSession {
        sub: GameSubState::Lobby,
        peers: net_ref.map(|n| n.current_peers()).unwrap_or_default(),
        network_status: net_ref
            .map(|n| format!("Listening on TCP port {} as \"{}\"", n.tcp_port, state.player_name))
            .unwrap_or_default(),
        ..Default::default()
    };
    Ok(())
}

fn stop_game(state: &mut AppState) {
    if let Some(net) = state.net.take() {
        net.quit_race();
        // `net` dropped here — worker threads see `running = false` and exit.
        drop(net);
    }
    state.game = GameSession::default();
    state.session = None;
}

/// Called when either `RaceStartedAsServer` or `RaceStartedAsClient` arrives.
/// Sets up the local Session and starts the countdown.
fn enter_race(state: &mut AppState, text: String, opponent: Peer) {
    state.session = Some(Session::new(&text));
    state.game.last_opponent_id = Some(opponent.id.clone());
    state.game.opponent = Some(opponent);
    state.game.text_len = text.chars().count();
    state.game.opponent_pos = 0;
    state.game.opponent_errors = 0;
    state.game.opponent_finished = false;
    state.game.opponent_wpm = 0.0;
    state.game.opponent_accuracy = 0.0;
    state.game.sub = GameSubState::Countdown;
    state.game.countdown_started_at = Some(Instant::now());
    state.game.countdown_secs = COUNTDOWN_SECS;
    state.game.message.clear();
    state.game.incoming_request = None;
}

fn quit_race_locally(state: &mut AppState) {
    if let Some(net) = state.net.as_ref() {
        net.quit_race();
    }
    state.session = None;
    state.game.sub = GameSubState::Lobby;
    state.game.opponent = None;
    state.game.text_len = 0;
    state.game.opponent_pos = 0;
    state.game.opponent_errors = 0;
    state.game.opponent_finished = false;
    state.game.countdown_started_at = None;
    state.game.incoming_request = None;
}

/// Apply a single `NetEvent` to the application state. The caller is
/// responsible for refreshing UI views afterwards (the live timer does this
/// on every tick).
fn apply_net_event(_ui: &AppWindow, state: &mut AppState, event: NetEvent) {
    match event {
        NetEvent::PeersUpdated(peers) => {
            state.game.peers = peers;
        }
        NetEvent::Status(msg) => {
            state.game.network_status = msg;
        }
        NetEvent::IncomingRaceRequest { opponent } => {
            // If we're not on the lobby screen, auto-decline so we don't
            // hijack a race that's already in progress.
            let can_show =
                matches!(state.mode, Mode::Game) && state.game.sub == GameSubState::Lobby;
            if can_show {
                state.game.incoming_request = Some(opponent);
            } else if let Some(net) = state.net.as_ref() {
                net.reject_incoming_race();
            }
        }
        NetEvent::RaceStartedAsServer { text, opponent }
        | NetEvent::RaceStartedAsClient { text, opponent } => {
            // Only honour race-start events while the user is on the Games tab.
            if matches!(state.mode, Mode::Game) {
                enter_race(state, text, opponent);
            }
        }
        NetEvent::OpponentProgress { position, errors } => {
            state.game.opponent_pos = position;
            state.game.opponent_errors = errors;
        }
        NetEvent::OpponentFinished { wpm, accuracy } => {
            state.game.opponent_finished = true;
            state.game.opponent_wpm = wpm;
            state.game.opponent_accuracy = accuracy;
            state.game.opponent_pos = state.game.text_len;
            // Once opponent is done, if we're also done, promote sub-state.
            if let Some(s) = state.session.as_ref() {
                if s.is_finished() {
                    state.game.sub = GameSubState::Finished;
                }
            }
        }
        NetEvent::OpponentDisconnected => {
            // If we were mid-race, treat as the opponent giving up.
            if matches!(state.mode, Mode::Game)
                && (state.game.sub == GameSubState::Countdown
                    || state.game.sub == GameSubState::Racing)
            {
                state.game.message = format!(
                    "{} left the race.",
                    state
                        .game
                        .opponent
                        .as_ref()
                        .map(|p| p.name.as_str())
                        .unwrap_or("Opponent")
                );
                state.session = None;
                state.game.sub = GameSubState::Lobby;
                state.game.opponent = None;
                state.game.text_len = 0;
                state.game.countdown_started_at = None;
            }
        }
        NetEvent::InviteRejected { opponent_id: _ } => {
            // The remote peer was busy or declined our invitation.
            state.game.message =
                "They're busy or declined the race. Try again in a moment.".to_string();
            // Make sure we're not waiting in any half-armed state.
            state.game.sub = GameSubState::Lobby;
            state.game.opponent = None;
            state.session = None;
        }
    }
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
        Mode::Home | Mode::Game => {}
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
        Mode::Home | Mode::Game => {}
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

    // In game mode, ignore keystrokes unless we're actually racing.
    if matches!(s.mode, Mode::Game) && s.game.sub != GameSubState::Racing {
        return;
    }

    // Capture layout up-front to avoid borrowing `s` again while a mutable
    // borrow of `s.session` is active.
    let layout = s.layout;
    let mode = s.mode;

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

    // Snapshot the values we may need to forward to the net layer below; we
    // do this before any potential `s.session` mutation that would invalidate
    // the borrow.
    let session_pos = session.position();
    let session_errors = session.errors;
    let target_len = session.target.len();

    // Did we just finish?
    let mut just_finished = false;
    if session_pos >= target_len {
        session.finished_at = Some(Instant::now());
        let final_wpm = session.wpm(session.finished_at.unwrap());
        just_finished = true;
        match mode {
            Mode::Lesson(i) => {
                s.lessons_completed.insert(i);
                if s.best_wpm.map_or(true, |b| final_wpm > b) {
                    s.best_wpm = Some(final_wpm);
                }
                refresh_home(ui, &s);
                refresh_picker_lists(ui, &s);
            }
            Mode::Practice(_) => {
                if s.best_wpm.map_or(true, |b| final_wpm > b) {
                    s.best_wpm = Some(final_wpm);
                }
                refresh_home(ui, &s);
                refresh_picker_lists(ui, &s);
            }
            Mode::Game => {
                s.game.sub = GameSubState::Finished;
            }
            Mode::Home => {}
        }
    }

    // For race mode, forward our progress to the opponent (after dropping
    // any borrows above).
    if matches!(mode, Mode::Game) {
        if let Some(net) = s.net.as_ref() {
            net.send_progress(session_pos, session_errors);
            if just_finished {
                let sess = s.session.as_ref().unwrap();
                let now = sess.finished_at.unwrap_or_else(Instant::now);
                net.send_done(sess.wpm(now), sess.accuracy());
            }
        }
    }

    if matches!(mode, Mode::Game) {
        refresh_game_view(ui, &s);
    } else {
        update_session_view(ui, &s);
    }
}

fn handle_backspace(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let mut s = state.borrow_mut();
    if matches!(s.mode, Mode::Game) && s.game.sub != GameSubState::Racing {
        return;
    }
    let mode = s.mode;
    let (pos, errors) = {
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
        (session.position(), session.errors)
    };
    if matches!(mode, Mode::Game) {
        if let Some(net) = s.net.as_ref() {
            net.send_progress(pos, errors);
        }
        refresh_game_view(ui, &s);
    } else {
        update_session_view(ui, &s);
    }
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
                if matches!(s.mode, Mode::Game) {
                    stop_game(&mut s);
                }
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
                if matches!(s.mode, Mode::Game) {
                    stop_game(&mut s);
                }
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
                if matches!(s.mode, Mode::Game) {
                    stop_game(&mut s);
                }
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
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_nav_game(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                // Re-clicking the Games tab while already there must not
                // tear down an in-progress race — just refresh the view.
                if !matches!(s.mode, Mode::Game) {
                    let _ = start_game(&mut s);
                }
                ui.set_page(AppPage::Game);
                ui.set_player_name_draft(s.player_name.as_str().into());
                refresh_game_view(&ui, &s);
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

    // ----- Game callbacks --------------------------------------------------
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_peer_clicked(move |peer_id| {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                if !matches!(s.mode, Mode::Game) {
                    return;
                }
                // Don't allow re-inviting while we're already racing.
                if s.game.sub != GameSubState::Lobby {
                    return;
                }
                let id: String = peer_id.into();
                if let Some(net) = s.net.as_ref() {
                    match net.invite(&id) {
                        Ok(()) => {
                            s.game.message = "Connecting\u{2026}".into();
                        }
                        Err(e) => {
                            s.game.message = format!("Could not start race: {}", e);
                        }
                    }
                    refresh_game_view(&ui, &s);
                }
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_quit_race(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                if !matches!(s.mode, Mode::Game) {
                    return;
                }
                quit_race_locally(&mut s);
                refresh_game_view(&ui, &s);
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_accept_incoming(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                if !matches!(s.mode, Mode::Game) {
                    return;
                }
                if let Some(net) = s.net.as_ref() {
                    net.accept_incoming_race();
                }
                s.game.incoming_request = None;
                s.game.message = "Starting race\u{2026}".into();
                refresh_game_view(&ui, &s);
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_reject_incoming(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                if !matches!(s.mode, Mode::Game) {
                    return;
                }
                if let Some(net) = s.net.as_ref() {
                    net.reject_incoming_race();
                }
                s.game.incoming_request = None;
                refresh_game_view(&ui, &s);
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_rematch(move || {
            if let Some(ui_now) = ui_handle.upgrade() {
                let opp_id = {
                    let mut s = state.borrow_mut();
                    if !matches!(s.mode, Mode::Game) {
                        return;
                    }
                    let id = s.game.last_opponent_id.clone();
                    quit_race_locally(&mut s);
                    s.game.message = "Requesting rematch\u{2026}".into();
                    refresh_game_view(&ui_now, &s);
                    id
                };
                // Wait a beat so the opponent's relay thread sees our QUIT
                // and clears its race slot before we send the new invite.
                if let Some(id) = opp_id {
                    let ui_handle = ui_handle.clone();
                    let state = state.clone();
                    Timer::single_shot(Duration::from_millis(300), move || {
                        if let Some(ui) = ui_handle.upgrade() {
                            let mut s = state.borrow_mut();
                            if !matches!(s.mode, Mode::Game) {
                                return;
                            }
                            if let Some(net) = s.net.as_ref() {
                                match net.invite(&id) {
                                    Ok(()) => {
                                        s.game.message = "Rematch invite sent\u{2026}".into();
                                    }
                                    Err(e) => {
                                        s.game.message = format!("Rematch failed: {}", e);
                                    }
                                }
                            }
                            refresh_game_view(&ui, &s);
                        }
                    });
                }
            }
        });
    }
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        ui.on_rename_player(move |new_name| {
            if let Some(ui) = ui_handle.upgrade() {
                let mut s = state.borrow_mut();
                let raw: String = new_name.into();
                let trimmed = raw.trim();
                let name = if trimmed.is_empty() {
                    "Player".to_string()
                } else {
                    trimmed.to_string()
                };
                s.player_name = name.clone();
                if let Some(net) = s.net.as_ref() {
                    net.set_name(name.clone());
                }
                s.game.network_status = match s.net.as_ref() {
                    Some(net) => format!("Listening on TCP port {} as \"{}\"", net.tcp_port, name),
                    None => String::new(),
                };
                // Mirror the canonical (trimmed / non-empty) name back into
                // the LineEdit so the user sees the normalised value.
                ui.set_player_name_draft(name.as_str().into());
                refresh_game_view(&ui, &s);
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

    // ----- Live timer to drive WPM / elapsed time and net events ----------
    let live_timer = Timer::default();
    {
        let ui_handle = ui.as_weak();
        let state = state.clone();
        live_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };
            let mut s = state.borrow_mut();

            // ----- Pump net events ------------------------------------------
            // Drain everything queued so we don't lag behind under load.
            // `try_iter()` would borrow `s.net` for the duration of the loop,
            // which prevents us from mutating `s` inside the loop body, so we
            // collect into a Vec first.
            let mut events: Vec<NetEvent> = Vec::new();
            if let Some(net) = s.net.as_ref() {
                while let Ok(ev) = net.event_rx.try_recv() {
                    events.push(ev);
                }
            }
            for ev in events {
                apply_net_event(&ui, &mut s, ev);
            }

            // ----- Countdown ------------------------------------------------
            if matches!(s.mode, Mode::Game) && s.game.sub == GameSubState::Countdown {
                if let Some(started) = s.game.countdown_started_at {
                    let elapsed = started.elapsed().as_secs_f32();
                    let remaining = (COUNTDOWN_SECS as f32 - elapsed).ceil() as i32;
                    if remaining <= 0 {
                        s.game.sub = GameSubState::Racing;
                        s.game.countdown_secs = 0;
                        if let Some(session) = s.session.as_mut() {
                            session.start_time = Some(Instant::now());
                        }
                        ui.invoke_focus_typing();
                    } else if remaining != s.game.countdown_secs {
                        s.game.countdown_secs = remaining;
                    }
                }
            }

            // ----- View refresh --------------------------------------------
            match s.mode {
                Mode::Lesson(_) | Mode::Practice(_) => {
                    if let Some(session) = &s.session {
                        if session.start_time.is_some() && !session.is_finished() {
                            update_session_view(&ui, &s);
                        }
                    }
                }
                Mode::Game => {
                    // Always mirror game state so the lobby's peer list,
                    // opponent progress bar, countdown number, etc. all stay
                    // current even when no typing is happening.
                    refresh_game_view(&ui, &s);
                    if s.game.sub == GameSubState::Racing
                        || s.game.sub == GameSubState::Finished
                    {
                        update_session_view(&ui, &s);
                    }
                }
                Mode::Home => {}
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
