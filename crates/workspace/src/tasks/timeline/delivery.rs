//! What the merge queue did with a run (ORC-19, `crate::merge_queue`), as its conversation shows it (`Urfxj`): each
//! event of its delivery in the order it happened (joining the queue, syncing with the base, its conflicts going to
//! the Integrate step, Check on the merged result, a check the base superseded, the person's approval, a block, the
//! merge and what was cleared, leaving the queue), and, while it waits for the person's approval, that it does; and
//! what its pull request's CI did ([`ci_lines`]). The lines sit among the chat's rows by time ([`places`]): before the
//! first step that started after them, and always before the run's end, which closes the chat.

use std::sync::Arc;

use orchester_core::{
  merge_queue::{QueueEvent, QueueState},
  pull_request::ci::fix::{FixEvent, RunFixes},
  time::Timestamp,
};

use super::{ItemKind, view::Row};
use crate::{merge_queue::RunDelivery, pull_requests::ci::PullRequestCi};

/// A line of the run's delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryLine {
  /// Stable while the queue goes on.
  pub key: Arc<str>,
  pub at: Timestamp,
  pub kind: DeliveryKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryKind {
  /// What happened to the run in the queue.
  Event(QueueEvent),
  /// Check passed on the merged result and the run waits for the person to review it and merge.
  WaitsForApproval,
  /// What the run's pull request's CI did ([`CiLine`]).
  Ci(CiLine),
}

/// What a run's pull request's CI did, as its conversation shows it (ORC-335): a Fix CI sent or stopped at, as
/// recorded; and, at the end, how the checks stand now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CiLine {
  Fix(FixEvent),
  /// The checks of pull request `number` as the last read gave them.
  Now {
    number: u64,
    ci: PullRequestCi,
  },
}

/// The lines of `delivery`, oldest first.
pub fn lines(delivery: &RunDelivery) -> Vec<DeliveryLine> {
  let mut lines: Vec<DeliveryLine> = delivery
    .events
    .iter()
    .enumerate()
    .map(|(index, recorded)| DeliveryLine {
      key: format!("queue-{index}").into(),
      at: recorded.at,
      kind: DeliveryKind::Event(recorded.event.clone()),
    })
    .collect();
  if delivery.entry.state == QueueState::WaitingForApproval {
    lines.push(DeliveryLine {
      key: "queue-waits-for-approval".into(),
      at: delivery.entry.since,
      kind: DeliveryKind::WaitsForApproval,
    });
  }
  lines
}

/// The CI lines of `fixes`, a run's Fix CI, oldest first, then how the checks of pull request `number` stand now
/// (`ci`, read at `now`).
pub fn ci_lines(fixes: &RunFixes, (number, ci): (u64, &PullRequestCi), now: Timestamp) -> Vec<DeliveryLine> {
  let mut lines: Vec<DeliveryLine> = fixes
    .events
    .iter()
    .enumerate()
    .map(|(index, event)| DeliveryLine {
      key: format!("ci-fix-{index}").into(),
      at: event.at,
      kind: DeliveryKind::Ci(CiLine::Fix(event.clone())),
    })
    .collect();
  lines.push(DeliveryLine {
    key: format!("ci-now-{number}").into(),
    at: now,
    kind: DeliveryKind::Ci(CiLine::Now { number, ci: ci.clone() }),
  });
  lines
}

/// Where each of `lines` goes among `rows`: the index of the row it goes before (`rows.len()`: after them all), in the
/// order of `lines`. A line goes before the first step, fold of steps or return that started after it, else before
/// the run's end.
pub fn places<R: AsRef<Row>>(rows: &[R], lines: &[DeliveryLine]) -> Vec<usize> {
  lines
    .iter()
    .map(|line| {
      rows
        .iter()
        .position(|row| match row.as_ref() {
          Row::Step { head, .. } => head.started.is_some_and(|started| started > line.at),
          Row::Folded { steps, .. } => steps
            .first()
            .and_then(|(_, head)| head.started)
            .is_some_and(|started| started > line.at),
          Row::Item(item) | Row::Line(item) => match item.kind {
            ItemKind::End(_) => true,
            ItemKind::Mark(_) => item.at > line.at,
            _ => false,
          },
          Row::NotStarted { .. } | Row::Agent(_) | Row::Approvals { .. } | Row::Entry { .. } => false,
        })
        .unwrap_or(rows.len())
    })
    .collect()
}
