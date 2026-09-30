//! Keeping a step's handoff as task memory once the step's fate is known: when it passed or failed,
//! and for a gated step when its plan is approved ([`orchester_core::memory::Handoff`]). The handoff
//! is read from the agent's final message in the transcript. One that cannot be kept (it holds a
//! secret's value, or the keychain cannot be read to check it) is reported in the transcript; the
//! run goes on without it.

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::StepExecutionId,
  memory::{AgentProvenance, Handoff, HandoffWrite, MemoryFilter, MemoryStore, ScopeFilter, SecretScreen, StepFate},
  run::{Run, StepExecutor, StepResult},
  secret::SecretVault,
  store::{ActivityStore, SecretStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ProjectDb, StoreError};

use super::Notifier;

/// Where a handoff is kept and its problems are reported.
pub(super) struct Keeper<'a> {
  pub(super) db: &'a mut ProjectDb,
  pub(super) notifier: &'a Notifier,
  pub(super) vault: &'a dyn SecretVault,
}

impl Keeper<'_> {
  /// Keeps the handoff of `execution` of `run`, which ended as `fate`. A follow-up leaves none: its
  /// answer is its reply in the conversation.
  pub(super) fn keep(&mut self, run: &Run, execution: StepExecutionId, fate: StepFate) -> Result<(), StoreError> {
    if run.is_follow_up(execution) {
      return Ok(());
    }
    let Some((entry, message)) = self.db.last_agent_message(run.id(), execution)? else {
      return Ok(());
    };
    let Some(handoff) = StepResult::parse(&message).and_then(|result| Handoff::from_result(&result)) else {
      return Ok(());
    };
    let Some(provenance) = provenance(run, execution) else {
      return Ok(());
    };
    let settings = self.db.memory_settings()?;
    let secrets = self.db.secrets()?;
    let screen = match SecretScreen::load(self.db.project().id(), &secrets, self.vault) {
      Ok(screen) => screen,
      Err(error) => return self.report(run, execution, &error),
    };
    let filter = MemoryFilter {
      scope: ScopeFilter::Task(run.task()),
      status: None,
      pinned_only: false,
    };
    let earlier = self.db.memories(&filter)?;
    let write = HandoffWrite {
      task: run.task(),
      provenance: &provenance,
      sources: &[entry],
      fate,
      settings: &settings,
      screen: &screen,
      now: Timestamp::now(),
    };
    match handoff.changes(&write, earlier) {
      Ok(changes) => self.db.commit_memories(&changes),
      Err(error) => self.report(run, execution, &error),
    }
  }

  /// Reports in `execution`'s transcript, under its latest invocation, why its handoff was not kept.
  fn report(&mut self, run: &Run, execution: StepExecutionId, error: &dyn std::error::Error) -> Result<(), StoreError> {
    let Some(invocation) = self
      .db
      .invocations(run.id())?
      .into_iter()
      .rev()
      .find(|invocation| invocation.execution == execution)
    else {
      return Ok(());
    };
    tracing::warn!(run = %run.id(), %execution, %error, "the step's handoff was not kept");
    let problem = RunnerProblem::MemoryNotSaved {
      detail: error.to_string(),
    };
    let events = [(Timestamp::now(), AgentEvent::RunnerProblem { problem })];
    let sequence = self.db.append_activity(&invocation, &events)?;
    self.notifier.send(Event::AgentActivity {
      run: run.id(),
      sequence,
    });
    Ok(())
  }
}

/// The agent that worked on `execution`: its profile, run, step and execution.
fn provenance(run: &Run, execution: StepExecutionId) -> Option<AgentProvenance> {
  let step = run.execution(execution)?.step().clone();
  let StepExecutor::Agent { profile, .. } = run.snapshot().step(&step)?.executor() else {
    return None;
  };
  Some(AgentProvenance {
    profile: profile.resource,
    run: run.id(),
    step,
    execution,
  })
}
