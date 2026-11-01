//! Auto mode (user decision 2026-10-06, modelled on Claude Code's auto mode): an independent reviewer
//! model judges each tool request that the rules leave to a person ([`super::auto_verdict`] and the run's
//! rules answer first). It approves what clearly serves the task and is neither destructive, nor
//! exfiltrating, nor escaping the step's permissions; everything else still goes to the person, with the
//! reviewer's reason. It is not "approve everything".
//!
//! Hard rules come before the reviewer and it cannot override them ([`review_hold`], every part of a
//! command line checked on its own): a credential read, a rewrite of history or a delete outside the
//! worktree, and anything outside the step's permissions (a write in a read-only step, the network while it
//! is off, a write outside the worktree and its writable folders) always go to the person, as do Codex
//! escalations and requests too long to be reviewed whole. A part Orchester could not read to the end goes to
//! the reviewer, told which and why ([`unread_parts`]), which judges it against the step's permissions.
//! The reviewer fails closed: an error, a timeout or an answer that cannot be read goes to the person too
//! ([`Review::of`]). What it reads holds no secret value: the request as the approval card shows it,
//! redacted the same way. Its reason is the reviewer's own text, so it is kept to one plain line.

mod choice;
mod decision;
mod hold;
mod input;
mod prompt;
mod push;
mod spend;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) use self::prompt::one_line;
pub use self::{
  choice::{
    AutoModeSettings, DEFAULT_REVIEW_MODEL, DecisionReviewerConnection, ResolvedReviewer, ReviewerChoice,
    ReviewerConnection, ReviewerConnectionId, ReviewerKind, ReviewerUnavailable, decision_reviewer_person_only,
    default_decision_threshold, default_review_model, resolve_reviewer, reviewer_kind,
  },
  decision::{DecisionVerdict, NotProbability, Probability, Risk, Risks},
  hold::{HoldRules, ReviewHold, StepLimit, review_hold, step_limit, unread_parts},
  input::{ReviewInput, ReviewedRequest, ReviewedStep, ReviewedTask, RootsQuestion, UnreadPart, UnreadWhy},
  prompt::{
    DATA_END, DATA_START, REVIEW_INSTRUCTIONS, REVIEW_SCHEMA, ROOTS_QUESTION, parse_verdict, parse_verdict_text,
    review_prompt, review_state,
  },
  push::PushFacts,
  spend::{AutoReview, ReviewCost, ReviewEnd, ReviewRecord, ReviewSpend, Reviewed, reviews_usage},
};
use super::answer::ApprovalSubject;
use crate::resource::ModelId;

/// The longest a reason is kept; a longer one is cut.
const REASON_MAX_CHARS: usize = 200;
/// Characters that change how text around them reads without showing: bidirectional controls and
/// zero-width characters. A reason loses them before it is stored and shown.
const INVISIBLE: [char; 18] = [
  '\u{061c}', '\u{180e}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}',
  '\u{202c}', '\u{202d}', '\u{202e}', '\u{2060}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}', '\u{feff}',
];
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
  /// The verdict on `input`, with what the review spent and the plan limits it reported.
  fn review(&self, input: &ReviewInput) -> Reviewed;
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
  /// A decision model judged it: approved at or above the threshold, left to the person below it.
  Decided { decision: DecisionVerdict },
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
        reason: one_plain_line(&reason),
        model,
      },
      Ok(ReviewVerdict::AskPerson { reason }) => Self::Asked {
        reason: one_plain_line(&reason),
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
  pub fn approves(&self) -> bool {
    match self {
      Self::Approved { .. } => true,
      Self::Decided { decision } => decision.approves(),
      Self::Asked { .. } | Self::Held { .. } | Self::Failed { .. } => false,
    }
  }

  /// The reviewer's own reason, when it gave one; a decision model gives none.
  pub fn reason(&self) -> Option<&str> {
    match self {
      Self::Approved { reason, .. } | Self::Asked { reason, .. } => Some(reason),
      Self::Decided { .. } | Self::Held { .. } | Self::Failed { .. } => None,
    }
  }
}

/// `reason`, a model's own text, as one plain line: control characters and line breaks become spaces, runs of
/// spaces one, double quotation marks single ones (the app quotes it), cut to [`REASON_MAX_CHARS`] with an ellipsis.
/// The reviewer's reasons and the story-point estimator's are kept so.
pub fn one_plain_line(reason: &str) -> String {
  let plain: String = reason
    .chars()
    .map(|c| match c {
      c if QUOTES.contains(&c) => '\'',
      c if c.is_control() || c.is_whitespace() => ' ',
      c => c,
    })
    .filter(|c| !INVISIBLE.contains(c))
    .collect();
  let line = plain.split_whitespace().collect::<Vec<_>>().join(" ");
  if line.chars().count() <= REASON_MAX_CHARS {
    return line;
  }
  let mut cut: String = line.chars().take(REASON_MAX_CHARS - 1).collect();
  cut.push('\u{2026}');
  cut
}

/// Auto mode's handling of a request about `subject`: a hard rule first, under `rules`; then a request
/// too long to be reviewed whole (`input`'s hold); then `reviewer`'s verdict on `input`, told how many
/// parts Orchester could not read, with what asking it cost; or a failure when the project has none.
pub fn review_request(
  reviewer: Option<&dyn RequestReviewer>,
  (subject, rules): (&ApprovalSubject, &HoldRules<'_>),
  input: Result<ReviewInput, ReviewHold>,
) -> AutoReview {
  if let Some(hold) = review_hold(subject, rules) {
    return held(hold);
  }
  match input {
    Ok(input) => ask(reviewer, &input.with_unread(unread_parts(subject))),
    Err(hold) => held(hold),
  }
}

/// Autopilot's one question about a request about `subject` that Orchester could not place: `reviewer`'s answer to
/// `question` on `input`. Of Manual's hard rules only the step's own limits under `rules` hold ([`step_limit`]), as
/// Autopilot never asks the reviewer whether to approve; a request too long to be reviewed whole is held.
pub fn review_roots(
  reviewer: Option<&dyn RequestReviewer>,
  (subject, question, rules): (&ApprovalSubject, RootsQuestion, &HoldRules<'_>),
  input: Result<ReviewInput, ReviewHold>,
) -> AutoReview {
  if let StepLimit::Held(hold) = step_limit(subject, rules.permissions, rules.build_output) {
    return held(hold);
  }
  match input {
    Ok(input) => ask(
      reviewer,
      &input.with_unread(unread_parts(subject)).with_roots_question(question),
    ),
    Err(hold) => held(hold),
  }
}

fn held(hold: ReviewHold) -> AutoReview {
  AutoReview {
    review: Review::Held { hold },
    cost: None,
  }
}

/// `reviewer`'s verdict on `input`, with what asking it cost; a failure when the project has none.
fn ask(reviewer: Option<&dyn RequestReviewer>, input: &ReviewInput) -> AutoReview {
  let Some(reviewer) = reviewer else {
    return AutoReview {
      review: Review::Failed {
        failure: ReviewFailure::NoReviewer,
        model: None,
      },
      cost: None,
    };
  };
  let reviewed = reviewer.review(input);
  let cost = ReviewCost::of(&reviewed, reviewer.model());
  AutoReview {
    review: Review::of(reviewed.verdict, reviewer.model()),
    cost,
  }
}
