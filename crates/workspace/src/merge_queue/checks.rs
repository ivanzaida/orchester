//! What a run's delivery adds to its verification on the task page (`Delivery check row` `QyqsJ`): the sync with the
//! base, the Integrate step's work on the conflicts, Check on the merged result and the person's approval, each as the
//! queue's record of the run says it stands.

use orchester_core::{
  merge_queue::{QueueEvent, QueueState},
  revision::Revision,
  time::Timestamp,
};

use super::RunDelivery;

/// One line of a delivery's verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryCheck {
  /// The base at `base` was merged into the run's branch at `at`; `conflicts` files conflicted, and the run synced
  /// `resyncs` times more because the base moved during its check.
  Synced {
    base: Revision,
    conflicts: usize,
    resyncs: usize,
    at: Timestamp,
  },
  /// The Integrate step works on `files` conflicting files, since `since`.
  Integrating { files: usize, since: Timestamp },
  /// The Integrate step resolved `files` files; the run passed every step again.
  Integrated { files: usize },
  /// Check runs on the merged result with the base at `base`, since `since`.
  Checking { base: Revision, since: Timestamp },
  /// Check on the merged result is still to run.
  CheckWaits,
  /// Check passed on the merged result with the base at `base`: `tool` ran.
  Checked {
    base: Revision,
    tool: String,
    at: Timestamp,
  },
  /// Check failed on the merged result with the base at `base`: `tool`.
  CheckFailed {
    base: Revision,
    tool: String,
    at: Timestamp,
  },
  /// A person approved the merge at `at`.
  Approved { at: Timestamp },
}

/// The verification lines of `delivery`, in the order they happen.
pub fn checks(delivery: &RunDelivery) -> Vec<DeliveryCheck> {
  let entry = &delivery.entry;
  let mut checks = Vec::new();
  let mut resyncs = 0;
  let mut synced = None;
  let mut integrated = None;
  let mut checked = None;
  for recorded in &delivery.events {
    match &recorded.event {
      QueueEvent::Synced { base, files } => {
        synced = Some((base.clone(), files.len(), recorded.at));
      }
      QueueEvent::CheckSuperseded { .. } => resyncs += 1,
      QueueEvent::IntegrateStarted { files, .. } => integrated = Some(files.len()),
      QueueEvent::CheckPassed { base, tool } => {
        checked = Some(DeliveryCheck::Checked {
          base: base.clone(),
          tool: tool.clone(),
          at: recorded.at,
        });
      }
      QueueEvent::CheckFailed { base, tool } => {
        checked = Some(DeliveryCheck::CheckFailed {
          base: base.clone(),
          tool: tool.clone(),
          at: recorded.at,
        });
      }
      _ => {}
    }
  }
  if let Some((base, conflicts, at)) = synced {
    checks.push(DeliveryCheck::Synced {
      base,
      conflicts,
      resyncs,
      at,
    });
  }
  match (&entry.state, integrated) {
    (QueueState::Integrating { files, .. }, _) => checks.push(DeliveryCheck::Integrating {
      files: files.len(),
      since: entry.since,
    }),
    (_, Some(files)) => checks.push(DeliveryCheck::Integrated { files }),
    (_, None) => {}
  }
  match &entry.state {
    QueueState::Rechecking { base, .. } => checks.push(DeliveryCheck::Checking {
      base: base.clone(),
      since: entry.since,
    }),
    QueueState::Queued | QueueState::Syncing | QueueState::Integrating { .. } => checks.push(DeliveryCheck::CheckWaits),
    _ => checks.extend(checked),
  }
  checks.extend(entry.approved_at.map(|at| DeliveryCheck::Approved { at }));
  checks
}
