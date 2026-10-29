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

/// A part of the command Orchester could not read to the end, and why it stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnreadPart {
  /// Which simple command of the line it is, from 1.
  pub at: usize,
  pub why: UnreadWhy,
}

/// Why Orchester stopped reading a part of a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnreadWhy {
  /// It runs a script file or inline code, whose content Orchester does not read.
  Script,
  /// Its program, subcommand, option or form (a variable, a substitution, a command in the background) is not one
  /// Orchester knows.
  Unknown,
}

impl UnreadWhy {
  /// The reason as the reviewer reads it.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Script => "it runs a script file or inline code whose content Orchester does not read",
      Self::Unknown => {
        "its program, subcommand, option or form (a variable, a substitution, a background command) is not one \
         Orchester's parser knows"
      }
    }
  }
}

/// Autopilot's one question about a request Orchester could not place (`crate::run::ReviewQuestion::Roots`): whether
/// it touches anything outside the run's roots, pushes to a remote, or changes a ref other than the run's branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootsQuestion {
  /// The run's worktree, its Sources and the agent's own folders.
  pub roots: Vec<String>,
  pub branch: String,
}

/// Everything the reviewer reads, within its sizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewInput {
  request: ReviewedRequest,
  step: ReviewedStep,
  task: ReviewedTask,
  messages: Vec<String>,
  /// The parts of the command Orchester could not read to the end.
  unread: Vec<UnreadPart>,
  /// The names of the project's variables that change what a build runs, never their values.
  build_variables: Vec<String>,
  /// Autopilot's question, which replaces whether to approve the request.
  roots: Option<RootsQuestion>,
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
      unread: Vec::new(),
      build_variables: Vec::new(),
      roots: None,
    })
  }

  /// The input, telling the reviewer which `parts` of the command Orchester could not read to the end, and why.
  #[must_use]
  pub fn with_unread(mut self, parts: Vec<UnreadPart>) -> Self {
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

  /// The input, asking the reviewer Autopilot's `question` instead of whether to approve the request.
  #[must_use]
  pub fn with_roots_question(mut self, question: RootsQuestion) -> Self {
    self.roots = Some(question);
    self
  }

  /// The names of the project's variables that change what a build runs.
  pub fn build_variables(&self) -> &[String] {
    &self.build_variables
  }

  /// Autopilot's question, when the reviewer is asked it.
  pub const fn roots_question(&self) -> Option<&RootsQuestion> {
    self.roots.as_ref()
  }

  /// How many parts of the command Orchester could not read to the end.
  pub fn unread(&self) -> usize {
    self.unread.len()
  }

  /// The parts of the command Orchester could not read to the end, and why.
  pub fn unread_parts(&self) -> &[UnreadPart] {
    &self.unread
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
