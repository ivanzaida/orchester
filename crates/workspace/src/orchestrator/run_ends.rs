//! The Orchestrator chat follows the ends of its project's runs as it follows its agents' waits ([`super::waits`],
//! `docs/design/orchestrator-toolset.md`, decision 5): when a task's latest run succeeds, fails, stops for a decision
//! on how it goes on, stops at a limit or is interrupted, a conversation keeps one entry for it, linking to the task's
//! Summary. Each end is known by the run and the event that put the run there, so a run that stops again after it
//! went on is a new end, and one that stays where it is stays one; the daily budget and a group's budget stop many
//! runs at once, so all the runs one of them stopped on a day are one end. Not ends: ends from before the project
//! followed them ([`ProjectDb::run_ends_since`]), a pause, a cancel, a *Quit now* or a stop a person asked for, an
//! interruption the runner continues by itself when it comes back, and a run that succeeded and was merged, in
//! Orchester or outside it, or has nothing to merge. Git is asked about runs that succeeded once per follow, in one
//! reading of the branches and outside the follower's lock ([`merge_checks`]).

use std::{
  collections::{HashMap, HashSet},
  path::Path,
};

use orchester_core::{
  id::{ProjectId, RunId},
  integration::RunWork,
  orchestrator::{AgentWait, RunEnd, StopReason, WaitKind, change::StepName},
  resource::StepKey,
  run::{InterruptCause, PauseReason, Run, RunFailure, RunLimit, RunStatus},
  store::{MergeStore, RunStore, TaskStore},
  task::Task,
};
use orchester_exec::programs::Programs;
use orchester_store::{EndedRun, ProjectDb, StoreError};

use crate::{
  links::{Link, TaskPage},
  tasks::{
    delivery::{self, Delivery},
    detail::run_store_error,
    hold,
  },
};

/// The kind of the event a person's decision on a stopped run is kept by; a failure right after it is their stop.
const STUCK_DECIDED: &str = "stuck_decided";

/// Where the ends are read: project `project` at `root`, whose git `programs` read a merge outside Orchester for a
/// run that `checks` does not tell yet.
pub(super) struct Reading<'a> {
  pub(super) project: ProjectId,
  pub(super) root: &'a Path,
  pub(super) programs: &'a Programs,
  pub(super) checks: &'a MergeChecks,
}

/// What git told of the project's runs that succeeded and that Orchester did not merge: those whose work it holds
/// merged outside Orchester, or that have nothing to merge (`quiet`), are never told as ready to merge; the others
/// wait to be merged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergeChecks {
  pub quiet: HashSet<SucceededAt>,
  pub waiting: HashSet<SucceededAt>,
}

/// A run that succeeded, by the sequence of the event it succeeded with: a run that succeeds again after a follow-up
/// is asked about again.
pub type SucceededAt = (RunId, u64);

/// Asks git, in one reading of the branches, about the runs that succeeded since the chat follows ends, that
/// Orchester did not merge and that `settled` does not settle, but those already known `quiet`. Blocks on SQLite and
/// git.
pub(super) fn merge_checks(
  db: &ProjectDb,
  (root, programs): (&Path, &Programs),
  (settled, quiet): (impl Fn(&str) -> bool, &HashSet<SucceededAt>),
) -> Result<MergeChecks, StoreError> {
  let mut runs = Vec::new();
  for ended in db.ended_runs_since(db.run_ends_since()?)? {
    let key = format!("end:{}:{}", ended.run, ended.sequence.get());
    let at = (ended.run, ended.sequence.get());
    if ended.kind != "succeeded" || settled(&key) || quiet.contains(&at) {
      continue;
    }
    if db.run_merge(ended.run)?.is_some() {
      continue;
    }
    if let Some(run) = db.run(ended.run).map_err(run_store_error)? {
      runs.push((run, at));
    }
  }
  let works: Vec<RunWork> = runs.iter().map(|(run, _)| RunWork::of(run)).collect();
  let mut checks = MergeChecks::default();
  for ((_, at), found) in runs.iter().zip(delivery::deliveries(programs, root, &works)) {
    let set = match found {
      Delivery::Waiting => &mut checks.waiting,
      Delivery::MergedOutside(_) | Delivery::NothingToMerge => &mut checks.quiet,
    };
    set.insert(*at);
  }
  Ok(checks)
}

/// The ends of the latest runs of the project's tasks that stand now, each by the key it is kept by, but those that
/// `settled` tells need no look: kept already, and starting no turn that still waits.
pub(super) fn current_ends(
  db: &ProjectDb,
  reading: &Reading,
  settled: impl Fn(&str) -> bool,
) -> Result<Vec<(String, AgentWait)>, StoreError> {
  let mut ends: Vec<(String, AgentWait)> = Vec::new();
  let mut batches: HashMap<String, usize> = HashMap::new();
  for ended in db.ended_runs_since(db.run_ends_since()?)? {
    let own_key = format!("end:{}:{}", ended.run, ended.sequence.get());
    // A run at a shared limit is one of a batch, kept under the batch's key: it is looked at to count it.
    if ended.kind != "paused" && settled(&own_key) {
      continue;
    }
    let Some((run, end)) = standing(db, reading, &ended)? else {
      continue;
    };
    let key = match batch_key(db, &run, end, &ended)? {
      Some(batch) => batch,
      None => own_key,
    };
    if let Some(index) = batches.get(&key) {
      ends[*index].1.others += 1;
      continue;
    }
    if settled(&key) {
      continue;
    }
    let Some(task) = db.task(run.task())? else {
      continue;
    };
    if key.starts_with("end:limit:") {
      batches.insert(key.clone(), ends.len());
    }
    ends.push((key, ended_wait(reading.project, &task, &run, end)));
  }
  Ok(ends)
}

/// `ended`'s run and its end, while it still stands where that event put it and that is an end.
fn standing(db: &ProjectDb, reading: &Reading, ended: &EndedRun) -> Result<Option<(Run, RunEnd)>, StoreError> {
  // A failure right after a person's decision on the stopped run is their stop.
  if ended.kind == "failed" && ended.after.as_deref() == Some(STUCK_DECIDED) {
    return Ok(None);
  }
  let Some(run) = db.run(ended.run).map_err(run_store_error)? else {
    return Ok(None);
  };
  let Some(end) = end_of(&run, &ended.kind) else {
    return Ok(None);
  };
  if end == RunEnd::Succeeded && merged(db, reading, (&run, ended))? {
    return Ok(None);
  }
  Ok(Some((run, end)))
}

/// How `run` ended or stopped, while its status is the one the event of kind `kind` (`RunEventKind::name`) put it in;
/// `None` for a run that went on since, or whose stop is no end.
fn end_of(run: &Run, kind: &str) -> Option<RunEnd> {
  match (run.status(), kind) {
    (RunStatus::Succeeded, "succeeded") => Some(RunEnd::Succeeded),
    (RunStatus::Failed(_), "failed") => Some(RunEnd::Failed),
    (RunStatus::WaitingForPerson(_), "waiting_for_person") => {
      StopReason::of_run(run).map(|why| RunEnd::Stopped { why })
    }
    (RunStatus::Paused(PauseReason::LimitReached(limit)), "paused") => Some(RunEnd::Limit { limit: *limit }),
    (RunStatus::Interrupted(InterruptCause::Crashed | InterruptCause::RunnerFailed), "interrupted") => {
      Some(RunEnd::Interrupted)
    }
    _ => None,
  }
}

/// Whether `run`, which succeeded with `ended`, is not to be told as ready to merge: merged in Orchester, or, as git
/// tells, merged outside it or with nothing to merge. Git is asked here only for a run the checks before did not tell.
fn merged(db: &ProjectDb, reading: &Reading, (run, ended): (&Run, &EndedRun)) -> Result<bool, StoreError> {
  let (checks, at) = (reading.checks, (run.id(), ended.sequence.get()));
  if db.run_merge(run.id())?.is_some() || checks.quiet.contains(&at) {
    return Ok(true);
  }
  if checks.waiting.contains(&at) {
    return Ok(false);
  }
  Ok(!matches!(
    delivery::of_run(reading.programs, reading.root, run),
    Delivery::Waiting
  ))
}

/// The key of the batch `run` is one of when the daily budget or its group's budget stopped it: those it stopped on
/// the day `ended` happened; `None` for an end of its own.
fn batch_key(db: &ProjectDb, run: &Run, end: RunEnd, ended: &EndedRun) -> Result<Option<String>, StoreError> {
  let day = ended.at.local_day_start().unix_millis();
  Ok(match end {
    RunEnd::Limit {
      limit: RunLimit::DailyBudget,
    } => Some(format!("end:limit:daily:{day}")),
    RunEnd::Limit {
      limit: RunLimit::GroupBudget,
    } => {
      let group = hold::group_of(db, run.task())?.map_or_else(String::new, |group| group.to_string());
      Some(format!("end:limit:group:{group}:{day}"))
    }
    _ => None,
  })
}

/// The end `end` of `task`'s latest run `run`, as the conversation keeps it: named after the step it stopped at, when
/// it stopped at one, and linking to the task's Summary.
fn ended_wait(project: ProjectId, task: &Task, run: &Run, end: RunEnd) -> AgentWait {
  let stopped = stopped_step(run).and_then(|key| StepName::in_run(run, key));
  AgentWait {
    kind: WaitKind::Ended(end),
    task: task.key().to_string(),
    title: task.details().title.as_str().to_owned(),
    run: run.id(),
    number: run.number().get(),
    place: stopped.as_ref().map(|step| step.place),
    step: stopped.map(|step| step.name),
    others: 0,
    agent: None,
    link: Link::Task(project, task.key().clone(), TaskPage::Summary).path(),
    own: false,
    automatic: false,
    continuing: None,
    merging: None,
  }
}

/// The step `run` stopped at: the one that failed, waits for a decision, or used its budget or its account's plan.
pub(super) fn stopped_step(run: &Run) -> Option<&StepKey> {
  match run.status() {
    RunStatus::Failed(RunFailure::StepFailed(step) | RunFailure::RetriesExhausted(step)) => Some(step),
    RunStatus::WaitingForPerson(stuck) => Some(stuck.step()),
    RunStatus::Paused(PauseReason::LimitReached(
      RunLimit::StepBudget { execution } | RunLimit::PlanLimit { execution },
    )) => run.execution(*execution).map(|execution| execution.step()),
    _ => None,
  }
}
