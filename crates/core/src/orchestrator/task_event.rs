//! Something that happened to a task the Orchestrator chat follows, besides an agent's wait and a run's end (the
//! module design's events, *Parts · Lines* e): a run that made no progress for a while, a merge that hit conflicts,
//! and the task a run waits on merged or closed without merging. Each is kept as a wait of the run
//! ([`super::WaitKind::Event`]), so it goes to the conversation that last dealt with its task and starts a turn by
//! itself as a wait does, unless the Orchestrator's own doing raised it ([`TaskEventKind::raised_by`]).

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use super::{
  change::Change,
  chat::{ChatAction, ChatEntry, EntryKind},
};
use crate::{
  id::{RunId, TaskId},
  time::Timestamp,
};

/// One event of a task's run, at the time it happened (`at`): the run and the task are the wait's that keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
  pub kind: TaskEventKind,
  pub at: Timestamp,
}

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TaskEventKind {
  /// The run made no progress (no transcript event, step change or tool call) since `since`, for the `minutes` the
  /// setting allows ([`no_progress`]).
  NoProgress { since: Timestamp, minutes: u32 },
  /// Merging the run hit conflicts.
  MergeConflict,
  /// Task `task`, which the run waits on, was merged: its run `run` was.
  WaitedMerged { task: TaskId, run: RunId },
  /// Task `task`, which the run waits on, was closed without merging; `run` is its latest run, when it had one.
  WaitedClosed {
    task: TaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    run: Option<RunId>,
  },
}

impl TaskEventKind {
  /// Whether `action`, carried out (by the chat itself, or applied by a person), raised this event of run `run`: its
  /// own message, answer, continuation or resume of a run that then stalls; its own merge of the run that hit
  /// conflicts; its own merge of the waited task's run, or cancel of it. A run the chat started that stalls is news to
  /// the conversation that leads its task, so a start does not count.
  pub(super) fn raised_by(self, action: &ChatAction, run: RunId) -> bool {
    let change = action.change.as_ref().map(|change| &change.change);
    match self {
      Self::NoProgress { .. } => {
        action.resumed == Some(run) || change.is_some_and(|change| change.run() == Some(run) && change.carries_run_on())
      }
      Self::MergeConflict => matches!(change, Some(Change::Merge { run: merged, .. }) if *merged == run),
      Self::WaitedMerged { run: waited, .. } => {
        matches!(change, Some(Change::Merge { run: merged, .. }) if *merged == waited)
      }
      Self::WaitedClosed { run: waited, .. } => {
        waited.is_some_and(|waited| matches!(change, Some(Change::Cancel { run: canceled, .. }) if *canceled == waited))
      }
    }
  }
}

/// How long a run may make no progress before that is an event: never (`Off`), or after `minutes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoProgressAfter {
  Off,
  Minutes { minutes: NonZeroU32 },
}

impl NoProgressAfter {
  /// The minutes a run may make no progress for when the person set nothing.
  pub const DEFAULT_MINUTES: NonZeroU32 = NonZeroU32::new(20).expect("20 is not zero");

  /// After `minutes`; 0 turns it off.
  pub const fn minutes(minutes: u32) -> Self {
    match NonZeroU32::new(minutes) {
      Some(minutes) => Self::Minutes { minutes },
      None => Self::Off,
    }
  }
}

impl Default for NoProgressAfter {
  fn default() -> Self {
    Self::Minutes {
      minutes: Self::DEFAULT_MINUTES,
    }
  }
}

/// The no-progress event of a run whose last progress was at `last`, at `now`, by the setting `after`: one once the
/// run made none for the setting's minutes, unless one was already raised for this stall (`raised`: the `since` of
/// the run's newest no-progress event, [`raised_stall`]). Progress again moves `last` past it, which re-arms the rule.
pub fn no_progress(
  last: Timestamp,
  now: Timestamp,
  after: NoProgressAfter,
  raised: Option<Timestamp>,
) -> Option<TaskEventKind> {
  let NoProgressAfter::Minutes { minutes } = after else {
    return None;
  };
  let waited = now.unix_millis().saturating_sub(last.unix_millis());
  let stalled = waited >= i64::from(minutes.get()) * 60_000;
  let already = raised.is_some_and(|since| since >= last);
  (stalled && !already).then_some(TaskEventKind::NoProgress {
    since: last,
    minutes: minutes.get(),
  })
}

/// The `since` of the newest no-progress event of run `run` kept among `entries` (those of every conversation, in the
/// order they were kept); `None` while none was.
pub fn raised_stall(entries: &[ChatEntry], run: RunId) -> Option<Timestamp> {
  entries.iter().rev().find_map(|entry| match &entry.kind {
    EntryKind::Wait { wait, .. } if wait.run == run => match wait.kind.event()?.kind {
      TaskEventKind::NoProgress { since, .. } => Some(since),
      _ => None,
    },
    _ => None,
  })
}
