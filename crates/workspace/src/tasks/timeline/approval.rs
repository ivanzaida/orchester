//! Tool requests in the timeline (ORC-41): a request that waits for the person, or that a person, automation or the
//! reviewer answered as it waited, is a card; one the run's approval mode answered itself as it came (Autopilot, the
//! profile's permissions, the reviewer) is a quiet row that says how ([`ApprovalCard::quiet`],
//! `docs/design/approval-modes.md`). A request a rule of the run answered as it came is neither: its call's entry in
//! the tool group carries the mark.

use std::sync::Arc;

use orchester_core::{
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  run::{AnswerKind, DecidedBy, DenialNote, PendingToolApproval, RequestAnswer, ToolApprovalDecision, review::Review},
  time::Timestamp,
};

use super::{Item, ItemKind, Sources};
use crate::tasks::transcript::{Entry, EntryKind};

/// A tool request a person decides (ORC-41): pending, decided or interrupted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalCard {
  pub request: PendingToolApproval,
  /// The step whose agent asked.
  pub step: Arc<str>,
  /// `None` while it waits; `Some(None)` when it was interrupted.
  pub decided: Option<Option<ToolApprovalDecision>>,
  /// Who answered it, once it is answered and that was recorded.
  pub decided_by: Option<DecidedBy>,
  /// When it was answered or ended, once it was.
  pub decided_at: Option<Timestamp>,
  /// What auto mode did with it, once it is answered; while it waits, the shown request says.
  pub review: Option<Review>,
  /// While it waits: auto mode's reviewer is judging it, so it waits for the reviewer, not for the person (who may
  /// still answer it).
  pub reviewing: bool,
  /// The person approved it for every run of the project ([`orchester_core::run::DecidedToolApproval::for_project`]).
  pub for_project: bool,
  /// The words its denial told the agent, when it was denied with a message.
  pub note: Option<DenialNote>,
  /// How the run's approval mode answered it, once answered and recorded so.
  pub answer: Option<RequestAnswer>,
}

impl ApprovalCard {
  /// How the run's approval mode approved it without the person — Autopilot, the profile or the reviewer — when it
  /// did: its quiet row says so instead of a card.
  pub fn quiet(&self) -> Option<&RequestAnswer> {
    let answer = self.answer.as_ref()?;
    let quiet = matches!(
      answer.kind,
      AnswerKind::Autopilot | AnswerKind::Profile | AnswerKind::Reviewer
    );
    (quiet && self.decided == Some(Some(ToolApprovalDecision::Approved))).then_some(answer)
  }

  /// Whether Autopilot approved it: its quiet rows group per step.
  pub fn by_autopilot(&self) -> bool {
    self.quiet().is_some_and(|answer| answer.kind == AnswerKind::Autopilot)
  }
}

/// Whether `item` is a request's quiet row ([`ApprovalCard::quiet`]).
pub fn is_quiet(item: &Item) -> bool {
  matches!(&item.kind, ItemKind::Approval(card) if card.quiet().is_some())
}

/// The requests of `execution`, which ran step `step`: those that wait, then those a person, automation or the
/// reviewer answered and those the run's mode approved itself.
pub(super) fn approval_items(sources: &Sources<'_>, execution: StepExecutionId, step: &Arc<str>) -> Vec<Item> {
  let answer_of = |id: &str| {
    sources
      .answers
      .iter()
      .find(|record| record.request == id)
      .and_then(|record| record.answer.clone())
  };
  let pending = sources
    .pending
    .iter()
    .filter(|request| request.execution == execution)
    .map(|request| ApprovalCard {
      request: request.clone(),
      step: step.clone(),
      decided: None,
      decided_by: None,
      decided_at: None,
      review: None,
      reviewing: sources.reviewing.contains(&request.id),
      for_project: false,
      note: None,
      answer: None,
    });
  let decided = sources
    .decided
    .iter()
    .filter(|decided| decided.request.execution == execution)
    .map(|decided| ApprovalCard {
      request: decided.request.clone(),
      step: step.clone(),
      decided: Some(decided.decision),
      decided_by: decided.decided_by,
      decided_at: Some(decided.decided_at),
      review: decided.review.clone(),
      reviewing: false,
      for_project: decided.for_project,
      note: decided.note.clone(),
      answer: answer_of(&decided.request.id),
    })
    .filter(|card| card.quiet().is_some() || card.decided_by.is_none_or(DecidedBy::answered_waiting));
  pending
    .chain(decided)
    .map(|card| Item {
      key: approval_key(&card.request.id),
      execution: Some(execution),
      at: card.request.requested_at,
      kind: ItemKind::Approval(card),
    })
    .collect()
}

/// The key of the card of tool request `id`.
pub fn approval_key(id: &str) -> Arc<str> {
  format!("approval-{id}").into()
}

/// The key of the card of the tool request problem `entry` names: one that ended without a decision.
pub fn named_request(entry: &Entry) -> Option<Arc<str>> {
  match &entry.kind {
    EntryKind::Problem(RunnerProblem::DecisionInterrupted { request, .. }) => Some(approval_key(request)),
    _ => None,
  }
}
