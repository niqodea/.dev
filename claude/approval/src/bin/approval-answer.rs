//! Answers the requests waiting in an approval queue, in the terminal: y or n
//! stages a decision on each, from the list or from a request's details, and
//! Enter carries the staged ones out.

use std::collections::BTreeMap;
use std::fs::{self, File, TryLockError};
use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::path::Path;
use std::process::ExitCode;

use approval::{
    Actions, Decision, Job, Outcome, Queue, QueuedJob, format_age, get_now_ns, read_queue_name,
    read_variable,
};
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};

const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
// Only for failures, since red reads as an error
const RED: &str = "\x1b[31m";
// For secondary text, like ages and key hints
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const CLEAR_SCREEN: &str = "\x1b[H\x1b[2J";
const HIDE_CURSOR: &str = "\x1b[?25l";
const SHOW_CURSOR: &str = "\x1b[?25h";
// The width the terminal gives a tab
const TAB_WIDTH: usize = 8;

/// Hides the terminal's cursor until it is dropped, since the > mark shows the
/// selected job, and the cursor would only point somewhere else.
struct HiddenCursor;

/// Keeps the terminal handing over each key as it is pressed, without echoing
/// it, until it is dropped.
struct CbreakMode {
    original: libc::termios,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let home = read_variable("HOME")?;
    let queue = Queue::new(&home, read_queue_name()?);
    let actions = Actions::new(&home, read_variable("PATH")?);
    let lock_path = Path::new(&home)
        .join(".local/state/approval-answer")
        .join(format!("{}.lock", queue.name()));
    let lock_directory = lock_path.parent().expect("a lock path is in a directory");
    fs::create_dir_all(lock_directory)
        .map_err(|error| format!("Cannot create {}: {error}", lock_directory.display()))?;
    let lock = File::create(&lock_path)
        .map_err(|error| format!("Cannot open {}: {error}", lock_path.display()))?;
    let _hidden_cursor = HiddenCursor::new()?;
    // One screen per queue, so no job is answered twice. The system drops the
    // lock when the process ends, however it ends
    match lock.try_lock() {
        Ok(()) => {
            let jobs = queue.read_queued_jobs()?;
            answer(&queue, &actions, &jobs)
        }
        Err(TryLockError::WouldBlock) => {
            println!("This queue is already open somewhere else. Any key closes");
            read_key()?;
            Ok(())
        }
        Err(TryLockError::Error(error)) => {
            Err(format!("Cannot lock {}: {error}", lock_path.display()))
        }
    }
}

/// Show every waiting job, stage decisions on them, and carry those out on
/// Enter.
fn answer(queue: &Queue, actions: &Actions, jobs: &[QueuedJob]) -> Result<(), String> {
    if jobs.is_empty() {
        println!("No request waits in this queue. Any key closes");
        read_key()?;
        return Ok(());
    }
    let mut answers: BTreeMap<usize, Decision> = BTreeMap::new();
    let mut cursor = 0;
    loop {
        draw_jobs(jobs, &answers, &BTreeMap::new(), Some(cursor));
        println!(
            "{DIM}j/k move   y approve   n refuse   o open details   Enter commit   N refuse all   q quit{RESET}"
        );
        let decision = match read_key()? {
            b'j' => {
                cursor = (cursor + 1).min(jobs.len() - 1);
                None
            }
            b'k' => {
                cursor = cursor.saturating_sub(1);
                None
            }
            b'y' => Some(Decision::Approve),
            b'n' => Some(Decision::Refuse),
            b'o' if !jobs[cursor].job.fields.is_empty() => {
                show_details(&jobs[cursor].job, answers.get(&cursor).copied())?
            }
            // cbreak mode turns the Enter key's carriage return into a newline
            b'\n' if answers.is_empty() => {
                // With nothing staged, there is no outcome to show
                return Ok(());
            }
            b'\n' => {
                let outcomes = commit(queue, actions, jobs, &answers);
                draw_jobs(jobs, &answers, &outcomes, None);
                println!("Any key closes");
                read_key()?;
                return Ok(());
            }
            b'N' => {
                let refusals = (0..jobs.len())
                    .map(|index| (index, Decision::Refuse))
                    .collect();
                commit(queue, actions, jobs, &refusals);
                return Ok(());
            }
            b'q' => return Ok(()),
            _ => None,
        };
        if let Some(decision) = decision {
            // Pressing y or n again on a job that already has that decision
            // clears it
            if answers.get(&cursor) == Some(&decision) {
                answers.remove(&cursor);
            } else {
                answers.insert(cursor, decision);
            }
            cursor = (cursor + 1).min(jobs.len() - 1);
        }
    }
}

/// Draw one row per job in aligned columns, with its answer, a mark when it has
/// fields, a line below for its outcome, and the cursor.
fn draw_jobs(
    jobs: &[QueuedJob],
    answers: &BTreeMap<usize, Decision>,
    outcomes: &BTreeMap<usize, Result<Outcome, String>>,
    cursor: Option<usize>,
) {
    // Each key press redraws the jobs in place
    print!("{CLEAR_SCREEN}");
    let now_ns = get_now_ns();
    let rows: Vec<Vec<String>> = jobs
        .iter()
        .map(|queued_job| {
            let age = format_age(now_ns.saturating_sub(queued_job.queued_at_ns));
            [age]
                .into_iter()
                .chain(
                    queued_job
                        .job
                        .columns
                        .iter()
                        .map(|cell| show_controls(cell)),
                )
                .collect()
        })
        .collect();
    // Actions of different CLIs print different numbers of columns
    let mut widths: Vec<usize> = Vec::new();
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            let width = cell.chars().count();
            match widths.get_mut(index) {
                Some(widest) => *widest = (*widest).max(width),
                None => widths.push(width),
            }
        }
    }
    for (index, (queued_job, row)) in jobs.iter().zip(&rows).enumerate() {
        let pointer = if cursor == Some(index) { ">" } else { " " };
        let decision_mark = format_mark(answers.get(&index).copied());
        let fields_mark = if queued_job.job.fields.is_empty() {
            " ".to_string()
        } else {
            format!("{DIM}+{RESET}")
        };
        let cells: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        let row_text = cells[1..].join("  ");
        println!(
            "{pointer} {decision_mark} {fields_mark}  {DIM}{}{RESET}  {}",
            cells[0],
            row_text.trim_end()
        );
        // Blank until the job has an outcome, so no row moves when one appears
        match outcomes.get(&index) {
            Some(outcome) => println!("       {}", format_outcome(outcome)),
            None => println!(),
        }
    }
    println!();
}

/// Show the job's CLI and action with its fields below, which j and k scroll,
/// until the person presses y or n to decide on the job, or q to go back
/// without deciding.
fn show_details(job: &Job, decision: Option<Decision>) -> Result<Option<Decision>, String> {
    let mut first_line = 0;
    loop {
        // Read on every key press, so the fields fit the terminal after a resize
        let (width, height) = get_terminal_size()?;
        let lines = format_fields(&job.fields, width);
        // The header, the keys, and a blank line below the header and above the
        // keys take 4 lines
        let line_count = height.saturating_sub(4).max(1);
        first_line = first_line.min(lines.len().saturating_sub(line_count));
        let shown = &lines[first_line..lines.len().min(first_line + line_count)];
        print!("{CLEAR_SCREEN}");
        println!(
            "{}  {BOLD}{} {}{RESET}",
            format_mark(decision),
            job.request.cli,
            job.request.action
        );
        println!();
        println!("{}", shown.join("\n"));
        println!();
        print!("{DIM}j/k scroll   y approve   n refuse   q back{RESET}");
        io::stdout()
            .flush()
            .map_err(|error| format!("Cannot draw: {error}"))?;
        match read_key()? {
            b'j' => first_line += 1,
            b'k' => first_line = first_line.saturating_sub(1),
            b'y' => return Ok(Some(Decision::Approve)),
            b'n' => return Ok(Some(Decision::Refuse)),
            b'q' => return Ok(None),
            _ => {}
        }
    }
}

/// Lay out the fields as a table with a name column and a value column.
///
/// A long value wraps to the width and continues down the value column.
fn format_fields(fields: &[(String, String)], width: usize) -> Vec<String> {
    let names: Vec<String> = fields.iter().map(|(name, _)| show_controls(name)).collect();
    let name_width = names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0);
    // Wrapped, not cut off, so no word of a value is hidden
    let value_width = width.saturating_sub(name_width + 2).max(1);
    let mut lines = Vec::new();
    for (name, (_, value)) in names.iter().zip(fields) {
        let value = show_controls(value);
        let value_lines: Vec<String> = value
            .split('\n')
            .flat_map(|line| wrap(&expand_tabs(line), value_width))
            .collect();
        for (index, value_line) in value_lines.iter().enumerate() {
            let label = match index {
                0 => format!("{DIM}{name:<name_width$}{RESET}"),
                _ => " ".repeat(name_width),
            };
            lines.push(format!("{label}  {value_line}").trim_end().to_string());
        }
    }
    lines
}

/// Write control characters and Unicode format characters as escapes, like
/// \u{1b}, except line breaks and tabs.
///
/// A control character can move the cursor, clear the screen or change colors.
/// A format character can be invisible or reverse the order text is shown in.
/// Either can make the screen show something other than the text.
fn show_controls(text: &str) -> String {
    text.chars()
        .map(|character| match character.general_category() {
            GeneralCategory::Control | GeneralCategory::Format
                if !matches!(character, '\n' | '\t') =>
            {
                character.escape_default().to_string()
            }
            _ => character.to_string(),
        })
        .collect()
}

/// Break the line into lines no wider than the width: after the last word that
/// fits, or inside a word wider than a whole line.
///
/// The spaces at the start of the line and between words stay as they are,
/// since they can be part of what the value says, like the indentation of a
/// quote.
fn wrap(line: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest: Vec<char> = line.chars().collect();
    while rest.len() > width {
        // A space right after the last character that fits is a break too
        let space_index = rest[..=width]
            .iter()
            .rposition(|&character| character == ' ')
            .filter(|&index| rest[..index].iter().any(|&character| character != ' '));
        let break_index = space_index.unwrap_or(width);
        let head: String = rest[..break_index].iter().collect();
        lines.push(head.trim_end().to_string());
        rest = rest[break_index..]
            .iter()
            .skip_while(|&&character| character == ' ')
            .copied()
            .collect();
    }
    lines.push(rest.into_iter().collect());
    lines
}

/// Replace each tab with the spaces up to the next tab stop, so the wrapped
/// lines are as wide as the terminal shows them.
fn expand_tabs(line: &str) -> String {
    let mut expanded = String::new();
    let mut column = 0;
    for character in line.chars() {
        if character == '\t' {
            let spaces = TAB_WIDTH - column % TAB_WIDTH;
            expanded.push_str(&" ".repeat(spaces));
            column += spaces;
        } else {
            expanded.push(character);
            column += 1;
        }
    }
    expanded
}

/// Carry out the staged decisions in queue order, and return the outcome of
/// each, or why it could not be carried out.
///
/// A decision that cannot be carried out leaves the others to go ahead, since
/// each job is answered on its own.
fn commit(
    queue: &Queue,
    actions: &Actions,
    jobs: &[QueuedJob],
    answers: &BTreeMap<usize, Decision>,
) -> BTreeMap<usize, Result<Outcome, String>> {
    let mut outcomes = BTreeMap::new();
    for (&index, &decision) in answers {
        let result = queue.decide(actions, jobs[index].queued_at_ns, decision);
        outcomes.insert(index, result.map(|answered_job| answered_job.outcome));
    }
    outcomes
}

/// Read one key press, without waiting for Enter.
fn read_key() -> Result<u8, String> {
    let _cbreak_mode = CbreakMode::new()?;
    let mut key = [0];
    io::stdin()
        .read_exact(&mut key)
        .map_err(|error| format!("Cannot read a key: {error}"))?;
    Ok(key[0])
}

fn get_terminal_size() -> Result<(usize, usize), String> {
    let mut size = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: TIOCGWINSZ writes a winsize to the pointer, which points at one
    let status = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) };
    if status == -1 {
        let error = io::Error::last_os_error();
        return Err(format!("Cannot read the terminal's size: {error}"));
    }
    Ok((usize::from(size.ws_col), usize::from(size.ws_row)))
}

fn format_mark(decision: Option<Decision>) -> String {
    match decision {
        None => " ".to_string(),
        Some(Decision::Approve) => format!("{GREEN}y{RESET}"),
        Some(Decision::Refuse) => format!("{YELLOW}n{RESET}"),
    }
}

fn format_outcome(outcome: &Result<Outcome, String>) -> String {
    match outcome {
        Ok(Outcome::Succeeded { .. }) => format!("{GREEN}succeeded{RESET}"),
        Ok(Outcome::Refused) => format!("{YELLOW}refused{RESET}"),
        Ok(Outcome::Failed { error }) => format!("{RED}failed: {error}{RESET}"),
        Err(error) => format!("{RED}not carried out: {error}{RESET}"),
    }
}

impl HiddenCursor {
    fn new() -> Result<Self, String> {
        print!("{HIDE_CURSOR}");
        io::stdout()
            .flush()
            .map_err(|error| format!("Cannot hide the cursor: {error}"))?;
        Ok(HiddenCursor)
    }
}

impl Drop for HiddenCursor {
    fn drop(&mut self) {
        print!("{SHOW_CURSOR}");
        // Nothing is left to report to once the screen closes
        let _ = io::stdout().flush();
    }
}

impl CbreakMode {
    fn new() -> Result<Self, String> {
        let mut settings = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: tcgetattr fills the termios when it returns 0
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, settings.as_mut_ptr()) } == -1 {
            let error = io::Error::last_os_error();
            return Err(format!("Cannot read the terminal's settings: {error}"));
        }
        // SAFETY: tcgetattr returned 0 above, so it filled the termios
        let original = unsafe { settings.assume_init() };
        let mut cbreak = original;
        cbreak.c_lflag &= !(libc::ICANON | libc::ECHO);
        cbreak.c_cc[libc::VMIN] = 1;
        cbreak.c_cc[libc::VTIME] = 0;
        // SAFETY: the termios is a valid copy of the terminal's own settings
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const cbreak) } == -1 {
            let error = io::Error::last_os_error();
            return Err(format!("Cannot change the terminal's settings: {error}"));
        }
        Ok(CbreakMode { original })
    }
}

impl Drop for CbreakMode {
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
