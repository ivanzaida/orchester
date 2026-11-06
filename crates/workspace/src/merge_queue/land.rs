//! Merging a run the queue delivers into its base branch, as the task page's *Merge into <base>* merges
//! (`tasks::integration::merge_in`): in the person's checkout, which must have the base branch checked out and no
//! uncommitted changes, or beside it when the run's base branch was set and the checkout has another branch;
//! recording the merge (its task is done) and clearing the run's worktree, code index and merged branch after it. It
//! lands exactly what was checked: the run's branch at the tip the check ran on, over the base at the head it held; a
//! base that moved since lands nothing, and the run syncs again. The Deliver step's method decides the commit: always
//! a merge commit, or one squashed commit titled `<task key>: <title>`.

use std::path::Path;

use orchester_core::{
  integration::{
    DeliveredAs, Delivering, MergeError, MergeKind, PreflightCause, PreflightStage,
    attempt::{AttemptOutcome, MergeAsker, MergeAttempt, MergeVia},
    base::Landing,
    branch_head,
    delivery::{self, Checkout},
    drift,
  },
  merge_queue::{Blocked, CheckedOn, Delivered},
  resource::{DeliverStep, MergeMethod},
  revision::Revision,
  run::Run,
  task::Task,
  time::Timestamp,
};
use orchester_exec::programs::Programs;

use crate::tasks::{
  changes::GIT,
  integration::{IntegrationProblem, merge_in_as},
  local_integration::CheckoutHolder,
  merge_attempts::{self, Asked},
  merge_retry::retried,
  worktree_removal::WorktreeRemover,
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
/// result again), into its base branch in the checkout of the project whose run worktrees are `worktrees`, or beside
/// it when the run's base branch was set and the checkout has another branch, as `deliver` says. Every attempt is
/// recorded but one the checkout blocks before the merge, which the queue polls until the checkout changes.
pub(super) fn land(
  (worktrees, programs): (&WorktreeRemover, &Programs),
  (run, task): (&Run, &Task),
  (deliver, checked): (&DeliverStep, Option<&CheckedOn>),
) -> Landed {
  let root = worktrees.root();
  let (base_head, tip) = match heads_to_land((root, programs), run, checked) {
    Ok(heads) => heads,
    Err(landed) => {
      let outcome = match &landed {
        Landed::Moved => AttemptOutcome::Moved { stage: None },
        Landed::Stopped(words) => AttemptOutcome::Stopped { words: words.clone() },
        Landed::Blocked(_) | Landed::Merged(_) => return landed,
      };
      merge_attempts::record(
        root,
        &MergeAttempt::new((run.id(), Timestamp::now()), BY_QUEUE, outcome, None),
      );
      return landed;
    }
  };
  let title = format!("{}: {}", task.key(), task.details().title.as_str());
  let body = squash_body(task, run);
  let as_delivered = match deliver.method() {
    MergeMethod::MergeCommit => DeliveredAs::MergeCommit,
    MergeMethod::Squash => DeliveredAs::Squash {
      title: &title,
      body: &body,
    },
  };
  // The heads were read before the integration lease is taken: the merge compares them again under it, so a merge
  // that lands meanwhile, from the task page say, sends the run back to sync instead.
  let delivering = Delivering {
    as_delivered,
    base: &base_head,
  };
  let landing = (run.id(), Some(&tip), Some(delivering));
  let first = merge_in_as(worktrees, programs, landing, BY_QUEUE);
  // The queue waits for a lease another operation holds itself, for as long as that holds it ([`not_landed`]);
  // Orchester tries again any other cause that passes.
  let merged = if matches!(&first, Err(problem) if lease_held(problem)) {
    first
  } else {
    retried(worktrees, programs, landing, BY_QUEUE, first)
  };
  match merged {
    Ok(merged) => Landed::Merged(cleared(programs, (root, run), &merged)),
    Err(problem) => not_landed(problem),
  }
}

/// Another merge or local integration holds the checkout's integration lease.
fn lease_held(problem: &IntegrationProblem) -> bool {
  matches!(
    problem,
    IntegrationProblem::Preflight {
      stage: PreflightStage::IntegrationLock,
      cause: PreflightCause::Held,
    }
  )
}

/// The queue asks for its merges itself.
const BY_QUEUE: Asked = (MergeAsker::MergeQueue, MergeVia::MergeQueue);

/// The heads of `run`'s base branch and branch it lands, once the checkout at `root` takes the merge and they are as
/// `checked` found them; else how landing went.
fn heads_to_land(
  (root, programs): (&Path, &Programs),
  run: &Run,
  checked: Option<&CheckedOn>,
) -> Result<(Revision, Revision), Landed> {
  let Some(base) = run.base_branch() else {
    return Err(Landed::Stopped("the run did not record its base branch".to_owned()));
  };
  let landing = Landing::of(run.base_source());
  let runner = programs.runner();
  match delivery::checkout(&runner, Path::new(GIT), (root, base)) {
    Ok(Checkout::OnBase { uncommitted }) if uncommitted.is_empty() => {}
    Ok(Checkout::OnBase { uncommitted }) => return Err(Landed::Blocked(Blocked::Uncommitted { files: uncommitted })),
    Ok(Checkout::OtherBranch(current)) if landing.beside(base, current.as_ref()) => {}
    Ok(Checkout::OtherBranch(current)) => {
      return Err(Landed::Blocked(Blocked::OtherBranch {
        current: current.map(|branch| branch.as_str().to_owned()),
      }));
    }
    Ok(Checkout::InProgress) => return Err(Landed::Blocked(Blocked::InProgress)),
    Err(error) => return Err(Landed::Stopped(error.to_string())),
  }
  let heads = branch_head(&runner, Path::new(GIT), (root, base))
    .and_then(|base_head| Ok((base_head, branch_head(&runner, Path::new(GIT), (root, run.branch()))?)));
  let (Some(base_head), Some(tip)) = heads.map_err(|error| Landed::Stopped(error.to_string()))? else {
    return Err(Landed::Stopped(format!("{base} or {} does not exist", run.branch())));
  };
  let synced = match checked {
    Some(checked) => checked.base == base_head && checked.tip == tip,
    // Without a check, the branch must still hold the base as it is now.
    None => drift(&runner, Path::new(GIT), (root, run.branch(), &base_head))
      .map_err(|error| Landed::Stopped(error.to_string()))?
      .is_none(),
  };
  if !synced {
    return Err(Landed::Moved);
  }
  Ok((base_head, tip))
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
    // Another merge or local integration holds the checkout's integration lease now: the run waits for it, and
    // delivery goes on by itself once it is free.
    IntegrationProblem::Preflight {
      stage: PreflightStage::IntegrationLock,
      cause: PreflightCause::Held,
    } => Landed::Blocked(Blocked::HeldLocally),
    // Any other refusal lasts until someone acts on its cause: waiting would hold the queue without saying why.
    IntegrationProblem::Preflight { stage, cause } => Landed::Stopped(format!("{}: {cause}", stage.name())),
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
  let delivered = format!(
    "Delivered by Orchester's merge queue: {} run #{}.",
    task.key(),
    run.number().get()
  );
  if summary.is_empty() {
    delivered
  } else {
    format!("{summary}\n\n{delivered}")
  }
}
