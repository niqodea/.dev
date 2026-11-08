//! The bottom of the terminal: the label row, which shows the context, the usage limits and what
//! Claude does, and below it the input box, where the user types with vi keys. The conversation
//! scrolls above them, so the lines that leave the screen reach the terminal's scrollback whole.
//!
//! The label row and the box follow the conversation's end, as a shell's prompt follows the output,
//! down to the bottom of the terminal, where they stay once the conversation fills the rows above.
//! The box has a fixed number of rows and scrolls its text to keep the cursor in view, so it never
//! grows over the conversation. The conversation is only ever appended to.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossterm::cursor::{self, Hide, MoveTo, SetCursorStyle, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::terminal::{self, Clear, ClearType};
use edtui::actions::{DeleteWordBackward, InsertChar, LineBreak, RemoveChar, ReplaceChar};
use edtui::events::{KeyEventHandler, KeyEventRegister, KeyInput};
use edtui::{EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Index2, Lines};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::limits::{Level, NEAR_BOUNDS_PASSED};
use crate::render::{BAR_COLOR, Output, Status, answer_label, marks, start_bar_row};
use crate::{Shared, lock};

// What Claude does stays in the background
const STATUS_COLOR: &str = "\x1b[38;5;244m";

// A limit near its end, before the red of a reached one
const ORANGE: &str = "\x1b[38;5;208m";
// A cold cache, and a limit reached
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

/// The rows of the input box.
// TODO: Let the user choose the input box's height
const INPUT_ROWS: u16 = 3;

/// The rows the conversation keeps at least, since tmux ignores a scroll region of one row.
const MIN_CONVERSATION_ROWS: u16 = 2;

/// How long the input waits for a key before it looks again for output and a changed label.
const TICK: Duration = Duration::from_millis(25);

// The terminal shows a frame between the two at once
const BEGIN_FRAME: &str = "\x1b[?2026h";
const END_FRAME: &str = "\x1b[?2026l";

/// Writes crossterm's commands to `out`, a frame in memory, which writing cannot fail.
macro_rules! put {
    ($out:expr, $($command:expr),+ $(,)?) => {
        $({
            let mut text = String::new();
            // Formatting into a String never fails
            let _ = crossterm::Command::write_ansi(&$command, &mut text);
            $out.extend(text.as_bytes());
        })+
    };
}

/// What the input line is for.
#[derive(Clone)]
pub enum Line {
    Message,
    /// The answer to what a tool asks, with the question.
    Answer(String),
    /// None, since the user has nothing more to say, while Claude finishes its work.
    Ending,
}

/// How reading the input line ended, with the text on it.
pub enum Outcome {
    /// Enter.
    Send(String),
    /// Ctrl-C.
    Interrupt(String),
    /// Ctrl-D.
    End(String),
    /// Claude Code asked something, which the main thread acts on first.
    Break(String),
}

/// The bottom of the terminal, and where the conversation above it ends.
pub struct InputLine {
    shared: Arc<Mutex<Shared>>,
    break_signal: Arc<AtomicBool>,
    editor_command: String,
    state: EditorState,
    keys: EditorEventHandler,
    /// Whether the last key edtui read takes a character next, like `f`, which then goes to
    /// edtui whatever it is.
    takes_character: bool,
    /// Whether `r` waits for the character that replaces the one under the cursor.
    is_replacing: bool,
    layout: Layout,
    /// The first row of the text the box shows.
    scroll: u16,
    end: End,
    /// Lines that wait for a screen with the rows to scroll them.
    held: Vec<String>,
    /// What the last draw left above the cursor, which a resize moves before the client hears
    /// of it.
    drawn: Drawn,
    /// The label last drawn, which the input line redraws when it changes.
    label: String,
    is_closed: bool,
}

impl InputLine {
    /// The label row and the box, from the row the cursor is on, where the conversation starts.
    /// A screen with too few rows below the cursor scrolls up to make them.
    pub fn new(
        editor_command: String,
        break_signal: Arc<AtomicBool>,
        shared: Arc<Mutex<Shared>>,
    ) -> Result<Self, String> {
        let (width, height) = terminal::size().map_err(|error| cannot_read_terminal(&error))?;
        let (_, row) = cursor::position().map_err(|error| cannot_read_terminal(&error))?;
        let layout = Layout::new(width, height);
        let mut out = Vec::new();
        let lowest = layout.lowest_label_row();
        if row > lowest {
            scroll_screen(&mut out, layout, row - lowest);
        }
        put!(out, EnableBracketedPaste);
        write_out(&out)?;
        Ok(InputLine {
            shared,
            break_signal,
            editor_command,
            state: insert_at_end(""),
            keys: vi_keys(),
            takes_character: false,
            is_replacing: false,
            layout,
            scroll: 0,
            end: End::Before(row.min(lowest)),
            held: Vec::new(),
            drawn: Drawn::default(),
            label: String::new(),
            is_closed: false,
        })
    }

    /// Reads the input line, starting from `draft`, while the conversation goes on above it.
    pub fn read(&mut self, line: Line, draft: &str) -> Result<Outcome, String> {
        lock(&self.shared).line = line;
        if self.text() != draft {
            self.state = insert_at_end(draft);
        }
        self.draw()?;
        loop {
            if self.break_signal.load(Ordering::Relaxed) {
                return Ok(Outcome::Break(self.text()));
            }
            self.print_output()?;
            if self.make_label() != self.label {
                self.draw()?;
            }
            if !event::poll(TICK).map_err(|error| cannot_read_terminal(&error))? {
                continue;
            }
            match event::read().map_err(|error| cannot_read_terminal(&error))? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(outcome) = self.key(key)? {
                        return Ok(outcome);
                    }
                }
                Event::Paste(text) if self.takes_text() => self.paste(&text),
                Event::Resize(width, height) => self.resize(width, height)?,
                _ => continue,
            }
            self.draw()?;
        }
    }

    /// Takes the bottom of the terminal away, once the session ended, and leaves the cursor at
    /// the start of the row below the conversation, where the end of the output follows.
    pub fn close(&mut self) -> Result<(), String> {
        if self.is_closed {
            return Ok(());
        }
        self.is_closed = true;
        let mut out = Vec::new();
        put!(
            out,
            MoveTo(0, self.label_row()),
            Clear(ClearType::FromCursorDown),
            SetCursorStyle::DefaultUserShape,
            DisableBracketedPaste,
        );
        put!(out, MoveTo(0, self.end.row()));
        if let End::On(_) = self.end {
            out.extend(b"\r\n");
        }
        for line in std::mem::take(&mut self.held) {
            out.extend(format!("{line}\r\n").as_bytes());
        }
        put!(out, Show);
        write_out(&out)
    }

    fn text(&self) -> String {
        self.state.lines.to_string()
    }

    /// The row of the label: right below the conversation's end, down to the lowest row the
    /// label row and the box fit above.
    fn label_row(&self) -> u16 {
        self.end.next_row().min(self.layout.lowest_label_row())
    }

    /// Whether the input line takes what the user types, which the ending has no one to read.
    fn takes_text(&self) -> bool {
        !matches!(lock(&self.shared).line, Line::Ending)
    }

    /// Acts on a key, with an outcome for one that ends the reading.
    fn key(&mut self, key: KeyEvent) -> Result<Option<Outcome>, String> {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.is_replacing {
            self.is_replacing = false;
            match key.code {
                KeyCode::Char(character) if !control => self.state.execute(ReplaceChar(character)),
                KeyCode::Enter => self.break_line_at_cursor(),
                _ => {}
            }
            return Ok(None);
        }
        match key.code {
            KeyCode::Char('c') if control => return Ok(Some(Outcome::Interrupt(self.text()))),
            KeyCode::Char('d') if control => return Ok(Some(Outcome::End(self.text()))),
            KeyCode::Enter => return Ok(Some(Outcome::Send(self.text()))),
            _ => {}
        }
        if !self.takes_text() {
            return Ok(None);
        }
        let is_normal = self.state.mode == EditorMode::Normal && !self.takes_character;
        match key.code {
            KeyCode::Char(':') if is_normal => self.edit_in_editor()?,
            KeyCode::Char('r') if is_normal && !control => self.is_replacing = true,
            _ => {
                let mode = self.state.mode;
                self.takes_character = !self.takes_character
                    && matches!(mode, EditorMode::Normal | EditorMode::Visual)
                    && matches!(key.code, KeyCode::Char('f' | 't' | 'F' | 'T'));
                self.keys.on_key_event(key, &mut self.state);
                return Ok(None);
            }
        }
        // A key the client acts on itself ends any sequence edtui was in the middle of
        self.keys = vi_keys();
        self.takes_character = false;
        Ok(None)
    }

    /// Replaces the character under the cursor with a line break, as `r` and Enter do in vim.
    fn break_line_at_cursor(&mut self) {
        let row = self.state.cursor.row;
        let length = self.state.lines.len_col(row).unwrap_or(0);
        if length == 0 {
            return;
        }
        let is_last = self.state.cursor.col + 1 >= length;
        self.state.execute(RemoveChar(1));
        if is_last {
            self.state.cursor = Index2::new(row, length - 1);
        }
        self.state.execute(LineBreak(1));
    }

    /// Inserts pasted text at the cursor, as typed, in normal mode too.
    fn paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.state.mode != EditorMode::Insert {
            self.state.mode = EditorMode::Insert;
        }
        for character in text.chars() {
            self.state.execute(InsertChar(character));
        }
    }

    /// Opens the text in the user's editor, and takes back what it saved, to add to or send.
    fn edit_in_editor(&mut self) -> Result<(), String> {
        let path = env::temp_dir().join(format!("claude-plain-{}.md", process::id()));
        fs::write(&path, self.text())
            .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;
        let mut out = Vec::new();
        put!(out, DisableBracketedPaste, SetCursorStyle::DefaultUserShape);
        write_out(&out)?;
        let status = process::Command::new(&self.editor_command)
            .arg(&path)
            .status()
            .map_err(|error| format!("Cannot run {}: {error}", self.editor_command))?;
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
        // The file goes with the draft, which the input line now holds
        let _ = fs::remove_file(&path);
        let mut out = Vec::new();
        put!(out, EnableBracketedPaste);
        write_out(&out)?;
        if status.success() {
            // Editors end a file with a line break, which the message does not have
            let text = text.strip_suffix('\n').unwrap_or(&text);
            self.state = insert_at_end(text);
        }
        Ok(())
    }

    /// Prints the conversation's new lines above the label row, in a scroll region over the
    /// conversation's rows, so the lines that leave the screen go to the scrollback.
    fn print_output(&mut self) -> Result<(), String> {
        let lines = lock(&self.shared).output.take_lines();
        self.held.extend(lines);
        if self.held.is_empty() || !self.layout.shows_conversation() {
            return Ok(());
        }
        let mut out = Vec::new();
        out.extend(BEGIN_FRAME.as_bytes());
        // The lines can take the rows of the label and the box, which follow them once drawn again
        put!(
            out,
            Hide,
            MoveTo(0, self.label_row()),
            Clear(ClearType::FromCursorDown)
        );
        // Setting the region puts the cursor home, so the moves follow it
        let conversation_rows = self.layout.conversation_rows();
        out.extend(format!("\x1b[1;{conversation_rows}r").as_bytes());
        let (row, mut is_first) = match self.end {
            End::Before(row) if row < conversation_rows => (row, true),
            // The label was at the bottom, below the region, so the first line scrolls the
            // region's rows up and starts its last row
            End::Before(_) => (conversation_rows - 1, false),
            End::On(row) => (row, false),
        };
        put!(out, MoveTo(0, row));
        for line in std::mem::take(&mut self.held) {
            for part in line.split('\n') {
                if !is_first {
                    out.extend(b"\r\n");
                }
                is_first = false;
                out.extend(part.as_bytes());
            }
        }
        write_out(&out)?;
        // The cursor is where the last line ends: past its last character, or on it when the
        // line ends at the row's end
        let (_, row) = cursor::position().map_err(|error| cannot_read_terminal(&error))?;
        self.end = End::On(row);
        // The frame ends with the label and the box drawn below the lines
        write_out(b"\x1b[r")?;
        self.draw()
    }

    /// Draws the label row and the input box, with the cursor in the box.
    fn draw(&mut self) -> Result<(), String> {
        self.label = self.make_label();
        let layout = self.layout;
        let mut out = Vec::new();
        out.extend(BEGIN_FRAME.as_bytes());
        put!(out, Hide);
        let label_row = self.label_row();
        let mut widths = Vec::new();
        if layout.label_rows > 0 {
            put!(out, MoveTo(0, label_row));
            out.extend(format!("{}{}{RESET}", start_bar_row(), self.label).as_bytes());
            widths.push(visible_width(&self.label));
        }
        // edtui scrolls by whole lines, which a line taller than the box defeats, so it draws the
        // whole text, and the box shows the rows around the cursor
        let area = Rect::new(0, 0, layout.width, text_rows(&self.text(), layout.width));
        let mut buffer = Buffer::empty(area);
        let base = Style::default().bg(Color::Indexed(BAR_COLOR));
        self.state.set_viewport_offset(0, 0);
        EditorView::new(&mut self.state)
            .theme(
                EditorTheme::default()
                    .base(base)
                    .selection_style(base.add_modifier(Modifier::REVERSED))
                    .hide_cursor()
                    .hide_status_line(),
            )
            .wrap(true)
            .render(area, &mut buffer);
        let cursor = self.state.cursor_screen_position().unwrap_or_default();
        let used_rows = (0..area.height)
            .rev()
            .find(|&y| row_text(&buffer, y, layout.width).1 > 0)
            .map_or(0, |last| last + 1);
        // The box scrolls as little as keeps the cursor in it, and shows no empty rows below the
        // text that the text above could fill
        let rows = layout.input_rows;
        self.scroll = self
            .scroll
            .min(used_rows.max(cursor.y + 1).saturating_sub(rows))
            .min(cursor.y)
            .max((cursor.y + 1).saturating_sub(rows));
        let mut row_widths = Vec::new();
        for y in 0..rows {
            let row = label_row + layout.label_rows + y;
            if row >= layout.height {
                break;
            }
            put!(out, MoveTo(0, row));
            let (text, width) = if self.scroll + y < area.height {
                row_text(&buffer, self.scroll + y, layout.width)
            } else {
                (String::new(), 0)
            };
            out.extend(format!("{}{text}{RESET}", start_bar_row()).as_bytes());
            row_widths.push(width);
        }
        let cursor_in_box = cursor.y - self.scroll;
        let cursor_row =
            (label_row + layout.label_rows + cursor_in_box).min(layout.height.saturating_sub(1));
        widths.extend(row_widths.iter().take(usize::from(cursor_in_box)));
        self.drawn = Drawn {
            widths_above_cursor: widths,
            cursor_column: cursor.x,
        };
        let shape = match self.state.mode {
            EditorMode::Insert => SetCursorStyle::SteadyBar,
            _ => SetCursorStyle::SteadyBlock,
        };
        put!(out, MoveTo(cursor.x, cursor_row), shape, Show);
        out.extend(END_FRAME.as_bytes());
        write_out(&out)
    }

    /// Puts the bottom of the terminal back in place after a resize. The terminal moved it
    /// with the cursor before the client heard of the resize: a narrower screen splits each of
    /// its rows wider than the screen, and a taller one can leave it in the middle.
    ///
    /// A screen shrunk to no more rows than the label row and the box's rows above the cursor
    /// takes those rows into the scrollback, where copies of them stay.
    fn resize(&mut self, width: u16, height: u16) -> Result<(), String> {
        let (_, cursor_row) = cursor::position().map_err(|error| cannot_read_terminal(&error))?;
        let width_now = usize::from(width.max(1));
        let rows_above: usize = self
            .drawn
            .widths_above_cursor
            .iter()
            .map(|width| width.div_ceil(width_now).max(1))
            .sum::<usize>()
            + usize::from(self.drawn.cursor_column) / width_now;
        let label_row = cursor_row.saturating_sub(u16::try_from(rows_above).unwrap_or(u16::MAX));
        let mut out = Vec::new();
        put!(out, MoveTo(0, label_row), Clear(ClearType::FromCursorDown));
        self.layout = Layout::new(width, height);
        // The label row is right below the conversation's end
        self.end = match self.end {
            End::Before(_) => End::Before(label_row),
            End::On(_) => label_row.checked_sub(1).map_or(End::Before(0), End::On),
        };
        // A shorter screen has the conversation reach into the rows the label row and the box
        // take, so it scrolls up
        let lowest = self.layout.lowest_label_row();
        if self.end.next_row() > lowest {
            let rows = self.end.next_row() - lowest;
            scroll_screen(&mut out, self.layout, rows);
            self.end = match self.end {
                End::Before(row) => End::Before(row - rows),
                End::On(row) => End::On(row - rows),
            };
        }
        write_out(&out)
    }

    /// The label row: for a message, `››` with the context's level, marked and red on a cold
    /// cache, the levels of the plan's usage limits, and what Claude does after them; for an
    /// answer, the question; at the end, the work Claude still does.
    fn make_label(&self) -> String {
        let shared = lock(&self.shared);
        match &shared.line {
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
                format!("{indicators}{state}")
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
                format!("{} {STATUS_COLOR}{}", marks(), parts.join(" │ "))
            }
        }
    }
}

impl Drop for InputLine {
    /// Gives the terminal back as it was, for a session that ended without closing the input
    /// line, on an error.
    fn drop(&mut self) {
        // The terminal is being left, with nowhere left to tell a failure
        let _ = self.close();
    }
}

/// Where the parts of the screen are, for a terminal of `width` and `height`.
#[derive(Clone, Copy)]
struct Layout {
    width: u16,
    height: u16,
    /// One for the label row, none on a screen of one row, which only the row typed in fits.
    label_rows: u16,
    input_rows: u16,
}

impl Layout {
    fn new(width: u16, height: u16) -> Self {
        // A short screen keeps a row to type in first, then the label row, then the
        // conversation's rows, and takes the rest from the box
        let label_rows = u16::from(height >= 2);
        let input_rows = INPUT_ROWS
            .min(height.saturating_sub(MIN_CONVERSATION_ROWS + label_rows))
            .max(1);
        Layout {
            width: width.max(1),
            height,
            label_rows,
            input_rows,
        }
    }

    /// The rows above the lowest label row, where the conversation scrolls.
    fn conversation_rows(self) -> u16 {
        self.height
            .saturating_sub(self.input_rows + self.label_rows)
    }

    /// The row of the label once the conversation reaches the bottom of the terminal, with the
    /// box below it.
    fn lowest_label_row(self) -> u16 {
        self.conversation_rows()
    }

    /// Whether the conversation has the rows to scroll in. On a shorter screen its lines wait.
    fn shows_conversation(self) -> bool {
        self.conversation_rows() >= MIN_CONVERSATION_ROWS
    }
}

/// Where the conversation ends on screen, which its next line follows.
#[derive(Clone, Copy)]
enum End {
    /// Nothing printed yet: the first line goes on this row, an empty one.
    Before(u16),
    /// The last line printed ends on this row.
    On(u16),
}

impl End {
    fn row(self) -> u16 {
        match self {
            End::Before(row) | End::On(row) => row,
        }
    }

    /// The first row below the conversation.
    fn next_row(self) -> u16 {
        match self {
            End::Before(row) => row,
            End::On(row) => row + 1,
        }
    }
}

/// What the last draw left above the cursor.
#[derive(Default)]
struct Drawn {
    /// The widths of the text on the label row and on the box's rows above the cursor, which
    /// a narrower screen splits into the rows that text then takes.
    widths_above_cursor: Vec<usize>,
    cursor_column: u16,
}

/// edtui's vi keys, with Ctrl-W in insert mode deleting the word before the cursor, as in vim.
fn vi_keys() -> EditorEventHandler {
    let mut keys = KeyEventHandler::vim_mode();
    keys.insert(
        KeyEventRegister::i(vec![KeyInput::ctrl('w')]),
        DeleteWordBackward(1),
    );
    EditorEventHandler::new(keys)
}

/// A state holding `text`, in insert mode with the cursor at its end, where typing goes on.
fn insert_at_end(text: &str) -> EditorState {
    let mut state = EditorState::new(Lines::from(text));
    let row = state.lines.len().saturating_sub(1);
    let column = state.lines.len_col(row).unwrap_or(0);
    state.cursor = Index2::new(row, column);
    state.mode = EditorMode::Insert;
    state
}

/// The rows `text` takes at `width`, at most: a row loses a column at most to a double-width
/// character that does not fit at its end, and the cursor can rest past a full last row.
fn text_rows(text: &str, width: u16) -> u16 {
    let columns = usize::from(width.saturating_sub(1).max(1));
    let rows: usize = text
        .split('\n')
        .map(|line| line.width().div_ceil(columns).max(1))
        .sum();
    u16::try_from(rows + 1).unwrap_or(u16::MAX)
}

/// The row `y` of `buffer` as text with its styles, without the empty cells at its end, and
/// the width of that text.
fn row_text(buffer: &Buffer, y: u16, width: u16) -> (String, usize) {
    let cells: Vec<_> = (0..width).map(|x| &buffer[(x, y)]).collect();
    let used = cells
        .iter()
        .rposition(|cell| cell.symbol() != " " || cell.modifier.contains(Modifier::REVERSED))
        .map_or(0, |last| last + 1);
    let mut text = String::new();
    let mut style = None;
    for cell in &cells[..used] {
        let cell_style = (cell.fg, cell.modifier);
        if style != Some(cell_style) {
            text.push_str(&sgr(cell.fg, cell.modifier));
            style = Some(cell_style);
        }
        text.push_str(cell.symbol());
    }
    (text, used)
}

/// The escape sequence for a cell's foreground and modifiers, on the bar.
fn sgr(foreground: Color, modifier: Modifier) -> String {
    let mut codes = vec!["0".to_string(), format!("48;5;{BAR_COLOR}")];
    match foreground {
        Color::Reset => {}
        Color::Indexed(index) => codes.push(format!("38;5;{index}")),
        Color::Rgb(red, green, blue) => codes.push(format!("38;2;{red};{green};{blue}")),
        named => codes.push(named_foreground(named).to_string()),
    }
    for (flag, code) in [
        (Modifier::BOLD, "1"),
        (Modifier::DIM, "2"),
        (Modifier::ITALIC, "3"),
        (Modifier::UNDERLINED, "4"),
        (Modifier::REVERSED, "7"),
    ] {
        if modifier.contains(flag) {
            codes.push(code.to_string());
        }
    }
    format!("\x1b[{}m", codes.join(";"))
}

fn named_foreground(color: Color) -> u8 {
    match color {
        Color::Black => 30,
        Color::Red => 31,
        Color::Green => 32,
        Color::Yellow => 33,
        Color::Blue => 34,
        Color::Magenta => 35,
        Color::Cyan => 36,
        Color::Gray => 37,
        Color::DarkGray => 90,
        Color::LightRed => 91,
        Color::LightGreen => 92,
        Color::LightYellow => 93,
        Color::LightBlue => 94,
        Color::LightMagenta => 95,
        Color::LightCyan => 96,
        _ => 97,
    }
}

/// The columns `text` takes on screen, without its escape sequences.
fn visible_width(text: &str) -> usize {
    let mut visible = String::new();
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character == '\x1b' {
            // Every sequence the label holds is a CSI, which ends with a letter
            for next in characters.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            visible.push(character);
        }
    }
    visible.width()
}

/// Scrolls the whole screen up by `rows`, which sends its top rows to the scrollback.
fn scroll_screen(out: &mut Vec<u8>, layout: Layout, rows: u16) {
    put!(out, MoveTo(0, layout.height.saturating_sub(1)));
    for _ in 0..rows {
        out.extend(b"\n");
    }
}

fn write_out(out: &[u8]) -> Result<(), String> {
    let mut stdout = io::stdout();
    stdout
        .write_all(out)
        .and_then(|()| stdout.flush())
        .map_err(|error| format!("Cannot write to the terminal: {error}"))
}

fn cannot_read_terminal(error: &io::Error) -> String {
    format!("Cannot read the terminal: {error}")
}

/// The last line of the session, on the bar, with the session's id once Claude Code told it.
pub fn final_line(session_id: Option<&str>) -> String {
    let session = session_id.map_or_else(String::new, |id| format!(" │ session {id}"));
    format!(
        "{}{} {STATUS_COLOR}ended{session}{RESET}",
        start_bar_row(),
        marks()
    )
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
