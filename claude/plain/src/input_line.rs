//! The input line at the bottom of the terminal: what it is for, its keys, and the prompt in
//! front of it, which shows the context, the usage limits and what Claude does.

use std::borrow::Cow;
use std::env;
use std::io;
use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use nu_ansi_term::{Color, Style};
use reedline::{
    ExternalOutput, Highlighter, KeyCode, KeyModifiers, Prompt, PromptEditMode,
    PromptHistorySearch, Reedline, ReedlineEvent, StyledText, Vi, default_vi_insert_keybindings,
    default_vi_normal_keybindings, default_vi_visual_keybindings,
};

use crate::limits::{Level, NEAR_BOUNDS_PASSED};
use crate::render::{BAR_COLOR, Output, Status, answer_label, marks, start_bar_row};
use crate::{Shared, lock};

// What Claude does stays in the background
const STATUS_COLOR: &str = "\x1b[38;5;244m";

// A limit near its end, before the red of a reached one
const ORANGE: &str = "\x1b[38;5;208m";
// A cold cache, and a limit reached
const RED: &str = "\x1b[31m";

// The host commands Enter and Ctrl-C send. reedline leaves the line of a host command in place,
// for the next input line to take the place of. Its own Enter would put the line in the
// scrollback at once, ahead of the end of the line Claude was writing, which came first, so the
// user's lines reach the scrollback only through the output, in the order Claude sees them
pub const SEND: &str = "send";

pub const CTRL_C: &str = "ctrl-c";

/// What the input line is for.
#[derive(Clone)]
pub enum Line {
    Message,
    /// The answer to what a tool asks, with the question.
    Answer(String),
    /// None, since the user has nothing more to say, while Claude finishes its work.
    Ending,
}

/// The prompt in front of the input line: for a message, `››` on a row above it, with the
/// context's level, marked and red on a cold cache, the levels of the plan's usage limits, and
/// what Claude does after them; for an answer, the question; at the end, the work Claude still
/// does.
pub struct Marker(Arc<Mutex<Shared>>);

impl Marker {
    pub fn new(shared: Arc<Mutex<Shared>>) -> Self {
        Marker(shared)
    }
}

impl Prompt for Marker {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        let shared = lock(&self.0);
        let label = match &shared.line {
            Line::Message => {
                let now = Instant::now();
                let level = shared.context.level();
                let is_cold = shared.context.is_cold(now);
                // On a cold cache, the space in front of T becomes *, which shows the cold cache
                // without color and keeps the label's width
                let (cold_mark, level_color) = if is_cold {
                    ("*", RED)
                } else {
                    (" ", STATUS_COLOR)
                };
                let indicators = format!(
                    "{}{level_color}{cold_mark}T{} {} {}",
                    marks(),
                    show_level(level),
                    limit_indicator('S', shared.limits.session_level()),
                    limit_indicator('W', shared.limits.week_level()),
                );
                let state = if shared.context.is_cold_confirmed(now) {
                    Some("cold, enter sends".to_string())
                } else {
                    describe_work(&shared.output)
                };
                let state =
                    state.map_or_else(String::new, |state| format!(" {STATUS_COLOR}│ {state}"));
                format!("{}{indicators}{state}", start_bar_row())
            }
            Line::Answer(question) => answer_label(question),
            Line::Ending => {
                let output = &shared.output;
                let tasks = output.background_tasks();
                let background =
                    (!tasks.is_empty()).then(|| format!("background: {}", tasks.join(", ")));
                let parts: Vec<String> = ["ending".to_string()]
                    .into_iter()
                    .chain(describe_work(output))
                    .chain(background)
                    .collect();
                format!(
                    "{}{} {STATUS_COLOR}{}",
                    start_bar_row(),
                    marks(),
                    parts.join(" │ ")
                )
            }
        };
        Cow::Owned(label)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        match lock(&self.0).line {
            // A message starts on a row of its own, so its lines copy back as they were written
            Line::Message => Cow::Owned(format!("\n{}", start_bar_row())),
            Line::Answer(_) => Cow::Borrowed(""),
            // The ending takes no input, and what the terminal shows after the client starts on
            // the row below
            Line::Ending => Cow::Borrowed("\n"),
        }
    }

    // reedline prints it at the start of each further row of the input
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Owned(start_bar_row())
    }

    fn render_prompt_history_search_indicator(
        &self,
        _history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Borrowed("search › ")
    }
}

/// The input line: vi keys, Enter and Ctrl-C as host commands, the bar, Ctrl-G for the editor,
/// and the lines of `output` printed above it.
pub fn create_editor(
    editor_command: String,
    break_signal: Arc<AtomicBool>,
    shared: Arc<Mutex<Shared>>,
) -> Reedline {
    let mut insert_keys = default_vi_insert_keybindings();
    let mut normal_keys = default_vi_normal_keybindings();
    let mut visual_keys = default_vi_visual_keybindings();
    for keys in [&mut insert_keys, &mut normal_keys, &mut visual_keys] {
        // In a history search, the first event takes the line found, as Enter does. Elsewhere it
        // completes a history hint, and with no hinter there is none, so Enter sends
        keys.add_binding(
            KeyModifiers::NONE,
            KeyCode::Enter,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::HistoryHintComplete,
                ReedlineEvent::ExecuteHostCommand(SEND.to_string()),
            ]),
        );
        keys.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('c'),
            ReedlineEvent::ExecuteHostCommand(CTRL_C.to_string()),
        );
        // Claude Code opens its prompt in the editor with Ctrl-G, where reedline takes Ctrl-O
        keys.remove_binding(KeyModifiers::CONTROL, KeyCode::Char('o'));
        keys.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('g'),
            ReedlineEvent::OpenEditor,
        );
    }
    Reedline::create()
        .with_edit_mode(Box::new(Vi::new(insert_keys, normal_keys, visual_keys)))
        // Like vim, the cursor of normal mode stays on its line, so `h` stops at the line's start
        // and leaving insert mode on an empty line keeps the cursor there
        .with_cross_line_cursor(false)
        .with_highlighter(Box::new(OnBar))
        .use_bracketed_paste(true)
        .with_buffer_editor(
            process::Command::new(editor_command),
            env::temp_dir().join(format!("claude-plain-{}.md", process::id())),
        )
        .with_break_signal(break_signal)
        .with_external_printer(Pending(shared))
}

/// A level, with ? for one not known yet.
fn show_level(level: Option<usize>) -> String {
    level.map_or_else(|| "?".to_string(), |level| level.to_string())
}

/// The letter of a usage limit and its level, orange from `NEAR_BOUNDS_PASSED` and red once
/// reached, so the color only repeats what the level says.
fn limit_indicator(letter: char, level: Option<Level>) -> String {
    let color = match level {
        None => STATUS_COLOR,
        Some(Level::Below(passed)) if passed < NEAR_BOUNDS_PASSED => STATUS_COLOR,
        Some(Level::Below(_)) => ORANGE,
        Some(Level::Reached) => RED,
    };
    format!("{color}{letter}{}", show_level(level.map(Level::to_number)))
}

/// What Claude does, none while it waits for the user.
fn describe_work(output: &Output) -> Option<String> {
    match output.status() {
        Status::Idle => None,
        Status::Working => Some("working".to_string()),
        Status::Writing if output.has_waiting_stderr() => {
            Some("writing, stderr waiting".to_string())
        }
        Status::Writing => Some("writing".to_string()),
        Status::Running(tool) => Some(format!("running {tool}")),
    }
}

/// Draws what the user types on the input line's bar.
struct OnBar;

impl Highlighter for OnBar {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        // reedline takes its styles from nu_ansi_term
        let bar = Style::new().on(Color::Fixed(BAR_COLOR));
        let mut styled = StyledText::new();
        styled.push((bar, line.to_string()));
        styled
    }
}

/// The lines of the output that wait for reedline, which prints them above the input line.
struct Pending(Arc<Mutex<Shared>>);

impl ExternalOutput for Pending {
    fn drain(&mut self) -> io::Result<Vec<String>> {
        Ok(lock(&self.0).output.take_lines())
    }
}
