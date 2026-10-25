//! *Stop turn* (ORC-92, `docs/design/orchester-workspace.md`, "Conversation (merged with Activity)"): a person ends
//! the turn of the agents at work in a run, as Esc does in Claude Code, without pausing the run. Each agent's CLI is
//! ended at once (a tool call under way is cut short with it), its work is committed as a checkpoint, the conversation
//! shows the stopped turn, and its step stays open: it continues its session with the messages queued for its agent,
//! which are its next instruction, or waits for the person's next one as it waits after a question. A tool request
//! that waited for a decision ends undecided with the turn, as it does when a run is cancelled. The run's usage, its
//! limits and its state go on as for any other invocation that ended.

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::WaitReason,
};
use orchester_ipc::Rejection;

use super::{
  Scheduler,
  control::{Ack, Control, answer},
  run::{ExecutionError, RunTask},
  step::{
    Active,
    worker::{Stop, StopCause},
  },
};

impl Scheduler {
  /// Stops the turn of the agents at work in `run`. Returns once the stop is asked; a run no task executes has no
  /// agent at work.
  pub async fn stop_turn(&self, run: RunId) -> Result<(), Rejection> {
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      let Some(answer) = self.ask(run, Control::StopTurn) else {
        return Err(Rejection::NoTurnToStop);
      };
      drop(one_at_a_time);
      if let Ok(result) = answer.await {
        return result;
      }
    }
  }
}

impl RunTask {
  /// Ends the turn of each agent of an agent CLI at work, unless the run is stopping anyway.
  pub(super) fn stop_turn(&mut self, ack: Ack) {
    if self.run.pause_requested() || !self.may_start() {
      return answer(ack, Err(Rejection::NoTurnToStop));
    }
    // An agent's turn: a command tool has none, and one being ended already ends anyway.
    let agents: Vec<&Active> = self
      .agents
      .values()
      .filter(|active| !active.abandoned && active.steers.is_some())
      .collect();
    if agents.is_empty() {
      return answer(ack, Err(Rejection::NoTurnToStop));
    }
    if agents.iter().any(|active| !active.turn_stoppable) {
      return answer(ack, Err(Rejection::TurnNotStoppable));
    }
    for active in agents {
      tracing::info!(run = %self.run.id(), execution = %active.invocation.execution, "a person stopped the agent's turn");
      active.tell(Stop::Now(StopCause::TurnStopped));
    }
    answer(ack, Ok(()));
  }

  /// `execution`'s agent ended because a person stopped its turn: its work is committed, and its step goes on with the
  /// messages queued for it, or waits for the person's next one.
  pub(super) fn turn_stopped(&mut self, execution: StepExecutionId, active: &Active) -> Result<(), ExecutionError> {
    self.save(execution, active, "saved when its turn was stopped")?;
    if self.queued_for(execution)?.is_empty() {
      self.transition(|run, now| run.wait_step(execution, WaitReason::Message, now))?;
    }
    Ok(())
  }
}
