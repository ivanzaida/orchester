//! The Orchestrator chat follows the ends of its project's runs as it follows its agents' waits ([`super::waits`],
//! `docs/design/orchestrator-toolset.md`, decision 5): when a task's latest run succeeds, fails, stops for a decision
//! on how it goes on, stops at a limit or is interrupted, a conversation keeps one entry for it, linking to the task's
//! Summary. Each end is known by the run and the event that put the run there, so a run that stops again after it
//! went on is a new end, and one that stays where it is stays one. Ends from before the project followed them
//! ([`ProjectDb::run_ends_since`]) are history, not news, and an interruption the runner continues by itself when it
//! comes back is not one.

use orchester_core::{
  id::ProjectId,
  orchestrator::{AgentWait, RunEnd, WaitKind},
  resource::StepKey,
  run::{PauseReason, Run, RunFailure, RunLimit, RunStatus},
  store::{RunStore, TaskStore},
  task::Task,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  links::{Link, TaskPage},
  tasks::detail::run_store_error,
};

/// The ends of the latest runs of project `project`'s tasks that stand now, each by the key it is kept by.
pub(super) fn current_ends(db: &ProjectDb, project: ProjectId) -> Result<Vec<(String, AgentWait)>, StoreError> {
  let since = db.run_ends_since()?;
  let mut ends = Vec::new();
  for task in db.tasks()? {
    let Some(Ok(run)) = db.runs_of_task(task.id()).map_err(run_store_error)?.pop() else {
      continue;
    };
    let Some((end, event)) = end_of(run.status()) else {
      continue;
    };
    let Some((sequence, at)) = db.latest_run_event(run.id(), event)? else {
      continue;
    };
    if at < since {
      continue;
    }
    let key = format!("end:{}:{}", run.id(), sequence.get());
    ends.push((key, ended(project, &task, &run, end)));
  }
  Ok(ends)
}

/// How a run that stands at `status` ended or stopped, and the kind of the event that put it there
/// (`RunEventKind::name`); `None` for a run at work, paused by a person, canceled, or interrupted so that the runner
/// continues it when it comes back.
fn end_of(status: &RunStatus) -> Option<(RunEnd, &'static str)> {
  match status {
    RunStatus::Succeeded => Some((RunEnd::Succeeded, "succeeded")),
    RunStatus::Failed(_) => Some((RunEnd::Failed, "failed")),
    RunStatus::WaitingForPerson(_) => Some((RunEnd::Stopped, "waiting_for_person")),
    RunStatus::Paused(PauseReason::LimitReached(limit)) => Some((RunEnd::Limit { limit: *limit }, "paused")),
    RunStatus::Interrupted(cause) if !cause.continues_on_reconnect() => Some((RunEnd::Interrupted, "interrupted")),
    RunStatus::Pending
    | RunStatus::Running
    | RunStatus::Paused(PauseReason::Requested)
    | RunStatus::Interrupted(_)
    | RunStatus::Cancelled => None,
  }
}

/// The end `end` of `task`'s latest run `run`, as the conversation keeps it: named after the step it stopped at,
/// when it stopped at one, and linking to the task's Summary.
fn ended(project: ProjectId, task: &Task, run: &Run, end: RunEnd) -> AgentWait {
  AgentWait {
    kind: WaitKind::Ended(end),
    task: task.key().to_string(),
    title: task.details().title.as_str().to_owned(),
    run: run.id(),
    number: run.number().get(),
    step: stopped_step(run).and_then(|key| {
      run
        .snapshot()
        .body()
        .step(key)
        .map(|step| step.name.as_str().to_owned())
    }),
    agent: None,
    link: Link::Task(project, task.key().clone(), TaskPage::Summary).path(),
    own: false,
    automatic: false,
  }
}

/// The step `run` stopped at: the one that failed, waits for a decision, or used its budget or its account's plan.
fn stopped_step(run: &Run) -> Option<&StepKey> {
  match run.status() {
    RunStatus::Failed(RunFailure::StepFailed(step) | RunFailure::RetriesExhausted(step)) => Some(step),
    RunStatus::WaitingForPerson(stuck) => Some(stuck.step()),
    RunStatus::Paused(PauseReason::LimitReached(
      RunLimit::StepBudget { execution } | RunLimit::PlanLimit { execution },
    )) => run.execution(*execution).map(|execution| execution.step()),
    _ => None,
  }
}
