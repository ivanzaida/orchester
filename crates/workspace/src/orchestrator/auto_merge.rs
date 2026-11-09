//! The chat's rule for merging a finished run on its own (ORC-69, `docs/design/orchestrator-toolset.md`, decision 5).
//! While the project lets the Orchestrator drive, the turn a task's run success starts by itself merges the run with
//! `merge_run`, only when all of these hold ([`merging_in`]): it is the task's latest run and succeeded; every step of
//! its pipeline passed, a step accepted as it was counting only when a person accepted it; the task does not say
//! *Don't merge automatically*, is not marked Blocked and no unfinished task blocks it; and new starts are not held for
//! its reviewed group or the whole project. `merge_run`'s own rules (the main checkout needs no local integration) hold
//! on top. In a turn nobody asked for, `merge_run` merges only a run this rule lets it ([`check`]). A run that
//! committed nothing and whose result is its report is completed by the same rule, with no turn (ORC-93,
//! [`completes_in`]).

use orchester_core::{
  id::RunId,
  orchestrator::{MergeLeft, Merging},
  resource::WithoutCommits,
  run::{Run, RunStatus, attempt_starts, run_delivers},
  store::RunStore,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::model::JsonObject;
use serde_json::Value;

use super::tools::ChatScope;
use crate::{
  app_mcp::{ToolError, blocking, find_run},
  messages::Message,
  tasks::{
    detail::run_store_error,
    hold,
    recorded::{self, RunRoute, route_of_run},
  },
};

/// Whether the turn of run `run`'s success merges the run on its own, or why it is left to the person. Blocks on
/// SQLite.
pub fn merging_in(db: &ProjectDb, run: RunId) -> Result<Merging, StoreError> {
  Ok(match left(db, run, Merge::Branch)? {
    Some(why) => Merging::Left { why },
    None => Merging::Merges,
  })
}

/// Whether the Orchestrator completes run `run` on its own, as it would merge it (ORC-93): `run` committed nothing (the
/// caller asked git), its result is its report or work done outside the repository, it was not completed yet, and the
/// chat's rule would merge it. The merge queue, which never delivers a run with nothing to merge, does not hold it, and
/// a run whose pipeline expects changes is always left to the person. Blocks on SQLite.
pub fn completes_in(db: &ProjectDb, run: RunId) -> Result<bool, StoreError> {
  let Some(found) = db.run(run).map_err(run_store_error)? else {
    return Ok(false);
  };
  // A team task's run (told by the run itself) goes into its module's branch and is delivered with the module, never
  // completed from here.
  if route_of_run(db, &found)? != RunRoute::Local {
    return Ok(false);
  }
  let Some(task) = recorded::task_of(db, found.task())? else {
    return Ok(false);
  };
  if run_delivers(&task, &found).without_commits() != WithoutCommits::Completes || db.run_completion(run)?.is_some() {
    return Ok(false);
  }
  Ok(left(db, run, Merge::Completion)?.is_none())
}

/// What the rule is asked about: a merge of the run's branch, which the merge queue may hold, or a completion.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Merge {
  Branch,
  Completion,
}

/// Why `run` is left to the person; `None` when the chat merges (or completes) it on its own.
fn left(db: &ProjectDb, run: RunId, merge: Merge) -> Result<Option<MergeLeft>, StoreError> {
  let Some(run) = db.run(run).map_err(run_store_error)? else {
    return Ok(Some(MergeLeft::NotReady));
  };
  // A team task's run is delivered with its module (`merge_run` requests that), which is not the chat's to do on its
  // own: left to the person.
  if route_of_run(db, &run)? != RunRoute::Local {
    return Ok(Some(MergeLeft::TeamDelivery));
  }
  let Some(task) = recorded::task_of(db, run.task())? else {
    return Ok(Some(MergeLeft::NotReady));
  };
  if !latest_succeeded(db, &run)? {
    return Ok(Some(MergeLeft::NotReady));
  }
  if merge == Merge::Branch && crate::merge_queue::delivered_by_queue(db, &run)? {
    return Ok(Some(MergeLeft::DeliveredByQueue));
  }
  let organization = task.organization();
  if organization.no_automatic_merge() {
    return Ok(Some(MergeLeft::OptedOut));
  }
  let starts = attempt_starts(&db.events(run.id()).map_err(run_store_error)?);
  if let Some(steps) = MergeLeft::of_steps(&run, &starts) {
    return Ok(Some(steps));
  }
  if let Some(reason) = organization.blocked_reason() {
    return Ok(Some(MergeLeft::MarkedBlocked {
      reason: reason.to_owned(),
    }));
  }
  let mut blockers = Vec::new();
  for blocker in db.unfinished_blocking_tasks(task.id())? {
    if let Some(blocker) = recorded::task_of(db, blocker)? {
      blockers.push(blocker.key().to_string());
    }
  }
  if !blockers.is_empty() {
    return Ok(Some(MergeLeft::BlockedByTasks { tasks: blockers }));
  }
  let holds = db.queue_holds()?;
  let group_held = hold::group_of(db, task.id())?.is_some_and(|group| holds.groups.contains(&group));
  if holds.project || group_held {
    return Ok(Some(MergeLeft::Held));
  }
  Ok(None)
}

/// Whether `run` succeeded and no later run of its task exists, read or not.
fn latest_succeeded(db: &ProjectDb, run: &Run) -> Result<bool, StoreError> {
  if *run.status() != RunStatus::Succeeded {
    return Ok(false);
  }
  let number = run.number().get();
  let later = db
    .runs_of_task(run.task())
    .map_err(run_store_error)?
    .iter()
    // A later run that cannot be read is still a later run.
    .any(|listed| match listed {
      Ok(listed) => listed.number().get() > number,
      Err(unreadable) => unreadable.number.is_some_and(|other| other.get() > number),
    });
  Ok(!later)
}

/// Checks `merge_run` with `arguments` in `scope`'s turn, which nobody asked for, against the rule: a run the rule
/// leaves to the person now is refused (`left_to_person`), with why.
pub(super) async fn check(scope: &ChatScope, arguments: &JsonObject) -> Result<(), ToolError> {
  let named = arguments
    .get("run")
    .and_then(Value::as_str)
    .ok_or_else(|| ToolError::InvalidArguments("run: required".to_owned()))?
    .to_owned();
  let root = scope.root.clone();
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    let run = find_run(&db, &named)?;
    match left(&db, run.id(), Merge::Branch)? {
      None => Ok(()),
      Some(why) => Err(ToolError::refused(
        "left_to_person",
        vec![left_message(&why, run.number().get())],
      )),
    }
  })
  .await
}

/// Why run `#number` is left to the person, in the words the chat's agent and the person read.
pub fn left_message(why: &MergeLeft, number: u32) -> Message {
  let run = number.to_string();
  match why {
    MergeLeft::NotReady => Message::new("orchestrator.tools.mergeLeft.notReady").with("run", run),
    MergeLeft::StepNotPassed { step } => Message::new("orchestrator.tools.mergeLeft.stepNotPassed")
      .with("run", run)
      .with("step", step.clone()),
    MergeLeft::AcceptedByAutomation { step } => Message::new("orchestrator.tools.mergeLeft.acceptedByAutomation")
      .with("run", run)
      .with("step", step.clone()),
    MergeLeft::MarkedBlocked { reason } => Message::new("orchestrator.tools.mergeLeft.markedBlocked")
      .with("run", run)
      .with("reason", reason.clone()),
    MergeLeft::BlockedByTasks { tasks } => Message::new("orchestrator.tools.mergeLeft.blockedByTasks")
      .with("run", run)
      .with("tasks", tasks.join(", ")),
    MergeLeft::Held => Message::new("orchestrator.tools.mergeLeft.held").with("run", run),
    MergeLeft::OptedOut => Message::new("orchestrator.tools.mergeLeft.optedOut").with("run", run),
    MergeLeft::DeliveredByQueue => Message::new("orchestrator.tools.mergeLeft.deliveredByQueue").with("run", run),
    MergeLeft::TeamDelivery => Message::new("orchestrator.tools.mergeLeft.teamDelivery").with("run", run),
  }
}
