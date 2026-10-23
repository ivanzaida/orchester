//! A person's decisions on a run of the queue. *Merge into <base>* on a run waiting for approval approves it: it goes
//! to the top of the waiting runs and merges in its turn, after syncing and checking again first if the base moved
//! since its check. *Reject* takes the run out of the queue without merging it: the base stays as it is. *Resolve
//! myself*, for a run whose conflicts the Integrate step could not resolve, merges the base into the run's worktree
//! and leaves the conflicts there for the person ([`resolve_myself`]); *I've resolved it* checks that no conflict
//! marker is left and commits the merge, sends the run's work back to its Integrate step with the person's
//! resolution, so its agent and the steps after it (Check, Review) pass again, and puts the run back in the queue,
//! where it syncs with the base as it is then and Check runs on the merged result before it merges ([`resolved`]).

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::resolve::{self, ResolveError, Started},
  merge_queue::{Left, QueueEntry, QueueEvent, QueueState},
  run::{Answerer, BranchName, ReviewNote, Run, RunStatus, StuckDecision},
  store::RunStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::{Command, WorkReturn};
use orchester_store::{ProjectDb, ResolutionStart};

use super::{QueueRefusal, order::to_top, wake};
use crate::{
  messages::Message,
  tasks::{
    changes::{GIT, run_worktree},
    detail::run_store_error,
    integration::file_list,
    resolve::markers_left,
  },
};

/// Approves the merge of `run`, which waits for approval in the queue of the project at `root`, at `now`.
pub fn approve(root: &Path, run: RunId, now: Timestamp) -> Result<(), QueueRefusal> {
  let mut db = ProjectDb::open(root)?;
  let mut entry = entry_in(&db, run, |state| *state == QueueState::WaitingForApproval)?;
  let stored = db.run(run).map_err(run_store_error)?.ok_or(QueueRefusal::NotThere)?;
  if *stored.status() != RunStatus::Succeeded {
    return Err(QueueRefusal::NotThere);
  }
  entry.approved_at = Some(now);
  entry.state = QueueState::Queued;
  entry.since = now;
  if !db.save_merge_queue_entry(&entry, &QueueState::WaitingForApproval, (&[QueueEvent::Approved], now))? {
    return Err(QueueRefusal::NotThere);
  }
  to_top(&mut db, run)?;
  wake(root);
  Ok(())
}

/// Rejects `run` in the queue of the project at `root` at `now`: waiting for approval, or out of the queue with
/// conflicts that need a person, it is not merged and does not join the queue again.
pub fn reject(root: &Path, run: RunId, now: Timestamp) -> Result<(), QueueRefusal> {
  let mut db = ProjectDb::open(root)?;
  let mut entry = entry_in(&db, run, |state| {
    matches!(state, QueueState::WaitingForApproval | QueueState::Left(_))
  })?;
  let expected = std::mem::replace(&mut entry.state, QueueState::Removed);
  entry.since = now;
  if !db.save_merge_queue_entry(&entry, &expected, (&[QueueEvent::Removed], now))? {
    return Err(QueueRefusal::NotThere);
  }
  wake(root);
  Ok(())
}

/// *Resolve myself* on `run`, out of the queue of the project at `root` with conflicts: the base branch is merged into
/// the run's branch in its worktree and the files that conflict are left there with git's markers, unless a merge is in
/// progress there already. Gives the files that conflict.
pub fn resolve_myself(root: &Path, programs: &Programs, run: RunId) -> Result<Vec<String>, QueueRefusal> {
  let mut db = ProjectDb::open(root)?;
  let entry = entry_in(&db, run, |state| matches!(state, QueueState::Left(Left::Conflicts { .. })))?;
  let QueueState::Left(Left::Conflicts { files }) = &entry.state else {
    return Err(QueueRefusal::NotThere);
  };
  let stored = db.run(run).map_err(run_store_error)?.ok_or(QueueRefusal::NotThere)?;
  let base = stored.base_branch().ok_or(QueueRefusal::NotThere)?.clone();
  let worktree = run_worktree(root, run);
  let git = programs.git_work();
  if resolve::in_merge(&git, Path::new(GIT), &worktree).map_err(|error| refusal(&error.into()))? {
    return Ok(files.clone());
  }
  match db.begin_resolution(run, Timestamp::now())? {
    ResolutionStart::Begun => {}
    ResolutionStart::NotEnded | ResolutionStart::Merged => return Err(QueueRefusal::NotThere),
  }
  let tip = head(programs, &worktree)?;
  let started = resolve::start(&git, Path::new(GIT), &worktree, (&base, stored.branch(), &tip));
  if !matches!(started, Ok(Started::Conflicts(_))) {
    db.end_resolution(run)?;
  }
  // The run's worktree changed: whoever shows the run reads it again.
  wake(root);
  match started {
    Ok(Started::Conflicts(conflicts)) => Ok(conflicts),
    Ok(Started::Merged(_)) => Ok(Vec::new()),
    Err(error) => Err(refusal(&error)),
  }
}

/// *I've resolved it* on `run`, out of the queue of the project at `root` with conflicts, at `now`: no conflict
/// marker may be left, the merge in its worktree is committed, and the run goes back in the queue at the top of the
/// waiting runs. Gives the command that sends its work back to its Integrate step with the person's resolution,
/// for the project's runner: the run joins in again once it passed every step again.
pub fn resolved(root: &Path, programs: &Programs, run: RunId, now: Timestamp) -> Result<Command, QueueRefusal> {
  let mut db = ProjectDb::open(root)?;
  let mut entry = entry_in(&db, run, |state| matches!(state, QueueState::Left(Left::Conflicts { .. })))?;
  let stored = db.run(run).map_err(run_store_error)?.ok_or(QueueRefusal::NotThere)?;
  let base = stored.base_branch().ok_or(QueueRefusal::NotThere)?.clone();
  let worktree = run_worktree(root, run);
  let git = programs.git_work();
  if resolve::in_merge(&git, Path::new(GIT), &worktree).map_err(|error| refusal(&error.into()))? {
    let tip = head(programs, &worktree)?;
    resolve::finish(&git, Path::new(GIT), &worktree, (&base, &tip)).map_err(|error| refusal(&error))?;
    db.end_resolution(run)?;
  } else if let QueueState::Left(Left::Conflicts { files }) = &entry.state {
    let marked: Vec<String> = files
      .iter()
      .filter(|file| {
        std::fs::read_to_string(worktree.join(file.as_str())).is_ok_and(|text| resolve::holds_markers(&text))
      })
      .cloned()
      .collect();
    if !marked.is_empty() {
      return Err(refusal(&ResolveError::MarkersLeft(marked)));
    }
  }
  let step = integrate_step_of(&db, run)?.ok_or(QueueRefusal::NotThere)?;
  let command = continued(&stored, step, &base);
  let expected = std::mem::replace(&mut entry.state, QueueState::Queued);
  entry.since = now;
  entry.resyncs = 0;
  entry.checked = None;
  if !db.save_merge_queue_entry(&entry, &expected, (&[QueueEvent::Requeued], now))? {
    return Err(QueueRefusal::NotThere);
  }
  to_top(&mut db, run)?;
  wake(root);
  Ok(command)
}

/// The command that sends `run`'s work back to `step` with the person's resolution of its conflicts with `base`.
fn continued(run: &Run, step: orchester_core::resource::StepKey, base: &BranchName) -> Command {
  let note = ReviewNote::new(&format!(
    "A person resolved the conflicts with {} by hand and committed the merge. Look over the resolution, keep both \
     sides' intent, and finish your step.",
    base.as_str()
  ))
  .expect("the note names the base branch, so it is never blank");
  match run.status() {
    RunStatus::WaitingForPerson(_) => Command::DecideStuckRun {
      run: run.id(),
      decision: StuckDecision::ReturnTo { step, note },
    },
    _ => Command::ReturnRun(WorkReturn {
      run: run.id(),
      step,
      note,
      conflicts: None,
      by: Answerer::Person,
      expected: run.last_sequence(),
    }),
  }
}

/// The step the run's conflicts last went to, as its queue events tell.
fn integrate_step_of(db: &ProjectDb, run: RunId) -> Result<Option<orchester_core::resource::StepKey>, QueueRefusal> {
  Ok(
    db.merge_queue_events(run)?
      .into_iter()
      .rev()
      .find_map(|recorded| match recorded.event {
        QueueEvent::IntegrateStarted { step, .. } => Some(step),
        _ => None,
      }),
  )
}

fn entry_in(db: &ProjectDb, run: RunId, state: impl Fn(&QueueState) -> bool) -> Result<QueueEntry, QueueRefusal> {
  db.merge_queue_entry(run)?
    .filter(|entry| state(&entry.state))
    .ok_or(QueueRefusal::NotThere)
}

fn head(programs: &Programs, worktree: &Path) -> Result<orchester_core::revision::Revision, QueueRefusal> {
  let runner = programs.runner();
  orchester_core::integration::delivery::head(&runner, Path::new(GIT), worktree)
    .map_err(|error| refusal(&error.into()))
}

/// What a person reads when resolving did not go on.
fn refusal(error: &ResolveError) -> QueueRefusal {
  QueueRefusal::Resolve(match error {
    ResolveError::MarkersLeft(files) => markers_left(files),
    ResolveError::Uncommitted(files) => {
      Message::new("tasks.resolve.uncommitted").with("files", file_list(files, ", "))
    }
    other => Message::new("mergeQueue.refused.resolve").with("detail", other.to_string()),
  })
}
