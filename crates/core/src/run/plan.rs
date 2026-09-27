//! Plans: what the agent of a step with an approval gate proposes before the steps after it may
//! start (`docs/design/orchester-workspace.md`, 02.11 "Review & approve plan"). When a gated step's
//! agent finishes, its output becomes the next plan of that step in the run and the step waits for
//! a person ([`super::WaitReason::Plan`]). Approving passes the step and the graph continues;
//! requesting changes sends the step back to its agent with the reviewer's note, and the revised
//! result is the next plan, which needs a decision of its own.

use std::{fmt, num::NonZeroU32};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::result::StepOutput;
use crate::{
  id::{RunId, StepExecutionId},
  resource::StepKey,
  time::Timestamp,
};

/// A plan's number among the plans of its step in its run, from 1 ("plan v2").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlanVersion(NonZeroU32);

impl PlanVersion {
  pub const FIRST: Self = Self(NonZeroU32::MIN);

  pub const fn new(number: NonZeroU32) -> Self {
    Self(number)
  }

  pub const fn get(self) -> u32 {
    self.0.get()
  }

  #[must_use]
  pub fn next(self) -> Self {
    Self(self.0.checked_add(1).expect("a step never proposes 2^32 plans"))
  }
}

impl fmt::Display for PlanVersion {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(f)
  }
}

/// What a reviewer asks to change in a plan; never blank.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ReviewNote(String);

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ReviewNoteError {
  #[error("a request for changes says what should change")]
  Blank,
}

impl ReviewNote {
  /// The note, without the whitespace around it.
  pub fn new(text: &str) -> Result<Self, ReviewNoteError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(ReviewNoteError::Blank);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for ReviewNote {
  type Error = ReviewNoteError;

  fn try_from(text: String) -> Result<Self, Self::Error> {
    Self::new(&text)
  }
}

impl From<ReviewNote> for String {
  fn from(note: ReviewNote) -> Self {
    note.0
  }
}

/// A person's decision on a plan.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PlanDecision {
  /// The step passes and the steps after it may start.
  Approved,
  /// The step returns to its agent with `note`; its revised result needs a new decision.
  ChangesRequested { note: ReviewNote },
}

/// A decision on a plan and when it was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanReview {
  pub decision: PlanDecision,
  pub at: Timestamp,
}

/// One result of a gated step, as its agent left it, and the decision on it once there is one.
/// Created by [`super::Run::propose_plan`] and decided by [`super::Run::decide_plan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
  pub run: RunId,
  pub execution: StepExecutionId,
  pub step: StepKey,
  pub version: PlanVersion,
  /// The agent's result: the summary of its structured result, else its final message.
  pub text: String,
  pub created_at: Timestamp,
  /// `None` while the plan waits for a decision.
  pub review: Option<PlanReview>,
}

impl Plan {
  pub fn decision(&self) -> Option<&PlanDecision> {
    self.review.as_ref().map(|review| &review.decision)
  }

  /// The latest plan `execution` proposed, among `plans`.
  pub fn latest_of(plans: &[Self], execution: StepExecutionId) -> Option<&Self> {
    plans
      .iter()
      .filter(|plan| plan.execution == execution)
      .max_by_key(|plan| plan.version)
  }

  /// What `execution` left for the steps after it, as they read it: for a gated step its latest
  /// plan (the approved one once it passed), else its output among `outputs`; `None` when it left
  /// nothing.
  pub fn output_of<'a>(execution: StepExecutionId, outputs: &'a [StepOutput], plans: &'a [Self]) -> Option<&'a str> {
    Self::latest_of(plans, execution)
      .map(|plan| plan.text.as_str())
      .or_else(|| {
        outputs
          .iter()
          .find(|output| output.execution == execution)
          .map(|output| output.text.as_str())
      })
  }

  /// The version after the plans of `step` among `plans`: v1 for the first.
  pub(super) fn next_version(plans: &[Self], run: RunId, step: &StepKey) -> PlanVersion {
    plans
      .iter()
      .filter(|plan| plan.run == run && plan.step == *step)
      .map(|plan| plan.version)
      .max()
      .map_or(PlanVersion::FIRST, PlanVersion::next)
  }

  /// The plan a reviewer sent back to `execution`'s agent, when its next invocation is the first
  /// since that decision: its latest plan has changes requested after `last_started`, when its
  /// latest invocation started. That invocation is told what should change; later ones continue
  /// the revision like any interrupted work.
  pub fn awaiting_revision(
    plans: &[Self],
    execution: StepExecutionId,
    last_started: Option<Timestamp>,
  ) -> Option<(&Self, &ReviewNote)> {
    let plan = Self::latest_of(plans, execution)?;
    let review = plan.review.as_ref()?;
    let PlanDecision::ChangesRequested { note } = &review.decision else {
      return None;
    };
    last_started
      .is_none_or(|started| started <= review.at)
      .then_some((plan, note))
  }
}
