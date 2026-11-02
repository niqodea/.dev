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

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
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

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub cli: String,
    pub action: String,
    pub arguments: Vec<String>,
}

/// A request whose action parsed its arguments, with the name the action gave
/// each one
#[derive(Serialize, Deserialize)]
pub struct ParsedRequest {
    #[serde(flatten)]
    pub request: Request,
    // One for each argument, in the same order
    pub argument_names: Vec<String>,
}

pub struct QueuedJob {
    // Also names the job's file
    pub queued_at_ns: u64,
    pub parsed_request: ParsedRequest,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum Outcome {
    // What the action printed
    Succeeded { output: String },
    Refused,
    // The last line the action printed to stderr, like why the change was
    // rejected
    Failed { error: String },
}

#[derive(Serialize, Deserialize)]
pub struct AnsweredJob {
    pub answered_at_ns: u64,
    // Entries already in the log use the key job
    #[serde(rename = "job")]
    pub request: Request,
    #[serde(flatten)]
    pub outcome: Outcome,
}

/// The actions CLIs install, and the environment they run in
pub struct Actions {
    // Where each CLI installs its actions and auto-approved file, under <cli>
    clis_directory: PathBuf,
    search_path: String,
    home: String,
}

/// The jobs waiting in one queue, and the log of the jobs answered
pub struct Queue {
    name: String,
    directory: PathBuf,
}

impl Request {
    /// Write the request on one line as the words that ask for it, each quoted
    /// the way zsh and bash 4.2 or later read it back, so a value with a space
    /// reads as one word.
    #[must_use]
    pub fn format(&self) -> String {
        [&self.cli, &self.action]
            .into_iter()
            .chain(&self.arguments)
            .map(|word| quote(word))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Copy)]
enum Verb {
    Parse,
    Run,
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

    /// Run the action's parse, which rejects arguments the action cannot run
    /// with, without reading or changing anything, and return the request with
    /// the name parse printed for each argument.
    ///
    /// # Errors
    ///
    /// When no such action is installed, its parse rejects the request, or it
    /// prints an empty name, or not exactly one name per argument.
    pub fn parse(&self, request: Request) -> Result<ParsedRequest, String> {
        let action_path = self.locate(&request)?;
        let output = match self.run_action(&action_path, Verb::Parse, &request.arguments) {
            Ok(output) => output,
            Err(ActionError::TimedOut) => {
                return Err(format!(
                    "Invalid request: took over {} s to parse",
                    ACTION_TIMEOUT.as_secs()
                ));
            }
            Err(ActionError::Io(error)) => return Err(format!("Invalid request: {error}")),
        };
        if !output.status.success() {
            return Err(format!("Invalid request: {}", get_error(&output)));
        }
        let argument_names: Vec<String> = output.stdout.lines().map(str::to_string).collect();
        // A name off by one would label every value after it with the wrong
        // name
        if argument_names.len() != request.arguments.len() {
            return Err(format!(
                "Invalid request: parse prints one name per argument, and printed {} for the {}",
                argument_names.len(),
                request.arguments.len()
            ));
        }
        if argument_names.iter().any(String::is_empty) {
            return Err("Invalid request: parse printed an empty name".to_string());
        }
        Ok(ParsedRequest {
            request,
            argument_names,
        })
    }

    /// Run the action, and return what it printed.
    ///
    /// # Errors
    ///
    /// Why the action failed, like the last line it printed to stderr.
    pub fn run(&self, request: &Request) -> Result<String, String> {
        let action_path = self.locate(request)?;
        match self.run_action(&action_path, Verb::Run, &request.arguments) {
            Ok(output) if output.status.success() => Ok(output.stdout.trim().to_string()),
            Ok(output) => Err(get_error(&output)),
            Err(ActionError::TimedOut) => Err(format!("took over {} s", ACTION_TIMEOUT.as_secs())),
            // Like an action without its execute bit
            Err(ActionError::Io(error)) => Err(error.to_string()),
        }
    }

    /// Whether a line of the CLI's auto-approved file equals the request's
    /// first words, the action and its first arguments.
    ///
    /// # Errors
    ///
    /// When the auto-approved file cannot be read.
    pub fn is_auto_approved(&self, request: &Request) -> Result<bool, String> {
        let path = self.clis_directory.join(&request.cli).join("auto-approved");
        if !path.exists() {
            return Ok(false);
        }
        let text = read_file(&path)?;
        let words: Vec<&str> = [request.action.as_str()]
            .into_iter()
            .chain(request.arguments.iter().map(String::as_str))
            .collect();
        let is_listed = text.lines().any(|line| {
            let rule: Vec<&str> = line.split_whitespace().collect();
            let is_comment = rule.first().is_some_and(|word| word.starts_with('#'));
            !rule.is_empty() && !is_comment && words.starts_with(&rule)
        });
        Ok(is_listed)
    }

    fn locate(&self, request: &Request) -> Result<PathBuf, String> {
        let path = self
            .clis_directory
            .join(&request.cli)
            .join("actions")
            .join(&request.action);
        // A name becomes part of a path, so it may not climb out of its
        // directory
        let is_named_plainly = is_plain_name(&request.cli) && is_plain_name(&request.action);
        if !is_named_plainly || !path.is_file() {
            return Err(format!(
                "No such request: {} {}",
                request.cli, request.action
            ));
        }
        Ok(path)
    }

    fn run_action(
        &self,
        action_path: &Path,
        verb: Verb,
        arguments: &[String],
    ) -> Result<ActionOutput, ActionError> {
        let verb = match verb {
            Verb::Parse => "parse",
            Verb::Run => "run",
        };
        let mut child = process::Command::new(action_path)
            .arg(verb)
            .args(arguments)
            // Only PATH, to find programs, and HOME, to find the person's
            // logins, so an action gets the same environment whoever runs it,
            // and names everything else it acts on itself
            .env_clear()
            .env("PATH", &self.search_path)
            .env("HOME", &self.home)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // A group of its own, so a timeout stops every process the action
            // started, and none of them makes the change after it was reported
            // as failed
            .process_group(0)
            .spawn()
            .map_err(ActionError::Io)?;
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
        Ok(ActionOutput {
            status,
            stdout: join_reader(stdout).map_err(ActionError::Io)?,
            stderr: join_reader(stderr).map_err(ActionError::Io)?,
        })
    }
}

impl Queue {
    /// The queue with the name, kept in the home folder.
    #[must_use]
    pub fn new(home: &str, name: String) -> Queue {
        Queue {
            directory: Path::new(home).join(".local/state/approval").join(&name),
            name,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add the job to the jobs waiting for a decision.
    ///
    /// # Errors
    ///
    /// When the job's file cannot be written.
    pub fn add(&self, parsed_request: &ParsedRequest) -> Result<(), String> {
        let directory = self.get_pending_directory();
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
        let path = self.get_pending_path(get_now_ns());
        fs::write(&path, encode(parsed_request))
            .map_err(|error| format!("Cannot write {}: {error}", path.display()))
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
        let ParsedRequest { request, .. } = decode(&read_file(&path)?, &path)?;
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
            let parsed_request = decode(&read_file(&path)?, &path)?;
            queued_jobs.push(QueuedJob {
                queued_at_ns,
                parsed_request,
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

/// Return the name of the queue to use, which `APPROVAL_QUEUE` holds.
///
/// # Errors
///
/// When `APPROVAL_QUEUE` is not set, or holds more than a plain name.
pub fn read_queue_name() -> Result<String, String> {
    let queue_name = env::var("APPROVAL_QUEUE")
        .map_err(|_| "APPROVAL_QUEUE is not set, so there is no queue to use".to_string())?;
    // The name becomes a directory, so it may not climb out of its parent
    let mut characters = queue_name.chars();
    let is_plain = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '-')
        });
    if !is_plain {
        return Err(format!(
            "APPROVAL_QUEUE is {queue_name}, which is not a plain name"
        ));
    }
    Ok(queue_name)
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
    match (output.stderr.trim().lines().last(), output.status.code()) {
        (Some(line), _) => line.to_string(),
        (None, Some(code)) => format!("exited with {code}"),
        (None, None) => "stopped by a signal".to_string(),
    }
}

/// Quote the word unless every character in it reads the same to a shell
/// without quotes.
///
/// A word with a control or format character goes in $'', with each such
/// character written as an escape, and any other word in single quotes.
fn quote(word: &str) -> String {
    // zsh expands a word that starts with =, like =ls, to the command's path
    let is_plain = !word.is_empty()
        && !word.starts_with('=')
        && word
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_./:=@%+,".contains(character));
    if is_plain {
        return word.to_string();
    }
    // A control character can move the cursor or clear the screen, a format
    // character can be invisible or reverse the text after it, and a line
    // break would split the request over several lines
    if word.chars().any(is_control_or_format) {
        let escaped: String = word
            .chars()
            .map(|character| match character {
                '\\' => r"\\".to_string(),
                '\'' => r"\'".to_string(),
                '\n' => r"\n".to_string(),
                '\t' => r"\t".to_string(),
                // \x writes one byte, which is the character itself only for
                // ASCII
                character if character.is_ascii_control() => {
                    format!(r"\x{:02x}", u32::from(character))
                }
                character if u32::from(character) <= 0xffff && is_control_or_format(character) => {
                    format!(r"\u{:04x}", u32::from(character))
                }
                character if is_control_or_format(character) => {
                    format!(r"\U{:08x}", u32::from(character))
                }
                character => character.to_string(),
            })
            .collect();
        return format!("$'{escaped}'");
    }
    // A single quote cannot be escaped inside single quotes, so it closes them,
    // follows escaped, and opens them again
    format!("'{}'", word.replace('\'', r"'\''"))
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
