//! Claude Code in plain text: its replies are appended to the terminal, which wraps them, above
//! an input line that stays open for each message, and for each answer when a tool asks for
//! one.
//!
//! Arguments are passed on to `claude`, like `--resume <session>`.

mod code;
mod context;
mod limits;
mod markdown;
mod protocol;
mod render;
mod table;

use std::borrow::Cow;
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::mem::MaybeUninit;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{self, ChildStderr, ChildStdin, ChildStdout, ExitCode, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use nu_ansi_term::{Color, Style};
use reedline::{
    EditCommand, ExternalOutput, Highlighter, HistoryItem, KeyCode, KeyModifiers, Prompt,
    PromptEditMode, PromptHistorySearch, Reedline, ReedlineEvent, Signal, StyledText, Vi,
    default_vi_insert_keybindings, default_vi_normal_keybindings, default_vi_visual_keybindings,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use context::Context;
use limits::{Level, Limits, NEAR_BOUNDS_PASSED};
use protocol::{Answer, Ask, AskKind, Command, Event, Question, read_events, repeats_a_message};
use render::{BAR_COLOR, Interruption, Output, Status, answer_label, marks, start_bar_row};

const REFUSAL: &str = "The user refused this tool use.";
// What Claude does stays in the background
const STATUS_COLOR: &str = "\x1b[38;5;244m";
// A limit near its end, before the red of a reached one
const ORANGE: Color = Color::Fixed(208);
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

/// What the threads share, behind one lock, so that no lock waits on another.
struct Shared {
    /// What the input line is for, which the prompt reads.
    line: Line,
    output: Output,
    context: Context,
    limits: Limits,
}

/// The terminal's side of the session: the input line, and what it shows of Claude's.
struct Session {
    editor: Reedline,
    /// The prompt in front of the open input line.
    prompt: Marker,
    shared: Arc<Mutex<Shared>>,
    inputs: Receiver<Input>,
    break_signal: Arc<AtomicBool>,
}

/// The session while the user talks with Claude, through Claude Code's input.
struct Conversation {
    session: Session,
    to_claude: ChildStdin,
    claude_pid: libc::pid_t,
    request_count: u64,
}

/// The session once the user has nothing more to say: Claude Code's input closed, and Claude
/// finishes its work.
struct Ending {
    session: Session,
    /// Claude Code's process, which Ctrl-C interrupts.
    claude_pid: libc::pid_t,
}

/// What the input line is for.
#[derive(Clone)]
enum Line {
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
struct Marker(Arc<Mutex<Shared>>);

/// Draws what the user types on the input line's bar.
struct OnBar;

/// The lines of the output that wait for reedline, which prints them above the input line.
struct Pending(Arc<Mutex<Shared>>);

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
        // Undocumented, read in Claude Code 2.1.316: makes it send its session state
        .env("CLAUDE_CODE_SDK_READS_SESSION_STATE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Its error output reaches the terminal through the output, above the input line that
        // reedline redraws
        .stderr(Stdio::piped())
        // Keys reach only this process, which turns Ctrl-C into an interrupt of the turn
        .process_group(0)
        .spawn()
        .map_err(|error| format!("Cannot start claude: {error}"))?;
    let to_claude = claude.stdin.take().expect("stdin is piped");
    let from_claude = claude.stdout.take().expect("stdout is piped");
    let errors_from_claude = claude.stderr.take().expect("stderr is piped");

    let shared = Arc::new(Mutex::new(Shared {
        line: Line::Message,
        output: Output::new(),
        context: Context::new(),
        limits: Limits::new(),
    }));
    let break_signal = Arc::new(AtomicBool::new(false));
    let (input_sender, inputs) = mpsc::channel();
    let (cold_at_sender, cold_ats) = mpsc::channel();
    let mut editor = create_editor(
        editor_command,
        Arc::clone(&break_signal),
        Arc::clone(&shared),
    );

    let reader_shared = Arc::clone(&shared);
    let reader_break_signal = Arc::clone(&break_signal);
    let repaint_signal = editor.repaint_signal();
    let watcher_repaint_signal = repaint_signal.clone();
    let errors_repaint_signal = repaint_signal.clone();
    let reader = thread::spawn(move || {
        read_claude(
            from_claude,
            &reader_shared,
            &cold_at_sender,
            &|input| {
                // The main thread stops reading only on its way out
                let _ = input_sender.send(input);
                reader_break_signal.store(true, Ordering::Relaxed);
            },
            &|| repaint_signal.request_repaint(),
        );
    });
    let errors_shared = Arc::clone(&shared);
    let errors_reader = thread::spawn(move || {
        read_claude_errors(errors_from_claude, &errors_shared, &|| {
            errors_repaint_signal.request_repaint();
        });
    });
    thread::spawn(move || {
        watch_cache(&cold_ats, &|| watcher_repaint_signal.request_repaint());
    });

    let _quiet_terminal = QuietTerminal::new()?;
    let conversation = Conversation {
        session: Session {
            editor,
            prompt: Marker(Arc::clone(&shared)),
            shared: Arc::clone(&shared),
            inputs,
            break_signal,
        },
        to_claude,
        claude_pid: claude.id().cast_signed(),
        request_count: 0,
    };
    let result = conversation
        .run()
        .and_then(|ending| ending.map_or(Ok(()), Ending::run));
    end(claude, [reader, errors_reader], &shared, result)
}

/// Ends the session the user or Claude Code ended, or that failed with `result`'s error, and
/// shows the rest of the output: all Claude Code wrote, then how it ended. The session is over,
/// so Claude Code's input closed, and Claude Code exits once its work is done.
fn end(
    mut claude: process::Child,
    readers: [JoinHandle<()>; 2],
    shared: &Mutex<Shared>,
    result: Result<(), String>,
) -> Result<ExitCode, String> {
    let status = claude
        .wait()
        .map_err(|error| format!("Cannot wait for claude: {error}"))?;
    // Undocumented, seen with Claude Code 2.1.316: the processes it starts in the background
    // write to files of their own, so its output closes as it exits, and each reader ends then.
    // Every line Claude Code wrote thus comes before its end. A reader that panicked already said
    // why
    for reader in readers {
        let _ = reader.join();
    }
    let output = &mut lock(shared).output;
    let code = match result {
        Ok(()) => exit_code(status),
        Err(error) => {
            output.show_error(&error);
            ExitCode::FAILURE
        }
    };
    output.show_end(describe_exit(status).as_deref());
    for line in output.take_lines() {
        println!("{line}");
    }
    Ok(code)
}

/// How Claude Code exited, none when with status 0.
fn describe_exit(status: ExitStatus) -> Option<String> {
    // Like "exit status: 1", or "signal: 9 (SIGKILL)"
    (!status.success()).then(|| format!("Claude Code ended with {status}"))
}

/// The status a shell gives for a program that ended as Claude Code did, which the client ends
/// with, so it passes Claude Code's on.
fn exit_code(status: ExitStatus) -> ExitCode {
    let code = status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1);
    ExitCode::from(code)
}

/// The input line: vi keys, Enter and Ctrl-C as host commands, the bar, Ctrl-G for the editor,
/// and the lines of `output` printed above it.
fn create_editor(
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

/// Shows what Claude Code reports, notes what its calls hold in the cache, with when the cache
/// turns cold for the cache's watcher, and hands the main thread what it has to act on.
fn read_claude(
    from_claude: ChildStdout,
    shared: &Mutex<Shared>,
    cold_ats: &Sender<Instant>,
    forward: &dyn Fn(Input),
    request_repaint: &dyn Fn(),
) {
    for line in BufReader::new(from_claude).lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                lock(shared)
                    .output
                    .show_error(&format!("Cannot read from claude: {error}"));
                break;
            }
        };
        let events = match read_events(&line) {
            Ok(events) => events,
            Err(error) => {
                lock(shared).output.show_error(&error);
                continue;
            }
        };
        for event in events {
            let mut shared = lock(shared);
            let status = shared.output.status();
            match event {
                Event::Text(text) => shared.output.show_text(&text),
                Event::TextEnd => shared.output.show_text_end(),
                Event::ApiError(text) => shared.output.show_error(&text),
                Event::ToolUse(tool_use) => shared.output.show_tool_use(&tool_use),
                Event::ToolResult(result) => shared.output.show_tool_result(&result),
                Event::TurnEnd { failure } => shared.output.show_turn_end(failure.as_deref()),
                Event::SettingsErrors(errors) => shared.output.show_settings_errors(&errors),
                Event::State(state) => shared.output.note_state(state),
                Event::Limits { session, week } => {
                    shared.limits.note(session, week);
                    request_repaint();
                }
                Event::BackgroundTasks(tasks) => {
                    shared.output.note_background_tasks(tasks);
                    request_repaint();
                }
                Event::Call(usage) => {
                    let cold_at = shared.context.note_call(&usage, Instant::now());
                    // The watcher stops only on the program's way out
                    let _ = cold_ats.send(cold_at);
                    request_repaint();
                }
                Event::LastCall(usage) => {
                    shared.context.note_last_call(usage.as_ref());
                    request_repaint();
                }
                Event::Ask(ask) => forward(Input::Ask(ask)),
                Event::Unsupported {
                    request_id,
                    subtype,
                } => forward(Input::Unsupported {
                    request_id,
                    subtype,
                }),
            }
            if shared.output.status() != status {
                request_repaint();
            }
        }
    }
    forward(Input::Closed);
}

/// Shows each line of Claude Code's error output that adds to what its messages show.
fn read_claude_errors(
    errors_from_claude: ChildStderr,
    shared: &Mutex<Shared>,
    request_repaint: &dyn Fn(),
) {
    for line in BufReader::new(errors_from_claude).lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                lock(shared)
                    .output
                    .show_error(&format!("Cannot read the error output of claude: {error}"));
                return;
            }
        };
        if !repeats_a_message(&line) {
            lock(shared).output.show_stderr(&line);
            // The prompt says when the line waits
            request_repaint();
        }
    }
}

/// Repaints the prompt as the cache turns cold, at the latest of the instants `cold_ats` sends,
/// so the context's level turns red while the user waits.
fn watch_cache(cold_ats: &Receiver<Instant>, request_repaint: &dyn Fn()) {
    let mut next_cold_at: Option<Instant> = None;
    loop {
        let received = match next_cold_at {
            Some(cold_at) => {
                cold_ats.recv_timeout(cold_at.saturating_duration_since(Instant::now()))
            }
            None => cold_ats.recv().map_err(RecvTimeoutError::from),
        };
        match received {
            Ok(cold_at) => next_cold_at = Some(cold_at),
            Err(RecvTimeoutError::Timeout) => {
                next_cold_at = None;
                request_repaint();
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

impl Session {
    /// What Claude Code asks of the main thread, if anything waits.
    fn next_input(&self) -> Option<Input> {
        // A break signal raised before the line opens has nothing left to break
        self.break_signal.store(false, Ordering::Relaxed);
        match self.inputs.try_recv() {
            Ok(input) => Some(input),
            Err(TryRecvError::Disconnected) => Some(Input::Closed),
            Err(TryRecvError::Empty) => None,
        }
    }

    /// Reads the input line, starting from `draft`. A host command or a break signal leaves the
    /// line on screen, for the next line to take the place of.
    fn read_line(&mut self, line: Line, draft: &str) -> Result<Signal, String> {
        lock(&self.shared).line = line;
        self.editor.run_edit_commands(&[
            EditCommand::Clear,
            EditCommand::InsertString(draft.to_string()),
        ]);
        self.editor
            .read_line(&self.prompt)
            .map_err(|error| format!("Cannot read the input line: {error}"))
    }
}

impl Conversation {
    /// Runs the conversation until Claude Code ends, or until the user has nothing more to say,
    /// which the ending follows.
    fn run(mut self) -> Result<Option<Ending>, String> {
        self.send(&Command::GetSettings)?;
        self.send(&Command::GetContext)?;
        let mut draft = String::new();
        loop {
            match self.session.next_input() {
                Some(Input::Ask(ask)) => {
                    self.answer(&ask)?;
                    continue;
                }
                Some(Input::Unsupported {
                    request_id,
                    subtype,
                }) => {
                    self.refuse(&request_id, &subtype)?;
                    continue;
                }
                Some(Input::Closed) => return Ok(None),
                None => {}
            }
            draft = match self.session.read_line(Line::Message, &draft)? {
                // A message on a cold cache stays on the input line the first time, and the
                // label says the next Enter sends it
                Signal::HostCommand(command) if command == SEND => {
                    let text = self.session.editor.current_buffer_contents().to_string();
                    if text.trim().is_empty() {
                        String::new()
                    } else if lock(&self.session.shared)
                        .context
                        .holds_message(Instant::now())
                    {
                        text
                    } else {
                        self.send_message(&text)?;
                        String::new()
                    }
                }
                // Ctrl-C on an empty line stops Claude, and on a line with text clears it
                Signal::HostCommand(command) if command == CTRL_C => {
                    let is_working = lock(&self.session.shared).output.status() != Status::Idle;
                    if self.session.editor.current_buffer_contents().is_empty() && is_working {
                        self.interrupt(Interruption::Stop)?;
                    }
                    String::new()
                }
                // Claude Code asked something, which the next round of the loop answers
                Signal::ExternalBreak(text) => text,
                // Ctrl-C in a history search, which leaves the search
                Signal::CtrlC(_) => String::new(),
                Signal::CtrlD => return Ok(Some(self.end_input())),
                signal => return Err(format!("Unexpected input: {signal:?}")),
            };
        }
    }

    /// Closes Claude Code's input, as the end of input does a program's in the terminal, since
    /// the user has nothing more to say.
    fn end_input(self) -> Ending {
        let Conversation {
            session,
            to_claude,
            claude_pid,
            request_count: _,
        } = self;
        drop(to_claude);
        Ending {
            session,
            claude_pid,
        }
    }

    /// Asks the user about the tool, and tells Claude Code the answer.
    fn answer(&mut self, ask: &Ask) -> Result<(), String> {
        lock(&self.session.shared).output.show_ask(ask);
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
            let output = &mut lock(&self.session.shared).output;
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
            match self
                .session
                .read_line(Line::Answer(question.to_string()), &draft)?
            {
                Signal::HostCommand(command) if command == SEND => {
                    let text = self.session.editor.current_buffer_contents().to_string();
                    lock(&self.session.shared)
                        .output
                        .show_answer(question, &text);
                    return Ok(Some(text));
                }
                // Claude Code asked something more, which waits for this answer
                Signal::ExternalBreak(text) => draft = text,
                Signal::HostCommand(_) | Signal::CtrlC(_) | Signal::CtrlD => return Ok(None),
                signal => return Err(format!("Unexpected input: {signal:?}")),
            }
        }
    }

    /// Shows the message and sends it. While Claude works, Claude Code makes way for it, as for
    /// a message sent with its own send-now key.
    fn send_message(&mut self, text: &str) -> Result<(), String> {
        let is_working = lock(&self.session.shared).output.status() != Status::Idle;
        lock(&self.session.shared).output.show_message(text);
        let id = Uuid::new_v4().to_string();
        self.send(&Command::Message { id: &id, text })?;
        if is_working {
            lock(&self.session.shared)
                .output
                .note_interrupt(Interruption::Message);
            let request_id = self.next_request_id();
            self.send(&Command::SendNow {
                request_id,
                message_id: &id,
            })?;
        }
        // reedline keeps in its history only the lines its own Enter submits
        self.session
            .editor
            .history_mut()
            .save(HistoryItem::from_command_line(text))
            .map(|_| ())
            .map_err(|error| format!("Cannot keep the message in the input history: {error}"))
    }

    fn interrupt(&mut self, interruption: Interruption) -> Result<(), String> {
        lock(&self.session.shared)
            .output
            .note_interrupt(interruption);
        let request_id = self.next_request_id();
        self.send(&Command::Interrupt { request_id })
    }

    /// An id for a request to Claude Code, which its reply carries back.
    fn next_request_id(&mut self) -> String {
        self.request_count += 1;
        format!("request-{}", self.request_count)
    }

    fn refuse(&mut self, request_id: &str, subtype: &str) -> Result<(), String> {
        lock(&self.session.shared).output.show_error(&format!(
            "Claude Code asked for {subtype}, which this client does not offer"
        ));
        self.send(&Command::Refuse {
            request_id,
            subtype,
        })
    }

    /// Sends the command, which Claude Code that already ended never reads. Its reader then
    /// says it closed, and the session's end says how it exited.
    fn send(&mut self, command: &Command) -> Result<(), String> {
        match self
            .to_claude
            .write_all(command.to_line().as_bytes())
            .and_then(|()| self.to_claude.flush())
        {
            Err(error) if error.kind() != io::ErrorKind::BrokenPipe => {
                Err(format!("Cannot write to claude: {error}"))
            }
            _ => Ok(()),
        }
    }
}

impl Ending {
    /// Shows Claude finish its work, until Claude Code exits once its turn and its background
    /// tasks end. Ctrl-C stops them.
    fn run(mut self) -> Result<(), String> {
        loop {
            match self.session.next_input() {
                Some(Input::Closed) => return Ok(()),
                // Undocumented, seen with Claude Code 2.1.316: once its input closed, Claude Code
                // fails each permission request, with "Stream closed" as the tool's result
                Some(Input::Ask(_) | Input::Unsupported { .. }) | None => {}
            }
            match self.session.read_line(Line::Ending, "")? {
                Signal::HostCommand(command) if command == CTRL_C => self.interrupt()?,
                // What the user types has no one left to read it
                Signal::HostCommand(_)
                | Signal::ExternalBreak(_)
                | Signal::CtrlC(_)
                | Signal::CtrlD => {}
                signal => return Err(format!("Unexpected input: {signal:?}")),
            }
        }
    }

    /// Undocumented, seen with Claude Code 2.1.316: SIGINT stops Claude's turn and background
    /// tasks, as Ctrl-C does a program's work in the terminal, and Claude Code then exits, since
    /// its input closed.
    fn interrupt(&self) -> Result<(), String> {
        lock(&self.session.shared)
            .output
            .note_interrupt(Interruption::Stop);
        // SAFETY: kill only sends a signal. The process keeps its id until it is waited for,
        // which the session's end does
        if unsafe { libc::kill(self.claude_pid, libc::SIGINT) } == -1 {
            return Err(format!(
                "Cannot stop claude: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(())
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

/// A level, with ? for one not known yet.
fn show_level(level: Option<usize>) -> String {
    level.map_or_else(|| "?".to_string(), |level| level.to_string())
}

/// The letter of a usage limit and its level, orange from `NEAR_BOUNDS_PASSED` and red once
/// reached, so the color only repeats what the level says.
fn limit_indicator(letter: char, level: Option<Level>) -> String {
    let color = match level {
        None => STATUS_COLOR.to_string(),
        Some(Level::Below(passed)) if passed < NEAR_BOUNDS_PASSED => STATUS_COLOR.to_string(),
        Some(Level::Below(_)) => ORANGE.prefix().to_string(),
        Some(Level::Reached) => Color::Red.prefix().to_string(),
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
                    ("*", Color::Red.prefix().to_string())
                } else {
                    (" ", STATUS_COLOR.to_string())
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

impl ExternalOutput for Pending {
    fn drain(&mut self) -> io::Result<Vec<String>> {
        Ok(lock(&self.0).output.take_lines())
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
