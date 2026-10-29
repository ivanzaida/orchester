//! Notices that the run's base branch moved (ORC-54, `orchester_core::base_move`), which the desktop queues: the
//! run's next agent of a step that implements the change takes its pending notice once the window of its move has
//! passed, never mid-edit, when the notice overlaps the run's changes (ORC-151). Such a step is one the pipeline has
//! take base moves (ORC-65, `orchester_core::resource::Pipeline::takes_base_moves`: its own choice, else a writing
//! step that does not review or verify) and whose agent writes in this run (repository access `write`). An invocation
//! that starts takes it into its prompt — a step's start, a session continued after a pause, a limit, a restart or a
//! follow-up — and such an agent that finishes its turn while one is pending does not end its step but continues its
//! session to read it, as for people's messages. A read-only, command, Review or Verify step never takes it: it waits
//! for the run's next step that does, when the work goes back to it, as it does in a paused, waiting or stopped run
//! until the run goes on. While the project's switch is off, none is delivered.
//!
//! A notice overlaps the run's changes when it names files both sides changed, or when a file the landed commits
//! changed is also one the agent's worktree changed since they forked, committed or not. One that does not is never
//! delivered: its step passes when its agent finishes, and before the run's next step that does not take base moves
//! starts in the run's worktree — the first Check, Review or Verify after the writing steps — the runner merges the
//! moved base there itself, with a merge commit as an agent would make, while no agent works in that worktree. Should
//! that merge conflict, it is undone and the notice keeps the files it conflicted in as its overlapping ones, so that
//! the next writing step's agent takes it. A newer move replaces a pending notice (the desktop), so pending moves
//! combine, and the combined one is delivered as soon as any of its files overlap.

use std::path::Path;

use orchester_core::{
  base_move::BaseNotice, id::StepExecutionId, permission::RepositoryAccess, revision::Revision, run::Invocation,
  time::Timestamp,
};

use super::{
  super::{
    checkpoint::{self, Merge},
    run::{ExecutionError, RunTask},
    workspace::Workspace,
  },
  Continuation,
};

impl RunTask {
  /// What `execution`, an agent step about to start in `workspace` on `judged`, meets of the run's moved base: the
  /// runner merges it first when the step does not take base moves ([`RunTask::merge_moved_base`]), else its agent
  /// takes the notice that overlaps the run's changes with `continuation`. The revision of the run's branch it starts
  /// on.
  pub(super) fn meet_moved_base(
    &mut self,
    execution: StepExecutionId,
    workspace: Result<&Workspace, &String>,
    continuation: &mut Continuation,
    judged: Option<Revision>,
  ) -> Result<Option<Revision>, ExecutionError> {
    let merged = self.merge_moved_base(execution, workspace.ok())?;
    let worktree = workspace.map_or_else(
      |_| self.worktree().to_path_buf(),
      |workspace| workspace.path().to_path_buf(),
    );
    continuation.notice = self.notice_for(execution, &worktree)?;
    Ok(merged.or(judged))
  }

  /// The notice `execution`'s agent, working in `worktree`, takes now: the run's pending one, past its window, when it
  /// overlaps the run's changes, its step takes base moves and the project tells its working agents when the base
  /// branch moves.
  pub(super) fn notice_for(
    &self,
    execution: StepExecutionId,
    worktree: &Path,
  ) -> Result<Option<BaseNotice>, ExecutionError> {
    if !self.takes_base_moves(execution) || !self.db.tells_base_moves()? {
      return Ok(None);
    }
    let notice = self.db.pending_base_notice(self.run.id())?;
    Ok(
      notice
        .filter(|notice| notice.deliverable(Timestamp::now()))
        .and_then(|notice| self.overlapping(notice, worktree)),
    )
  }

  /// `notice` with the files it overlaps in, when it overlaps the run's changes: those it names, else those both its
  /// landed commits and the work in `worktree` changed. When git cannot tell, it is taken as overlapping: an agent
  /// merging it is the safe side.
  fn overlapping(&self, mut notice: BaseNotice, worktree: &Path) -> Option<BaseNotice> {
    if !notice.overlapping.is_empty() {
      return Some(notice);
    }
    match checkpoint::overlap(worktree, &notice.revision) {
      Ok(files) if files.is_empty() => None,
      Ok(files) => {
        notice.overlapping = files;
        Some(notice)
      }
      Err(error) => {
        tracing::warn!(run = %self.run.id(), %error, "cannot tell whether the base move overlaps the run's changes");
        Some(notice)
      }
    }
  }

  /// Before `execution`, which works in `workspace`, starts: when its step does not take base moves and works in the
  /// run's worktree, while no other agent works there, merges the run's pending notice that overlaps none of the run's
  /// changes into it, with a merge commit recorded as `execution`'s checkpoint. The revision the run's branch is at
  /// then, once it merged; `None` when it merged nothing. A merge that conflicts is undone and leaves the notice to a
  /// writing step's agent; one git refuses is logged and tried again before the next such step.
  pub(in super::super) fn merge_moved_base(
    &mut self,
    execution: StepExecutionId,
    workspace: Option<&Workspace>,
  ) -> Result<Option<Revision>, ExecutionError> {
    let Some(Workspace::Run(worktree)) = workspace else {
      return Ok(None);
    };
    if self.takes_base_moves(execution)
      || self
        .agents
        .values()
        .any(|active| matches!(active.workspace, Workspace::Run(_)))
      || !self.db.tells_base_moves()?
    {
      return Ok(None);
    }
    let Some(notice) = self
      .db
      .pending_base_notice(self.run.id())?
      .filter(|notice| notice.deliverable(Timestamp::now()) && notice.overlapping.is_empty())
    else {
      return Ok(None);
    };
    let before = checkpoint::head(worktree).ok();
    let message = format!(
      "{}: merge the moved base {} (run {})",
      self.task.key(),
      notice.branch,
      self.run.number()
    );
    let merged = match checkpoint::merge_moved_base(worktree, &notice.revision, &message) {
      Ok(Merge::Merged(merged)) => merged,
      Ok(Merge::Conflict(files)) => {
        tracing::info!(run = %self.run.id(), revision = %notice.revision, ?files, "the moved base conflicts; an agent merges it");
        if self.db.note_base_notice_overlap(&notice, &files)? {
          self.announce_notice();
        }
        return Ok(None);
      }
      Err(error) => {
        tracing::warn!(run = %self.run.id(), revision = %notice.revision, %error, "the moved base could not be merged");
        return Ok(None);
      }
    };
    if before.as_ref() != Some(&merged) {
      self.checkpoint(execution, merged.clone())?;
    }
    if self
      .db
      .merge_base_notice(&notice, (execution, &merged), Timestamp::now())?
    {
      tracing::info!(run = %self.run.id(), %execution, revision = %notice.revision, %merged, "the runner merged the moved base");
      self.announce_notice();
    }
    Ok(Some(merged))
  }

  /// `notice` went to `execution`'s agent with `invocation`, which started with it, with the files it overlaps in as
  /// [`RunTask::notice_for`] found them. A newer move that replaced it meanwhile stays pending: the agent's next turn
  /// reads that one.
  pub(super) fn deliver_notice(
    &mut self,
    execution: StepExecutionId,
    invocation: &Invocation,
    notice: Option<BaseNotice>,
  ) -> Result<(), ExecutionError> {
    let Some(notice) = notice else {
      return Ok(());
    };
    let at = invocation.started_at;
    self.db.note_base_notice_overlap(&notice, &notice.overlapping)?;
    if self.db.deliver_base_notice(&notice, (execution, invocation.id), at)? {
      tracing::info!(run = %self.run.id(), %execution, revision = %notice.revision, "the base move was told");
      self.announce_notice();
    }
    Ok(())
  }

  /// Tells the desktop that a notice of the run changed.
  fn announce_notice(&self) {
    self.notifier.send(orchester_ipc::Event::RunChanged {
      run: self.run.id(),
      task: self.run.task(),
    });
  }

  /// Whether `execution`'s step is one the run's pipeline has merge the base when it moves, and its agent may write in
  /// its worktree, as the run gives its step's profile permissions.
  fn takes_base_moves(&self, execution: StepExecutionId) -> bool {
    let Ok((step, profile, _)) = self.agent_step(execution) else {
      return false;
    };
    if !self.run.snapshot().body().takes_base_moves(&step.key) {
      return false;
    }
    let Ok((body, _)) = self.profile_and_connection(profile, &step.key) else {
      return false;
    };
    self
      .run
      .step_permissions(&step.key, body.permissions())
      .is_some_and(|permissions| permissions.repository() == RepositoryAccess::Write)
  }
}
