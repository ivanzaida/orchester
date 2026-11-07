//! What the review page shows of a run (ORC-697; `crates/desktop-app/design/orchester.pen`, board `Review · redesign`,
//! 09i `wLyFt`): the run as its task names it, where it waits for review, its diff from where its work starts to where
//! its branch is, the person's review of it ([`super::read`]), the head that was reviewed and the commits after it, and
//! the implementing step whose agent answers and changes the code. A run shows here while it waits for review, while
//! Implement works on comments sent from here or on the conflicts of its merge, and, read-only, once a person approved
//! it ([`ApprovedRun`]: merged, or approved in the merge queue); not once a merge of it was tried and did not find
//! conflicts, which the attempt page tells. Blocks on SQLite and git.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{RunId, TaskId},
  integration::{LoggedCommit, attempt::AttemptOutcome},
  merge_queue::QueueState,
  project::TaskKey,
  resource::StepKey,
  revision::Revision,
  run::{BranchName, Run, RunNumber, RunStatus, StepStatus},
  store::RunStore,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{
  ReviewRefusal,
  approved_run::{ApprovedRun, approved_run},
  conflicts::{self, ReviewConflicts},
  read::{Review, review_of},
};
use crate::tasks::{
  changes::{Checkpointed, RunRange, checkpointed_in, own_start},
  data::ProjectTasks,
  detail::{RunView, TaskDetail, run_store_error},
  merge_attempts::{
    merges::MergeRun,
    read::{ReviewIn, commits_after, reviewed_head},
  },
  return_work,
};

/// The step whose agent answers the person's questions and changes the code for their comments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Implement {
  pub key: StepKey,
  /// Its position in the pipeline, from 1.
  pub number: usize,
  pub name: Arc<str>,
  /// The agent role of its profile; `None` once that is gone.
  pub agent: Option<Arc<str>>,
}

/// A run as its review page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewPage {
  pub run: MergeRun,
  pub branch: BranchName,
  /// Where the run waits for review; `None` while it does not, as Implement works on comments sent to it, and once a
  /// person approved it.
  pub review_in: Option<ReviewIn>,
  /// A person approved the run: merged, or approved in the merge queue. Its review is read-only.
  pub approved: Option<ApprovedRun>,
  /// The head that was reviewed: the tip the merge queue checked, else the run's latest checkpoint.
  pub reviewed: Option<Revision>,
  /// The run's commits after `reviewed` up to where its branch is, newest first: what changed since the review.
  pub since_review: Vec<LoggedCommit>,
  /// The run's diff from where its own work starts to where its branch is (`review.head`).
  pub changes: Checkpointed,
  pub review: Review,
  pub implement: Option<Implement>,
  /// How many of the run's steps passed, of how many.
  pub steps: (usize, usize),
  /// The approved merge into the run's base branch found conflicts and merged nothing; the run waits for review again.
  pub conflicts: Option<ReviewConflicts>,
}

impl ReviewPage {
  /// The step's agent as the threads name it: *Implement · Builder*.
  pub fn agent(&self) -> Option<(&str, Option<&str>)> {
    self
      .implement
      .as_ref()
      .map(|implement| (&*implement.name, implement.agent.as_deref()))
  }
}

/// The review page of the task `key`: of its latest run that [`review_page`] shows, the run that waits for review, was
/// sent back from it or was approved; `None` when the task has none (no such task, or no run is reviewed now).
pub fn task_review_page(
  tasks: &ProjectTasks,
  (root, programs): (&Path, &Programs),
  key: &TaskKey,
) -> Result<Option<ReviewPage>, ReviewRefusal> {
  let Some(task) = tasks
    .tasks
    .iter()
    .chain(tasks.not_shared.iter())
    .find(|task| task.key() == key)
  else {
    return Ok(None);
  };
  let mut numbers: Vec<RunNumber> = ProjectDb::open(root)?
    .runs_of_task(task.id())
    .map_err(run_store_error)?
    .into_iter()
    .filter_map(Result::ok)
    .map(|run| run.number())
    .collect();
  numbers.sort_unstable_by(|first, second| second.cmp(first));
  for number in numbers {
    if let Some(page) = review_page(tasks, (root, programs), (key, number))? {
      return Ok(Some(page));
    }
  }
  Ok(None)
}

/// The review page of run `number` of the task `key` among `tasks`, the project at `root`'s; `None` when there is no
/// such task or run, or when the run is not reviewed here: it neither waits for review nor has comments Implement works
/// on nor was approved, or a merge of it was tried and found no conflicts. `programs` read the run's branch and diff.
pub fn review_page(
  tasks: &ProjectTasks,
  (root, programs): (&Path, &Programs),
  (key, number): (&TaskKey, RunNumber),
) -> Result<Option<ReviewPage>, ReviewRefusal> {
  let Some(task) = tasks
    .tasks
    .iter()
    .chain(tasks.not_shared.iter())
    .find(|task| task.key() == key)
  else {
    return Ok(None);
  };
  let db = ProjectDb::open(root)?;
  let Some(run) = db
    .runs_of_task(task.id())
    .map_err(run_store_error)?
    .into_iter()
    .filter_map(Result::ok)
    .find(|run| run.number() == number)
  else {
    return Ok(None);
  };
  let approved = approved_run(&db, run.id())?;
  if approved.is_none() && merge_tried(&db, &run)? {
    return Ok(None);
  }
  let queue = db.merge_queue_entry(run.id())?;
  let in_queue = queue
    .as_ref()
    .filter(|entry| entry.state == QueueState::WaitingForApproval);
  let waiting = *run.status() == RunStatus::Succeeded && approved.is_none();
  let standing = if waiting { conflicts::standing(&db, &run)? } else { None };
  let reviewed = reviewed_head(&db, run.id(), in_queue)?;
  drop(db);
  let review = review_of(root, programs, run.id())?;
  let sent = review.threads.iter().any(|view| view.thread.sent().is_some());
  let resolving = return_work::resolving_conflicts(&run).is_some();
  if approved.is_none() && !waiting && !sent && !review.implement_at_work && !resolving {
    return Ok(None);
  }
  let changes = changes((root, programs), &run, review.head.as_ref())?;
  let conflicts = standing.map(|standing| conflicts::read((root, programs), &run, (standing, &changes)));
  let since_review = match (&reviewed, &review.head) {
    (Some(reviewed), Some(head)) if approved.is_none() => commits_after((root, programs), (reviewed, head)),
    _ => Vec::new(),
  };
  let (implement, steps) = implementing(root, (task.id(), run.id()))?;
  Ok(Some(ReviewPage {
    run: MergeRun {
      task: task.id(),
      key: task.key().clone(),
      title: task.details().title.as_str().into(),
      run: run.id(),
      number,
      base: run.base_branch().cloned(),
    },
    branch: run.branch().clone(),
    review_in: waiting.then_some(if in_queue.is_some() {
      ReviewIn::MergeQueue
    } else {
      ReviewIn::TaskPage
    }),
    approved,
    reviewed,
    since_review,
    changes,
    review,
    implement,
    steps,
    conflicts,
  }))
}

/// The step whose agent answers on `run` of `task` and how many of its steps passed, of how many.
fn implementing(
  root: &Path,
  (task, run): (TaskId, RunId),
) -> Result<(Option<Implement>, (usize, usize)), ReviewRefusal> {
  let detail = TaskDetail::load_in(root, task)?;
  let view = detail.run.as_ref().filter(|view| view.id() == run);
  let implement = view.and_then(|view| {
    let key = return_work::implementing_step(view).or_else(|| at_work(view))?;
    let step = view.steps.iter().find(|step| step.key == key)?;
    Some(Implement {
      key,
      number: step.number,
      name: step.name.clone(),
      agent: step.agent.clone(),
    })
  });
  let steps = view.map_or((0, 0), |view| {
    let passed = view
      .steps
      .iter()
      .filter(|step| step.status == Some(StepStatus::Passed))
      .count();
    (passed, view.steps.len())
  });
  Ok((implement, steps))
}

/// Whether a merge of `run` was tried and left something the attempt page tells: an attempt landed, or none of them
/// found conflicts. A run whose merge conflicted is the review's own, also once its work changed and it waits again.
fn merge_tried(db: &ProjectDb, run: &Run) -> Result<bool, ReviewRefusal> {
  let attempts = db.merge_attempts(run.id())?;
  let conflicted = attempts
    .iter()
    .any(|recorded| matches!(recorded.attempt.outcome(), AttemptOutcome::Conflicts { .. }));
  let landed = attempts.iter().any(|recorded| recorded.attempt.outcome().landed());
  Ok(!attempts.is_empty() && (landed || !conflicted))
}

/// The writing step at work on `view`'s run, which has not ended, so no step is one its work can go back to: the
/// one whose work its last checkpoint holds, else the one that runs now.
fn at_work(view: &RunView) -> Option<StepKey> {
  let checkpointed = view
    .run
    .last_checkpoint()
    .and_then(|checkpoint| view.run.execution(checkpoint.execution()))
    .map(|execution| execution.step().clone());
  let writing = || view.steps.iter().filter(|step| step.writes());
  checkpointed
    .filter(|key| writing().any(|step| step.key == *key))
    .or_else(|| {
      writing()
        .find(|step| step.status.is_some_and(StepStatus::is_open))
        .map(|step| step.key.clone())
    })
}

/// `run`'s diff from where its own work starts to `head`, where its branch is.
fn changes(
  (root, programs): (&Path, &Programs),
  run: &Run,
  head: Option<&Revision>,
) -> Result<Checkpointed, ReviewRefusal> {
  let Some(head) = head else {
    return Ok(Checkpointed::NoCheckpoint);
  };
  let range = RunRange::of(run);
  let read =
    own_start(root, programs, &range, head).and_then(|base| checkpointed_in(root, programs, &base, Some(head)));
  read.map_err(|error| {
    tracing::warn!(%error, run = %run.id(), "cannot read the diff of a run under review");
    ReviewRefusal::Store(error.message())
  })
}
