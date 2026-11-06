//! The runner's merges of the run's base branch into a writing agent's worktree, before the agent starts on it: for
//! a notice that the base moved ([`super::base_notice`]), and for a step the conflicts of a merge into the base branch
//! were returned to (*Ask an agent to resolve*, `continue_run`'s `resolve_conflicts`, and the Deliver step's Integrate
//! step; [`orchester_core::run::Run::returned_work`]).
//!
//! An agent never merges itself: a sandboxed one (Codex's `workspace-write`) may write its worktree's files but not
//! the repository's git data, which lives in the main checkout's `.git` (the worktree's index lock under
//! `.git/worktrees/<id>`, the shared objects, the branch refs), and granting those would let it move every branch. So
//! while the agent does not work, the runner commits its work as a checkpoint and merges the base into its worktree
//! with a merge commit. A merge that conflicts stays in progress with git's conflict markers in the files: the agent
//! only edits them, and the step's save commits the merge with its work once no marker is left — a step that ends
//! with markers left fails, nothing of them committed.
//!
//! A step returned to resolve conflicts has the base branch's tip merged before its first invocation; a later one of
//! the same execution finds the merge where that one left it. Its prompt says how the merge stands in its worktree
//! ([`RunTask::resolving_merge`]).

use std::path::Path;

use orchester_core::{id::StepExecutionId, revision::Revision};

use super::super::{
  checkpoint::{self, Merge},
  run::{ExecutionError, RunTask},
  workspace::Workspace,
};

impl RunTask {
  /// Before the first invocation of `execution`, whose step the conflicts of a merge into the run's base branch were
  /// returned to, merges the base branch's tip into its `workspace`, leaving the conflicts there for its agent. The
  /// workspace, which is gone when git refused that merge, and the revision of the run's branch the step starts on,
  /// `judged` unless the merge moved it.
  pub(super) fn merge_for_resolving(
    &mut self,
    execution: StepExecutionId,
    workspace: Result<Workspace, String>,
    judged: Option<Revision>,
  ) -> Result<(Result<Workspace, String>, Option<Revision>), ExecutionError> {
    let Ok(workspace) = workspace else {
      return Ok((workspace, judged));
    };
    let base = match self.run.returned_work(execution) {
      Some((_, Some(conflicts))) => conflicts.base.clone(),
      _ => return Ok((Ok(workspace), judged)),
    };
    if self.last_invocation(execution)?.is_some() {
      return Ok((Ok(workspace), judged));
    }
    let tip = match checkpoint::branch_tip(workspace.path(), base.as_str()) {
      Ok(tip) => tip,
      Err(error) => return Ok((Err(format!("cannot read the base branch {base}: {error}")), judged)),
    };
    let merge = match self.begin_agent_merge(execution, workspace.path(), (&tip, base.as_str(), "the base branch"))? {
      Ok(merge) => merge,
      Err(detail) => return Ok((Err(detail), judged)),
    };
    match &merge {
      Merge::Merged(merged) => {
        tracing::info!(run = %self.run.id(), %execution, revision = %tip, %merged, "the runner merged the base branch for the agent resolving its conflicts");
      }
      Merge::Conflict(files) => {
        tracing::info!(run = %self.run.id(), %execution, revision = %tip, ?files, "the base branch conflicts; its agent resolves the conflicts");
      }
    }
    let judged = match &workspace {
      // A head git cannot read leaves the step judged on where it was before the merge, as a step whose work has
      // no checkpoint.
      Workspace::Run(worktree) => checkpoint::head(worktree).ok().or(judged),
      Workspace::Own { .. } => judged,
    };
    Ok((Ok(workspace), judged))
  }

  /// How the base branch stands in `worktree` for `execution`, whose step the conflicts of a merge were returned to,
  /// as its prompt tells it: the merge in progress with the files git stopped at, else merged, at the commit checked
  /// out. `None` for any other step, and when git cannot tell.
  pub(super) fn resolving_merge(&self, execution: StepExecutionId, worktree: &Path) -> Option<Merge> {
    self.run.returned_work(execution)?.1?;
    let state = checkpoint::merge_in_progress(worktree).and_then(|in_progress| match in_progress {
      Some(files) => Ok(Merge::Conflict(files)),
      None => checkpoint::head(worktree).map(Merge::Merged),
    });
    match state {
      Ok(merge) => Some(merge),
      Err(error) => {
        tracing::warn!(run = %self.run.id(), %execution, %error, "cannot tell how the base branch stands in the worktree");
        None
      }
    }
  }

  /// Merges `revision`, the tip of `branch`, the run's base branch, into `worktree`, where `execution`'s agent is
  /// about to start, `what` naming it in commit messages and failures ("the moved base"): the agent's uncommitted work
  /// committed first as a checkpoint, then a merge commit, never a fast-forward. A merge that conflicts stays in
  /// progress for the agent to resolve. Why it could not merge, as the step's failure, otherwise — also when a merge
  /// is in progress there already, whose markers committing the work would take in.
  pub(super) fn begin_agent_merge(
    &mut self,
    execution: StepExecutionId,
    worktree: &Path,
    (revision, branch, what): (&Revision, &str, &str),
  ) -> Result<Result<Merge, String>, ExecutionError> {
    match checkpoint::in_merge(worktree) {
      Ok(false) => {}
      Ok(true) => {
        return Ok(Err(format!(
          "cannot merge {what} {branch}: a merge is in progress in the worktree"
        )));
      }
      Err(error) => {
        return Ok(Err(format!(
          "cannot tell whether a merge is in progress in the worktree: {error}"
        )));
      }
    }
    let checkpoint = self.commit_message(execution, Some(&format!("saved before merging {what}")));
    let message = format!("{}: merge {what} {branch} (run {})", self.task.key(), self.run.number());
    let saved = match checkpoint::commit(worktree, &checkpoint) {
      Ok(saved) => saved,
      Err(error) => return Ok(Err(format!("cannot commit the work before merging {what}: {error}"))),
    };
    if let Some(saved) = saved {
      self.checkpoint(execution, saved)?;
    }
    let before = checkpoint::head(worktree).ok();
    let merge = match checkpoint::begin_base_merge(worktree, revision, &message) {
      Ok(merge) => merge,
      Err(error) => return Ok(Err(format!("cannot merge {what} {branch}: {error}"))),
    };
    if let Merge::Merged(merged) = &merge
      && before.as_ref() != Some(merged)
    {
      self.checkpoint(execution, merged.clone())?;
    }
    Ok(Ok(merge))
  }
}
