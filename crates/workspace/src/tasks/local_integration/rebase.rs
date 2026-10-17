//! The base and the last checkpoint external evidence reads a run's commits from: the ones it
//! recorded, unless git proves its branch was rebased onto a newer commit of the target since
//! (`orchester_core::integration::local::RunRebase`). A rebase proven now is recorded; one proven
//! before stands while its base is still below the branch tip, e.g. once the reflog that proved it
//! expired.

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
  let stored = db.run_rebase(run.id())?;
  let rebase = match prove(db, main, run, (tip, target))? {
    Some(proven) => {
      if !stored.as_ref().is_some_and(|stored| stored.same_as(&proven)) {
        ProjectDb::open(main.root())?.save_run_rebase(&proven)?;
      }
      Some(proven)
    }
    None => match stored {
      Some(stored) if main.is_ancestor(stored.base().as_str(), tip)? => Some(stored),
      _ => None,
    },
  };
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
  let mut own_work_below = false;
  for checkpoint in db.checkpoint_revisions(run.id())? {
    let checkpoint = checkpoint.as_str();
    if present(main, checkpoint)?
      && main.is_ancestor(checkpoint, onto.as_str())?
      && !main.is_ancestor(checkpoint, base)?
    {
      own_work_below = true;
      break;
    }
  }
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
/// ancestor of the tip, each of its commits since the run's base has its rebased form at the tip,
/// and the tip has no commit since `onto` that the checkpoint lacks.
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
  let equivalent = |listed: &str| !listed.is_empty() && listed.lines().all(|line| line.starts_with("- "));
  let old = main.text(&["cherry", tip, last, run.base().as_str()])?;
  let new = main.text(&["cherry", last, tip, onto])?;
  if !(equivalent(&old) && equivalent(&new)) {
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
