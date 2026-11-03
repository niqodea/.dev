//! Runs an action a CLI requests right away when the CLI's auto-approved file
//! lists the request, and otherwise queues it for a person to decide on.

use std::process::ExitCode;

use approval::{
    Actions, Outcome, ParsedRequest, Queue, Request, format_age, get_last_line, get_now_ns,
    read_queue_directory, read_variable, show_controls,
};
use clap::{Parser, Subcommand};

const EXAMPLES: &str = "\
Examples:
  ,approval submit avatar approve-consent prod c8169abc-... 2026-10-29T11:00:00Z
                                   request a CLI's action
  ,approval status                 what waits, and what was decided";
// Few enough that the person reads every one, and the agent waits for decisions
// instead of piling up more
const MAX_PENDING_JOBS: usize = 5;
const STATUS_ANSWERED_COUNT: usize = 10;

/// Run an action a CLI requests, or queue it for a person to decide on.
#[derive(Parser)]
#[command(name = ",approval", after_help = EXAMPLES)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a CLI's action, or queue it for a person to decide on
    Submit {
        cli: String,
        action: String,
        // Everything after the action belongs to it, even what looks like a
        // flag of ours
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
    /// Print what waits, and what was decided
    Status,
    /// Print how many requests wait
    Count,
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    match run(arguments.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
    let home = read_variable("HOME")?;
    let queue = Queue::new(read_queue_directory(&home)?);
    match command {
        Command::Submit {
            cli,
            action,
            arguments,
        } => {
            let actions = Actions::new(&home, read_variable("PATH")?);
            let request = Request {
                cli,
                action,
                arguments,
            };
            submit(&actions, &queue, request)
        }
        Command::Status => print_status(&queue),
        Command::Count => {
            println!("{}", queue.count_pending_jobs()?);
            Ok(())
        }
    }
}

fn submit(actions: &Actions, queue: &Queue, request: Request) -> Result<(), String> {
    let parsed_request = actions.parse(request)?;
    if actions.is_auto_approved(&parsed_request)? {
        return match actions.run(&parsed_request) {
            Ok(output) => {
                let answered_job = queue.record(parsed_request, Outcome::Succeeded { output })?;
                println!(
                    "{}",
                    format_outcome(&answered_job.request, &answered_job.outcome)
                );
                Ok(())
            }
            Err(error) => {
                let answered_job = queue.record(parsed_request, Outcome::Failed { error })?;
                Err(format_outcome(&answered_job.request, &answered_job.outcome))
            }
        };
    }
    if queue.count_pending_jobs()? >= MAX_PENDING_JOBS {
        return Err(format!(
            "Not queued: {MAX_PENDING_JOBS} requests already wait for a decision in this queue"
        ));
    }
    queue.add(&parsed_request)?;
    println!(
        "waiting: {}. Someone with write access will decide whether to do it",
        parsed_request.format()
    );
    Ok(())
}

fn print_status(queue: &Queue) -> Result<(), String> {
    let now_ns = get_now_ns();
    let queued_jobs = queue.read_queued_jobs()?;
    println!(
        "waiting ({} of at most {MAX_PENDING_JOBS}):",
        queued_jobs.len()
    );
    for queued_job in &queued_jobs {
        let age = format_age(now_ns.saturating_sub(queued_job.queued_at_ns));
        println!("  {age}  {}", queued_job.parsed_request.format());
    }
    let answered_jobs = queue.read_answered_jobs()?;
    let shown = &answered_jobs[answered_jobs.len().saturating_sub(STATUS_ANSWERED_COUNT)..];
    println!(
        "answered (last {} of {}):",
        shown.len(),
        answered_jobs.len()
    );
    for answered_job in shown {
        let age = format_age(now_ns.saturating_sub(answered_job.answered_at_ns));
        // Indented past the age, so an output's later lines stay under their
        // row instead of reading as rows of their own
        let outcome =
            format_outcome(&answered_job.request, &answered_job.outcome).replace('\n', "\n       ");
        println!("  {age}  {outcome}");
    }
    Ok(())
}

/// Write the outcome after the request, with all an action printed when it
/// succeeded, and the last line, which says what went wrong, when it failed.
fn format_outcome(request: &ParsedRequest, outcome: &Outcome) -> String {
    let row = request.format();
    let text = match outcome {
        Outcome::Succeeded { output } if output.is_empty() => format!("succeeded: {row}"),
        Outcome::Succeeded { output } => format!("succeeded: {row}: {output}"),
        Outcome::Refused => format!("refused: {row}"),
        Outcome::Failed { error } => format!("failed: {row}: {}", get_last_line(error)),
    };
    // What an action prints could move the cursor or hide text, like the rows
    // around it
    show_controls(&text)
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;

    fn note() -> ParsedRequest {
        ParsedRequest {
            cli: "demo".to_string(),
            action: "note".to_string(),
            arguments: IndexMap::from([(
                "title".to_string(),
                approval::Value::One("it's done".to_string()),
            )]),
        }
    }

    #[test]
    fn format_outcome_keeps_all_of_an_output_and_the_last_line_of_an_error() {
        let succeeded = Outcome::Succeeded {
            output: "saved\n3 lines".to_string(),
        };
        assert_eq!(
            format_outcome(&note(), &succeeded),
            "succeeded: demo note 'it'\\''s done': saved\n3 lines"
        );
        let failed = Outcome::Failed {
            error: "Traceback\nOSError: disk full".to_string(),
        };
        assert_eq!(
            format_outcome(&note(), &failed),
            "failed: demo note 'it'\\''s done': OSError: disk full"
        );
        let empty = Outcome::Succeeded {
            output: String::new(),
        };
        assert_eq!(
            format_outcome(&note(), &empty),
            "succeeded: demo note 'it'\\''s done'"
        );
    }

    #[test]
    fn format_outcome_escapes_what_the_action_printed() {
        let succeeded = Outcome::Succeeded {
            output: "\x1b[2Jcleared\rover\u{202e}".to_string(),
        };
        assert_eq!(
            format_outcome(&note(), &succeeded),
            "succeeded: demo note 'it'\\''s done': \\u{1b}[2Jcleared\\rover\\u{202e}"
        );
    }
}
