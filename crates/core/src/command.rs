//! The process boundary. Core decides which programs run with which arguments and reads their
//! output; running them is the job of a [`CommandRunner`] (`orchester-exec` for real processes,
//! captured output in tests).

use std::{
  fmt,
  path::{Path, PathBuf},
  sync::Arc,
};

use thiserror::Error;

use crate::secret::SecretValue;

/// An environment variable whose value is a secret: the program gets it, and nothing else shows it.
/// `Debug` shows the name only, and the value is read only through [`SecretVariable::expose`], which
/// the process start calls.
#[derive(Clone)]
pub struct SecretVariable {
  name: String,
  value: Arc<SecretValue>,
}

impl SecretVariable {
  pub fn new(name: &str, value: Arc<SecretValue>) -> Self {
    Self {
      name: name.to_owned(),
      value,
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  /// The value, for the one place that hands it to the starting process.
  pub fn expose(&self) -> &str {
    self.value.expose()
  }
}

impl fmt::Debug for SecretVariable {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "SecretVariable({}=<redacted>)", self.name)
  }
}

/// Two are the same when they set the same name to the same value in memory; values are never
/// compared.
impl PartialEq for SecretVariable {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name && Arc::ptr_eq(&self.value, &other.value)
  }
}

impl Eq for SecretVariable {}

/// Removes process secrets, including their JSON-escaped form, before text leaves the process boundary.
pub fn redact_secrets(text: &str, secrets: &[SecretVariable]) -> String {
  let mut secrets: Vec<_> = secrets.iter().filter(|secret| !secret.expose().is_empty()).collect();
  secrets.sort_by_key(|secret| std::cmp::Reverse(secret.expose().len()));
  secrets.iter().fold(text.to_owned(), |text, secret| {
    let encoded = serde_json::to_string(secret.expose()).expect("a string always serializes to JSON");
    text
      .replace(&encoded[1..encoded.len() - 1], "<redacted>")
      .replace(secret.expose(), "<redacted>")
  })
}

/// A program to run to completion. Everything that differs from the calling process is spelled
/// out here: arguments, working directory, environment changes and standard input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
  program: PathBuf,
  args: Vec<String>,
  current_dir: Option<PathBuf>,
  env: Vec<(String, String)>,
  env_removed: Vec<String>,
  /// Set after [`Self::env`]; a name set here is not in it.
  secret_env: Vec<SecretVariable>,
  inherit_env: bool,
  stdin: Option<String>,
  interactive_stdin: bool,
}

impl Command {
  /// Runs `program` with `args`, inheriting the caller's directory and environment, with empty
  /// standard input.
  pub fn new<I, S>(program: &Path, args: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    Self {
      program: program.to_path_buf(),
      args: args.into_iter().map(Into::into).collect(),
      current_dir: None,
      env: Vec::new(),
      env_removed: Vec::new(),
      secret_env: Vec::new(),
      inherit_env: true,
      stdin: None,
      interactive_stdin: false,
    }
  }

  #[must_use]
  pub fn with_current_dir(mut self, directory: &Path) -> Self {
    self.current_dir = Some(directory.to_path_buf());
    self
  }

  /// Sets `key` for the program only.
  #[must_use]
  pub fn with_env(mut self, key: &str, value: &str) -> Self {
    self.env.push((key.to_owned(), value.to_owned()));
    self
  }

  /// Sets `key` for the program only, replacing a variable of the same name (compared without regard to case, as
  /// Windows does).
  #[must_use]
  pub fn with_env_replaced(mut self, key: &str, value: &str) -> Self {
    self.env.retain(|(name, _)| !name.eq_ignore_ascii_case(key));
    self.env.push((key.to_owned(), value.to_owned()));
    self
  }

  /// Hides `key` of the calling process from the program.
  #[must_use]
  pub fn without_env(mut self, key: &str) -> Self {
    self.env_removed.push(key.to_owned());
    self
  }

  /// Gives the program exactly `variables` instead of the caller's environment.
  #[must_use]
  pub fn with_environment(mut self, variables: Vec<(String, String)>) -> Self {
    self.inherit_env = false;
    self.env_removed.clear();
    self.env = variables;
    self
  }

  /// Sets `variable` for the program only, replacing a variable of the same name. Its value stays
  /// out of the arguments and out of this command's `Debug` output.
  #[must_use]
  pub fn with_secret_env(mut self, variable: SecretVariable) -> Self {
    self.env.retain(|(name, _)| !name.eq_ignore_ascii_case(variable.name()));
    self
      .secret_env
      .retain(|existing| !existing.name().eq_ignore_ascii_case(variable.name()));
    self.secret_env.push(variable);
    self
  }

  /// Writes `input` to the program's standard input, then closes it.
  #[must_use]
  pub fn with_stdin(mut self, input: String) -> Self {
    self.stdin = Some(input);
    self
  }

  /// Keeps standard input open for a bidirectional line protocol after the child starts.
  #[must_use]
  pub fn with_interactive_stdin(mut self) -> Self {
    self.stdin = None;
    self.interactive_stdin = true;
    self
  }

  /// Keeps standard input open for a line protocol whose first line is `first`, written as the
  /// child starts.
  #[must_use]
  pub fn with_interactive_input(mut self, first: String) -> Self {
    self.stdin = Some(first);
    self.interactive_stdin = true;
    self
  }

  pub fn program(&self) -> &Path {
    &self.program
  }

  pub fn args(&self) -> &[String] {
    &self.args
  }

  pub fn current_dir(&self) -> Option<&Path> {
    self.current_dir.as_deref()
  }

  pub fn env(&self) -> &[(String, String)] {
    &self.env
  }

  pub fn env_removed(&self) -> &[String] {
    &self.env_removed
  }

  /// Variables whose values are secrets, set after [`Command::env`].
  pub fn secret_env(&self) -> &[SecretVariable] {
    &self.secret_env
  }

  /// Whether the program starts from the caller's environment; otherwise it sees only
  /// [`Command::env`].
  pub const fn inherits_env(&self) -> bool {
    self.inherit_env
  }

  pub fn stdin(&self) -> Option<&str> {
    self.stdin.as_deref()
  }

  pub const fn interactive_stdin(&self) -> bool {
    self.interactive_stdin
  }
}

/// Output of a finished command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutput {
  /// The exit code; `None` for a program ended by a signal (macOS). Some programs answer a
  /// question with it (git's `--quiet` queries: 1 is "no", anything else a failure).
  pub code: Option<i32>,
  pub stdout: String,
  pub stderr: String,
}

impl CommandOutput {
  /// The command exited with status 0.
  pub const fn success(&self) -> bool {
    matches!(self.code, Some(0))
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommandError {
  #[error("the program does not exist")]
  NotFound,
  #[error("the program did not finish in time")]
  TimedOut,
  #[error("the program could not be run: {0}")]
  Failed(String),
  /// The program was never started: it could not be prepared or its process not be created, so it ran
  /// nothing. A runner that cannot tell this apart from a failure reports [`Self::Failed`].
  #[error("the program could not be started: {0}")]
  NotStarted(String),
  /// Windows only: the program is a `.cmd`/`.bat` script, which `cmd.exe` runs by reading its
  /// arguments again; argument `position` (from 0) would be split or changed on the way to the
  /// program, so it is not run. Free-form text belongs on standard input instead.
  #[error(
    "argument {position} would be changed by cmd.exe on its way through the command script that starts the program"
  )]
  UnsafeScriptArgument { position: usize },
}

/// Runs a program to completion.
pub trait CommandRunner {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError>;
}

impl<R: CommandRunner + ?Sized> CommandRunner for &R {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    (**self).run(command)
  }
}
