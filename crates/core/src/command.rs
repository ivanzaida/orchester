//! The process boundary. Core decides which programs run with which arguments and reads their
//! output; running them is the job of a [`CommandRunner`] (`orchester-exec` for real processes,
//! captured output in tests).

use std::path::{Path, PathBuf};

use thiserror::Error;

/// A program to run to completion. Everything that differs from the calling process is spelled
/// out here: arguments, working directory, environment changes and standard input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
  program: PathBuf,
  args: Vec<String>,
  current_dir: Option<PathBuf>,
  env: Vec<(String, String)>,
  env_removed: Vec<String>,
  inherit_env: bool,
  stdin: Option<String>,
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
      inherit_env: true,
      stdin: None,
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

  /// Writes `input` to the program's standard input, then closes it.
  #[must_use]
  pub fn with_stdin(mut self, input: String) -> Self {
    self.stdin = Some(input);
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

  /// Whether the program starts from the caller's environment; otherwise it sees only
  /// [`Command::env`].
  pub const fn inherits_env(&self) -> bool {
    self.inherit_env
  }

  pub fn stdin(&self) -> Option<&str> {
    self.stdin.as_deref()
  }
}

/// Output of a finished command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutput {
  /// The command exited with status 0.
  pub success: bool,
  pub stdout: String,
  pub stderr: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommandError {
  #[error("the program does not exist")]
  NotFound,
  #[error("the program did not finish in time")]
  TimedOut,
  #[error("the program could not be run: {0}")]
  Failed(String),
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
