//! The base and the last checkpoint external evidence reads a run's commits from: the ones it
//! recorded, unless git proves its branch was rebased onto a newer commit of the target since
//! (`orchester_core::integration::local::RunRebase`), merged a newer commit of the target into it (*Resolve in
//! editor*, or an agent the work was returned to), or took commits of the target in another way, a fast-forward onto
//! it included (ORC-60, `orchester_core::integration::fork_point`): its own work starts above them. A rebase
//! or merge proven now is recorded; one recorded before stands while its base is still below the branch tip, e.g.
//! once the reflog that proved it expired, unless a proof now has a newer base. A branch tip its agent committed
//! above the last checkpoint stands for it ([`continues`]).

use std::collections::HashSet;

use orchester_core::{
  integration::{
    Ancestry, RunFork, fork_point,
    local::{RebaseEvidence, RebaseStore, RebasedCheckpoint, RunRebase},
    made_on_branch, parse_log,
  },
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
  let rebase = past(main, rebase, forked(db, main, run, (tip, target))?)?;
  if let Some(rebase) = &rebase
    && !stored.as_ref().is_some_and(|stored| stored.same_as(rebase))
  {
    ProjectDb::open(main.root())?.save_run_rebase(rebase)?;
  }
  let (base, checkpoint) = recorded(run, rebase.as_ref(), last.as_ref());
  match checkpoint {
    Some(checkpoint) if checkpoint != tip && continues(main, run.branch(), (&checkpoint, tip, target))? => {
      Ok((base, Some(tip.to_owned())))
    }
    checkpoint => Ok((base, checkpoint)),
  }
}

/// Whether `branch`, at `tip`, continues the run's last checkpoint `checkpoint`: the tip is above it, and every commit
/// between them the target at `target` does not hold was made on the branch, as its reflog shows. An agent that
/// commits on the run's branch itself, as one told to merge the target in does (ORC-54), records no checkpoint
/// (ORC-67): its commits are the run's work all the same, and the tip stands for the checkpoint.
fn continues(
  main: &LocalGit,
  branch: &BranchName,
  (checkpoint, tip, target): (&str, &str, &str),
) -> Result<bool, IntegrationError> {
  if !present(main, checkpoint)? || !main.is_ancestor(checkpoint, tip)? {
    return Ok(false);
  }
  let made: HashSet<String> = Checkout(main)
    .branch_log(branch)?
    .into_iter()
    .filter(|(_, subject)| made_on_branch(subject))
    .map(|(commit, _)| commit)
    .collect();
  let checkpoint = format!("^{checkpoint}");
  let target = format!("^{target}");
  let above = main.text(&["rev-list", tip, &checkpoint, &target])?;
  Ok(above.lines().all(|commit| made.contains(commit)))
}

/// The base `run`'s branch at `tip` took in from the target at `target` (`orchester_core::integration::fork_point`):
/// a fast-forward onto it, or the newest commit both share, with no commit of the run's own below it. A base recorded
/// before is weighed by `effective` itself.
fn forked(
  db: &ProjectDb,
  main: &LocalGit,
  run: &Run,
  (tip, target): (&str, &str),
) -> Result<Option<RunRebase>, IntegrationError> {
  let checkpoints = db.checkpoint_revisions(run.id())?;
  let fork = RunFork {
    branch: run.branch(),
    base: run.base().as_str(),
    head: tip,
    target,
    checkpoints: &checkpoints,
    proven: None,
  };
  let Some(onto) = fork_point(&Checkout(main), &fork)? else {
    return Ok(None);
  };
  // `fork_point` proved all of these.
  let evidence = RebaseEvidence {
    onto: Revision::new(&onto).map_err(|_| IntegrationError::from(LocalGitError::Data))?,
    newer_than_base: true,
    under_tip_and_target: true,
    own_work_below: false,
    checkpoint: None,
  };
  Ok(RunRebase::proven(run.id(), evidence, Timestamp::now()))
}

/// `forked` in place of `rebase` when its base is newer. A checkpoint the rebase rewrote into the branch tip is
/// still held there, so it is kept.
fn past(
  main: &LocalGit,
  rebase: Option<RunRebase>,
  forked: Option<RunRebase>,
) -> Result<Option<RunRebase>, IntegrationError> {
  let (rebase, forked) = match (rebase, forked) {
    (rebase, None) => return Ok(rebase),
    (None, forked) => return Ok(forked),
    (Some(rebase), Some(forked)) => (rebase, forked),
  };
  if rebase.base() == forked.base() || !main.is_ancestor(rebase.base().as_str(), forked.base().as_str())? {
    return Ok(Some(rebase));
  }
  Ok(Some(RunRebase::restore(
    forked.run(),
    forked.base().clone(),
    rebase.checkpoint().cloned(),
    forked.recorded_at(),
  )))
}

/// The main checkout's git, as `fork_point` asks it.
struct Checkout<'a>(&'a LocalGit);

impl Ancestry for Checkout<'_> {
  type Error = IntegrationError;

  fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, IntegrationError> {
    Ok(self.0.is_ancestor(ancestor, descendant)?)
  }

  fn merge_base(&self, one: &str, other: &str) -> Result<Option<String>, IntegrationError> {
    match self.0.text(&["merge-base", one, other]) {
      Ok(commit) => Ok(Some(commit).filter(|commit| !commit.is_empty())),
      // Git exits with 1 for commits that share no ancestor.
      Err(LocalGitError::Refused) => Ok(None),
      Err(error) => Err(error.into()),
    }
  }

  fn commits_between(&self, from: &str, to: &str) -> Result<Vec<String>, IntegrationError> {
    let excluded = format!("^{from}");
    let listed = self.0.text(&["rev-list", to, &excluded])?;
    Ok(listed.lines().map(str::to_owned).collect())
  }

  fn branch_log(&self, branch: &BranchName) -> Result<Vec<(String, String)>, IntegrationError> {
    let reference = format!("refs/heads/{branch}");
    let listed = self
      .0
      .text(&["reflog", "show", "--format=%H%x09%gs", &reference, "--"])?;
    Ok(parse_log(&listed))
  }
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
