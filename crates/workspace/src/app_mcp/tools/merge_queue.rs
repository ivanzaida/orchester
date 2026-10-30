//! How the run reads (`get_run`, `list_runs`, `get_daily_report`) tell a run's place in its project's merge queue
//! (ORC-19, `crate::merge_queue`): the lane it delivers in, named by its base branch (ORC-406), queued with its place
//! in that lane, syncing, re-checking, integrating its conflicts, waiting for approval, blocked and why, failed or out
//! of the queue and why, merged, or taken out by a person.

use std::collections::HashMap;

use orchester_core::{
  id::RunId,
  merge_queue::{Blocked, Left, QueueEntry, QueueState, lanes},
  store::TaskStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::{Value, json};

use super::wire;

/// `entry`, at `place` among the runs of its lane in the queue while it is in it, as the reads give it.
pub(super) fn entry_json(entry: &QueueEntry, place: Option<usize>) -> Value {
  let mut answer = json!({
    "lane": entry.base.as_ref().map(|base| base.as_str()),
    "state": state_name(&entry.state),
    "since": wire::time(entry.since),
    "place": place,
    "resyncs": entry.resyncs,
    "approved_at": entry.approved_at.map(wire::time),
    "checked_on": entry.checked.as_ref().map(|checked| checked.base.to_string()),
  });
  let fields = answer.as_object_mut().expect("an object");
  match &entry.state {
    QueueState::Integrating { step, files, .. } => {
      fields.insert("integrate_step".to_owned(), json!(step.as_str()));
      fields.insert("files".to_owned(), json!(files));
    }
    QueueState::Blocked(blocked) => {
      fields.insert("blocked".to_owned(), blocked_json(blocked));
    }
    QueueState::Left(left) => {
      fields.insert("reason".to_owned(), left_json(left));
    }
    QueueState::Merged(delivered) => {
      fields.insert(
        "merged".to_owned(),
        json!({
          "revision": delivered.revision.to_string(),
          "kind": delivered.kind,
          "worktree_removed": delivered.worktree_removed,
          "branch_deleted": delivered.branch_deleted,
        }),
      );
    }
    _ => {}
  }
  answer
}

/// `run`'s place in the merge queue of the project in `db`; `null` for a run that never joined it.
pub(super) fn run_delivery(db: &ProjectDb, run: RunId) -> Result<Value, StoreError> {
  Ok(
    crate::merge_queue::delivery_of(db, run)?
      .map_or(Value::Null, |delivery| entry_json(&delivery.entry, delivery.place)),
  )
}

/// The place in the queue of each run of the project in `db` that ever joined it, by run.
pub(super) fn deliveries(db: &ProjectDb) -> Result<HashMap<RunId, Value>, StoreError> {
  let entries = db.merge_queue()?;
  let places = places(&entries);
  Ok(
    entries
      .iter()
      .map(|entry| (entry.run, entry_json(entry, places.get(&entry.run).copied())))
      .collect(),
  )
}

/// The project's queue for its report of a day starting at `day_start`: the runs in it now, and those merged, out of it
/// or taken out on that day or after, with their tasks.
pub(super) fn queue_of_day(db: &ProjectDb, day_start: Timestamp) -> Result<Value, StoreError> {
  let entries = db.merge_queue()?;
  let places = places(&entries);
  let mut listed = Vec::new();
  for entry in entries
    .iter()
    .filter(|entry| entry.state.in_queue() || entry.since >= day_start)
  {
    let task = db.task(entry.task)?.map(|task| task.key().to_string());
    let mut shown = entry_json(entry, places.get(&entry.run).copied());
    shown["run"] = json!(entry.run.to_string());
    shown["task"] = json!(task);
    listed.push(shown);
  }
  Ok(json!(listed))
}

/// Each run's place among the runs of its lane in the queue, from 1: the one being delivered first.
fn places(entries: &[QueueEntry]) -> HashMap<RunId, usize> {
  lanes(entries)
    .iter()
    .flat_map(|lane| {
      lane
        .entries
        .iter()
        .filter_map(|entry| Some((entry.run, lane.place(entry.run)?)))
        .collect::<Vec<_>>()
    })
    .collect()
}

const fn state_name(state: &QueueState) -> &'static str {
  match state {
    QueueState::Queued => "queued",
    QueueState::Syncing => "syncing",
    QueueState::Rechecking { .. } => "rechecking",
    QueueState::Integrating { .. } => "integrating",
    QueueState::WaitingForApproval => "waiting_for_approval",
    QueueState::Blocked(_) => "blocked",
    QueueState::Left(left) if left.failed() => "failed",
    QueueState::Left(_) => "needs_you",
    QueueState::Merged(_) => "merged",
    QueueState::Removed => "removed",
  }
}

fn blocked_json(blocked: &Blocked) -> Value {
  match blocked {
    Blocked::Uncommitted { files } => json!({ "why": "uncommitted_changes", "files": files }),
    Blocked::OtherBranch { current } => json!({ "why": "other_branch_checked_out", "branch": current }),
    Blocked::InProgress => json!({ "why": "operation_in_progress" }),
    Blocked::HeldLocally => json!({ "why": "local_integration" }),
  }
}

fn left_json(left: &Left) -> Value {
  match left {
    Left::Conflicts { files } => json!({ "why": "conflicts", "files": files }),
    Left::CheckFailed { tool, detail } => json!({ "why": "check_failed", "tool": tool, "detail": detail }),
    Left::KeepsMoving { base, heads } => json!({
      "why": "base_keeps_moving",
      "base": base,
      "heads": heads.iter().map(ToString::to_string).collect::<Vec<_>>(),
    }),
    Left::Stopped { detail } => json!({ "why": "stopped", "detail": detail }),
    Left::LaterRun => json!({ "why": "later_run" }),
  }
}
