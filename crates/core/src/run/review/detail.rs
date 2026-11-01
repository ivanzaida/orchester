//! Why a review gave no verdict, kept with the review and written to the runner's log: the reviewer's error in a few
//! lines (its CLI's exit status with the error it reported or the last lines of its error output, a model client's
//! error), with every known secret value of the invocation masked and every line that repeats what the reviewer was
//! given (its prompt) left out, so neither reaches the store or a log.

use std::cmp::Reverse;

use super::{ReviewError, ReviewFailure, plain_line};
use crate::{
  command::{Command, SecretVariable},
  connection::invocation::is_agent_variable,
  repository::redact_userinfo,
};

/// The longest a failure's detail is kept; a longer one is cut.
const DETAIL_MAX_CHARS: usize = 300;
/// A value of a variable the invocation sets beyond the agent environment is taken for a secret from this length:
/// shorter ones (`1`, `true`) say nothing and would only mangle the text.
const SECRET_MIN_CHARS: usize = 8;
/// A line holding this many characters in a row of what the reviewer was given repeats it, and is left out.
const ECHO_CHARS: usize = 24;
/// The longest line of an error looked at; the rest of it is cut.
const LINE_MAX_CHARS: usize = 300;
const REDACTED: &str = "<redacted>";

/// What a failure's detail must not hold: the secret values of the invocation and the text it was given.
pub struct DetailRedaction<'a> {
  secrets: Vec<&'a str>,
  given: &'a str,
}

impl<'a> DetailRedaction<'a> {
  /// Masks `secrets` and leaves out lines that repeat `given`, the text the reviewer was given.
  pub fn new(secrets: Vec<&'a str>, given: &'a str) -> Self {
    Self { secrets, given }
  }

  /// For a CLI's `command`: its secret variables, the values of the variables it sets beyond the agent environment
  /// (the project's own), and its standard input.
  pub fn of(command: &'a Command) -> Self {
    let own = command
      .env()
      .iter()
      .filter(|(name, value)| !is_agent_variable(name) && value.chars().count() >= SECRET_MIN_CHARS)
      .map(|(_, value)| value.as_str());
    let secrets = command
      .secret_env()
      .iter()
      .map(SecretVariable::expose)
      .chain(own)
      .collect();
    Self::new(secrets, command.stdin().unwrap_or_default())
  }

  /// `text` with the user part of URLs and the secrets masked, without the lines that repeat what was given.
  fn apply(&self, text: &str) -> String {
    let mut secrets: Vec<&str> = self
      .secrets
      .iter()
      .copied()
      .filter(|secret| !secret.is_empty())
      .collect();
    secrets.sort_by_key(|secret| Reverse(secret.len()));
    // URLs first: a masked secret's `<` would end the URL's user part early.
    let masked = secrets.iter().fold(redact_userinfo(text), |text, secret| {
      let encoded = serde_json::to_string(secret).expect("a string always serializes to JSON");
      text
        .replace(&encoded[1..encoded.len() - 1], REDACTED)
        .replace(secret, REDACTED)
    });
    masked
      .lines()
      .map(|line| line.trim().chars().take(LINE_MAX_CHARS).collect::<String>())
      .filter(|line| !self.repeats(line))
      .collect::<Vec<_>>()
      .join("\n")
  }

  /// Whether `line` repeats what was given: [`ECHO_CHARS`] characters of it in a row, or all of a shorter one.
  fn repeats(&self, line: &str) -> bool {
    let starts: Vec<usize> = line.char_indices().map(|(at, _)| at).collect();
    if starts.len() < ECHO_CHARS {
      return !line.is_empty() && self.given.contains(line);
    }
    (0..=starts.len() - ECHO_CHARS).any(|first| {
      let end = starts.get(first + ECHO_CHARS).copied().unwrap_or(line.len());
      self.given.contains(&line[starts[first]..end])
    })
  }
}

impl ReviewError {
  /// This error with its text cleaned by `redaction`, before it is kept or logged.
  #[must_use]
  pub fn redacted(self, redaction: &DetailRedaction<'_>) -> Self {
    match self {
      Self::TimedOut => Self::TimedOut,
      Self::Unparseable(text) => Self::Unparseable(redaction.apply(&text)),
      Self::CouldNotRun(text) => Self::CouldNotRun(redaction.apply(&text)),
    }
  }

  /// How a request that failed so is recorded.
  pub const fn failure(&self) -> ReviewFailure {
    match self {
      Self::TimedOut => ReviewFailure::TimedOut,
      Self::Unparseable(_) => ReviewFailure::Unreadable,
      Self::CouldNotRun(_) => ReviewFailure::CouldNotRun,
    }
  }

  /// Why the review gave no verdict, as it is kept with the review and logged: one plain line, cut to
  /// [`DETAIL_MAX_CHARS`].
  pub fn detail(&self) -> String {
    plain_line(&self.to_string(), DETAIL_MAX_CHARS)
  }
}
