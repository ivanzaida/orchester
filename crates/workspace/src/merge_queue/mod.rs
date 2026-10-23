//! The project's merge queue (`docs/design/orchester-workspace.md` › *Delivery*, ORC-19): the runs of pipelines that
//! end in a Deliver step join it once every step passed ([`join`]), and it delivers them into their base branch one
//! at a time, first finished first ([`advance`]): the current base is merged into the run's branch in its worktree,
//! Check runs again on the merged result, and the run merges (a merge commit or squashed), its worktree, code index and
//! branch are cleared and its task is done. Conflicts go back to the run's Integrate step, the agent step whose profile
//! the Deliver step names; a run whose base moved while it was checked syncs and is checked again, at most
//! [`orchester_core::merge_queue::MAX_RESYNCS`] times in a row, and is never merged on a check of an older base. A
//! checkout of the base with uncommitted changes blocks delivery, which goes on by itself once it is clean. A run that
//! waits for a person's approval does not hold the queue.
//!
//! Every rule is here; the screens and automation read the queue ([`read`]) and change it through these functions
//! only. The store keeps the entries (`orchester_core::merge_queue`). Blocks on SQLite, git and the check's programs:
//! callers with a UI run it off their thread.

mod approve;
mod check;
mod checks;
mod drive;
mod integrate;
mod join;
mod land;
mod order;
mod read;
mod wake;

pub use self::{
  approve::{approve, reject, requeue, resolve_myself, resolved},
  check::rechecks,
  checks::{DeliveryCheck, checks},
  drive::{Advance, advance, integrate_sent},
  join::join,
  order::{Move, move_run, remove},
  read::{
    BaseState, CheckoutState, PhaseState, PipelineDelivery, QueueRead, QueueRow, RowPhase, RunDelivery, TaskDelivery,
    delivered_by_queue, delivery_of, in_queue, read, task_delivery,
  },
  wake::{changes, wake},
};
use crate::messages::Message;

/// Why a person's change to the queue did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueRefusal {
  /// The run is not in the queue, or no longer in the state the change needs (it moved on meanwhile).
  NotThere,
  /// The run is first among the waiting runs already.
  First,
  /// Only a person makes this change; automation asked.
  PersonOnly,
  /// The project's data could not be read or written.
  Store(Message),
  /// What git or the run's worktree said.
  Resolve(Message),
}

impl QueueRefusal {
  /// The code automation reads.
  pub const fn code(&self) -> &'static str {
    match self {
      Self::NotThere => "not_in_queue",
      Self::First => "already_first",
      Self::PersonOnly => "person_only",
      Self::Store(_) => "store",
      Self::Resolve(_) => "not_resolved",
    }
  }

  /// What a person reads.
  pub fn message(&self) -> Message {
    match self {
      Self::NotThere => Message::new("mergeQueue.refused.notThere"),
      Self::First => Message::new("mergeQueue.refused.first"),
      Self::PersonOnly => Message::new("mergeQueue.refused.personOnly"),
      Self::Store(message) | Self::Resolve(message) => message.clone(),
    }
  }
}

impl From<orchester_store::StoreError> for QueueRefusal {
  fn from(error: orchester_store::StoreError) -> Self {
    tracing::warn!(%error, "cannot read or change the merge queue");
    Self::Store(crate::messages::store_error(&error))
  }
}
