//! What an agent says its step is doing (`report_progress`, `docs/plan.md` M12): one short line, shown on the run's
//! Summary; a later report replaces the earlier one.

use thiserror::Error;

/// Why a progress line is refused. The message is the error result the agent gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProgressError {
  #[error("the progress line must not be blank")]
  Blank,
  #[error("the progress line is one line of at most {max} characters", max = StepProgress::MAX_CHARS)]
  TooLong,
}

/// One short status line for a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepProgress(String);

impl StepProgress {
  pub const MAX_CHARS: usize = 200;

  /// `text` without the whitespace around it: one line, not blank, at most [`Self::MAX_CHARS`] characters.
  pub fn new(text: &str) -> Result<Self, ProgressError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(ProgressError::Blank);
    }
    if text.chars().count() > Self::MAX_CHARS || text.chars().any(char::is_control) {
      return Err(ProgressError::TooLong);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}
