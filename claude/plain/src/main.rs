//! Claude Code in plain text: its replies are appended to the terminal, which wraps them, above
//! an input line that stays open for each message, and for each answer when a tool asks for
//! one.
//!
//! Arguments are passed on to `claude`, like `--resume <session>`.

mod code;
mod context;
mod input_line;
mod limits;
mod markdown;
mod protocol;
mod render;
mod session;
mod table;
mod terminal;

use std::env;
use std::io::{BufRead, BufReader};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{self, ChildStderr, ChildStdout, ExitCode, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use context::Context;
use input_line::{InputLine, Line, final_line};
use limits::Limits;
use protocol::{Event, read_events, repeats_a_message};
use render::Output;
use session::{Conversation, Ending, Input};
use terminal::QuietTerminal;

/// What the threads share, behind one lock, so that no lock waits on another.
struct Shared {
    /// What the input line is for, which the prompt reads.
    line: Line,
    output: Output,
    context: Context,
    limits: Limits,
    /// The session's id, once Claude Code tells it.
    session_id: Option<String>,
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
        env::var("EDITOR").map_err(|_| "Set EDITOR to the editor : opens".to_string())?;
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
        session_id: None,
    }));
    let break_signal = Arc::new(AtomicBool::new(false));
    let (input_sender, inputs) = mpsc::channel();

    let reader_shared = Arc::clone(&shared);
    let reader_break_signal = Arc::clone(&break_signal);
    let reader = thread::spawn(move || {
        read_claude(from_claude, &reader_shared, &|input| {
            // The main thread stops reading only on its way out
            let _ = input_sender.send(input);
            reader_break_signal.store(true, Ordering::Relaxed);
        });
    });
    let errors_shared = Arc::clone(&shared);
    let errors_reader = thread::spawn(move || {
        read_claude_errors(errors_from_claude, &errors_shared);
    });

    let _quiet_terminal = QuietTerminal::new()?;
    let input = InputLine::new(
        editor_command,
        Arc::clone(&break_signal),
        Arc::clone(&shared),
    )?;
    let conversation = Conversation::new(
        input,
        Arc::clone(&shared),
        inputs,
        break_signal,
        to_claude,
        &claude,
    );
    let result = conversation
        .run()
        .and_then(|ending| ending.map_or(Ok(()), Ending::run));
    end(claude, [reader, errors_reader], &shared, result)
}

/// Ends the session the user or Claude Code ended, or that failed with `result`'s error, and
/// shows the rest of the output: all Claude Code wrote, how it ended, and the final line. The
/// session is over, so Claude Code's input closed, and Claude Code exits once its work is done.
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
    let shared = &mut lock(shared);
    let output = &mut shared.output;
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
    println!("{}", final_line(shared.session_id.as_deref()));
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

/// Shows what Claude Code reports, notes what its calls hold in the cache, and hands the main
/// thread what it has to act on.
fn read_claude(from_claude: ChildStdout, shared: &Mutex<Shared>, forward: &dyn Fn(Input)) {
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
            match event {
                Event::Text(text) => shared.output.show_text(&text),
                Event::TextEnd => shared.output.show_text_end(),
                Event::ApiError(text) => shared.output.show_error(&text),
                Event::ToolUse(tool_use) => shared.output.show_tool_use(&tool_use),
                Event::ToolResult(result) => shared.output.show_tool_result(&result),
                Event::TurnEnd { failure } => shared.output.show_turn_end(failure.as_deref()),
                Event::SettingsErrors(errors) => shared.output.show_settings_errors(&errors),
                Event::PayingKey(key) => shared.output.show_paying_key(&key),
                Event::SessionId(id) => shared.session_id = Some(id),
                Event::State(state) => shared.output.note_state(state),
                Event::Limits { session, week } => shared.limits.note(session, week),
                Event::BackgroundTasks(tasks) => shared.output.note_background_tasks(tasks),
                Event::Call(usage) => {
                    shared.context.note_call(&usage, Instant::now());
                }
                Event::LastCall(usage) => shared.context.note_last_call(usage.as_ref()),
                Event::Ask(ask) => forward(Input::Ask(ask)),
                Event::Unsupported {
                    request_id,
                    subtype,
                } => forward(Input::Unsupported {
                    request_id,
                    subtype,
                }),
            }
        }
    }
    forward(Input::Closed);
}

/// Shows each line of Claude Code's error output that adds to what its messages show.
fn read_claude_errors(errors_from_claude: ChildStderr, shared: &Mutex<Shared>) {
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
        }
    }
}

/// Locks a value the threads share. A thread that panicked while holding it ends the program
/// anyway.
fn lock<T>(shared: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}
