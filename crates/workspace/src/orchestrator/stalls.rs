//! The Orchestrator chat follows the runs that stall (ORC-767) as it follows its agents' waits ([`super::waits`]):
//! when a run an agent works on made no progress (no run event such as a step change, no transcript event such as a
//! tool call, no agent launch, no decision on a tool request or answer to a question) for the project's *A run without
//! progress for* minutes, a conversation keeps one *no progress* event for it ([`TaskEventKind::NoProgress`]), routed
//! and starting its turn as a wait does. The event is kept by the run and the time of its last progress, so it is
//! raised once per stall and progress again re-arms it; it stands until then, and reads as settled after
//! ([`settled`]). A run that waits for a person (a step waiting for a message or a plan decision, a tool request or a
//! question waiting for them, ORC-894), is queued (pending, or waiting for an agent slot or a workspace), paused, asked
//! to pause or held on another task never stalls: its clock starts again once it no longer waits. With the setting off
//! nothing is raised. No timer of its own: the follower reads the stalls each time it follows, and the app follows
//! again by [`next_stall_in`].

use std::{collections::HashSet, path::Path};

use orchester_core::{
  id::{ProjectId, RunId},
  orchestrator::{AgentWait, TaskEvent, TaskEventKind, WaitKind, change::StepName, task_event::no_progress},
  run::{Run, TaskPhase},
  store::{RunStore, WorkspaceStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::chat::StallSettled;
use crate::{
  links::{Link, TaskPage},
  messages::{self, Message},
  tasks::{detail::run_store_error, recorded::task_of},
};

const MINUTE_MILLIS: i64 = 60_000;

/// The no-progress events of the project's runs that stall at `now`, each by the key it is kept by.
pub(super) fn current_stalls(
  db: &ProjectDb,
  project: ProjectId,
  now: Timestamp,
) -> Result<Vec<(String, AgentWait)>, StoreError> {
  let Some(minutes) = db.orchestrator_watch()?.no_progress_minutes() else {
    return Ok(Vec::new());
  };
  let mut stalls = Vec::new();
  for (task, run, last) in working(db)? {
    // One event per stall: its key carries `last`, so the follower keeps it once and progress makes a new one.
    let Some(kind) = no_progress(last, now, Some(minutes), None) else {
      continue;
    };
    let at = Timestamp::from_unix_millis(last.unix_millis() + i64::from(minutes) * MINUTE_MILLIS);
    let key = format!("stall:{}:{}", run.id(), last.unix_millis());
    stalls.push((key, stalled_wait(project, (&task, &run), TaskEvent { kind, at })));
  }
  Ok(stalls)
}

/// When following the waits of the project at `root` again may raise a stall, after `now`: the soonest a run an agent
/// works on reaches the set minutes without progress, or, for one that stalls already, the soonest it could stall
/// again once it progressed; `None` while no agent works or the setting is off. Blocks on SQLite.
pub fn next_stall_in(root: &Path, now: Timestamp) -> Result<Option<Timestamp>, Message> {
  let next = || {
    let db = ProjectDb::open_read_only(root)?;
    let Some(minutes) = db.orchestrator_watch()?.no_progress_minutes() else {
      return Ok(None);
    };
    let after = i64::from(minutes) * MINUTE_MILLIS;
    Ok(
      working(&db)?
        .into_iter()
        .map(|(_, _, last)| {
          let due = last.unix_millis() + after;
          Timestamp::from_unix_millis(if due > now.unix_millis() {
            due
          } else {
            now.unix_millis() + after
          })
        })
        .min(),
    )
  };
  next().map_err(|error: StoreError| messages::store_error(&error))
}

/// The project's runs an agent works on now, each with its task and when it last progressed.
fn working(db: &ProjectDb) -> Result<Vec<(Task, Run, Timestamp)>, StoreError> {
  let held = held(db)?;
  let mut working = Vec::new();
  for listed in db.running_runs().map_err(run_store_error)? {
    // A run that cannot be read back is told on its task's page; it is no stall to follow.
    let Ok(run) = listed else {
      continue;
    };
    if held.contains(&run.id()) || run.pause_requested() || run.running_executions().is_empty() {
      continue;
    }
    let Some(task) = task_of(db, run.task())? else {
      continue;
    };
    if TaskPhase::of(&task, Some(&run), None) != TaskPhase::Running {
      continue;
    }
    if let Some(last) = db.run_progressed_at(run.id())? {
      working.push((task, run, last));
    }
  }
  Ok(working)
}

/// The runs held: waiting on another task or for a workspace, or whose agent waits for a decision on its tool request
/// (a person's, or auto mode's reviewer's) or for the answer to its question, while its step still runs.
fn held(db: &ProjectDb) -> Result<HashSet<RunId>, StoreError> {
  let mut held: HashSet<RunId> = db.active_run_waits()?.into_iter().map(|kept| kept.wait.run()).collect();
  held.extend(db.workspace_waits()?.into_iter().map(|(run, _)| run));
  held.extend(
    db.pending_tool_approvals_all()?
      .into_iter()
      .map(|approval| approval.run),
  );
  held.extend(db.waiting_questions()?.into_iter().map(|asked| asked.request.run()));
  Ok(held)
}

/// What became of `run` (`None`: the project no longer keeps it) since its stall without progress since `since`, once
/// that stall no longer stands: that it ended (or is gone), when it progressed again, or that it waits now without
/// progress since (for a person, in a queue, paused or held). Blocks on SQLite.
pub(super) fn settled(db: &ProjectDb, run: Option<&Run>, since: Timestamp) -> Result<StallSettled, StoreError> {
  let Some(run) = run.filter(|run| !run.status().is_terminal()) else {
    return Ok(StallSettled::Ended);
  };
  Ok(match db.run_progressed_after(run.id(), since)? {
    Some(at) => StallSettled::Moving(at),
    None => StallSettled::Waits,
  })
}

/// The no-progress `event` of `task`'s run `run`, as the conversation keeps it: named after the step an agent works
/// on, and linking to the task's Summary.
fn stalled_wait(project: ProjectId, (task, run): (&Task, &Run), event: TaskEvent) -> AgentWait {
  let step = run
    .running_executions()
    .first()
    .and_then(|execution| run.execution(*execution))
    .and_then(|execution| StepName::in_run(run, execution.step()));
  AgentWait {
    kind: WaitKind::Event(event),
    task: task.key().to_string(),
    title: task.details().title.as_str().to_owned(),
    run: run.id(),
    number: run.number().get(),
    place: step.as_ref().map(|step| step.place),
    step: step.map(|step| step.name),
    others: 0,
    agent: None,
    link: Link::Task(project, task.key().clone(), TaskPage::Summary).path(),
    automatic: false,
    continuing: None,
    merging: None,
  }
}

/// Whether `kind` is a run's stall.
pub(super) const fn is_stall(kind: WaitKind) -> bool {
  matches!(
    kind,
    WaitKind::Event(TaskEvent {
      kind: TaskEventKind::NoProgress { .. },
      ..
    })
  )
}
