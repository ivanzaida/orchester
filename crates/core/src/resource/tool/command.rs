//! Command tools (06.01 "Run test suite"): a program run with fixed arguments in the worktree of
//! the step that calls it, without a shell. Every part is a validated value: the program and its
//! arguments are tokens passed to the program as they are, the timeout and the output limit are
//! bounded, the expected exit codes are a non-empty set and the environment additions have valid,
//! distinct names. Command tools hold no secrets: environment values are stored in the tool's
//! body in plain text.

use std::{
  collections::{BTreeMap, BTreeSet},
  fmt,
  path::Path,
  time::Duration,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommandError {
  #[error("a command tool needs a program")]
  BlankProgram,
  #[error("an argument must not be empty")]
  EmptyArgument,
  /// Line breaks, tabs and other control characters cannot be passed through every platform's
  /// process start unchanged.
  #[error("`{0}` contains a control character such as a line break")]
  ControlCharacter(String),
  #[error("`{0}` is longer than {max} characters", max = MAX_TOKEN_CHARS)]
  TooLong(String),
  /// A relative program path is looked up in the worktree and must stay inside it.
  #[error("the program `{0}` leaves the worktree")]
  OutsideWorktree(String),
  /// `C:x.exe` names a file relative to the current folder of drive C, which depends on the process.
  #[error("the program `{0}` names a drive without a folder from its root")]
  DriveRelative(String),
  #[error("the timeout must be between 1 second and {max} hours", max = CommandTimeout::MAX_HOURS)]
  Timeout,
  #[error("a command tool expects at least one exit code")]
  NoExitCodes,
  #[error("a command tool expects at most {max} exit codes", max = ExitCodes::MAX)]
  TooManyExitCodes,
  #[error(
    "`{0}` is not an environment variable name: a letter or underscore, then letters, digits or underscores, at \
     most {max} characters",
    max = MAX_NAME_CHARS
  )]
  VariableName(String),
  /// Windows does not tell names apart by case, so neither does a tool.
  #[error("the environment variable `{0}` is set twice")]
  DuplicateVariable(String),
  #[error("the value of `{0}` contains a NUL character")]
  VariableValue(String),
  #[error("the output limit must be between 1 and {max} KiB", max = OutputLimit::MAX_KIB)]
  OutputLimit,
}

/// Longest program or argument, in characters.
const MAX_TOKEN_CHARS: usize = 4096;
const MAX_NAME_CHARS: usize = 128;

fn check_token(value: &str) -> Result<(), CommandError> {
  if value.chars().any(char::is_control) {
    return Err(CommandError::ControlCharacter(value.to_owned()));
  }
  if value.chars().count() > MAX_TOKEN_CHARS {
    return Err(CommandError::TooLong(value.to_owned()));
  }
  Ok(())
}

/// Whether `value` starts with a drive (`C:`) not followed by a separator. Read the same on every
/// platform, so a body means the same on Windows and macOS.
fn drive_relative(value: &str) -> bool {
  let mut chars = value.chars();
  let drive = chars.next().is_some_and(|letter| letter.is_ascii_alphabetic()) && chars.next() == Some(':');
  drive && !matches!(chars.next(), Some('/' | '\\'))
}

/// The program a command tool starts: a name looked up on the `PATH` (`cargo`), a path inside
/// the worktree (`scripts/check.sh`) or an absolute path. Surrounding spaces are not part of it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommandProgram(String);

/// Where a program is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramLocation<'a> {
  /// A bare name, looked up in the folders of the `PATH`.
  OnPath(&'a str),
  /// A path relative to the worktree.
  InWorktree(&'a Path),
  Absolute(&'a Path),
}

impl CommandProgram {
  pub fn new(value: &str) -> Result<Self, CommandError> {
    let value = value.trim();
    if value.is_empty() {
      return Err(CommandError::BlankProgram);
    }
    check_token(value)?;
    if drive_relative(value) {
      return Err(CommandError::DriveRelative(value.to_owned()));
    }
    let program = Self(value.to_owned());
    // Both separators count on every platform, so a body means the same on Windows and macOS.
    let leaves = value.split(['/', '\\']).any(|segment| segment == "..");
    if matches!(program.location(), ProgramLocation::InWorktree(_)) && leaves {
      return Err(CommandError::OutsideWorktree(value.to_owned()));
    }
    Ok(program)
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  pub fn location(&self) -> ProgramLocation<'_> {
    let path = Path::new(&self.0);
    // A path from the root of the current drive (`\tools\x.exe` on Windows) is not in the worktree
    // either.
    if path.is_absolute() || path.has_root() {
      ProgramLocation::Absolute(path)
    } else if self.0.contains(['/', '\\']) {
      ProgramLocation::InWorktree(path)
    } else {
      ProgramLocation::OnPath(&self.0)
    }
  }
}

impl TryFrom<String> for CommandProgram {
  type Error = CommandError;

  fn try_from(value: String) -> Result<Self, CommandError> {
    Self::new(&value)
  }
}

impl From<CommandProgram> for String {
  fn from(program: CommandProgram) -> Self {
    program.0
  }
}

/// One argument, passed to the program as it is: spaces and quotes are part of it, and no shell
/// reads it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommandArg(String);

impl CommandArg {
  pub fn new(value: &str) -> Result<Self, CommandError> {
    if value.is_empty() {
      return Err(CommandError::EmptyArgument);
    }
    check_token(value)?;
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for CommandArg {
  type Error = CommandError;

  fn try_from(value: String) -> Result<Self, CommandError> {
    Self::new(&value)
  }
}

impl From<CommandArg> for String {
  fn from(arg: CommandArg) -> Self {
    arg.0
  }
}

/// How long the program may run before it and everything it started are ended and the step
/// fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct CommandTimeout(u64);

impl CommandTimeout {
  pub const MAX_HOURS: u64 = 24;
  pub const DEFAULT: Self = Self(10 * 60);

  pub fn from_secs(secs: u64) -> Result<Self, CommandError> {
    if (1..=Self::MAX_HOURS * 3600).contains(&secs) {
      Ok(Self(secs))
    } else {
      Err(CommandError::Timeout)
    }
  }

  pub const fn secs(self) -> u64 {
    self.0
  }

  pub const fn duration(self) -> Duration {
    Duration::from_secs(self.0)
  }
}

impl TryFrom<u64> for CommandTimeout {
  type Error = CommandError;

  fn try_from(secs: u64) -> Result<Self, CommandError> {
    Self::from_secs(secs)
  }
}

impl From<CommandTimeout> for u64 {
  fn from(timeout: CommandTimeout) -> Self {
    timeout.0
  }
}

/// Exit codes that mean the program did what the step needs; any other code fails the step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BTreeSet<i64>", into = "BTreeSet<i64>")]
pub struct ExitCodes(BTreeSet<i64>);

impl ExitCodes {
  pub const MAX: usize = 32;

  pub fn new(codes: BTreeSet<i64>) -> Result<Self, CommandError> {
    if codes.is_empty() {
      return Err(CommandError::NoExitCodes);
    }
    if codes.len() > Self::MAX {
      return Err(CommandError::TooManyExitCodes);
    }
    Ok(Self(codes))
  }

  /// Exit code 0 only.
  pub fn success() -> Self {
    Self(BTreeSet::from([0]))
  }

  pub fn contains(&self, code: i64) -> bool {
    self.0.contains(&code)
  }

  pub fn iter(&self) -> impl Iterator<Item = i64> + '_ {
    self.0.iter().copied()
  }
}

impl TryFrom<BTreeSet<i64>> for ExitCodes {
  type Error = CommandError;

  fn try_from(codes: BTreeSet<i64>) -> Result<Self, CommandError> {
    Self::new(codes)
  }
}

impl From<ExitCodes> for BTreeSet<i64> {
  fn from(codes: ExitCodes) -> Self {
    codes.0
  }
}

/// Name of an environment variable, e.g. `RUST_LOG`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VariableName(String);

impl VariableName {
  pub fn new(value: &str) -> Result<Self, CommandError> {
    let mut chars = value.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let rest = chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if first && rest && value.len() <= MAX_NAME_CHARS {
      Ok(Self(value.to_owned()))
    } else {
      Err(CommandError::VariableName(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for VariableName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for VariableName {
  type Error = CommandError;

  fn try_from(value: String) -> Result<Self, CommandError> {
    Self::new(&value)
  }
}

impl From<VariableName> for String {
  fn from(name: VariableName) -> Self {
    name.0
  }
}

/// Variables added on top of the cleaned environment every program Orchester starts gets (see
/// [`crate::connection::invocation::agent_environment`]): the ones a command tool sets for its
/// program, and the ones a project adds for every agent CLI and command tool its runs start
/// (Project settings › Environment). They are stored and shown in plain text, so they hold no
/// secrets.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<VariableName, String>", into = "BTreeMap<VariableName, String>")]
pub struct CommandEnvironment(BTreeMap<VariableName, String>);

impl CommandEnvironment {
  pub fn new(variables: BTreeMap<VariableName, String>) -> Result<Self, CommandError> {
    let mut seen = BTreeSet::new();
    for (name, value) in &variables {
      if !seen.insert(name.as_str().to_ascii_uppercase()) {
        return Err(CommandError::DuplicateVariable(name.as_str().to_owned()));
      }
      if value.contains('\0') {
        return Err(CommandError::VariableValue(name.as_str().to_owned()));
      }
    }
    Ok(Self(variables))
  }

  pub fn iter(&self) -> impl Iterator<Item = (&VariableName, &str)> {
    self.0.iter().map(|(name, value)| (name, value.as_str()))
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// The value of `name`, compared without regard to case as Windows does.
  pub fn get(&self, name: &str) -> Option<&str> {
    self
      .iter()
      .find(|(variable, _)| variable.as_str().eq_ignore_ascii_case(name))
      .map(|(_, value)| value)
  }

  /// Sets these variables in `variables`, replacing ones of the same name without regard to case.
  pub fn add_to(&self, variables: &mut Vec<(String, String)>) {
    for (name, value) in self.iter() {
      variables.retain(|(existing, _)| !existing.eq_ignore_ascii_case(name.as_str()));
      variables.push((name.as_str().to_owned(), value.to_owned()));
    }
  }
}

impl TryFrom<BTreeMap<VariableName, String>> for CommandEnvironment {
  type Error = CommandError;

  fn try_from(variables: BTreeMap<VariableName, String>) -> Result<Self, CommandError> {
    Self::new(variables)
  }
}

impl From<CommandEnvironment> for BTreeMap<VariableName, String> {
  fn from(environment: CommandEnvironment) -> Self {
    environment.0
  }
}

/// How much of the program's output (both streams) the transcript keeps: the last bytes up to the
/// limit. What came before is counted and marked as not kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct OutputLimit(u32);

impl OutputLimit {
  pub const MAX_KIB: u32 = 16 * 1024;
  pub const DEFAULT: Self = Self(256);

  pub fn from_kib(kib: u32) -> Result<Self, CommandError> {
    if (1..=Self::MAX_KIB).contains(&kib) {
      Ok(Self(kib))
    } else {
      Err(CommandError::OutputLimit)
    }
  }

  pub const fn kib(self) -> u32 {
    self.0
  }

  pub const fn bytes(self) -> usize {
    self.0 as usize * 1024
  }
}

impl TryFrom<u32> for OutputLimit {
  type Error = CommandError;

  fn try_from(kib: u32) -> Result<Self, CommandError> {
    Self::from_kib(kib)
  }
}

impl From<OutputLimit> for u32 {
  fn from(limit: OutputLimit) -> Self {
    limit.0
  }
}

/// A program run with fixed arguments. Its working directory is always the worktree of the step
/// that runs it; see [`super::launch`] for how it starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandAction {
  program: CommandProgram,
  args: Vec<CommandArg>,
  timeout_secs: CommandTimeout,
  expected_exit_codes: ExitCodes,
  environment: CommandEnvironment,
  output_limit_kib: OutputLimit,
}

impl CommandAction {
  /// `program` with `args`, the default timeout (10 minutes), exit code 0, no environment
  /// additions and the default output limit (256 KiB).
  pub fn new(program: CommandProgram, args: Vec<CommandArg>) -> Self {
    Self {
      program,
      args,
      timeout_secs: CommandTimeout::DEFAULT,
      expected_exit_codes: ExitCodes::success(),
      environment: CommandEnvironment::default(),
      output_limit_kib: OutputLimit::DEFAULT,
    }
  }

  #[must_use]
  pub const fn with_timeout(mut self, timeout: CommandTimeout) -> Self {
    self.timeout_secs = timeout;
    self
  }

  #[must_use]
  pub fn with_exit_codes(mut self, codes: ExitCodes) -> Self {
    self.expected_exit_codes = codes;
    self
  }

  #[must_use]
  pub fn with_environment(mut self, environment: CommandEnvironment) -> Self {
    self.environment = environment;
    self
  }

  #[must_use]
  pub const fn with_output_limit(mut self, limit: OutputLimit) -> Self {
    self.output_limit_kib = limit;
    self
  }

  pub const fn program(&self) -> &CommandProgram {
    &self.program
  }

  pub fn args(&self) -> &[CommandArg] {
    &self.args
  }

  pub const fn timeout(&self) -> CommandTimeout {
    self.timeout_secs
  }

  pub const fn exit_codes(&self) -> &ExitCodes {
    &self.expected_exit_codes
  }

  pub const fn environment(&self) -> &CommandEnvironment {
    &self.environment
  }

  pub const fn output_limit(&self) -> OutputLimit {
    self.output_limit_kib
  }

  /// The command line as a person reads it: the program and its arguments, each quoted when it
  /// has spaces or quotes. No shell reads it; the program gets the arguments as they are.
  pub fn command_line(&self) -> String {
    std::iter::once(self.program.as_str())
      .chain(self.args.iter().map(CommandArg::as_str))
      .map(quoted)
      .collect::<Vec<_>>()
      .join(" ")
  }
}

fn quoted(token: &str) -> String {
  if token.contains([' ', '"', '\'']) {
    format!("\"{}\"", token.replace('"', "\\\""))
  } else {
    token.to_owned()
  }
}
