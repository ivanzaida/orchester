//! The base and the last checkpoint external evidence reads a run's commits from: the ones it
//! recorded, unless git proves its branch was rebased onto a newer commit of the target since
//! (`orchester_core::integration::local::RunRebase`), or merged a newer commit of the target into it (*Resolve in
//! editor*, or an agent the work was returned to). A rebase or merge proven now is recorded; one recorded before
//! stands while its base is still below the branch tip, e.g. once the reflog that proved it expired, unless a proof
//! now has a newer base.

use orchester_core::{
  integration::local::{RebaseEvidence, RebaseStore, RebasedCheckpoint, RunRebase},
  revision::Revision,
  run::{BranchName, Run},
  time::Timestamp,
};
use orchester_exec::local_git::{LocalGit, LocalGitError};
use orchester_store::ProjectDb;

use super::IntegrationError;

/// `run`'s base and last checkpoint revision as external evidence reads them, with its branch at
/// `tip` and the target at `target`.
pub(super) fn effective(
  db: &ProjectDb,
  main: &LocalGit,
  run: &Run,
  (tip, target): (Option<&str>, &str),
) -> Result<(String, Option<String>), IntegrationError> {
  let last = run.last_checkpoint().map(|checkpoint| checkpoint.revision().clone());
  let Some(tip) = tip else {
    return Ok(recorded(run, None, last.as_ref()));
  };
  let stored = match db.run_rebase(run.id())? {
    Some(stored) if main.is_ancestor(stored.base().as_str(), tip)? => Some(stored),
    _ => None,
  };
  let proven = newer(
    main,
    prove(db, main, run, (tip, target))?,
    merged_in(db, main, (run, stored.as_ref()), (tip, target))?,
  )?;
  let rebase = newer(main, stored.clone(), proven)?;
  if let Some(rebase) = &rebase
    && !stored.as_ref().is_some_and(|stored| stored.same_as(rebase))
  {
    ProjectDb::open(main.root())?.save_run_rebase(rebase)?;
  }
  Ok(recorded(run, rebase.as_ref(), last.as_ref()))
}

fn recorded(run: &Run, rebase: Option<&RunRebase>, last: Option<&Revision>) -> (String, Option<String>) {
  let base = rebase.map_or_else(|| run.base(), RunRebase::base);
  let checkpoint = rebase.map_or(last, |rebase| rebase.checkpoint_for(last));
  (
    base.as_str().to_owned(),
    checkpoint.map(|revision| revision.as_str().to_owned()),
  )
}

/// The rebase git proves for `run` now, its branch at `tip` and the target at `target`.
fn prove(
  db: &ProjectDb,
  main: &LocalGit,
  run: &Run,
  (tip, target): (&str, &str),
) -> Result<Option<RunRebase>, IntegrationError> {
  let Some(onto) = newest_onto(main, run.branch())? else {
    return Ok(None);
  };
  let base = run.base().as_str();
  let newer_than_base = onto.as_str() != base && main.is_ancestor(base, onto.as_str())?;
  let under_tip_and_target = main.is_ancestor(onto.as_str(), tip)? && main.is_ancestor(onto.as_str(), target)?;
  if !(newer_than_base && under_tip_and_target) {
    return Ok(None);
  }
  let own_work_below = own_work_below(db, main, run, onto.as_str())?;
  let checkpoint = rebased_checkpoint(main, run, (onto.as_str(), tip))?;
  let evidence = RebaseEvidence {
    onto,
    newer_than_base,
    under_tip_and_target,
    own_work_below,
    checkpoint,
  };
  Ok(RunRebase::proven(run.id(), evidence, Timestamp::now()))
}

/// Whether a checkpoint of `run` lies between its recorded base and `onto`: then commits of its own are below `onto`,
/// and moving its base there would hide that the target has them.
fn own_work_below(db: &ProjectDb, main: &LocalGit, run: &Run, onto: &str) -> Result<bool, IntegrationError> {
  let base = run.base().as_str();
  for checkpoint in db.checkpoint_revisions(run.id())? {
    let checkpoint = checkpoint.as_str();
    if present(main, checkpoint)? && main.is_ancestor(checkpoint, onto)? && !main.is_ancestor(checkpoint, base)? {
      return Ok(true);
    }
  }
  Ok(false)
}

/// Of two recorded or proven rebases, the one whose base is newer: a base that has the other's below it.
fn newer(
  main: &LocalGit,
  one: Option<RunRebase>,
  other: Option<RunRebase>,
) -> Result<Option<RunRebase>, IntegrationError> {
  Ok(match (one, other) {
    (Some(one), Some(other)) if main.is_ancestor(one.base().as_str(), other.base().as_str())? => Some(other),
    (Some(one), _) => Some(one),
    (None, other) => other,
  })
}

/// The base merged into `run`'s branch that git proves, its branch at `tip` and the target at `target`: the newest
/// merge commit on the branch's own line since the run's base, whose second parent is a commit the target has and
/// newer than the run's base (Orchester's *Resolve in editor* commits one, and so may an agent the work was returned
/// to). Its own commits sit on that commit now, as after a rebase onto it. The last checkpoint stands for itself when
/// it is the tip, or when the merge is the tip and was made right on it; else nothing is proven.
fn merged_in(
  db: &ProjectDb,
  main: &LocalGit,
  (run, stored): (&Run, Option<&RunRebase>),
  (tip, target): (&str, &str),
) -> Result<Option<RunRebase>, IntegrationError> {
  let base = run.base().as_str();
  let excluded = format!("^{base}");
  let merges = main.text(&[
    "rev-list",
    "--first-parent",
    "--merges",
    "--parents",
    "--max-count=1",
    tip,
    &excluded,
  ])?;
  let Some((merge, first, second)) = merges.lines().next().and_then(|line| {
    let mut commits = line.split(' ');
    Some((commits.next()?, commits.next()?, commits.next()?))
  }) else {
    return Ok(None);
  };
  let revision = |sha: &str| Revision::new(sha).map_err(|_| IntegrationError::from(LocalGitError::Data));
  let onto = revision(second)?;
  // The commit that holds the last checkpoint now: the checkpoint, or the tip a recorded rebase made of it.
  let last = run.last_checkpoint().map(|checkpoint| checkpoint.revision());
  let held = stored.map_or(last, |stored| stored.checkpoint_for(last));
  let checkpoint = match (last, held) {
    (Some(_), Some(held)) if held.as_str() == tip => stored.and_then(|stored| stored.checkpoint().cloned()),
    (Some(last), Some(held)) if merge == tip && first == held.as_str() => Some(RebasedCheckpoint {
      replaces: last.clone(),
      tip: revision(tip)?,
    }),
    (Some(_), _) => return Ok(None),
    (None, _) => None,
  };
  let evidence = RebaseEvidence {
    newer_than_base: second != base && main.is_ancestor(base, second)?,
    under_tip_and_target: main.is_ancestor(second, target)?,
    own_work_below: own_work_below(db, main, run, second)?,
    onto,
    checkpoint,
  };
  Ok(RunRebase::proven(run.id(), evidence, Timestamp::now()))
}

/// The commit the newest rebase of `branch` put it onto, as its reflog names it: `rebase (finish):
/// refs/heads/<branch> onto <commit>` (also `rebase -i`, `pull --rebase`), or `rebase finished: …`
/// of older git.
fn newest_onto(main: &LocalGit, branch: &BranchName) -> Result<Option<Revision>, IntegrationError> {
  let reference = format!("refs/heads/{branch}");
  let subjects = main.text(&["reflog", "show", "--format=%gs", &reference, "--"])?;
  let finished = [
    format!("(finish): {reference} onto "),
    format!("rebase finished: {reference} onto "),
  ];
  let onto = subjects.lines().find_map(|subject| {
    finished
      .iter()
      .find_map(|marker| subject.find(marker.as_str()).map(|at| &subject[at + marker.len()..]))
  });
  onto
    .map(|commit| Revision::new(commit.trim()).map_err(|_| LocalGitError::Data.into()))
    .transpose()
}

/// The last checkpoint of `run`, when the rebase onto `onto` rewrote it into `tip`: it is no
/// ancestor of the tip, and its commits since the run's base and the tip's since `onto` are the
/// same patches, whitespace included (`git patch-id --verbatim`).
fn rebased_checkpoint(
  main: &LocalGit,
  run: &Run,
  (onto, tip): (&str, &str),
) -> Result<Option<RebasedCheckpoint>, IntegrationError> {
  let Some(last) = run.last_checkpoint().map(|checkpoint| checkpoint.revision().as_str()) else {
    return Ok(None);
  };
  if last == tip || !present(main, last)? || main.is_ancestor(last, tip)? {
    return Ok(None);
  }
  let old = main.exact_patch_ids(run.base().as_str(), last)?;
  if old.is_empty() || old != main.exact_patch_ids(onto, tip)? {
    return Ok(None);
  }
  let revision = |sha: &str| Revision::new(sha).map_err(|_| IntegrationError::from(LocalGitError::Data));
  Ok(Some(RebasedCheckpoint {
    replaces: revision(last)?,
    tip: revision(tip)?,
  }))
}

/// Whether the repository has the commit `revision`: a checkpoint a rebase rewrote may be gone.
fn present(main: &LocalGit, revision: &str) -> Result<bool, IntegrationError> {
  match main.text(&["cat-file", "-e", &format!("{revision}^{{commit}}")]) {
    Ok(_) => Ok(true),
    Err(LocalGitError::Refused) => Ok(false),
    Err(error) => Err(error.into()),
  }
}
