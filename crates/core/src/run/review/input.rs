//! What the reviewer reads about one request: the request as the approval card shows it (already
//! redacted), the step's permissions, the task, and the last few messages the step's agent wrote. The
//! request itself is never cut: one too long to be read whole is held for the person
//! ([`ReviewHold::TooLong`]), as a cut one could hide its harmful end. Only the context (the agent's
//! note and messages, the task) is kept within a size, and what was cut says so.

use super::ReviewHold;
use crate::permission::{NetworkAccess, RepositoryAccess};

/// The most messages of the agent the reviewer reads, newest last…
const MESSAGES_KEPT: usize = 3;
/// …each cut to this many characters…
const MESSAGE_MAX_CHARS: usize = 2_000;
/// …the task's brief and each criterion to these…
const BRIEF_MAX_CHARS: usize = 6_000;
const CRITERION_MAX_CHARS: usize = 500;
const CRITERIA_KEPT: usize = 20;
/// …and the agent's note to this.
const NOTE_MAX_CHARS: usize = 2_000;
/// The longest request reviewed: its command or tool, arguments, server and working folder together.
const REQUEST_MAX_CHARS: usize = 8_000;

/// The request under review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewedRequest {
  /// A shell command, or else a tool of the MCP server `server`.
  pub command: bool,
  pub server: String,
  /// The command, or the tool's name.
  pub action: String,
  /// An MCP call's arguments as recorded, secret values removed.
  pub arguments: Option<String>,
  /// What the agent said the command is for.
  pub note: Option<String>,
  /// Where a command runs.
  pub working_folder: Option<String>,
}

/// The step whose agent asks, and what its permissions allow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewedStep {
  pub name: String,
  pub repository: RepositoryAccess,
  pub network: NetworkAccess,
  /// Folders outside the worktree the step may write to.
  pub writable_folders: Vec<String>,
}

/// The task the run works on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewedTask {
  pub title: String,
  pub brief: String,
  pub criteria: Vec<String>,
}

/// Everything the reviewer reads, within its sizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewInput {
  request: ReviewedRequest,
  step: ReviewedStep,
  task: ReviewedTask,
  messages: Vec<String>,
  /// How many parts of the command Orchester could not read to the end.
  unread: usize,
  /// The names of the project's variables that change what a build runs, never their values.
  build_variables: Vec<String>,
}

impl ReviewInput {
  /// The input of a review; `messages` are the agent's, oldest first, of which the last few are kept.
  /// [`ReviewHold::TooLong`] when the request is longer than a review reads whole.
  pub fn new(
    request: ReviewedRequest,
    step: ReviewedStep,
    task: ReviewedTask,
    messages: &[String],
  ) -> Result<Self, ReviewHold> {
    let length = [
      Some(&request.action),
      Some(&request.server),
      request.arguments.as_ref(),
      request.working_folder.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|text| text.chars().count())
    .sum::<usize>();
    if length > REQUEST_MAX_CHARS {
      return Err(ReviewHold::TooLong);
    }
    let request = ReviewedRequest {
      note: request.note.as_deref().map(|text| cut(text, NOTE_MAX_CHARS)),
      ..request
    };
    let mut criteria: Vec<String> = task
      .criteria
      .iter()
      .take(CRITERIA_KEPT)
      .map(|criterion| cut(criterion, CRITERION_MAX_CHARS))
      .collect();
    if task.criteria.len() > CRITERIA_KEPT {
      criteria.push(format!("(cut: {} more criteria)", task.criteria.len() - CRITERIA_KEPT));
    }
    let task = ReviewedTask {
      brief: cut(&task.brief, BRIEF_MAX_CHARS),
      criteria,
      ..task
    };
    let skipped = messages.len().saturating_sub(MESSAGES_KEPT);
    let messages = messages[skipped..]
      .iter()
      .map(|message| cut(message, MESSAGE_MAX_CHARS))
      .collect();
    Ok(Self {
      request,
      step,
      task,
      messages,
      unread: 0,
      build_variables: Vec::new(),
    })
  }

  /// The input, telling the reviewer that Orchester could not read `parts` parts of the command.
  #[must_use]
  pub const fn with_unread(mut self, parts: usize) -> Self {
    self.unread = parts;
    self
  }

  /// The input, telling the reviewer the `names` of the project's variables that change what a build runs
  /// ([`crate::permission::category::build_variables`]); never their values.
  #[must_use]
  pub fn with_build_variables(mut self, names: Vec<String>) -> Self {
    self.build_variables = names;
    self
  }

  /// The names of the project's variables that change what a build runs.
  pub fn build_variables(&self) -> &[String] {
    &self.build_variables
  }

  /// How many parts of the command Orchester could not read to the end.
  pub const fn unread(&self) -> usize {
    self.unread
  }

  pub const fn request(&self) -> &ReviewedRequest {
    &self.request
  }

  pub const fn step(&self) -> &ReviewedStep {
    &self.step
  }

  pub const fn task(&self) -> &ReviewedTask {
    &self.task
  }

  /// The agent's last messages, oldest first.
  pub fn messages(&self) -> &[String] {
    &self.messages
  }
}

/// `text` cut to `limit` characters, saying so when it was.
fn cut(text: &str, limit: usize) -> String {
  if text.chars().count() <= limit {
    return text.to_owned();
  }
  let kept: String = text.chars().take(limit).collect();
  format!("{kept}… (cut)")
}
