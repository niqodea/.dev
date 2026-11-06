//! Claude Code in plain text: its replies are appended to the terminal, which wraps them, above
//! an input line that stays open for each message, and for each answer when a tool asks for
//! one.
//!
//! Arguments are passed on to `claude`, like `--resume <session>`.

mod code;
mod markdown;
mod protocol;
mod render;
mod table;

use std::borrow::Cow;
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::mem::MaybeUninit;
use std::os::unix::process::CommandExt;
use std::process::{self, ChildStdin, ChildStdout, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use nu_ansi_term::{Color, Style};
use reedline::{
    EditCommand, ExternalOutput, Highlighter, HistoryItem, KeyCode, KeyModifiers, Prompt,
    PromptEditMode, PromptHistorySearch, Reedline, ReedlineEvent, Signal, StyledText, Vi,
    default_vi_insert_keybindings, default_vi_normal_keybindings, default_vi_visual_keybindings,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use protocol::{Answer, Ask, AskKind, Command, Event, Question, read_events};
use render::{BAR_COLOR, Interruption, Output, Status, answer_label, marks, start_bar_row};

const REFUSAL: &str = "The user refused this tool use.";
// What Claude does stays in the background
const STATUS_COLOR: &str = "\x1b[38;5;244m";
// The host commands Enter and Ctrl-C send. reedline leaves the line of a host command in place,
// for the next input line to take the place of. Its own Enter would put the line in the
// scrollback at once, ahead of the end of the line Claude was writing, which came first, so the
// user's lines reach the scrollback only through the output, in the order Claude sees them
const SEND: &str = "send";
const CTRL_C: &str = "ctrl-c";

/// What Claude Code asks of the main thread.
enum Input {
    Ask(Ask),
    Unsupported {
        request_id: String,
        subtype: String,
    },
    /// Claude Code ended.
    Closed,
}

struct Session {
    editor: Reedline,
    /// The prompt in front of the open input line.
    prompt: Marker,
    /// What the input line is for, which the prompt reads.
    line: Arc<Mutex<Line>>,
    output: Arc<Mutex<Output>>,
    inputs: Receiver<Input>,
    break_signal: Arc<AtomicBool>,
    to_claude: ChildStdin,
    request_count: u64,
}

/// What the input line is for.
#[derive(Clone)]
enum Line {
    Message,
    /// The answer to what a tool asks, with the question.
    Answer(String),
}

/// The prompt in front of the input line: for a message, `››` on a row above it, with what
/// Claude does after them; for an answer, the question.
struct Marker {
    line: Arc<Mutex<Line>>,
    output: Arc<Mutex<Output>>,
}

/// Draws what the user types on the input line's bar.
struct OnBar;

/// The lines of the output that wait for reedline, which prints them above the input line.
struct Pending(Arc<Mutex<Output>>);

/// Keeps the terminal from echoing keys, from holding them for a whole line, and from turning
/// Ctrl-C into a signal, until it is dropped. The input line reads keys this way while it is
/// open, and between two input lines a key then waits for the next one instead of showing up
/// where Claude's text goes.
struct QuietTerminal {
    original: libc::termios,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let editor_command =
        env::var("EDITOR").map_err(|_| "Set EDITOR to the editor Ctrl-O opens".to_string())?;
    let mut claude = process::Command::new("claude")
        .args([
            "--print",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            // stream-json output needs it
            "--verbose",
            "--include-partial-messages",
            "--permission-prompt-tool",
            "stdio",
        ])
        .args(env::args().skip(1))
        .env("CLAUDE_CODE_SDK_READS_SESSION_STATE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Keys reach only this process, which turns Ctrl-C into an interrupt of the turn
        .process_group(0)
        .spawn()
        .map_err(|error| format!("Cannot start claude: {error}"))?;
    let to_claude = claude.stdin.take().expect("stdin is piped");
    let from_claude = claude.stdout.take().expect("stdout is piped");

    let output = Arc::new(Mutex::new(Output::new()));
    let line = Arc::new(Mutex::new(Line::Message));
    let break_signal = Arc::new(AtomicBool::new(false));
    let (input_sender, inputs) = mpsc::channel();
    let mut editor = create_editor(
        editor_command,
        Arc::clone(&break_signal),
        Arc::clone(&output),
    );

    let reader_output = Arc::clone(&output);
    let reader_break_signal = Arc::clone(&break_signal);
    let repaint_signal = editor.repaint_signal();
    thread::spawn(move || {
        read_claude(
            from_claude,
            &reader_output,
            &input_sender,
            &reader_break_signal,
            &|| repaint_signal.request_repaint(),
        );
    });

    let _quiet_terminal = QuietTerminal::new()?;
    let mut session = Session {
        editor,
        prompt: Marker {
            line: Arc::clone(&line),
            output: Arc::clone(&output),
        },
        line,
        output: Arc::clone(&output),
        inputs,
        break_signal,
        to_claude,
        request_count: 0,
    };
    session.send(&Command::GetSettings)?;
    session.run()?;
    // Claude Code ends once its input closes
    drop(session);
    for line in lock(&output).take_lines() {
        println!("{line}");
    }
    let status = claude
        .wait()
        .map_err(|error| format!("Cannot wait for claude: {error}"))?;
    Ok(match status.code() {
        Some(0) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    })
}

/// The input line: vi keys, Enter and Ctrl-C as host commands, the bar, Ctrl-G for the editor,
/// and the lines of `output` printed above it.
fn create_editor(
    editor_command: String,
    break_signal: Arc<AtomicBool>,
    output: Arc<Mutex<Output>>,
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
        .with_external_printer(Pending(output))
}

/// Shows what Claude Code reports, and hands the main thread what it has to act on.
fn read_claude(
    from_claude: ChildStdout,
    output: &Mutex<Output>,
    inputs: &Sender<Input>,
    break_signal: &AtomicBool,
    request_repaint: &dyn Fn(),
) {
    let forward = |input: Input| {
        // The main thread stops reading only on its way out
        let _ = inputs.send(input);
        break_signal.store(true, Ordering::Relaxed);
    };
    for line in BufReader::new(from_claude).lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                lock(output).show_error(&format!("Cannot read from claude: {error}"));
                break;
            }
        };
        let events = match read_events(&line) {
            Ok(events) => events,
            Err(error) => {
                lock(output).show_error(&error);
                continue;
            }
        };
        for event in events {
            let mut output = lock(output);
            let status = output.status();
            match event {
                Event::Text(text) => output.show_text(&text),
                Event::TextEnd => output.show_text_end(),
                Event::ToolUse(tool_use) => output.show_tool_use(&tool_use),
                Event::ToolResult(result) => output.show_tool_result(&result),
                Event::TurnEnd { failure } => output.show_turn_end(failure.as_deref()),
                Event::SettingsErrors(errors) => output.show_settings_errors(&errors),
                Event::State(state) => output.note_state(state),
                Event::Ask(ask) => forward(Input::Ask(ask)),
                Event::Unsupported {
                    request_id,
                    subtype,
                } => forward(Input::Unsupported {
                    request_id,
                    subtype,
                }),
            }
            if output.status() != status {
                request_repaint();
            }
        }
    }
    forward(Input::Closed);
}

impl Session {
    /// Runs the conversation until the user ends it, or Claude Code does.
    fn run(&mut self) -> Result<(), String> {
        let mut draft = String::new();
        loop {
            // A break signal raised before the line opens has nothing left to break
            self.break_signal.store(false, Ordering::Relaxed);
            match self.inputs.try_recv() {
                Ok(Input::Ask(ask)) => {
                    self.answer(&ask)?;
                    continue;
                }
                Ok(Input::Unsupported {
                    request_id,
                    subtype,
                }) => {
                    self.refuse(&request_id, &subtype)?;
                    continue;
                }
                Ok(Input::Closed) | Err(TryRecvError::Disconnected) => return Ok(()),
                Err(TryRecvError::Empty) => {}
            }
            draft = match self.read_line(Line::Message, &draft)? {
                Signal::HostCommand(command) if command == SEND => {
                    let text = self.editor.current_buffer_contents().to_string();
                    if !text.trim().is_empty() {
                        self.send_message(&text)?;
                    }
                    String::new()
                }
                // Ctrl-C on an empty line stops Claude, and on a line with text clears it
                Signal::HostCommand(command) if command == CTRL_C => {
                    let is_working = lock(&self.output).status() != Status::Idle;
                    if self.editor.current_buffer_contents().is_empty() && is_working {
                        self.interrupt(Interruption::Stop)?;
                    }
                    String::new()
                }
                // Claude Code asked something, which the next round of the loop answers
                Signal::ExternalBreak(text) => text,
                // Ctrl-C in a history search, which leaves the search
                Signal::CtrlC(_) => String::new(),
                Signal::CtrlD => {
                    if lock(&self.output).status() != Status::Idle {
                        self.interrupt(Interruption::Stop)?;
                    }
                    return Ok(());
                }
                signal => return Err(format!("Unexpected input: {signal:?}")),
            };
        }
    }

    /// Asks the user about the tool, and tells Claude Code the answer.
    fn answer(&mut self, ask: &Ask) -> Result<(), String> {
        lock(&self.output).show_ask(ask);
        let answer = match &ask.kind {
            AskKind::Permission { rules, .. } => {
                let always = if rules.is_empty() { "" } else { "/a" };
                let question = format!("allow {}? y/n{always}", ask.tool_name);
                self.read_permission(ask, &question, rules)?
            }
            // Leaving plan mode, Claude Code returns to the mode the session had before it
            AskKind::Plan(_) => self.read_permission(ask, "approve the plan? y/n", &[])?,
            AskKind::Questions(questions) => self.read_questions(ask, questions)?,
        };
        if let Answer::Deny { interrupt, .. } = &answer {
            let mut output = lock(&self.output);
            output.hide_result(&ask.tool_use_id);
            if *interrupt {
                output.note_interrupt(Interruption::Stop);
            }
        }
        self.send(&Command::Answer(ask, answer))
    }

    /// Reads whether the tool may run: once, for good with `rules` when there are any, or not,
    /// for a reason the user gives or none.
    fn read_permission(
        &mut self,
        ask: &Ask,
        question: &str,
        rules: &[Value],
    ) -> Result<Answer, String> {
        loop {
            let Some(text) = self.read_answer(question)? else {
                return Ok(refuse_and_stop());
            };
            return Ok(match text.trim() {
                "y" => Answer::Allow {
                    input: ask.input.clone(),
                    rules: vec![],
                },
                "a" if !rules.is_empty() => Answer::Allow {
                    input: ask.input.clone(),
                    rules: rules.to_vec(),
                },
                // Offered no rule, a lone a is a slip rather than a reason
                "a" => continue,
                "n" | "" => refuse_and_stop(),
                reason => Answer::Deny {
                    message: format!("{REFUSAL} The user said: {reason}"),
                    interrupt: false,
                },
            });
        }
    }

    fn read_questions(&mut self, ask: &Ask, questions: &[Question]) -> Result<Answer, String> {
        let mut answers = Map::new();
        for question in questions {
            let several = if question.is_multi_select {
                ", several apart"
            } else {
                ""
            };
            let label = format!("{} 1-{}{several}", question.header, question.options.len());
            let choice = loop {
                let Some(text) = self.read_answer(&label)? else {
                    return Ok(refuse_and_stop());
                };
                if !text.trim().is_empty() {
                    break choose(question, text.trim());
                }
            };
            answers.insert(question.text.clone(), Value::String(choice));
        }
        let mut input = ask.input.clone();
        input.insert("answers".to_string(), Value::Object(answers));
        Ok(Answer::Allow {
            input,
            rules: vec![],
        })
    }

    /// Reads one answer to `question`, none when the user gives up on it with Ctrl-C or Ctrl-D.
    fn read_answer(&mut self, question: &str) -> Result<Option<String>, String> {
        let mut draft = String::new();
        loop {
            match self.read_line(Line::Answer(question.to_string()), &draft)? {
                Signal::HostCommand(command) if command == SEND => {
                    let text = self.editor.current_buffer_contents().to_string();
                    lock(&self.output).show_answer(question, &text);
                    return Ok(Some(text));
                }
                // Claude Code asked something more, which waits for this answer
                Signal::ExternalBreak(text) => draft = text,
                Signal::HostCommand(_) | Signal::CtrlC(_) | Signal::CtrlD => return Ok(None),
                signal => return Err(format!("Unexpected input: {signal:?}")),
            }
        }
    }

    /// Reads the input line, starting from `draft`. A host command or a break signal leaves the
    /// line on screen, for the next line to take the place of.
    fn read_line(&mut self, line: Line, draft: &str) -> Result<Signal, String> {
        *lock(&self.line) = line;
        self.editor.run_edit_commands(&[
            EditCommand::Clear,
            EditCommand::InsertString(draft.to_string()),
        ]);
        self.editor
            .read_line(&self.prompt)
            .map_err(|error| format!("Cannot read the input line: {error}"))
    }

    /// Shows the message and sends it. While Claude works, Claude Code makes way for it, as for
    /// a message sent with its own send-now key.
    fn send_message(&mut self, text: &str) -> Result<(), String> {
        let is_working = lock(&self.output).status() != Status::Idle;
        lock(&self.output).show_message(text);
        let id = Uuid::new_v4().to_string();
        self.send(&Command::Message { id: &id, text })?;
        if is_working {
            lock(&self.output).note_interrupt(Interruption::Message);
            let request_id = self.next_request_id();
            self.send(&Command::SendNow {
                request_id,
                message_id: &id,
            })?;
        }
        // reedline keeps in its history only the lines its own Enter submits
        self.editor
            .history_mut()
            .save(HistoryItem::from_command_line(text))
            .map(|_| ())
            .map_err(|error| format!("Cannot keep the message in the input history: {error}"))
    }

    fn interrupt(&mut self, interruption: Interruption) -> Result<(), String> {
        lock(&self.output).note_interrupt(interruption);
        let request_id = self.next_request_id();
        self.send(&Command::Interrupt { request_id })
    }

    /// An id for a request to Claude Code, which its reply carries back.
    fn next_request_id(&mut self) -> String {
        self.request_count += 1;
        format!("request-{}", self.request_count)
    }

    fn refuse(&mut self, request_id: &str, subtype: &str) -> Result<(), String> {
        lock(&self.output).show_error(&format!(
            "Claude Code asked for {subtype}, which this client does not offer"
        ));
        self.send(&Command::Refuse {
            request_id,
            subtype,
        })
    }

    fn send(&mut self, command: &Command) -> Result<(), String> {
        self.to_claude
            .write_all(command.to_line().as_bytes())
            .and_then(|()| self.to_claude.flush())
            .map_err(|error| format!("Cannot write to claude: {error}"))
    }
}

impl QuietTerminal {
    fn new() -> Result<Self, String> {
        let mut settings = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: tcgetattr fills the termios when it returns 0
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, settings.as_mut_ptr()) } == -1 {
            let error = io::Error::last_os_error();
            return Err(format!("Cannot read the terminal's settings: {error}"));
        }
        // SAFETY: tcgetattr returned 0 above, so it filled the termios
        let original = unsafe { settings.assume_init() };
        let mut quiet = original;
        quiet.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
        // SAFETY: the termios is a valid copy of the terminal's own settings
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const quiet) } == -1 {
            let error = io::Error::last_os_error();
            return Err(format!("Cannot change the terminal's settings: {error}"));
        }
        Ok(QuietTerminal { original })
    }
}

impl Drop for QuietTerminal {
    fn drop(&mut self) {
        // SAFETY: the termios holds the settings the terminal had before.
        // Restoring them can only fail if the terminal is gone, which leaves
        // nothing to restore
        unsafe {
            libc::tcsetattr(
                libc::STDIN_FILENO,
                libc::TCSADRAIN,
                &raw const self.original,
            )
        };
    }
}

impl Prompt for Marker {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        let label = match &*lock(&self.line) {
            Line::Message => {
                let status = match lock(&self.output).status() {
                    Status::Idle => String::new(),
                    Status::Working => format!(" {STATUS_COLOR}working"),
                    Status::Writing => format!(" {STATUS_COLOR}writing"),
                    Status::Running(tool) => format!(" {STATUS_COLOR}running {tool}"),
                };
                format!("{}{}{status}", start_bar_row(), marks())
            }
            Line::Answer(question) => answer_label(question),
        };
        Cow::Owned(label)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        match &*lock(&self.line) {
            // A message starts on a row of its own, so its lines copy back as they were written
            Line::Message => Cow::Owned(format!("\n{}", start_bar_row())),
            Line::Answer(_) => Cow::Borrowed(""),
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

impl ExternalOutput for Pending {
    fn drain(&mut self) -> io::Result<Vec<String>> {
        Ok(lock(&self.0).take_lines())
    }
}

impl Highlighter for OnBar {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let bar = Style::new().on(Color::Fixed(BAR_COLOR));
        let mut styled = StyledText::new();
        styled.push((bar, line.to_string()));
        styled
    }
}

/// The labels of the options the numbers in `text` pick, or `text` itself as the user's own
/// answer.
fn choose(question: &Question, text: &str) -> String {
    let picks: Option<Vec<&str>> = text
        .split([' ', ','])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let number: usize = word.parse().ok()?;
            let choice = question.options.get(number.checked_sub(1)?)?;
            Some(choice.label.as_str())
        })
        .collect();
    match picks {
        Some(labels) if labels.len() == 1 || question.is_multi_select => labels.join(", "),
        _ => text.to_string(),
    }
}

/// Refuses the tool and ends the turn, so Claude waits for the user's next message.
fn refuse_and_stop() -> Answer {
    Answer::Deny {
        message: REFUSAL.to_string(),
        interrupt: true,
    }
}

/// Locks a value the threads share. A thread that panicked while holding it ends the program
/// anyway.
fn lock<T>(shared: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}
