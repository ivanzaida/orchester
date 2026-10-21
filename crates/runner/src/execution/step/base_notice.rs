//! Notices that the run's base branch moved (ORC-54, `orchester_core::base_move`), which the desktop queues: the
//! run's next agent of a step that implements the change takes its pending notice once the window of its move has
//! passed, never mid-edit. Such a step is one the pipeline has take base moves (ORC-65,
//! `orchester_core::resource::Pipeline::takes_base_moves`: its own choice, else a writing step that does not review
//! or verify) and whose agent writes in this run (repository access `write`). An invocation that starts takes it into
//! its prompt — a step's start, a session continued after a pause, a limit, a restart or a follow-up — and such an
//! agent that finishes its turn while one is pending does not end its step but continues its session to read it, as
//! for people's messages. A read-only, command, Review or Verify step never takes it: it waits for the run's next step
//! that does, when the work goes back to it, as it does in a paused, waiting or stopped run until the run goes on.
//! While the project's switch is off, none is delivered.

use orchester_core::{
  base_move::BaseNotice, id::StepExecutionId, permission::RepositoryAccess, run::Invocation, time::Timestamp,
};

use super::super::run::{ExecutionError, RunTask};

impl RunTask {
  /// The notice `execution`'s agent takes now: the run's pending one, past its window, when its step takes base moves
  /// and the project tells its working agents when the base branch moves.
  pub(super) fn notice_for(&self, execution: StepExecutionId) -> Result<Option<BaseNotice>, ExecutionError> {
    if !self.takes_base_moves(execution) || !self.db.tells_base_moves()? {
      return Ok(None);
    }
    let notice = self.db.pending_base_notice(self.run.id())?;
    Ok(notice.filter(|notice| notice.deliverable(Timestamp::now())))
  }

  /// `notice` went to `execution`'s agent with `invocation`, which started with it. A newer move that replaced it
  /// meanwhile stays pending: the agent's next turn reads that one.
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
    if self.db.deliver_base_notice(&notice, (execution, invocation.id), at)? {
      tracing::info!(run = %self.run.id(), %execution, revision = %notice.revision, "the base move was told");
      self.notifier.send(orchester_ipc::Event::RunChanged {
        run: self.run.id(),
        task: self.run.task(),
      });
    }
    Ok(())
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
