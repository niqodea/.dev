//! The approval queue: the actions CLIs install, the requests waiting in a
//! queue for a person's decision, and the log of the requests answered.

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{self, Child, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};

// Long enough for a write over the network, short enough that a stuck action
// does not hold the person deciding
const ACTION_TIMEOUT: Duration = Duration::from_secs(60);
// Short enough that waiting for an action to exit adds no delay anyone notices
const ACTION_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, PartialEq)]
pub enum Decision {
    Approve,
    Refuse,
}

/// What a CLI asks for: one of its actions, and the arguments the action runs
/// with, each under its name, in the order the CLI gave them
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub cli: String,
    pub action: String,
    pub arguments: IndexMap<String, Value>,
}

/// A request that runs without asking: any request for the action whose
/// arguments include these
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    action: String,
    arguments: IndexMap<String, Value>,
}

pub struct QueuedJob {
    // Also names the job's file
    pub queued_at_ns: u64,
    pub request: Request,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum Outcome {
    // What the action printed
    Succeeded { output: String },
    Refused,
    // What the action printed to stderr, like why the change was rejected, or
    // how it exited when it printed nothing there
    Failed { error: String },
}

#[derive(Serialize, Deserialize)]
pub struct AnsweredJob {
    pub answered_at_ns: u64,
    pub request: Request,
    #[serde(flatten)]
    pub outcome: Outcome,
}

/// The actions CLIs install, and the environment they run in
pub struct Actions {
    // Where each CLI installs its action executable and auto-approved file,
    // under <cli>
    clis_directory: PathBuf,
    search_path: String,
    home: String,
}

/// The jobs waiting in one queue, and the log of the jobs answered
pub struct Queue {
    directory: PathBuf,
}

impl Request {
    /// Write the request on one line: its CLI, its action, and its arguments as
    /// a JSON object, with every control and format character escaped.
    #[must_use]
    pub fn format(&self) -> String {
        show_controls(&format!(
            "{} {} {}",
            self.cli,
            self.action,
            encode(&self.arguments)
        ))
    }
}

struct ActionOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

enum ActionError {
    TimedOut,
    Io(io::Error),
}

impl Actions {
    /// The actions installed in the home folder, which run with the search path
    /// as their PATH.
    #[must_use]
    pub fn new(home: &str, search_path: String) -> Actions {
        Actions {
            clis_directory: Path::new(home).join(".config/approval/clis"),
            search_path,
            home: home.to_string(),
        }
    }

    /// Return the directory each CLI installs its folder in.
    #[must_use]
    pub fn get_clis_directory(&self) -> &Path {
        &self.clis_directory
    }

    /// Check that the request names a CLI that installs an action executable.
    ///
    /// The action itself checks the request's arguments when it runs, since it
    /// alone knows them.
    ///
    /// # Errors
    ///
    /// When the CLI or the action is not named plainly, or the CLI installs no
    /// action executable.
    pub fn check(&self, request: &Request) -> Result<(), String> {
        self.locate(&request.cli, &request.action).map(|_| ())
    }

    /// Run the action, and return what it printed.
    ///
    /// # Errors
    ///
    /// Why the action failed, like the last line it printed to stderr.
    pub fn run(&self, request: &Request) -> Result<String, String> {
        let action_path = self.locate(&request.cli, &request.action)?;
        match self.run_action(&action_path, &request.action, &request.arguments) {
            Ok(output) if output.status.success() => Ok(output.stdout.trim().to_string()),
            Ok(output) => Err(get_error(&output)),
            Err(ActionError::TimedOut) => Err(format!("took over {} s", ACTION_TIMEOUT.as_secs())),
            // Like an action without its execute bit
            Err(ActionError::Io(error)) => Err(error.to_string()),
        }
    }

    /// Whether a rule in the CLI's auto-approved file names the request's
    /// action, and only arguments the request has, with the same values.
    ///
    /// # Errors
    ///
    /// When the auto-approved file cannot be read, or holds something other
    /// than a list of rules.
    pub fn is_auto_approved(&self, request: &Request) -> Result<bool, String> {
        let path = self
            .clis_directory
            .join(&request.cli)
            .join("auto-approved.json");
        if !path.exists() {
            return Ok(false);
        }
        let rules: Vec<Rule> = decode(&read_file(&path)?, &path)?;
        let is_listed = rules.iter().any(|rule| {
            rule.action == request.action
                && rule
                    .arguments
                    .iter()
                    .all(|(name, value)| request.arguments.get(name) == Some(value))
        });
        Ok(is_listed)
    }

    /// Return the executable that makes the CLI's changes, which takes the
    /// action's name as its argument.
    fn locate(&self, cli: &str, action: &str) -> Result<PathBuf, String> {
        let path = self.clis_directory.join(cli).join("action");
        // The CLI's name becomes part of a path, so it may not climb out of its
        // directory, and the action's is the executable's argument, so it may
        // not pass for an option like --help
        let is_named_plainly = is_plain_name(cli) && is_plain_name(action);
        if !is_named_plainly || !path.is_file() {
            return Err(format!("No such request: {cli} {action}"));
        }
        Ok(path)
    }

    /// Run the action executable with the action's name, and the arguments as a
    /// JSON object on its standard input.
    fn run_action(
        &self,
        action_path: &Path,
        action: &str,
        arguments: &IndexMap<String, Value>,
    ) -> Result<ActionOutput, ActionError> {
        let mut child = process::Command::new(action_path)
            .arg(action)
            // Only PATH, to find programs, and HOME, to find the person's
            // logins, so an action gets the same environment whoever runs it,
            // and names everything else it acts on itself
            .env_clear()
            .env("PATH", &self.search_path)
            .env("HOME", &self.home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // A group of its own, so a timeout stops every process the action
            // started, and none of them makes the change after it was reported
            // as failed
            .process_group(0)
            .spawn()
            .map_err(ActionError::Io)?;
        // Written while the action runs, since a write to a full pipe waits for
        // the action to read it, and closed after it, so the action reads to
        // the end
        let stdin = write_in_background(
            child.stdin.take().expect("stdin is piped"),
            encode(arguments),
        );
        // Read while the action runs, since an action that fills a pipe waits
        // for someone to read it
        let stdout = read_in_background(child.stdout.take().expect("stdout is piped"));
        let stderr = read_in_background(child.stderr.take().expect("stderr is piped"));
        let deadline = Instant::now() + ACTION_TIMEOUT;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(ActionError::Io)? {
                break status;
            }
            if Instant::now() >= deadline {
                stop_process_group(&child).map_err(ActionError::Io)?;
                child.wait().map_err(ActionError::Io)?;
                return Err(ActionError::TimedOut);
            }
            thread::sleep(ACTION_POLL_INTERVAL);
        };
        // An action that exits without reading its arguments closes the pipe,
        // which fails the write, and says nothing about the action itself
        let _ = stdin.join().expect("writing a pipe does not panic");
        Ok(ActionOutput {
            status,
            stdout: join_reader(stdout).map_err(ActionError::Io)?,
            stderr: join_reader(stderr).map_err(ActionError::Io)?,
        })
    }
}

impl Queue {
    /// The queue kept in the directory.
    #[must_use]
    pub fn new(directory: PathBuf) -> Queue {
        Queue { directory }
    }

    /// Return the file that one answering screen at a time holds a lock on.
    #[must_use]
    pub fn get_lock_path(&self) -> PathBuf {
        self.directory.join("answer.lock")
    }

    /// Add the job to the jobs waiting for a decision.
    ///
    /// # Errors
    ///
    /// When the job's file cannot be written.
    pub fn add(&self, request: &Request) -> Result<(), String> {
        let directory = self.get_pending_directory();
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
        let path = self.get_pending_path(get_now_ns());
        fs::write(&path, encode(request))
            .map_err(|error| format!("Cannot write {}: {error}", path.display()))
    }

    /// Tell the screen that answers this queue, if one is open, that a job was
    /// added, so it shows the job at once.
    ///
    /// Only while a screen holds the lock, since the process ID in the lock
    /// file is then that screen's, and not one a closed screen left behind.
    pub fn signal_answering_screen(&self) {
        let path = self.get_lock_path();
        // The screen showing the job at once is a convenience: without the
        // signal, the job still waits, and shows when the screen opens again
        let Ok(lock) = fs::File::open(&path) else {
            return;
        };
        if lock.try_lock().is_ok() {
            return;
        }
        let Ok(process_id) = read_file(&path).map(|text| text.trim().parse::<libc::pid_t>()) else {
            return;
        };
        if let Ok(process_id) = process_id {
            // SAFETY: kill only sends a signal, here one the screen listens for
            unsafe { libc::kill(process_id, libc::SIGUSR1) };
        }
    }

    /// Carry out the person's decision on a waiting job: run it or refuse it,
    /// log the outcome, and take the job out of the queue.
    ///
    /// # Errors
    ///
    /// When no such job waits, or the queue's files cannot be read or written.
    pub fn decide(
        &self,
        actions: &Actions,
        queued_at_ns: u64,
        decision: Decision,
    ) -> Result<AnsweredJob, String> {
        let path = self.get_pending_path(queued_at_ns);
        if !path.exists() {
            return Err("the request no longer waits in this queue".to_string());
        }
        let request: Request = decode(&read_file(&path)?, &path)?;
        let outcome = match decision {
            Decision::Approve => match actions.run(&request) {
                Ok(output) => Outcome::Succeeded { output },
                Err(error) => Outcome::Failed { error },
            },
            Decision::Refuse => Outcome::Refused,
        };
        let answered_job = self.record(request, outcome)?;
        fs::remove_file(&path)
            .map_err(|error| format!("Cannot remove {}: {error}", path.display()))?;
        Ok(answered_job)
    }

    /// Append the job and its outcome to the log, and return them as logged.
    ///
    /// # Errors
    ///
    /// When the log cannot be written.
    pub fn record(&self, request: Request, outcome: Outcome) -> Result<AnsweredJob, String> {
        let answered_job = AnsweredJob {
            answered_at_ns: get_now_ns(),
            request,
            outcome,
        };
        let path = self.get_log_path();
        fs::create_dir_all(&self.directory)
            .map_err(|error| format!("Cannot create {}: {error}", self.directory.display()))?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
        writeln!(file, "{}", encode(&answered_job))
            .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;
        Ok(answered_job)
    }

    /// Return the jobs waiting for a decision, oldest first.
    ///
    /// # Errors
    ///
    /// When the queue's files cannot be read.
    pub fn read_queued_jobs(&self) -> Result<Vec<QueuedJob>, String> {
        let mut queued_jobs = Vec::new();
        for queued_at_ns in self.list_queue_times_ns()? {
            let path = self.get_pending_path(queued_at_ns);
            let request = decode(&read_file(&path)?, &path)?;
            queued_jobs.push(QueuedJob {
                queued_at_ns,
                request,
            });
        }
        Ok(queued_jobs)
    }

    /// Return the jobs answered, oldest first.
    ///
    /// # Errors
    ///
    /// When the log cannot be read.
    pub fn read_answered_jobs(&self) -> Result<Vec<AnsweredJob>, String> {
        let path = self.get_log_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        read_file(&path)?
            .lines()
            .map(|line| decode(line, &path))
            .collect()
    }

    /// Return how many jobs wait for a decision.
    ///
    /// # Errors
    ///
    /// When the queue's files cannot be read.
    pub fn count_pending_jobs(&self) -> Result<usize, String> {
        Ok(self.list_queue_times_ns()?.len())
    }

    /// Return when each pending job was queued, in nanoseconds, oldest first.
    fn list_queue_times_ns(&self) -> Result<Vec<u64>, String> {
        let directory = self.get_pending_directory();
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("Cannot read {}: {error}", directory.display())),
        };
        let mut queue_times_ns = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Cannot read {}: {error}", directory.display()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let queued_at_ns = name
                .strip_suffix(".json")
                .and_then(|stem| stem.parse().ok())
                .ok_or_else(|| format!("Unexpected file {name} in {}", directory.display()))?;
            queue_times_ns.push(queued_at_ns);
        }
        queue_times_ns.sort_unstable();
        Ok(queue_times_ns)
    }

    fn get_pending_path(&self, queued_at_ns: u64) -> PathBuf {
        self.get_pending_directory()
            .join(format!("{queued_at_ns}.json"))
    }

    fn get_pending_directory(&self) -> PathBuf {
        self.directory.join("pending")
    }

    fn get_log_path(&self) -> PathBuf {
        self.directory.join("log.jsonl")
    }
}

/// Return the directory of the queue to use: the approval folder in the atdir
/// of the project the shell is in, so the queue moves with the project.
///
/// # Errors
///
/// When the shell is not in a project, or the atdir has no name.
pub fn read_queue_directory(home: &str) -> Result<PathBuf, String> {
    let project = env::var("DOTDEV_PROJECT").map_err(|_| {
        "DOTDEV_PROJECT is not set: requests wait in a project's atdir, so run this in a project"
            .to_string()
    })?;
    let atdir = read_variable("DOTDEV_ATDIR")?;
    Ok(Path::new(home).join(project).join(atdir).join("approval"))
}

/// Return the environment variable's value.
///
/// # Errors
///
/// When the variable is not set.
pub fn read_variable(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} is not set"))
}

/// Return the time since 1970, in nanoseconds.
///
/// # Panics
///
/// When the clock is before 1970, or after 2554, since a `u64` holds
/// nanoseconds only up to the year 2554.
#[must_use]
pub fn get_now_ns() -> u64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970");
    u64::try_from(since_epoch.as_nanos()).expect("the clock is before 2554")
}

/// Write the age in at most three characters, like 5m, 12h or 3d.
#[must_use]
pub fn format_age(age_ns: u64) -> String {
    let minutes = age_ns / 60_000_000_000;
    let age = if minutes < 60 {
        format!("{minutes}m")
    } else if minutes < 48 * 60 {
        format!("{}h", minutes / 60)
    } else {
        format!("{}d", minutes / (24 * 60))
    };
    format!("{age:>3}")
}

fn stop_process_group(child: &Child) -> io::Result<()> {
    let group_id = libc::pid_t::try_from(child.id()).expect("a process id fits a pid_t");
    // SAFETY: kill only sends a signal, here to the group the action leads
    if unsafe { libc::kill(-group_id, libc::SIGKILL) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn write_in_background<W: Write + Send + 'static>(
    mut pipe: W,
    text: String,
) -> JoinHandle<io::Result<()>> {
    thread::spawn(move || pipe.write_all(text.as_bytes()))
}

fn read_in_background<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<io::Result<String>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
}

fn join_reader(reader: JoinHandle<io::Result<String>>) -> io::Result<String> {
    reader.join().expect("reading a pipe does not panic")
}

fn get_error(output: &ActionOutput) -> String {
    match (output.stderr.trim(), output.status.code()) {
        ("", Some(code)) => format!("exited with {code}"),
        ("", None) => "stopped by a signal".to_string(),
        (stderr, _) => stderr.to_string(),
    }
}

/// Return the last line of what a program printed, which says what went wrong,
/// like the end of a traceback.
#[must_use]
pub fn get_last_line(text: &str) -> &str {
    text.trim_end().lines().last().unwrap_or_default()
}

/// Write control characters and Unicode format characters as escapes, like
/// \u{1b}, except line breaks and tabs.
///
/// A control character can move the cursor, clear the screen or change colors.
/// A format character can be invisible or reverse the order text is shown in.
/// Either can make the screen show something other than the text.
#[must_use]
pub fn show_controls(text: &str) -> String {
    text.chars()
        .map(|character| {
            if is_control_or_format(character) && !matches!(character, '\n' | '\t') {
                character.escape_default().to_string()
            } else {
                character.to_string()
            }
        })
        .collect()
}

fn is_control_or_format(character: char) -> bool {
    matches!(
        character.general_category(),
        GeneralCategory::Control | GeneralCategory::Format
    )
}

fn is_plain_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn read_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("Cannot read {}: {error}", path.display()))
}

fn decode<T: DeserializeOwned>(text: &str, path: &Path) -> Result<T, String> {
    serde_json::from_str(text)
        .map_err(|error| format!("Unexpected content in {}: {error}", path.display()))
}

fn encode<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("the types written here have only string keys")
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use serde_json::json;

    use super::*;

    const CLI: &str = "test";

    /// A home folder whose test CLI installs the action script, and the actions
    /// it holds
    struct Home {
        directory: tempfile::TempDir,
        actions: Actions,
    }

    impl Home {
        fn new(script: &str) -> Home {
            let directory = tempfile::tempdir().unwrap();
            let home = directory.path().to_str().unwrap().to_string();
            let cli_directory = directory.path().join(".config/approval/clis").join(CLI);
            fs::create_dir_all(&cli_directory).unwrap();
            let action_path = cli_directory.join("action");
            fs::write(&action_path, format!("#!/bin/sh\n{script}\n")).unwrap();
            fs::set_permissions(&action_path, fs::Permissions::from_mode(0o755)).unwrap();
            let actions = Actions::new(&home, env::var("PATH").unwrap());
            Home { directory, actions }
        }

        fn write_auto_approved(&self, rules: &str) {
            let path = self
                .directory
                .path()
                .join(".config/approval/clis")
                .join(CLI)
                .join("auto-approved.json");
            fs::write(path, rules).unwrap();
        }
    }

    fn request(cli: &str, action: &str, arguments: &Value) -> Request {
        let Value::Object(arguments) = arguments else {
            panic!("the arguments are an object");
        };
        Request {
            cli: cli.to_string(),
            action: action.to_string(),
            arguments: arguments.clone().into_iter().collect(),
        }
    }

    // Prints back what it runs with: the action's name, then the arguments
    const ECHO_SCRIPT: &str = r#"echo "$1"; cat"#;

    #[test]
    fn run_gets_the_action_s_name_and_the_arguments_on_its_input() {
        let home = Home::new(ECHO_SCRIPT);
        let arguments = json!({"file": "notes.txt", "tags": ["a", "b"]});
        let output = home.actions.run(&request(CLI, "tag", &arguments)).unwrap();
        assert_eq!(
            output,
            r#"tag
{"file":"notes.txt","tags":["a","b"]}"#
        );
    }

    #[test]
    fn run_reports_what_the_action_printed_to_stderr() {
        let home = Home::new("echo 'usage: move' >&2; echo 'unknown key: from' >&2; exit 1");
        let error = home
            .actions
            .run(&request(CLI, "move", &json!({"from": "a"})))
            .err()
            .unwrap();
        assert_eq!(error, "usage: move\nunknown key: from");
    }

    #[test]
    fn check_rejects_names_that_are_not_plain() {
        let home = Home::new(ECHO_SCRIPT);
        for (cli, action) in [
            ("..", "move"),
            ("test/..", "move"),
            (CLI, "--help"),
            (CLI, ""),
            (CLI, "Move"),
        ] {
            assert_eq!(
                home.actions
                    .check(&request(cli, action, &json!({})))
                    .err()
                    .unwrap(),
                format!("No such request: {cli} {action}")
            );
        }
    }

    #[test]
    fn check_rejects_a_cli_without_an_action_executable() {
        let home = Home::new(ECHO_SCRIPT);
        assert_eq!(
            home.actions
                .check(&request("other", "move", &json!({})))
                .err()
                .unwrap(),
            "No such request: other move"
        );
    }

    #[test]
    fn auto_approved_rules_match_a_subset_of_the_arguments_with_equal_values() {
        let home = Home::new(ECHO_SCRIPT);
        home.write_auto_approved(
            r#"[{"action": "tag", "arguments": {"file": "notes.txt"}},
               {"action": "tag", "arguments": {"tags": ["x"]}}]"#,
        );
        let is_auto_approved = |arguments: Value| {
            home.actions
                .is_auto_approved(&request(CLI, "tag", &arguments))
                .unwrap()
        };
        assert!(is_auto_approved(
            json!({"file": "notes.txt", "tags": ["a"]})
        ));
        // A list of one is a list, as the rule writes it
        assert!(is_auto_approved(
            json!({"file": "other.txt", "tags": ["x"]})
        ));
        assert!(!is_auto_approved(json!({"file": "other.txt", "tags": "x"})));
        assert!(!is_auto_approved(
            json!({"file": "other.txt", "tags": ["x", "y"]})
        ));
    }

    #[test]
    fn requests_logged_before_arguments_were_json_still_read() {
        let request: Request = serde_json::from_str(
            r#"{"cli":"test","action":"run","arguments":{"pod":"p","command":["ls","-la"]}}"#,
        )
        .unwrap();
        assert_eq!(request.arguments["command"], json!(["ls", "-la"]));
    }

    #[test]
    fn show_controls_escapes_all_but_line_breaks_and_tabs() {
        assert_eq!(
            show_controls("a\tb\nc\x1b[2J\r\u{202e}d"),
            "a\tb\nc\\u{1b}[2J\\r\\u{202e}d"
        );
    }

    #[test]
    fn format_writes_the_arguments_as_json_with_controls_escaped() {
        let arguments = json!({"title": "it's done", "text": "a\u{202e}b", "count": 2});
        assert_eq!(
            request(CLI, "note", &arguments).format(),
            r#"test note {"title":"it's done","text":"a\u{202e}b","count":2}"#
        );
    }
}
