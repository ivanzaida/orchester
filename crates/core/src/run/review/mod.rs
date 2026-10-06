//! Auto mode (user decision 2026-10-06, modelled on Claude Code's auto mode): an independent reviewer
//! model judges each tool request that the rules leave to a person ([`super::auto_verdict`] and the run's
//! rules answer first). It approves what clearly serves the task and is neither destructive, nor
//! exfiltrating, nor escaping the step's permissions; everything else still goes to the person, with the
//! reviewer's reason. It is not "approve everything".
//!
//! Hard rules come before the reviewer and it cannot override them ([`review_hold`], every part of a
//! command line checked on its own): a credential read, a part Orchester could not read to the end, a
//! rewrite of history or a delete outside the worktree, and anything outside the step's permissions (a
//! write in a read-only step, the network while it is off, a write outside the worktree and its writable
//! folders) always go to the person, as do Codex escalations and requests too long to be reviewed whole.
//! The reviewer fails closed: an error, a timeout or an answer that cannot be read goes to the person too
//! ([`Review::of`]). What it reads holds no secret value: the request as the approval card shows it,
//! redacted the same way. Its reason is the reviewer's own text, so it is kept to one plain line.

mod hold;
mod input;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  hold::{ReviewHold, review_hold},
  input::{ReviewInput, ReviewedRequest, ReviewedStep, ReviewedTask},
};
use super::answer::ApprovalSubject;
use crate::{permission::AgentPermissions, resource::ModelId};

/// The reviewer's model unless a project names another: small and quick, as one review holds the agent.
pub const DEFAULT_REVIEW_MODEL: &str = "claude-haiku-4-5-20251001";
/// The longest a reason is kept; a longer one is cut.
const REASON_MAX_CHARS: usize = 200;
/// The quotation marks a reason may not hold, as the app quotes it.
const QUOTES: [char; 6] = ['"', '\u{201c}', '\u{201d}', '\u{201e}', '\u{ab}', '\u{bb}'];

/// What the reviewer decided about one request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewVerdict {
  Approve { reason: String },
  AskPerson { reason: String },
}

/// Why a review gave no verdict.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ReviewError {
  #[error("the reviewer did not answer in time")]
  TimedOut,
  #[error("the reviewer's answer could not be read: {0}")]
  Unparseable(String),
  #[error("the reviewer could not run: {0}")]
  CouldNotRun(String),
}

/// A model that judges a request in its context: a provider boundary, so the runner's reviewer (Claude
/// Code) and the tests' stand-in share the rules around it.
pub trait RequestReviewer {
  /// The model that reviews.
  fn model(&self) -> &ModelId;
  fn review(&self, input: &ReviewInput) -> Result<ReviewVerdict, ReviewError>;
}

/// Why a review that ran gave no verdict, as it is recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFailure {
  TimedOut,
  Unreadable,
  CouldNotRun,
  /// The project has no Claude Code connection to review with.
  NoReviewer,
}

/// How auto mode handled a request, as it is recorded with the request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Review {
  /// The reviewer approved it, for this reason.
  Approved { reason: String, model: String },
  /// The reviewer left it to the person, for this reason.
  Asked { reason: String, model: String },
  /// A hard rule left it to the person without a review.
  Held { hold: ReviewHold },
  /// The review gave no verdict, so the person decides.
  Failed {
    failure: ReviewFailure,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
  },
}

impl Review {
  /// What a review by `model` ended with: a verdict, or a failure, which asks the person.
  pub fn of(result: Result<ReviewVerdict, ReviewError>, model: &ModelId) -> Self {
    let model = model.as_str().to_owned();
    match result {
      Ok(ReviewVerdict::Approve { reason }) => Self::Approved {
        reason: kept(&reason),
        model,
      },
      Ok(ReviewVerdict::AskPerson { reason }) => Self::Asked {
        reason: kept(&reason),
        model,
      },
      Err(error) => Self::Failed {
        failure: match error {
          ReviewError::TimedOut => ReviewFailure::TimedOut,
          ReviewError::Unparseable(_) => ReviewFailure::Unreadable,
          ReviewError::CouldNotRun(_) => ReviewFailure::CouldNotRun,
        },
        model: Some(model),
      },
    }
  }

  /// Whether the request is approved without the person.
  pub const fn approves(&self) -> bool {
    matches!(self, Self::Approved { .. })
  }

  /// The reviewer's own reason, when it gave one.
  pub fn reason(&self) -> Option<&str> {
    match self {
      Self::Approved { reason, .. } | Self::Asked { reason, .. } => Some(reason),
      Self::Held { .. } | Self::Failed { .. } => None,
    }
  }
}

/// `reason` as one plain line: control characters and line breaks become spaces, runs of spaces one,
/// double quotation marks single ones (the app quotes it), cut to [`REASON_MAX_CHARS`] with an ellipsis.
fn kept(reason: &str) -> String {
  let plain: String = reason
    .chars()
    .map(|c| match c {
      c if QUOTES.contains(&c) => '\'',
      c if c.is_control() || c.is_whitespace() => ' ',
      c => c,
    })
    .collect();
  let line = plain.split_whitespace().collect::<Vec<_>>().join(" ");
  if line.chars().count() <= REASON_MAX_CHARS {
    return line;
  }
  let mut cut: String = line.chars().take(REASON_MAX_CHARS - 1).collect();
  cut.push('\u{2026}');
  cut
}

/// Auto mode's handling of a request about `subject`: a hard rule first, under the step's recorded
/// `permissions`; then a request too long to be reviewed whole (`input`'s hold); then `reviewer`'s
/// verdict on `input`, or a failure when the project has none.
pub fn review_request(
  reviewer: Option<&dyn RequestReviewer>,
  (subject, permissions): (&ApprovalSubject, Option<&AgentPermissions>),
  input: Result<ReviewInput, ReviewHold>,
) -> Review {
  if let Some(hold) = review_hold(subject, permissions) {
    return Review::Held { hold };
  }
  let input = match input {
    Ok(input) => input,
    Err(hold) => return Review::Held { hold },
  };
  match reviewer {
    Some(reviewer) => Review::of(reviewer.review(&input), reviewer.model()),
    None => Review::Failed {
      failure: ReviewFailure::NoReviewer,
      model: None,
    },
  }
}

/// The settings of auto mode in a project: whether new runs use it, and the reviewer's model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoModeSettings {
  pub new_runs: bool,
  pub model: ModelId,
}

impl Default for AutoModeSettings {
  fn default() -> Self {
    Self {
      new_runs: false,
      model: ModelId::new(DEFAULT_REVIEW_MODEL).expect("the default review model is a valid id"),
    }
  }
}
