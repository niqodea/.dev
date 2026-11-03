//! Runs an action a CLI requests right away when the CLI's auto-approved file
//! lists the request, and otherwise queues it for a person to decide on.
//!
//! A CLI requests an action through the submit link in its own folder of the
//! clis directory, which starts this program under that name, so the folder
//! names the CLI and a CLI can only request its own actions.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use approval::{
    Actions, Outcome, Queue, Request, format_age, get_last_line, get_now_ns, read_queue_directory,
    read_variable, show_controls,
};
use clap::{Parser, Subcommand};

const EXAMPLES: &str = "\
Examples:
  ,approval status                 what waits, and what was decided
  ,approval count                  how many requests wait

A CLI requests an action with the submit link in its folder, like
  ~/.config/approval/clis/avatar/submit approve-consent '{\"environment\": \"prod\", ...}'";
// Few enough that the person reads every one, and the agent waits for decisions
// instead of piling up more
const MAX_PENDING_JOBS: usize = 5;
const STATUS_ANSWERED_COUNT: usize = 10;

/// Print what waits in the queue, and what was decided.
#[derive(Parser)]
#[command(name = ",approval", after_help = EXAMPLES)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Subcommand)]
enum Command {
    /// Print what waits, and what was decided
    Status,
    /// Print how many requests wait
    Count,
}

/// Run one of this CLI's actions, or queue it for a person to decide on.
#[derive(Parser)]
#[command(name = "submit")]
struct SubmitArguments {
    action: String,
    /// The action's arguments, as a JSON object from each name to its value
    arguments: String,
}

fn main() -> ExitCode {
    let program = env::args_os().next().unwrap_or_default();
    let result = if Path::new(&program).file_name() == Some("submit".as_ref()) {
        let arguments = SubmitArguments::parse();
        run_submit(&program, &arguments.action, &arguments.arguments)
    } else {
        run(Arguments::parse().command)
    };
    match result {
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
        Command::Status => print_status(&queue),
        Command::Count => {
            println!("{}", queue.count_pending_jobs()?);
            Ok(())
        }
    }
}

fn run_submit(program: &OsString, action: &str, arguments: &str) -> Result<(), String> {
    let home = read_variable("HOME")?;
    let actions = Actions::new(&home, read_variable("PATH")?);
    let cli = read_submitting_cli(&actions, Path::new(program))?;
    let arguments = serde_json::from_str(arguments)
        .map_err(|error| format!("Invalid request: the arguments are no JSON object: {error}"))?;
    let request = Request {
        cli,
        action: action.to_string(),
        arguments,
    };
    let queue = Queue::new(read_queue_directory(&home)?);
    submit(&actions, &queue, request)
}

/// Return the CLI whose submit link started this program: the folder the link
/// is in, which has to be one of the clis directory's.
fn read_submitting_cli(actions: &Actions, program: &Path) -> Result<String, String> {
    // The folder, not the link, since the link leads to this program
    let folder = program
        .parent()
        .map(fs::canonicalize)
        .transpose()
        .ok()
        .flatten();
    let clis_directory = fs::canonicalize(actions.get_clis_directory()).ok();
    match (folder, clis_directory) {
        (Some(folder), Some(clis_directory)) if folder.parent() == Some(&clis_directory) => {
            let name = folder
                .file_name()
                .expect("a folder inside another has a name");
            Ok(name.to_string_lossy().into_owned())
        }
        _ => Err(format!(
            "Started as {}, which is no submit link in a CLI's folder of {}",
            program.display(),
            actions.get_clis_directory().display()
        )),
    }
}

fn submit(actions: &Actions, queue: &Queue, request: Request) -> Result<(), String> {
    actions.check(&request)?;
    if actions.is_auto_approved(&request)? {
        return match actions.run(&request) {
            Ok(output) => {
                let answered_job = queue.record(request, Outcome::Succeeded { output })?;
                println!(
                    "{}",
                    format_outcome(&answered_job.request, &answered_job.outcome)
                );
                Ok(())
            }
            Err(error) => {
                let answered_job = queue.record(request, Outcome::Failed { error })?;
                Err(format_outcome(&answered_job.request, &answered_job.outcome))
            }
        };
    }
    if queue.count_pending_jobs()? >= MAX_PENDING_JOBS {
        return Err(format!(
            "Not queued: {MAX_PENDING_JOBS} requests already wait for a decision in this queue"
        ));
    }
    queue.add(&request)?;
    queue.signal_answering_screen();
    actions.run_change_hook();
    println!(
        "waiting: {}. Someone with write access will decide whether to do it",
        request.format()
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
        println!("  {age}  {}", queued_job.request.format());
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
fn format_outcome(request: &Request, outcome: &Outcome) -> String {
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
    use serde_json::json;

    use super::*;

    fn note() -> Request {
        Request {
            cli: "demo".to_string(),
            action: "note".to_string(),
            arguments: IndexMap::from([("title".to_string(), json!("it's done"))]),
        }
    }

    #[test]
    fn format_outcome_keeps_all_of_an_output_and_the_last_line_of_an_error() {
        let succeeded = Outcome::Succeeded {
            output: "saved\n3 lines".to_string(),
        };
        assert_eq!(
            format_outcome(&note(), &succeeded),
            r#"succeeded: demo note {"title":"it's done"}: saved
3 lines"#
        );
        let failed = Outcome::Failed {
            error: "Traceback\nOSError: disk full".to_string(),
        };
        assert_eq!(
            format_outcome(&note(), &failed),
            r#"failed: demo note {"title":"it's done"}: OSError: disk full"#
        );
        let empty = Outcome::Succeeded {
            output: String::new(),
        };
        assert_eq!(
            format_outcome(&note(), &empty),
            r#"succeeded: demo note {"title":"it's done"}"#
        );
    }

    #[test]
    fn format_outcome_escapes_what_the_action_printed() {
        let succeeded = Outcome::Succeeded {
            output: "\x1b[2Jcleared\rover\u{202e}".to_string(),
        };
        assert_eq!(
            format_outcome(&note(), &succeeded),
            r#"succeeded: demo note {"title":"it's done"}: \u{1b}[2Jcleared\rover\u{202e}"#
        );
    }

    #[test]
    fn read_submitting_cli_takes_the_folder_of_the_link_in_the_clis_directory() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().to_str().unwrap();
        let actions = Actions::new(home, String::new());
        let cli_directory = directory.path().join(".config/approval/clis/demo");
        fs::create_dir_all(&cli_directory).unwrap();
        assert_eq!(
            read_submitting_cli(&actions, &cli_directory.join("submit")).unwrap(),
            "demo"
        );
        let elsewhere = directory.path().join("submit");
        assert!(read_submitting_cli(&actions, &elsewhere).is_err());
        let too_deep = cli_directory.join("nested");
        fs::create_dir_all(&too_deep).unwrap();
        assert!(read_submitting_cli(&actions, &too_deep.join("submit")).is_err());
    }
}
