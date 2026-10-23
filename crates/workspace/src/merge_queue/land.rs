//! Merging a run the queue delivers into its base branch, as the task page's *Merge into <base>* merges
//! (`tasks::integration::merge_in`): in the person's checkout, which must have the base branch checked out and no
//! uncommitted changes, recording the merge (its task is done) and clearing the run's worktree, code index and merged
//! branch after it. It lands exactly what was checked: the run's branch at the tip the check ran on, over the base at
//! the head it held; a base that moved since lands nothing, and the run syncs again. The Deliver step's method decides
//! the commit: always a merge commit, or one squashed commit titled `<task key>: <title>`.

use std::path::Path;

use orchester_core::{
  integration::{
    DeliveredAs, MergeError, MergeKind, branch_head,
    delivery::{self, Checkout},
    drift,
  },
  merge_queue::{Blocked, CheckedOn, Delivered},
  resource::{DeliverStep, MergeMethod},
  run::Run,
  task::Task,
};
use orchester_exec::programs::Programs;

use crate::tasks::{
  changes::GIT,
  integration::{IntegrationProblem, merge_in_as},
  local_integration::CheckoutHolder,
};

/// Characters of the task's brief the squash commit's body keeps.
const SUMMARY_CHARS: usize = 200;

/// How landing a run went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Landed {
  Merged(Delivered),
  /// The checkout keeps the merge out; nothing changed.
  Blocked(Blocked),
  /// The base branch or the run's branch moved since the check: the run syncs and is checked again.
  Moved,
  /// Git or the store stopped it, in their words.
  Stopped(String),
}

/// Merges `run`, of `task`, which was synced and checked on `checked` (`None`: its pipeline does not check the merged
/// result again), into its base branch in the checkout at `root`, as `deliver` says.
pub(super) fn land(
  (root, programs): (&Path, &Programs),
  (run, task): (&Run, &Task),
  (deliver, checked): (&DeliverStep, Option<&CheckedOn>),
) -> Landed {
  let Some(base) = run.base_branch() else {
    return Landed::Stopped("the run did not record its base branch".to_owned());
  };
  let runner = programs.runner();
  match delivery::checkout(&runner, Path::new(GIT), (root, base)) {
    Ok(Checkout::OnBase { uncommitted }) if uncommitted.is_empty() => {}
    Ok(Checkout::OnBase { uncommitted }) => return Landed::Blocked(Blocked::Uncommitted { files: uncommitted }),
    Ok(Checkout::OtherBranch(current)) => {
      return Landed::Blocked(Blocked::OtherBranch {
        current: current.map(|branch| branch.as_str().to_owned()),
      });
    }
    Ok(Checkout::InProgress) => return Landed::Blocked(Blocked::InProgress),
    Err(error) => return Landed::Stopped(error.to_string()),
  }
  let heads = branch_head(&runner, Path::new(GIT), (root, base))
    .and_then(|base_head| Ok((base_head, branch_head(&runner, Path::new(GIT), (root, run.branch()))?)));
  let (Some(base_head), Some(tip)) = (match heads {
    Ok(heads) => heads,
    Err(error) => return Landed::Stopped(error.to_string()),
  }) else {
    return Landed::Stopped(format!("{base} or {} does not exist", run.branch()));
  };
  let synced = match checked {
    Some(checked) => checked.base == base_head && checked.tip == tip,
    // Without a check, the branch must still hold the base as it is now.
    None => match drift(&runner, Path::new(GIT), (root, run.branch(), &base_head)) {
      Ok(lacks) => lacks.is_none(),
      Err(error) => return Landed::Stopped(error.to_string()),
    },
  };
  if !synced {
    return Landed::Moved;
  }
  let title = format!("{}: {}", task.key(), task.details().title.as_str());
  let body = squash_body(task, run);
  let as_delivered = match deliver.method() {
    MergeMethod::MergeCommit => DeliveredAs::MergeCommit,
    MergeMethod::Squash => DeliveredAs::Squash {
      title: &title,
      body: &body,
    },
  };
  match merge_in_as(root, programs, (run.id(), Some(&tip), Some(as_delivered))) {
    Ok(merged) => Landed::Merged(cleared(programs, (root, run), &merged)),
    Err(problem) => not_landed(problem),
  }
}

/// What was cleared after `merged`; a squashed run's branch, which git does not see merged, is deleted once its
/// worktree went.
fn cleared(programs: &Programs, (root, run): (&Path, &Run), merged: &crate::tasks::integration::Merged) -> Delivered {
  let (worktree_removed, mut branch_deleted) = match &merged.cleanup {
    Ok(cleanup) => (
      cleanup.removed_run_worktree(),
      cleanup.deleted.iter().any(|branch| branch == run.branch()),
    ),
    Err(_) => (false, false),
  };
  if merged.merge.kind == MergeKind::Squash && worktree_removed && !branch_deleted {
    match delivery::delete_branch(&programs.git_work(), Path::new(GIT), (root, run.branch())) {
      Ok(()) => branch_deleted = true,
      Err(error) => tracing::warn!(%error, branch = %run.branch(), "cannot delete the squashed run's branch"),
    }
  }
  Delivered {
    revision: merged.merge.revision.clone(),
    kind: merged.merge.kind,
    worktree_removed,
    branch_deleted,
  }
}

fn not_landed(problem: IntegrationProblem) -> Landed {
  match problem {
    IntegrationProblem::Merge(MergeError::UncommittedInTheWay { files }) => {
      Landed::Blocked(Blocked::Uncommitted { files })
    }
    IntegrationProblem::Merge(MergeError::WrongBranch { current, .. }) => Landed::Blocked(Blocked::OtherBranch {
      current: current.map(|branch| branch.as_str().to_owned()),
    }),
    IntegrationProblem::Merge(MergeError::InProgress) => Landed::Blocked(Blocked::InProgress),
    IntegrationProblem::HeldLocally(CheckoutHolder::ThisRun | CheckoutHolder::OtherRun { .. }) => {
      Landed::Blocked(Blocked::HeldLocally)
    }
    IntegrationProblem::Merge(MergeError::BaseMoved { .. } | MergeError::BranchMoved { .. }) => Landed::Moved,
    IntegrationProblem::Merge(error) => Landed::Stopped(error.to_string()),
    IntegrationProblem::Cleanup(error) => Landed::Stopped(error.to_string()),
    IntegrationProblem::Read(error) => Landed::Stopped(error.to_string()),
    IntegrationProblem::Store(message) | IntegrationProblem::NotRecorded { problem: message, .. } => {
      Landed::Stopped(message.key.to_owned())
    }
    other => Landed::Stopped(format!("{other:?}")),
  }
}

/// The squash commit's body: the first line of the task's brief, then the run it delivers.
fn squash_body(task: &Task, run: &Run) -> String {
  let summary: String = task
    .details()
    .brief
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .unwrap_or_default()
    .chars()
    .take(SUMMARY_CHARS)
    .collect();
  let delivered = format!("Delivered by Orchester's merge queue: {} run #{}.", task.key(), run.number().get());
  if summary.is_empty() {
    delivered
  } else {
    format!("{summary}\n\n{delivered}")
  }
}
