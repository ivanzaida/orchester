//! Side questions about a run answered for the desktop ([`orchester_ipc::Command::AnswerSideQuestion`]): by a separate
//! agent on the Orchestrator's connection and model, beside the runs, taking no agent slot and touching no run: it
//! reads only what the desktop gave it and sends nothing to any agent. A person's *Stop* ends it.

use std::collections::HashMap;

use orchester_core::{
  id::SideQuestionId,
  run::side_question::{SideFailure, SideInput, SideOutcome},
};
use parking_lot::Mutex;
use tokio::sync::watch;

use super::{Scheduler, step::worker::answer_side};

/// The side questions being answered, each with what stops its agent.
#[derive(Default)]
pub(super) struct SideAgents {
  answering: Mutex<HashMap<SideQuestionId, watch::Sender<()>>>,
}

impl Scheduler {
  /// Answers side question `question` from `input`; how it ended, answered or not.
  pub async fn answer_side_question(&self, question: SideQuestionId, input: SideInput) -> SideOutcome {
    if self.inner.board.is_stopping() {
      return SideOutcome::Failed {
        failure: SideFailure::Interrupted,
      };
    }
    let (stop, stopped) = watch::channel(());
    self.inner.side_agents.answering.lock().insert(question, stop);
    let outcome = answer_side(
      self.inner.project_root.clone(),
      self.inner.vault.clone(),
      input,
      stopped,
    )
    .await;
    self.inner.side_agents.answering.lock().remove(&question);
    outcome
  }

  /// Stops the side agent answering `question`, if one is.
  pub fn stop_side_question(&self, question: SideQuestionId) {
    let answering = self.inner.side_agents.answering.lock();
    if let Some(stop) = answering.get(&question)
      && stop.send(()).is_err()
    {
      tracing::debug!(%question, "the side agent had ended before it was stopped");
    }
  }
}
