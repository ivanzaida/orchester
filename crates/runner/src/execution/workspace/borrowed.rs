//! Where a step of a run that borrows workspaces works (ORC-114): alone, in the run's workspace; beside others, in a
//! workspace of its own it borrows, on its step branch off the run's branch. An execution whose step branch exists
//! works on it again — also when it continues after a pause, a limit or an interruption — in whichever workspace it
//! takes.

use std::path::Path;

use orchester_core::{
  id::StepExecutionId,
  integration::workspace::SwitchTo,
  run::{AcceptRefusal, BranchName, StepExecution},
  workspaces::Holder,
};

use super::{Start, Workspace, branch};
use crate::{
  execution::{
    checkpoint,
    pool::{self, Borrowed},
    run::RunTask,
  },
  git::Git,
};

impl RunTask {
  /// Where `execution` of the borrowing run works when it starts as `start` says; `Ok(None)` while every workspace
  /// is in use, so it waits. Why it cannot work anywhere, otherwise.
  pub(in crate::execution) fn borrowed_workspace(
    &mut self,
    execution: StepExecutionId,
    start: Start,
  ) -> Result<Option<Workspace>, String> {
    let owner = self.step_branch_owner(execution).map_err(|error| error.to_string())?;
    let owner = match (owner, start) {
      (Some(owner), _) => owner,
      (None, Start::BesideOthers) => {
        let branch = branch(&self.run, execution);
        let base = checkpoint::head(self.worktree()).map_err(|error| error.to_string())?;
        Git::new(&self.root)
          .stdout(&["branch", "--no-track", &branch, base.as_str()])
          .map_err(|error| error.to_string())?;
        execution
      }
      (None, Start::Alone | Start::Continues) => return Ok(Some(Workspace::Run(self.worktree().to_path_buf()))),
    };
    let name = branch(&self.run, owner);
    let step_branch = BranchName::new(&name).map_err(|error| error.to_string())?;
    let holder = Holder::Step {
      run: self.run.id(),
      execution: owner,
    };
    let base = self.run.base().as_str().to_owned();
    match pool::borrow(&mut self.db, &self.root, holder, (SwitchTo::Branch(&step_branch), &base)) {
      Ok(Borrowed::Folder(path)) => Ok(Some(Workspace::Own {
        path,
        branch: name,
        owner,
        borrowed: true,
      })),
      Ok(Borrowed::Wait(_)) => Ok(None),
      Err(error) => Err(error.to_string()),
    }
  }

  /// The execution whose step branch `execution` works on again: its own, or that of the execution it recovers,
  /// nearest first; `None` when none has one.
  fn step_branch_owner(&self, execution: StepExecutionId) -> Result<Option<StepExecutionId>, crate::worktree::WorktreeError> {
    let mut candidate = Some(execution);
    while let Some(owner) = candidate {
      if branch_exists(&self.root, &branch(&self.run, owner))? {
        return Ok(Some(owner));
      }
      candidate = self.run.recovered_execution(owner);
    }
    Ok(None)
  }

  /// Where the work of `accepted`, an execution a person accepted, is for a borrowing run: on its step branch, which
  /// passing merges into the run's branch in the run's workspace, if it worked beside others; else on the run's
  /// branch. It must build on the revision it was judged on.
  pub(in crate::execution) fn borrowed_accepted_work(
    &self,
    accepted: &StepExecution,
  ) -> Result<Result<Workspace, AcceptRefusal>, crate::worktree::WorktreeError> {
    let Some(judged) = accepted.judged() else {
      return Ok(Err(AcceptRefusal::NotJudged));
    };
    let name = branch(&self.run, accepted.id());
    let (workspace, tip) = if branch_exists(&self.root, &name)? {
      let own = Workspace::Own {
        path: self.worktree().to_path_buf(),
        branch: name.clone(),
        owner: accepted.id(),
        borrowed: true,
      };
      (own, format!("refs/heads/{name}"))
    } else {
      (Workspace::Run(self.worktree().to_path_buf()), "HEAD".to_owned())
    };
    let git = Git::new(self.worktree());
    if !git.answer(&["merge-base", "--is-ancestor", judged.as_str(), &tip])? {
      return Ok(Err(AcceptRefusal::NotBuiltOn {
        revision: judged.as_str().chars().take(12).collect(),
      }));
    }
    if !git
      .stdout(&["status", "--porcelain", "--untracked-files=all"])?
      .trim()
      .is_empty()
    {
      return Ok(Err(AcceptRefusal::Uncommitted));
    }
    Ok(Ok(workspace))
  }
}

fn branch_exists(root: &Path, name: &str) -> Result<bool, crate::worktree::WorktreeError> {
  Git::new(root).succeeds(&["show-ref", "--verify", "--quiet", &format!("refs/heads/{name}")])
}
