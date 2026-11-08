//! The session in its two phases: the conversation, which sends messages and answers to Claude
//! Code, then the ending, while Claude finishes its work.

use std::io::{self, Write};
use std::process::{Child, ChildStdin};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::{Map, Value};
use uuid::Uuid;

use crate::input_line::{InputLine, Line, Outcome};
use crate::protocol::{Answer, Ask, AskKind, Command, Question};
use crate::render::{Interruption, Status};
use crate::{Shared, lock};

const REFUSAL: &str = "The user refused this tool use.";

/// What Claude Code asks of the main thread.
pub enum Input {
    Ask(Ask),
    Unsupported {
        request_id: String,
        subtype: String,
    },
    /// Claude Code ended.
    Closed,
}

/// The session while the user talks with Claude, through Claude Code's input.
pub struct Conversation<'a> {
    session: Session,
    to_claude: ChildStdin,
    claude: &'a Child,
    request_count: u64,
}

impl<'a> Conversation<'a> {
    pub fn new(
        input: InputLine,
        shared: Arc<Mutex<Shared>>,
        inputs: Receiver<Input>,
        break_signal: Arc<AtomicBool>,
        to_claude: ChildStdin,
        claude: &'a Child,
    ) -> Self {
        Conversation {
            session: Session {
                input,
                shared,
                inputs,
                break_signal,
            },
            to_claude,
            claude,
            request_count: 0,
        }
    }

    /// Runs the conversation until Claude Code ends, or until the user has nothing more to say,
    /// which the ending follows.
    pub fn run(mut self) -> Result<Option<Ending<'a>>, String> {
        self.send(&Command::Initialize)?;
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
                Some(Input::Closed) => {
                    self.session.close_line()?;
                    return Ok(None);
                }
                None => {}
            }
            draft = match self.session.read_line(&Line::Message, &draft)? {
                // A message on a cold cache stays on the input line the first time, and the
                // label says the next Enter sends it
                Outcome::Send(text) => {
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
                Outcome::Interrupt(text) => {
                    let is_working = lock(&self.session.shared).output.status() != Status::Idle;
                    if text.is_empty() && is_working {
                        self.interrupt(Interruption::Stop)?;
                    }
                    String::new()
                }
                // Claude Code asked something, which the next round of the loop answers
                Outcome::Break(text) => text,
                Outcome::End(_) => return Ok(Some(self.end_input())),
            };
        }
    }

    /// Closes Claude Code's input, as the end of input does a program's in the terminal, since
    /// the user has nothing more to say.
    fn end_input(self) -> Ending<'a> {
        let Conversation {
            session,
            to_claude,
            claude,
            request_count: _,
        } = self;
        drop(to_claude);
        Ending { session, claude }
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
                .read_line(&Line::Answer(question.to_string()), &draft)?
            {
                Outcome::Send(text) => {
                    lock(&self.session.shared)
                        .output
                        .show_answer(question, &text);
                    return Ok(Some(text));
                }
                // Claude Code asked something more, which waits for this answer
                Outcome::Break(text) => draft = text,
                Outcome::Interrupt(_) | Outcome::End(_) => return Ok(None),
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
        Ok(())
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

/// The session once the user has nothing more to say: Claude Code's input closed, and Claude
/// finishes its work.
pub struct Ending<'a> {
    session: Session,
    /// Claude Code's process, which Ctrl-C interrupts.
    claude: &'a Child,
}

impl Ending<'_> {
    /// Shows Claude finish its work, until Claude Code exits once its turn and its background
    /// tasks end. Ctrl-C stops them.
    pub fn run(mut self) -> Result<(), String> {
        loop {
            match self.session.next_input() {
                Some(Input::Closed) => return self.session.close_line(),
                // Undocumented, seen with Claude Code 2.1.316: once its input closed, Claude Code
                // fails each permission request, with "Stream closed" as the tool's result
                Some(Input::Ask(_) | Input::Unsupported { .. }) | None => {}
            }
            match self.session.read_line(&Line::Ending, "")? {
                Outcome::Interrupt(_) => self.interrupt()?,
                // What the user types has no one left to read it
                Outcome::Send(_) | Outcome::End(_) | Outcome::Break(_) => {}
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
        // SAFETY: kill only sends a signal. The id still names Claude Code's process: the system
        // keeps a child's id until its parent waits for it, and waiting needs exclusive access
        // to the child, which this borrow of it rules out
        if unsafe { libc::kill(self.claude.id().cast_signed(), libc::SIGINT) } == -1 {
            return Err(format!(
                "Cannot stop claude: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

/// The terminal's side of the session: the input line, and what it shows of Claude's.
struct Session {
    input: InputLine,
    shared: Arc<Mutex<Shared>>,
    inputs: Receiver<Input>,
    break_signal: Arc<AtomicBool>,
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

    /// Reads the input line, starting from `draft`. Ctrl-D ends the input only on an empty line,
    /// as in a shell, so on a line with text the line stays open as it was.
    fn read_line(&mut self, line: &Line, draft: &str) -> Result<Outcome, String> {
        let mut draft = draft.to_string();
        loop {
            match self.input.read(line.clone(), &draft)? {
                Outcome::End(text) if !text.is_empty() => draft = text,
                outcome => return Ok(outcome),
            }
        }
    }

    /// Closes the input line once Claude Code ended, so the end of the output takes its place.
    fn close_line(&mut self) -> Result<(), String> {
        self.input.close()
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
