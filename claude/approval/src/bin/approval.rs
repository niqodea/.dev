//! Runs an action a CLI requests right away when the CLI's auto-approved file
//! lists the request, and otherwise queues it for a person to decide on.

use std::process::ExitCode;

use approval::{
    Actions, Outcome, Queue, Request, format_age, get_now_ns, read_queue_name, read_variable,
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
    let queue = Queue::new(&home, read_queue_name()?);
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
    let job = actions.describe(request)?;
    if actions.is_auto_approved(&job.request)? {
        return match actions.run(&job.request) {
            Ok(output) => {
                let answered_job = queue.record(job, Outcome::Succeeded { output })?;
                println!(
                    "{}",
                    format_outcome(&answered_job.job.columns, &answered_job.outcome)
                );
                Ok(())
            }
            Err(error) => {
                let answered_job = queue.record(job, Outcome::Failed { error })?;
                Err(format_outcome(
                    &answered_job.job.columns,
                    &answered_job.outcome,
                ))
            }
        };
    }
    if queue.count_pending_jobs()? >= MAX_PENDING_JOBS {
        return Err(format!(
            "Not queued: {MAX_PENDING_JOBS} requests already wait for a decision in this queue"
        ));
    }
    queue.add(&job)?;
    println!(
        "waiting: {}. Someone with write access will decide whether to do it",
        format_row(&job.columns)
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
        println!("  {age}  {}", format_row(&queued_job.job.columns));
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
        let outcome = format_outcome(&answered_job.job.columns, &answered_job.outcome);
        println!("  {age}  {outcome}");
    }
    Ok(())
}

fn format_row(columns: &[String]) -> String {
    columns.join("  ")
}

fn format_outcome(columns: &[String], outcome: &Outcome) -> String {
    let row = format_row(columns);
    match outcome {
        Outcome::Succeeded { output } if output.is_empty() => format!("succeeded: {row}"),
        Outcome::Succeeded { output } => format!("succeeded: {row}: {output}"),
        Outcome::Refused => format!("refused: {row}"),
        Outcome::Failed { error } => format!("failed: {row}: {error}"),
    }
}
