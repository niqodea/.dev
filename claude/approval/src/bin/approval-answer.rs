//! Answers the requests waiting in an approval queue, in the terminal: y or n
//! stages a decision on each, from the list or from a request's details, and
//! Enter carries the staged ones out.

use std::collections::BTreeMap;
use std::fs::{self, File, TryLockError};
use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::path::Path;
use std::process::{self, ExitCode};

use approval::{
    Actions, Decision, Outcome, Queue, QueuedJob, Request, format_age, get_last_line, get_now_ns,
    read_queue_directory, read_variable, show_controls,
};
use serde_json::Value;

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
// The alternate screen keeps no history, so the frames drawn on it never pile
// up in the terminal's scrollback, and leaving it brings back what was there
const ENTER_ALTERNATE_SCREEN: &str = "\x1b[?1049h";
const LEAVE_ALTERNATE_SCREEN: &str = "\x1b[?1049l";
// The width the terminal gives a tab
const TAB_WIDTH: usize = 8;

/// Draws on the terminal's alternate screen, with the cursor hidden, until it is
/// dropped. The > mark shows the selected job, and the cursor would only point
/// somewhere else.
struct FullScreen;

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
    let queue = Queue::new(read_queue_directory(&home)?);
    let actions = Actions::new(&home, read_variable("PATH")?);
    let lock_path = queue.get_lock_path();
    let lock_directory = lock_path.parent().expect("a lock path is in a directory");
    fs::create_dir_all(lock_directory)
        .map_err(|error| format!("Cannot create {}: {error}", lock_directory.display()))?;
    // Not emptied on opening, since it holds the process ID of the screen
    // that has the lock, which a screen that finds it taken must leave alone
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|error| format!("Cannot open {}: {error}", lock_path.display()))?;
    interrupt_on_signals()?;
    let _full_screen = FullScreen::new()?;
    // One screen per queue, so no job is answered twice. The system drops the
    // lock when the process ends, however it ends
    match lock.try_lock() {
        Ok(()) => {
            // Before the jobs are read, so a job queued before ,approval can
            // read the ID is among them
            write_process_id(&lock, &lock_path)?;
            answer(&queue, &actions)
        }
        Err(TryLockError::WouldBlock) => {
            println!("This queue is already open somewhere else. Any key closes");
            while read_key()?.is_none() {}
            Ok(())
        }
        Err(TryLockError::Error(error)) => {
            Err(format!("Cannot lock {}: {error}", lock_path.display()))
        }
    }
}

/// Write this process's ID in the lock file, so ,approval can tell the screen
/// when it queues a job.
fn write_process_id(lock: &File, lock_path: &Path) -> Result<(), String> {
    let mut file = lock;
    file.set_len(0)
        .and_then(|()| write!(file, "{}", process::id()))
        .map_err(|error| format!("Cannot write {}: {error}", lock_path.display()))
}

/// Show every waiting job, stage decisions on them, and carry those out on
/// Enter.
fn answer(queue: &Queue, actions: &Actions) -> Result<(), String> {
    let mut jobs = queue.read_queued_jobs()?;
    if jobs.is_empty() {
        println!("No request waits in this queue. Any key closes");
        while read_key()?.is_none() {}
        return Ok(());
    }
    let mut answers: BTreeMap<usize, Decision> = BTreeMap::new();
    let mut cursor = 0;
    loop {
        draw_jobs(&jobs, &answers, &BTreeMap::new(), Some(cursor), None)?;
        print_keys(
            "j/k move   y approve   n refuse   o open details   Enter commit   N refuse all   q quit",
        )?;
        let decision = match read_key()? {
            Some(b'j') => {
                cursor = (cursor + 1).min(jobs.len() - 1);
                None
            }
            Some(b'k') => {
                cursor = cursor.saturating_sub(1);
                None
            }
            Some(b'y') => Some(Decision::Approve),
            Some(b'n') => Some(Decision::Refuse),
            Some(b'o') => show_details(
                &jobs[cursor].request,
                answers.get(&cursor).copied(),
                &Stage::Deciding,
            )?,
            // cbreak mode turns the Enter key's carriage return into a newline
            Some(b'\n') if answers.is_empty() => {
                // With nothing staged, there is no outcome to show
                return Ok(());
            }
            Some(b'\n') => {
                let outcomes = commit(queue, actions, &jobs, &answers)?;
                return show_outcomes(&jobs, &answers, &outcomes);
            }
            Some(b'N') => {
                let refusals = (0..jobs.len())
                    .map(|index| (index, Decision::Refuse))
                    .collect();
                commit(queue, actions, &jobs, &refusals)?;
                return Ok(());
            }
            Some(b'q') => return Ok(()),
            // A resize redraws the jobs at the new size, and a job queued
            // while the screen is open, which ,approval signals, adds one
            // below. Only this screen takes jobs out, so the list only grows,
            // and every row keeps its place and its decision
            None => {
                jobs = queue.read_queued_jobs()?;
                None
            }
            Some(_) => None,
        };
        if let Some(decision) = decision {
            // Pressing y or n again on a job that already has that decision
            // clears it, and leaves the cursor on the job, since clearing
            // corrects it instead of finishing it
            if answers.get(&cursor) == Some(&decision) {
                answers.remove(&cursor);
            } else {
                answers.insert(cursor, decision);
                cursor = (cursor + 1).min(jobs.len() - 1);
            }
        }
    }
}

/// Whether the decisions were carried out yet, and so whether the details
/// screen decides on its job or shows how it went
enum Stage<'a> {
    Deciding,
    // The job's outcome, when it was answered
    Answered(Option<&'a Result<Outcome, String>>),
}

/// Show the outcome of each answered job below it, and the details of the job
/// under the cursor on o, until the person presses q.
///
/// At least one job was decided on, since Enter with none closes the screen.
fn show_outcomes(
    jobs: &[QueuedJob],
    answers: &BTreeMap<usize, Decision>,
    outcomes: &BTreeMap<usize, Result<Outcome, String>>,
) -> Result<(), String> {
    // The cursor moves only through the jobs decided on, the others staying in
    // view as they were
    let decided: Vec<usize> = answers.keys().copied().collect();
    let mut position = 0;
    loop {
        let cursor = decided[position];
        draw_jobs(jobs, answers, outcomes, Some(cursor), None)?;
        print_keys("j/k move   o open details   q close")?;
        match read_key()? {
            Some(b'j') => position = (position + 1).min(decided.len() - 1),
            Some(b'k') => position = position.saturating_sub(1),
            Some(b'o') => {
                show_details(
                    &jobs[cursor].request,
                    answers.get(&cursor).copied(),
                    &Stage::Answered(outcomes.get(&cursor)),
                )?;
            }
            Some(b'q') => return Ok(()),
            // A resize, like a zoom, draws the outcomes again at the new size
            _ => {}
        }
    }
}

/// Draw one row per job with its age and request, its answer, a line below for
/// its outcome, and the cursor.
///
/// A request longer than the terminal is cut at its edge, so every row takes
/// one line.
fn draw_jobs(
    jobs: &[QueuedJob],
    answers: &BTreeMap<usize, Decision>,
    outcomes: &BTreeMap<usize, Result<Outcome, String>>,
    cursor: Option<usize>,
    running: Option<usize>,
) -> Result<(), String> {
    // Read on every draw, so the rows fit the terminal after a resize
    let (width, _) = get_terminal_size()?;
    // Each key press redraws the jobs in place
    print!("{CLEAR_SCREEN}");
    let now_ns = get_now_ns();
    for (index, queued_job) in jobs.iter().enumerate() {
        let pointer = if cursor == Some(index) { ">" } else { " " };
        let decision_mark = format_mark(answers.get(&index).copied());
        let age = format_age(now_ns.saturating_sub(queued_job.queued_at_ns));
        let prefix = format!("{pointer} {decision_mark}  {DIM}{age}{RESET}  ");
        let request_width = width.saturating_sub(count_shown_chars(&prefix));
        println!(
            "{prefix}{}",
            cut(&queued_job.request.format(), request_width)
        );
        // Blank until the job has an outcome, so no row moves when one appears
        match outcomes.get(&index) {
            Some(outcome) => println!(
                "       {}",
                format_outcome(outcome, width.saturating_sub(7))
            ),
            None if running == Some(index) => println!("       {DIM}running…{RESET}"),
            None => println!(),
        }
    }
    println!();
    Ok(())
}

/// Print the keys on the terminal's last row, so they stay put however many
/// rows the screen above them has, like a pager's.
///
/// Cut at the terminal's edge, since a line that wraps past the last row
/// scrolls the whole screen up.
fn print_keys(keys: &str) -> Result<(), String> {
    let (width, height) = get_terminal_size()?;
    print!("\x1b[{height};1H{DIM}{}{RESET}", cut(keys, width));
    io::stdout()
        .flush()
        .map_err(|error| format!("Cannot draw: {error}"))
}

/// Count the characters the terminal shows, leaving out color codes like
/// \x1b[2m.
fn count_shown_chars(text: &str) -> usize {
    let mut count = 0;
    let mut is_in_code = false;
    for character in text.chars() {
        match (is_in_code, character) {
            (false, '\x1b') => is_in_code = true,
            (false, _) => count += 1,
            (true, 'm') => is_in_code = false,
            (true, _) => {}
        }
    }
    count
}

/// Cut the text to the width, ending it with … when it is longer.
fn cut(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Show the job's CLI and action with each argument below, next to its name,
/// which j and k scroll, until the person presses y or n to decide on the job,
/// or q to go back without deciding.
///
/// Once the decisions were carried out, the job's outcome, if it has one, shows
/// below the arguments, and y and n do nothing.
///
/// The outcome has no name column, and its first line is colored, which no
/// argument can be, since every argument shows its control characters as
/// escapes. So no argument, whatever its name or value, can pass for the
/// outcome.
fn show_details(
    request: &Request,
    decision: Option<Decision>,
    stage: &Stage,
) -> Result<Option<Decision>, String> {
    let rows: Vec<(String, String)> = request
        .arguments
        .iter()
        .map(|(name, value)| {
            let value = match value {
                // Without the quotes JSON puts around it, as text reads
                Value::String(text) => text.clone(),
                value => value.to_string(),
            };
            (name.clone(), value)
        })
        .collect();
    let outcome = match stage {
        Stage::Answered(outcome) => *outcome,
        Stage::Deciding => None,
    };
    let is_deciding = matches!(stage, Stage::Deciding);
    let keys = if is_deciding {
        "j/k scroll   y approve   n refuse   q back"
    } else {
        "j/k scroll   q back"
    };
    // The outcome is what the person opened an answered job for, and its
    // arguments stay a { away
    let mut first_line = match outcome {
        Some(_) => {
            let (width, _) = get_terminal_size()?;
            let (_, field_starts) = format_details(&rows, outcome, width);
            field_starts[rows.len()]
        }
        None => 0,
    };
    loop {
        // Read on every key press, so the rows fit the terminal after a resize
        let (width, height) = get_terminal_size()?;
        let (lines, field_starts) = format_details(&rows, outcome, width);
        // The header, the keys, and a blank line below the header and above the
        // keys take 4 lines
        let line_count = height.saturating_sub(4).max(1);
        first_line = first_line.min(lines.len().saturating_sub(line_count));
        let shown = &lines[first_line..lines.len().min(first_line + line_count)];
        print!("{CLEAR_SCREEN}");
        println!(
            "{}  {BOLD}{} {}{RESET}",
            format_mark(decision),
            request.cli,
            request.action
        );
        println!();
        println!("{}", shown.join("\n"));
        println!();
        // Only when the rows overflow, since then nothing else shows there is
        // more to scroll to
        let position = if lines.len() > line_count {
            format!(
                "   lines {}–{} of {}",
                first_line + 1,
                first_line + shown.len(),
                lines.len()
            )
        } else {
            String::new()
        };
        print_keys(&format!("{keys}{position}"))?;
        match read_key()? {
            Some(b'j') => first_line += 1,
            Some(b'k') => first_line = first_line.saturating_sub(1),
            Some(b'd') => first_line += (line_count / 2).max(1),
            Some(b'u') => first_line = first_line.saturating_sub((line_count / 2).max(1)),
            Some(b'g') => first_line = 0,
            // Past the last line, which the next draw brings back to the last
            // screenful
            Some(b'G') => first_line = lines.len(),
            Some(b'}') => {
                first_line = field_starts
                    .iter()
                    .copied()
                    .find(|&start| start > first_line)
                    .unwrap_or(lines.len());
            }
            Some(b'{') => {
                first_line = field_starts
                    .iter()
                    .copied()
                    .rfind(|&start| start < first_line)
                    .unwrap_or(0);
            }
            Some(b'y') if is_deciding => return Ok(Some(Decision::Approve)),
            Some(b'n') if is_deciding => return Ok(Some(Decision::Refuse)),
            Some(b'q') => return Ok(None),
            // A resize, like a zoom, lays the rows out again at the new size
            _ => {}
        }
    }
}

/// Lay out the arguments as a table, and the outcome, if any, below them: its
/// word in its color, then what the action printed, in full and as wide as the
/// terminal. Return the lines with the line each argument and the outcome
/// starts on.
fn format_details(
    rows: &[(String, String)],
    outcome: Option<&Result<Outcome, String>>,
    width: usize,
) -> (Vec<String>, Vec<usize>) {
    let (mut lines, mut starts) = format_rows(rows, width);
    if let Some(outcome) = outcome {
        let (color, word, text) = describe_outcome(outcome);
        lines.push(String::new());
        starts.push(lines.len());
        lines.push(format!("{color}{word}{RESET}"));
        if !text.is_empty() {
            // What an action prints could move the cursor or hide text, like
            // its arguments
            lines.extend(
                show_controls(text)
                    .split('\n')
                    .flat_map(|line| wrap(&expand_tabs(line), width.max(1))),
            );
        }
    }
    (lines, starts)
}

/// Return the outcome's color, its word, and what the action printed, which is
/// empty for a refusal.
fn describe_outcome(outcome: &Result<Outcome, String>) -> (&'static str, &'static str, &str) {
    match outcome {
        Ok(Outcome::Succeeded { output }) => (GREEN, "succeeded", output),
        Ok(Outcome::Refused) => (YELLOW, "refused", ""),
        Ok(Outcome::Failed { error }) => (RED, "failed", error),
        Err(error) => (RED, "not carried out", error),
    }
}

/// Lay out the rows as a table with a name column and a value column, and
/// return its lines with the line each row starts on.
///
/// A long value wraps to the width and continues down the value column.
fn format_rows(rows: &[(String, String)], width: usize) -> (Vec<String>, Vec<usize>) {
    let names: Vec<String> = rows.iter().map(|(name, _)| show_controls(name)).collect();
    let name_width = names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0);
    // Wrapped, not cut off, so no word of a value is hidden
    let value_width = width.saturating_sub(name_width + 2).max(1);
    let mut lines = Vec::new();
    let mut row_starts = Vec::new();
    for (name, (_, value)) in names.iter().zip(rows) {
        row_starts.push(lines.len());
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
    (lines, row_starts)
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
) -> Result<BTreeMap<usize, Result<Outcome, String>>, String> {
    let mut outcomes = BTreeMap::new();
    for (&index, &decision) in answers {
        // An action can take a while, so the jobs show which one runs, and the
        // outcomes so far, instead of a screen that waits without a sign
        if decision == Decision::Approve {
            draw_jobs(jobs, answers, &outcomes, None, Some(index))?;
            print_keys("carrying out the decisions, one at a time")?;
        }
        let result = queue.decide(actions, jobs[index].queued_at_ns, decision);
        outcomes.insert(index, result.map(|answered_job| answered_job.outcome));
    }
    Ok(outcomes)
}

/// Read one key press, without waiting for Enter, or nothing when the terminal
/// was resized first, so the screen is drawn again at its new size.
fn read_key() -> Result<Option<u8>, String> {
    let _cbreak_mode = CbreakMode::new()?;
    let mut key = [0];
    match io::stdin().read(&mut key) {
        Ok(0) => Err("Cannot read a key: the terminal closed".to_string()),
        Ok(_) => Ok(Some(key[0])),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(error) => Err(format!("Cannot read a key: {error}")),
    }
}

/// Make a resize, or a job ,approval queues, interrupt the wait for a key,
/// which `read_key` reports, so the screen is drawn again at once instead of on
/// the next key press.
///
/// A signal that comes while the screen is drawn, before the wait starts, is
/// only seen on the next key, which a draw takes too little time to matter.
fn interrupt_on_signals() -> Result<(), String> {
    // The system ignores a resize by default, which never interrupts a read,
    // and ends the process on SIGUSR1, so both get a handler that does nothing
    // but interrupt
    extern "C" fn interrupt(_: libc::c_int) {}
    // SAFETY: an all-zero sigaction is a valid one, with no flags and an empty
    // mask, and the handler touches nothing, so it is safe to run at any point
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = interrupt as *const () as libc::sighandler_t;
    // Without SA_RESTART among the flags, the read stops instead of resuming
    for signal in [libc::SIGWINCH, libc::SIGUSR1] {
        // SAFETY: the action is fully set, and the old one is not asked for
        if unsafe { libc::sigaction(signal, &raw const action, std::ptr::null_mut()) } == -1 {
            let error = io::Error::last_os_error();
            return Err(format!("Cannot listen for signals: {error}"));
        }
    }
    Ok(())
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

/// Write the outcome on one line no wider than the width, with the line of
/// what the action printed that says most: the first of an output, or the last
/// of an error, which says what went wrong.
fn format_outcome(outcome: &Result<Outcome, String>, width: usize) -> String {
    let (color, word, text) = describe_outcome(outcome);
    let line = match outcome {
        Ok(Outcome::Succeeded { .. }) => text.lines().next().unwrap_or_default(),
        _ => get_last_line(text),
    };
    let text = if line.is_empty() {
        word.to_string()
    } else {
        format!("{word}: {line}")
    };
    // What an action prints could move the cursor or hide text, like its
    // arguments
    format!("{color}{}{RESET}", cut(&show_controls(&text), width))
}

impl FullScreen {
    fn new() -> Result<Self, String> {
        print!("{ENTER_ALTERNATE_SCREEN}{HIDE_CURSOR}");
        io::stdout()
            .flush()
            .map_err(|error| format!("Cannot take over the screen: {error}"))?;
        Ok(FullScreen)
    }
}

impl Drop for FullScreen {
    fn drop(&mut self) {
        print!("{SHOW_CURSOR}{LEAVE_ALTERNATE_SCREEN}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_shown_chars_leaves_out_color_codes() {
        assert_eq!(count_shown_chars(&format!("{DIM}5m{RESET} > y")), 6);
    }

    #[test]
    fn cut_keeps_text_that_fits_and_ends_longer_text_with_an_ellipsis() {
        assert_eq!(cut("abcd", 4), "abcd");
        assert_eq!(cut("abcde", 4), "abc…");
        assert_eq!(cut("äöüß", 3), "äö…");
        assert_eq!(cut("abc", 0), "…");
    }

    #[test]
    fn wrap_breaks_after_the_last_word_that_fits() {
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
        assert_eq!(wrap("one two three", 8), ["one two", "three"]);
        assert_eq!(wrap("short", 7), ["short"]);
        assert_eq!(wrap("", 7), [""]);
    }

    #[test]
    fn wrap_breaks_inside_a_word_wider_than_a_line() {
        assert_eq!(wrap("abcdefgh ij", 3), ["abc", "def", "gh", "ij"]);
    }

    #[test]
    fn wrap_keeps_the_spaces_that_start_the_line_and_sit_between_words() {
        assert_eq!(wrap("    quoted  text", 20), ["    quoted  text"]);
        assert_eq!(wrap("    quoted text", 10), ["    quoted", "text"]);
    }

    #[test]
    fn expand_tabs_fills_up_to_the_next_tab_stop() {
        assert_eq!(expand_tabs("a\tb"), format!("a{}b", " ".repeat(7)));
        assert_eq!(
            expand_tabs("abcdefgh\tb"),
            format!("abcdefgh{}b", " ".repeat(8))
        );
    }

    #[test]
    fn format_rows_aligns_values_and_wraps_them_down_their_column() {
        let rows = [
            ("to".to_string(), "a@b".to_string()),
            ("subject".to_string(), "one two three".to_string()),
        ];
        let (lines, starts) = format_rows(&rows, 16);
        assert_eq!(
            lines,
            [
                format!("{DIM}to     {RESET}  a@b"),
                format!("{DIM}subject{RESET}  one two"),
                "         three".to_string(),
            ]
        );
        assert_eq!(starts, [0, 1]);
    }

    #[test]
    fn format_rows_shows_a_name_or_value_s_control_characters_as_escapes() {
        let rows = [("na\x1bme".to_string(), "va\u{202e}lue".to_string())];
        let (lines, _) = format_rows(&rows, 80);
        assert_eq!(
            lines,
            [format!("{DIM}na\\u{{1b}}me{RESET}  va\\u{{202e}}lue")]
        );
    }

    #[test]
    fn format_outcome_shows_the_first_line_of_an_output_and_the_last_of_an_error() {
        let succeeded = Ok(Outcome::Succeeded {
            output: "first\nsecond".to_string(),
        });
        assert_eq!(
            format_outcome(&succeeded, 80),
            format!("{GREEN}succeeded: first{RESET}")
        );
        let failed = Ok(Outcome::Failed {
            error: "Traceback\n  raise\nQuotaError: no quota\n".to_string(),
        });
        assert_eq!(
            format_outcome(&failed, 80),
            format!("{RED}failed: QuotaError: no quota{RESET}")
        );
        assert_eq!(
            format_outcome(&Ok(Outcome::Refused), 80),
            format!("{YELLOW}refused{RESET}")
        );
        assert_eq!(
            format_outcome(&Err("gone".to_string()), 80),
            format!("{RED}not carried out: gone{RESET}")
        );
    }

    #[test]
    fn format_outcome_escapes_and_cuts_what_the_action_printed() {
        let succeeded = Ok(Outcome::Succeeded {
            output: "\x1b[2Jhidden".to_string(),
        });
        assert_eq!(
            format_outcome(&succeeded, 20),
            format!("{GREEN}succeeded: \\u{{1b}}[2…{RESET}")
        );
    }
}
