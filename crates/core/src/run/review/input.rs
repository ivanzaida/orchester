//! What the reviewer reads about one request: the request as the approval card shows it (already
//! redacted), the step's permissions, the task, and the last few messages the step's agent wrote. Each
//! part is kept within a size, so a long brief or transcript never makes the review slow or costly.

use crate::permission::{NetworkAccess, RepositoryAccess};

/// The most messages of the agent the reviewer reads, newest last…
const MESSAGES_KEPT: usize = 3;
/// …each cut to this many characters…
const MESSAGE_MAX_CHARS: usize = 2_000;
/// …the task's brief and each criterion to these…
const BRIEF_MAX_CHARS: usize = 6_000;
const CRITERION_MAX_CHARS: usize = 500;
const CRITERIA_KEPT: usize = 20;
/// …and the request's arguments or note to this.
const DETAIL_MAX_CHARS: usize = 6_000;

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
}

impl ReviewInput {
  /// The input of a review; `messages` are the agent's, oldest first, of which the last few are kept.
  pub fn new(request: ReviewedRequest, step: ReviewedStep, task: ReviewedTask, messages: &[String]) -> Self {
    let request = ReviewedRequest {
      arguments: request.arguments.as_deref().map(|text| cut(text, DETAIL_MAX_CHARS)),
      note: request.note.as_deref().map(|text| cut(text, DETAIL_MAX_CHARS)),
      action: cut(&request.action, DETAIL_MAX_CHARS),
      ..request
    };
    let task = ReviewedTask {
      brief: cut(&task.brief, BRIEF_MAX_CHARS),
      criteria: task
        .criteria
        .iter()
        .take(CRITERIA_KEPT)
        .map(|criterion| cut(criterion, CRITERION_MAX_CHARS))
        .collect(),
      ..task
    };
    let skipped = messages.len().saturating_sub(MESSAGES_KEPT);
    let messages = messages[skipped..]
      .iter()
      .map(|message| cut(message, MESSAGE_MAX_CHARS))
      .collect();
    Self {
      request,
      step,
      task,
      messages,
    }
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
