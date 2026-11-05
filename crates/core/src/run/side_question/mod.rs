//! A side question about a run (ORC-337, `docs/design/side-question-and-plan-mode.md`, "Side question"): a person
//! asks about a run without writing to its agent. A separate agent answers on the Orchestrator's connection and
//! model, from the run's transcript and its changes only ([`SideInput`]); it is given no tools at all, so it can
//! neither write nor run anything (`crate::connection::reviewer::side`). The question and its answer are the task
//! page's: they are kept with the task, never in the run's conversation, and nothing reaches the run's agent until
//! the person chooses *Send to {agent}*, which posts them as one ordinary message ([`sent_on_text`]).

mod input;
mod prompt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  input::{DIFF_MAX_CHARS, Exchange, SideAbout, SideInput, TRANSCRIPT_MAX_CHARS},
  prompt::{SIDE_INSTRUCTIONS, SIDE_SCHEMA, parse_side_reply, parse_side_reply_text, sent_on_text, side_prompt},
};
use crate::{
  id::{RunId, SideQuestionId, TaskId},
  time::Timestamp,
};

/// The most characters a question may have.
pub const QUESTION_MAX_CHARS: usize = 4_000;
/// The most sources an answer names; more are left out.
pub const MAX_SOURCES: usize = 6;

/// What a person asks: not blank, at most [`QUESTION_MAX_CHARS`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SideQuestionText(String);

/// Why a question cannot be asked.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum QuestionError {
  #[error("the question is blank")]
  Blank,
  #[error("the question is longer than {QUESTION_MAX_CHARS} characters")]
  TooLong,
}

impl SideQuestionText {
  /// `text` without the white space around it.
  pub fn new(text: &str) -> Result<Self, QuestionError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(QuestionError::Blank);
    }
    if text.chars().count() > QUESTION_MAX_CHARS {
      return Err(QuestionError::TooLong);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for SideQuestionText {
  type Error = QuestionError;

  fn try_from(value: String) -> Result<Self, QuestionError> {
    Self::new(&value)
  }
}

impl From<SideQuestionText> for String {
  fn from(text: SideQuestionText) -> Self {
    text.0
  }
}

/// What kind of thing an answer relies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
  /// A changed file, by its path.
  File,
  /// A command the agent ran.
  Command,
  /// A message of the agent's.
  Message,
}

/// One thing an answer relies on: a file, a command or a message, by its name, and a few words about it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideSource {
  pub kind: SourceKind,
  pub name: String,
  pub detail: String,
}

/// The side agent's answer: its text, and what it relies on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideReply {
  pub text: String,
  pub sources: Vec<SideSource>,
}

/// Why a side question got no answer.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SideFailure {
  /// Nothing was chosen for the Orchestrator to run on, so nothing can answer.
  #[error("nothing was chosen for the Orchestrator to run on")]
  NoOrchestrator,
  /// The Orchestrator's connection cannot run on its account: signed out, or its folder is gone.
  #[error("the Orchestrator's connection cannot run on its account")]
  SignedOut,
  /// The project's daily budget is used.
  #[error("the project's daily budget is used")]
  DailyBudgetUsed,
  /// The values of the project's secrets could not be read, so what the agent would read could not be checked.
  #[error("the project's secrets could not be read to keep them out")]
  SecretsUnreadable,
  /// The side agent did not answer in time.
  #[error("the side agent did not answer in time")]
  TimedOut,
  /// The side agent ran, but its answer could not be used or it could not run: why, its secrets and prompt kept out.
  #[error("the side agent could not answer: {detail}")]
  Failed { detail: String },
  /// The person stopped it.
  #[error("the answer was stopped")]
  Stopped,
  /// The app or the runner stopped while it answered.
  #[error("the answer was cut short")]
  Interrupted,
}

/// How the side agent's work on one question ended, as the runner tells it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SideOutcome {
  /// It answered with `reply`, on `model`.
  Answered {
    reply: SideReply,
    model: String,
  },
  Failed {
    failure: SideFailure,
  },
}

/// Where a side question stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideState {
  /// The side agent is answering.
  Answering,
  /// It answered with `reply`, on `model`, at `at`; `sent` once the person passed it on to the run's agent.
  Answered {
    reply: SideReply,
    model: String,
    at: Timestamp,
    sent: Option<Timestamp>,
  },
  /// It got no answer.
  Failed { failure: SideFailure, at: Timestamp },
}

/// One side question about a run, kept with its task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideQuestion {
  pub id: SideQuestionId,
  pub task: TaskId,
  pub run: RunId,
  pub question: SideQuestionText,
  pub asked_at: Timestamp,
  pub state: SideState,
}

impl SideQuestion {
  /// A question just asked, which the side agent answers now.
  pub fn asked(id: SideQuestionId, (task, run): (TaskId, RunId), question: SideQuestionText, at: Timestamp) -> Self {
    Self {
      id,
      task,
      run,
      question,
      asked_at: at,
      state: SideState::Answering,
    }
  }

  /// The answer, once there is one.
  pub const fn reply(&self) -> Option<&SideReply> {
    match &self.state {
      SideState::Answered { reply, .. } => Some(reply),
      SideState::Answering | SideState::Failed { .. } => None,
    }
  }

  /// Whether it was passed on to the run's agent.
  pub const fn sent(&self) -> bool {
    matches!(self.state, SideState::Answered { sent: Some(_), .. })
  }
}
