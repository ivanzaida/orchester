//! A run's facts for the cards of a cancel, merge or start: its number and pipeline, its task, where it stands, its
//! branch and the branch it merges into, and what its last checkpoint changed.

use std::path::Path;

use orchester_core::{
  id::RunId,
  run::{Run, RunStatus},
  store::{MergeStore, ResourceStore, RunStore, TaskStore},
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::{RunFacts, Stands};
use crate::{
  display::short_revision,
  tasks::{
    changes::{Checkpointed, RunRange, run_changes_in},
    delivery::{self, Delivery},
    detail::run_store_error,
    run_times::finished_at,
    standing::CurrentStep,
  },
};

/// The facts of run `id` of the project at `root`; `None` when it is gone. `programs` read what a run that
/// succeeded changed.
pub(super) fn read(
  root: &Path,
  db: &ProjectDb,
  programs: &Programs,
  id: RunId,
) -> Result<Option<RunFacts>, StoreError> {
  let Some(run) = db.run(id).map_err(run_store_error)? else {
    return Ok(None);
  };
  let task = db
    .task(run.task())?
    .map(|task| (task.key().to_string(), task.details().title.as_str().to_owned()));
  let pipeline = db
    .resource(run.snapshot().pipeline().resource)?
    .map(|found| found.name().as_str().to_owned())
    .unwrap_or_default();
  let stands = stands(db, (root, programs), &run)?;
  let changes = matches!(
    stands,
    Stands::Ready { .. } | Stands::Merged { .. } | Stands::MergedOutside { .. }
  )
  .then(|| changes(root, programs, &run))
  .flatten();
  Ok(Some(RunFacts {
    run: run.id(),
    number: run.number().get(),
    pipeline,
    task,
    stands,
    branch: run.own_branch().map(|branch| branch.as_str().to_owned()),
    into: run.base_branch().map(|branch| branch.as_str().to_owned()),
    changes,
  }))
}

/// Where `run` stands; git in the checkout at `root` tells a succeeded run Orchester did not merge that was merged
/// outside it, as the task's Summary does.
fn stands(db: &ProjectDb, (root, programs): (&Path, &Programs), run: &Run) -> Result<Stands, StoreError> {
  let step = || CurrentStep::of(run).map(|step| (step.number, step.name.to_string()));
  let tokens = run.used().get();
  Ok(match run.status() {
    RunStatus::Succeeded => match db.run_merge(run.id())? {
      Some(merge) => Stands::Merged {
        at: merge.merged_at,
        revision: short_revision(merge.revision.as_str()).to_owned(),
      },
      None => match delivery::of_run(programs, root, run) {
        Delivery::MergedOutside(into) => Stands::MergedOutside { into: into.to_string() },
        Delivery::Waiting | Delivery::NothingToMerge => Stands::Ready {
          since: finished_at(db, run)?,
        },
      },
    },
    RunStatus::Cancelled => Stands::Cancelled,
    RunStatus::Failed(_) => Stands::Failed,
    RunStatus::WaitingForPerson(_) => Stands::Decision { tokens },
    _ => Stands::Going { step: step(), tokens },
  })
}

/// Files, lines added and removed by `run`'s last checkpoint over where its own work starts. Not shown when git
/// cannot read them: the task's Changes tab, which the card links to, says why.
fn changes(root: &Path, programs: &Programs, run: &Run) -> Option<(usize, u64, u64)> {
  match run_changes_in(root, programs, &RunRange::of(run)) {
    Ok(Checkpointed::Read { changes, .. }) => Some((
      changes.diff.files().len(),
      u64::from(changes.diff.additions()),
      u64::from(changes.diff.deletions()),
    )),
    Ok(Checkpointed::TooLarge { size, .. }) => Some((
      usize::try_from(size.files).ok()?,
      u64::from(size.additions),
      u64::from(size.deletions),
    )),
    Ok(Checkpointed::NoCheckpoint) => None,
    Err(error) => {
      tracing::warn!(run = %run.id(), ?error, "the changes of a run on an Orchestrator card cannot be read");
      None
    }
  }
}
