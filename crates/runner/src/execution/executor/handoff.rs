//! Keeping a step's handoff as task memory once the step's fate is known: when it passed or failed,
//! and for a gated step when its plan is approved ([`orchester_core::memory::Handoff`]). The handoff
//! is read from the agent's final message in the transcript. One that cannot be kept (it holds a
//! secret's value, or the keychain cannot be read to check it) is reported in the transcript; the
//! run goes on without it. The project's Memory setting is read through the coordinator link.

use orchester_core::{
  computer::ComputerError,
  connection::stream::{AgentEvent, MemoryNotSavedReason, RunnerProblem},
  id::StepExecutionId,
  memory::{AgentProvenance, Handoff, HandoffWrite, MemoryFilter, MemoryStore, ScopeFilter, StepFate},
  run::{Run, StepExecutor, StepResult},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ActivityRecord, ProjectDb, StoreError};
use thiserror::Error;

use super::{
  Notifier,
  link::{
    LinkError,
    memory::{MemoryLink, SettingsRequest},
  },
  sink::Sink,
};
use crate::computer::ComputerResources;

/// Where a handoff is kept and its problems are reported: the transcript through the coordinator's sink. The secrets it
/// is screened against are read on the run's computer.
pub(in crate::execution) struct Keeper<'a> {
  pub(in crate::execution) db: &'a mut ProjectDb,
  /// Where the project's Memory setting is read.
  pub(in crate::execution) link: &'a dyn MemoryLink,
  pub(in crate::execution) sink: &'a mut dyn Sink,
  pub(in crate::execution) notifier: &'a Notifier,
  pub(in crate::execution) computer: &'a dyn ComputerResources,
}

/// Why a handoff was not kept: the project database or the coordinator link failed, or the
/// computer's secrets could not be read.
#[derive(Debug, Error)]
enum KeepError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Link(#[from] LinkError),
  #[error(transparent)]
  Computer(#[from] ComputerError),
}

impl Keeper<'_> {
  /// Keeps the handoff of `execution` of `run`, which ended as `fate`. A follow-up leaves none: its
  /// answer is its reply in the conversation.
  /// [`Keeper::keep`], where the step goes on whatever happens: a handoff the database could not
  /// take is logged and reported in the transcript, and a report that fails as well is logged.
  pub(super) fn keep_or_report(&mut self, run: &Run, execution: StepExecutionId, fate: StepFate) {
    let Err(error) = self.keep(run, execution, fate) else {
      return;
    };
    tracing::error!(run = %run.id(), %execution, %error, "the step's handoff was not kept");
    let reason = MemoryNotSavedReason::StorageFailed {
      detail: error.to_string(),
    };
    if let Err(error) = self.report(run, execution, reason) {
      tracing::error!(run = %run.id(), %execution, %error, "the failed handoff could not be reported");
    }
  }

  fn keep(&mut self, run: &Run, execution: StepExecutionId, fate: StepFate) -> Result<(), KeepError> {
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
    let settings = self.link.settings(&SettingsRequest)?;
    let secrets = self.computer.secrets()?;
    let screen = match self.computer.secret_screen(self.db.project().id(), &secrets) {
      Ok(screen) => screen,
      Err(error) => {
        let reason = MemoryNotSavedReason::KeychainUnavailable {
          secret: error.secret.to_string(),
        };
        return Ok(self.report(run, execution, reason)?);
      }
    };
    let filter = MemoryFilter {
      scope: ScopeFilter::Task(run.task()),
      status: None,
      pinned_only: false,
    };
    let write = HandoffWrite {
      task: run.task(),
      provenance: &provenance,
      sources: &[entry],
      fate,
      settings: &settings,
      screen: &screen,
      now: Timestamp::now(),
    };
    // The task's records are read and changed in one transaction: no person's decision on them
    // comes between.
    match self
      .db
      .change_memories(&filter, |earlier| handoff.changes(&write, earlier))?
    {
      Ok(()) => Ok(()),
      Err(error) => {
        let reason = MemoryNotSavedReason::ContainsSecret {
          secret: error.secret.to_string(),
        };
        Ok(self.report(run, execution, reason)?)
      }
    }
  }

  /// Reports in `execution`'s transcript, under its latest invocation, why its handoff was not kept.
  fn report(&mut self, run: &Run, execution: StepExecutionId, reason: MemoryNotSavedReason) -> Result<(), StoreError> {
    let Some(invocation) = self
      .db
      .invocations(run.id())?
      .into_iter()
      .rev()
      .find(|invocation| invocation.execution == execution)
    else {
      return Ok(());
    };
    tracing::warn!(run = %run.id(), %execution, ?reason, "the step's handoff was not kept");
    let problem = RunnerProblem::MemoryNotSaved { reason };
    let events = vec![(Timestamp::now(), AgentEvent::RunnerProblem { problem })];
    let sequence = self.sink.activity(ActivityRecord::new(&invocation, events))?;
    self.notifier.send(Event::AgentActivity {
      run: run.id(),
      sequence,
    });
    Ok(())
  }
}

/// The agent that worked on `execution`: its profile, run, step and execution.
pub(super) fn provenance(run: &Run, execution: StepExecutionId) -> Option<AgentProvenance> {
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
