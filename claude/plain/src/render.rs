//! The conversation as the terminal shows it: plain lines, appended and never redrawn, so the
//! terminal wraps them to its width and copies them whole.
//!
//! The input line stays open below the conversation, so the user can type at any time, and the
//! conversation reaches the terminal above it a whole line at a time. A line shown before it
//! ended would have to be rewritten in place as the rest of it arrives, and nothing above the
//! input line is ever rewritten. Claude writes each paragraph as one line, so its prose appears
//! a paragraph at a time, and the input line's label says what Claude does meanwhile.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::markdown::Highlighter;
use crate::protocol::{Ask, AskKind, SettingsError, State, ToolResult, ToolUse};

const GREEN: &str = "\x1b[32m";
// The marks of the user's lines
const LIGHT_CYAN: &str = "\x1b[96m";
const YELLOW: &str = "\x1b[33m";
// Only for failures, since red reads as an error
const RED: &str = "\x1b[31m";
// For what Claude did, which supports what Claude wrote
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
/// The background of the user's lines, which sets them apart from Claude's.
pub const BAR_COLOR: u8 = 236;
// Erases to the end of the row in the background color that is set, which fills the row's
// rest with the bar without writing spaces into the text
const FILL_ROW: &str = "\x1b[K";

// Past this, a tool's input in JSON is noise to read through
const INPUT_SUMMARY_CHARACTERS: usize = 200;

/// The field that sums up a call of each tool that has one.
const SUMMARY_FIELDS: [(&str, &str); 10] = [
    ("Agent", "description"),
    ("Bash", "command"),
    ("Edit", "file_path"),
    // Its ask shows the plan
    ("ExitPlanMode", "planFilePath"),
    ("Glob", "pattern"),
    ("Grep", "pattern"),
    ("Read", "file_path"),
    ("WebFetch", "url"),
    ("WebSearch", "query"),
    ("Write", "file_path"),
];

/// The conversation's lines that wait for reedline to print them above the input line, and
/// what Claude does meanwhile.
pub struct Output {
    lines: Vec<String>,
    /// The start of a line of Claude's reply, which waits for its end.
    partial: String,
    position: Position,
    /// What sets the next line apart from the last one. It is written with that line, so
    /// nothing the user sends in between can land between the two.
    pending_break: Option<Break>,
    status: Status,
    /// The tool uses whose result repeats what the screen already shows.
    hidden_results: HashSet<String>,
    /// Each tool use that waits for its result. Claude can run several tools at once, so a
    /// result can come after a line of another tool.
    pending_calls: HashMap<String, PendingCall>,
    /// The tag of the next tool use.
    next_tag: u8,
    /// How the user stopped the turn that runs, if they did.
    interruption: Option<Interruption>,
    highlighter: Highlighter,
    /// The lines of Claude Code's error output that wait for Claude to finish the text it
    /// writes.
    waiting_stderr: Vec<String>,
    /// What each task that runs in the background does.
    background_tasks: Vec<String>,
}

/// A tool use that waits for its result, whose line names the tool and repeats the tag.
struct PendingCall {
    name: String,
    /// What ties the result's line to the call's line: two hexadecimal digits, which keep the
    /// columns of both lines in place. Tags count up and wrap from ff to 00, on the assumption
    /// that a call's result arrives before 255 more calls start. A call that waits longer
    /// shares its tag with a newer call.
    tag: u8,
}

/// What Claude does, which the marker in front of the input line shows.
#[derive(Clone, PartialEq)]
pub enum Status {
    /// Claude waits for the user's next message.
    Idle,
    Working,
    Writing,
    /// Claude runs the tool with this name.
    Running(String),
}

/// How the user stopped a turn.
#[derive(Clone, Copy)]
pub enum Interruption {
    /// With Ctrl-C, or by refusing a tool, which the turn's end then says.
    Stop,
    /// With a message, which says it already. The turn may instead move its work to the
    /// background and end without being stopped.
    Message,
}

/// Where the conversation stands, which decides what sets the next line apart.
#[derive(Clone, Copy, PartialEq)]
enum Position {
    /// Before the first line, which nothing comes before.
    Start,
    /// In a section, which the next line continues when it belongs to the same section.
    In(Section),
    /// After the end of a turn, which nothing continues.
    TurnEnd,
}

/// What sets a section apart from the one before.
#[derive(Clone, Copy)]
enum Break {
    /// The mark that starts Claude's reply to the user's messages.
    Reply,
    Blank,
}

/// A part of the conversation, set apart from the next part.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Message,
    Text,
    Tools,
    Notice,
}

impl Output {
    pub fn new() -> Self {
        Output {
            lines: Vec::new(),
            partial: String::new(),
            position: Position::Start,
            pending_break: None,
            status: Status::Idle,
            hidden_results: HashSet::new(),
            pending_calls: HashMap::new(),
            next_tag: 0,
            interruption: None,
            highlighter: Highlighter::new(),
            waiting_stderr: Vec::new(),
            background_tasks: Vec::new(),
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    pub fn background_tasks(&self) -> &[String] {
        &self.background_tasks
    }

    pub fn note_background_tasks(&mut self, tasks: Vec<String>) {
        self.background_tasks = tasks;
    }

    /// Takes the lines that wait to be printed above the input line.
    pub fn take_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    /// Shows the message the user sends, after all that Claude wrote before it, so the
    /// conversation reads in the order Claude sees it.
    pub fn show_message(&mut self, text: &str) {
        // Claude stops mid-line when the message interrupts it, and keeps the line as far as it
        // got
        if !self.partial.is_empty() {
            let line = std::mem::take(&mut self.partial);
            let colored = self.highlighter.color_line(&line);
            self.push_all(colored);
        }
        // A message follows the line before it, and starts what the reply mark then introduces
        self.pending_break = None;
        self.lines
            .push(format!("{}{}{RESET}", start_bar_row(), marks()));
        for line in text.split('\n') {
            self.lines.push(format!("{}{line}{RESET}", start_bar_row()));
        }
        self.position = Position::In(Section::Message);
        self.set_status(Status::Working);
        // An interrupt sent as its turn ended had no turn left to stop
        self.interruption = None;
    }

    /// Shows the user's answer to what a tool asks, after the question.
    pub fn show_answer(&mut self, question: &str, text: &str) {
        self.push(format!("{}{text}{RESET}", answer_label(question)));
    }

    /// Notes that the user stopped the turn, whose end then says so in the user's terms.
    pub fn note_interrupt(&mut self, interruption: Interruption) {
        self.interruption = Some(interruption);
    }

    /// Hides the result of a tool the user refused, which repeats the refusal.
    pub fn hide_result(&mut self, tool_use_id: &str) {
        self.hidden_results.insert(tool_use_id.to_string());
    }

    pub fn note_state(&mut self, state: State) {
        match state {
            State::Idle => self.set_status(Status::Idle),
            // What Claude does in its turn says more, once it is known
            State::Running if self.status == Status::Idle => self.set_status(Status::Working),
            // The input line asks the user, which says more than any label
            State::Running | State::RequiresAction => {}
        }
    }

    pub fn show_text(&mut self, text: &str) {
        self.begin(Section::Text);
        self.set_status(Status::Writing);
        self.partial.push_str(text);
        while let Some(end) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=end).collect();
            let colored = self.highlighter.color_line(line.trim_end_matches('\n'));
            self.push_all(colored);
        }
    }

    pub fn show_text_end(&mut self) {
        self.end_text();
        self.set_status(Status::Working);
    }

    /// Ends the conversation, since Claude Code ended. The text Claude was writing ends where it
    /// stopped, the error output that waited for it follows, and then how Claude Code exited,
    /// when it says more than that it exited with status 0. The line states it and leaves to
    /// the user what it means, since a status that is not 0 can come from the user too, like
    /// from quitting while Claude works.
    pub fn show_end(&mut self, exit: Option<&str>) {
        self.end_text();
        self.set_status(Status::Idle);
        if let Some(exit) = exit {
            self.begin(Section::Notice);
            self.push(format!("{DIM}{exit}{RESET}"));
        }
    }

    /// Adds the rest of the text Claude wrote, which its last line and the lines the
    /// highlighter holds wait in.
    fn end_text(&mut self) {
        if !self.partial.is_empty() {
            let line = std::mem::take(&mut self.partial);
            let colored = self.highlighter.color_line(&line);
            self.push_all(colored);
        }
        let held = self.highlighter.finish();
        self.push_all(held);
    }

    pub fn show_tool_use(&mut self, tool_use: &ToolUse) {
        if !tool_use.is_subagent {
            self.set_status(Status::Running(tool_use.name.clone()));
        }
        // Its ask shows the questions, and the input line the answers
        if tool_use.name == "AskUserQuestion" {
            self.hidden_results.insert(tool_use.id.clone());
            return;
        }
        // What a subagent did shows in what it reports back to Claude, so its calls have no
        // result line to tie to
        let mark = if tool_use.is_subagent {
            format!("  {BOLD}●")
        } else {
            let tag = self.next_tag;
            self.next_tag = tag.wrapping_add(1);
            self.pending_calls.insert(
                tool_use.id.clone(),
                PendingCall {
                    name: tool_use.name.clone(),
                    tag,
                },
            );
            format!("{BOLD}●{RESET} {DIM}#{tag:02x}{RESET}{BOLD}")
        };
        self.begin(Section::Tools);
        let indent = if tool_use.is_subagent { "  " } else { "" };
        let summary = summarize(&tool_use.name, &tool_use.input);
        let mut summary_lines = summary.lines();
        let first = summary_lines.next().unwrap_or_default();
        self.push(format!("{mark} {}{RESET} {first}", tool_use.name));
        for line in summary_lines {
            self.push(format!("{indent}  {line}"));
        }
        if tool_use.name == "Edit" {
            let removed = read_field(&tool_use.input, "old_string");
            let added = read_field(&tool_use.input, "new_string");
            for line in removed.lines() {
                self.push(format!("{indent}  {RED}- {line}{RESET}"));
            }
            for line in added.lines() {
                self.push(format!("{indent}  {GREEN}+ {line}{RESET}"));
            }
        }
    }

    pub fn show_tool_result(&mut self, result: &ToolResult) {
        if !result.is_subagent {
            self.set_status(Status::Working);
        }
        let call = self.pending_calls.remove(&result.tool_use_id);
        // What a subagent did shows in what it reports back to Claude. A tool the interrupt
        // stopped fails with Claude Code's message to Claude.
        if result.is_subagent
            || self.hidden_results.remove(&result.tool_use_id)
            || (self.interruption.is_some() && result.is_error)
        {
            return;
        }
        self.begin(Section::Tools);
        // The outcome is all the line adds to the call's line, so the rest stays in the back
        let outcome = if result.is_error {
            format!("{RED}✗")
        } else {
            format!("{GREEN}✓")
        };
        let text = result.text.as_deref().map_or("(no text)", str::trim);
        let mut lines = text.lines();
        let first = lines.next().unwrap_or("(no output)");
        let more = match lines.count() {
            0 => String::new(),
            count => format!(" (+{count} lines)"),
        };
        // Other lines can come between a call and its result, so the result stands on its own
        let label = call.map_or_else(String::new, |call| {
            format!("#{:02x} {}: ", call.tag, call.name)
        });
        self.push(format!("{outcome}{RESET} {DIM}{label}{first}{more}{RESET}"));
    }

    pub fn show_ask(&mut self, ask: &Ask) {
        self.begin(Section::Tools);
        match &ask.kind {
            AskKind::Permission { reason, .. } => {
                if let Some(reason) = reason {
                    self.push(format!("{YELLOW}{}{RESET}", reason.trim()));
                }
                // The tool's line shows only the start of an input it has no summary for
                if !SUMMARY_FIELDS
                    .iter()
                    .any(|(tool, _)| *tool == ask.tool_name)
                {
                    let input = serde_json::to_string_pretty(&ask.input)
                        .expect("a JSON object turns back into JSON");
                    for line in input.lines() {
                        self.push(format!("{DIM}{line}{RESET}"));
                    }
                }
            }
            AskKind::Plan(plan) => {
                let mut highlighter = Highlighter::new();
                for line in plan.trim().lines() {
                    let colored = highlighter.color_line(line);
                    self.push_all(colored);
                }
                let held = highlighter.finish();
                self.push_all(held);
            }
            AskKind::Questions(questions) => {
                for question in questions {
                    self.push(format!(
                        "{BOLD}{}{RESET} {}",
                        question.header, question.text
                    ));
                    for (number, choice) in (1..).zip(&question.options) {
                        self.push(format!(
                            "  {number}. {} {DIM}{}{RESET}",
                            choice.label, choice.description
                        ));
                        if let Some(preview) = &choice.preview {
                            for line in preview.lines() {
                                self.push(format!("     {DIM}{line}{RESET}"));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Shows how a turn ended, when it ended before its work was done. Claude Code ends a turn
    /// the user stopped as one that failed, like any other, so the line says it in the user's
    /// terms.
    pub fn show_turn_end(&mut self, failure: Option<&str>) {
        // A message makes the turn either stop or move its work to the background and end well
        let line = match (failure, self.interruption.take()) {
            (None, _) | (Some(_), Some(Interruption::Message)) => None,
            (Some(_), Some(Interruption::Stop)) => Some("Interrupted".to_string()),
            (Some(reason), None) => Some(format!("Turn ended: {reason}")),
        };
        if let Some(line) = line {
            self.begin(Section::Notice);
            self.push(format!("{DIM}{line}{RESET}"));
        }
        // A message that waits for its reply still waits when an older turn ends
        if self.position != Position::In(Section::Message) {
            self.position = Position::TurnEnd;
        }
    }

    pub fn show_settings_errors(&mut self, errors: &[SettingsError]) {
        self.begin(Section::Notice);
        for error in errors {
            let file = error.file.as_deref().unwrap_or("A settings file");
            self.push(format!(
                "{RED}Skipped {file}, so none of its settings apply: {} {}{RESET}",
                error.path, error.message
            ));
        }
    }

    pub fn show_error(&mut self, error: &str) {
        self.begin(Section::Notice);
        self.push(format!("{RED}{error}{RESET}"));
    }

    /// Shows a line of Claude Code's error output. Claude's text has nothing that names it, so
    /// the line waits until Claude finishes writing and the paragraphs stay whole. Every other
    /// line says what it is, like a tool's result, which names its tool, so the line goes in at
    /// once, between a tool's call and its result too.
    pub fn show_stderr(&mut self, line: &str) {
        if self.status == Status::Writing {
            self.waiting_stderr.push(line.to_string());
        } else {
            self.push_stderr(line);
        }
    }

    pub fn has_waiting_stderr(&self) -> bool {
        !self.waiting_stderr.is_empty()
    }

    fn push_stderr(&mut self, line: &str) {
        self.begin(Section::Notice);
        self.push(format!("{DIM}stderr │{RESET} {RED}{line}{RESET}"));
    }

    /// Sets what Claude does. Once Claude stops writing, the lines of error output that waited
    /// follow its text.
    fn set_status(&mut self, status: Status) {
        if self.status == Status::Writing && status != Status::Writing {
            for line in std::mem::take(&mut self.waiting_stderr) {
                self.push_stderr(&line);
            }
        }
        self.status = status;
    }

    /// Starts a section, set apart by the next line written: Claude's reply from the user's
    /// messages with a mark, which tells them apart in copied text too, where colors are gone,
    /// and another section, or what follows the end of a turn, with a blank line.
    fn begin(&mut self, section: Section) {
        let pending_break = match (self.pending_break, self.position) {
            // A reply that printed nothing yet still starts with its mark
            (Some(Break::Reply), _) | (_, Position::In(Section::Message)) => Some(Break::Reply),
            (_, Position::TurnEnd) => Some(Break::Blank),
            (_, Position::In(current)) if current != section => Some(Break::Blank),
            (pending_break, Position::Start | Position::In(_)) => pending_break,
        };
        self.pending_break = pending_break;
        self.position = Position::In(section);
    }

    /// Adds a line, after the break that sets it apart, if one waits.
    fn push(&mut self, line: String) {
        match self.pending_break.take() {
            Some(Break::Reply) => self.lines.push(format!("{DIM}‹‹{RESET}")),
            Some(Break::Blank) => self.lines.push(String::new()),
            None => {}
        }
        self.lines.push(line);
    }

    fn push_all(&mut self, lines: Vec<String>) {
        for line in lines {
            self.push(line);
        }
    }
}

/// The start of a row on the bar, which fills the whole row.
pub fn start_bar_row() -> String {
    format!("\x1b[48;5;{BAR_COLOR}m{FILL_ROW}")
}

/// The marks in front of a message, on a row of their own.
pub fn marks() -> String {
    format!("{LIGHT_CYAN}››")
}

/// The question in front of an answer, on the bar.
pub fn answer_label(question: &str) -> String {
    let bar = start_bar_row();
    format!("{bar}{YELLOW}{question}{RESET}{bar} {LIGHT_CYAN}›{RESET}{bar} ")
}

/// The input's summary field for a tool that has one, and its start in JSON otherwise.
fn summarize(tool: &str, input: &Map<String, Value>) -> String {
    if let Some((_, field)) = SUMMARY_FIELDS.iter().find(|(name, _)| *name == tool) {
        return read_field(input, field).to_string();
    }
    let json = serde_json::to_string(input).expect("a JSON object turns back into JSON");
    match json.char_indices().nth(INPUT_SUMMARY_CHARACTERS) {
        Some((end, _)) => format!("{DIM}{}…{RESET}", &json[..end]),
        None => format!("{DIM}{json}{RESET}"),
    }
}

fn read_field<'a>(input: &'a Map<String, Value>, field: &str) -> &'a str {
    input.get(field).and_then(Value::as_str).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply_mark() -> String {
        format!("{DIM}‹‹{RESET}")
    }

    fn output_after_message() -> Output {
        let mut output = Output::new();
        output.show_message("Hi");
        output.take_lines();
        output
    }

    #[test]
    fn a_reply_starts_with_its_mark() {
        let mut output = output_after_message();
        output.show_text("Hello.\n");
        assert_eq!(output.take_lines(), [reply_mark(), "Hello.".to_string()]);
    }

    #[test]
    fn error_output_waits_for_the_text_claude_writes() {
        let mut output = output_after_message();
        output.show_text("One.\n\nTw");
        output.show_stderr("Failed");
        output.show_text("o.\n");
        output.show_text_end();
        let lines = output.take_lines();
        assert_eq!(
            lines[..4],
            [
                reply_mark(),
                "One.".to_string(),
                String::new(),
                "Two.".to_string()
            ]
        );
        assert_eq!(lines[5], format!("{DIM}stderr │{RESET} {RED}Failed{RESET}"));
    }

    #[test]
    fn a_message_sent_mid_line_follows_what_claude_wrote_of_the_line() {
        let mut output = output_after_message();
        output.show_text("Hel");
        output.show_message("Stop");
        let lines = output.take_lines();
        assert_eq!(lines[..2], [reply_mark(), "Hel".to_string()]);
        assert!(lines[3].ends_with(&format!("Stop{RESET}")));
    }

    #[test]
    fn a_reply_that_starts_with_a_tool_starts_with_its_mark() {
        let mut output = output_after_message();
        let mut input = Map::new();
        input.insert("command".to_string(), Value::String("ls".to_string()));
        output.show_tool_use(&ToolUse {
            id: "1".to_string(),
            name: "Bash".to_string(),
            input,
            is_subagent: false,
        });
        assert_eq!(output.take_lines()[0], reply_mark());
    }

    #[test]
    fn a_turn_a_message_stopped_ends_without_a_word() {
        let mut output = output_after_message();
        output.show_message("Stop");
        output.note_interrupt(Interruption::Message);
        output.take_lines();
        output.show_turn_end(Some("error_during_execution"));
        assert_eq!(output.take_lines(), Vec::<String>::new());
    }

    #[test]
    fn a_turn_claude_starts_by_itself_is_set_apart_from_the_turn_before() {
        let mut output = output_after_message();
        output.show_text("Hi.\n");
        output.show_turn_end(None);
        output.show_text("Done.\n");
        assert_eq!(
            output.take_lines(),
            [
                reply_mark(),
                "Hi.".to_string(),
                String::new(),
                "Done.".to_string()
            ]
        );
    }

    #[test]
    fn a_reply_keeps_its_mark_when_the_turn_its_message_stopped_ends_first() {
        let mut output = output_after_message();
        output.show_message("Stop");
        output.note_interrupt(Interruption::Message);
        output.take_lines();
        output.show_turn_end(Some("error_during_execution"));
        output.show_text("Stopped.\n");
        assert_eq!(output.take_lines(), [reply_mark(), "Stopped.".to_string()]);
    }

    fn read_call(id: &str, path: &str) -> ToolUse {
        let mut input = Map::new();
        input.insert("file_path".to_string(), Value::String(path.to_string()));
        ToolUse {
            id: id.to_string(),
            name: "Read".to_string(),
            input,
            is_subagent: false,
        }
    }

    fn result(id: &str, text: &str) -> ToolResult {
        ToolResult {
            tool_use_id: id.to_string(),
            text: Some(text.to_string()),
            is_error: false,
            is_subagent: false,
        }
    }

    #[test]
    fn a_result_repeats_its_call_tag_and_names_its_tool() {
        let mut output = output_after_message();
        output.show_tool_use(&read_call("1", "a.txt"));
        output.show_tool_use(&read_call("2", "b.txt"));
        output.show_tool_result(&result("2", "b"));
        let lines = output.take_lines();
        assert_eq!(
            lines[2],
            format!("{BOLD}●{RESET} {DIM}#01{RESET}{BOLD} Read{RESET} b.txt")
        );
        assert_eq!(lines[3], format!("{GREEN}✓{RESET} {DIM}#01 Read: b{RESET}"));
    }

    #[test]
    fn tags_wrap_from_ff_to_00() {
        let mut output = output_after_message();
        for id in 0..257 {
            output.show_tool_use(&read_call(&id.to_string(), "a.txt"));
        }
        let lines = output.take_lines();
        assert!(lines[256].contains("#ff"));
        assert!(lines[257].contains("#00"));
    }
}
