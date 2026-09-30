//! Task memory around an agent step: the records its invocation's prompt gets, recalled as it
//! starts with the project's Memory setting of that moment, and the handoff a finished step leaves
//! as task memory. A handoff that cannot be kept (it holds a secret's value, or the keychain cannot
//! be read to check it) is reported in the transcript; the step goes on without it.

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::StepExecutionId,
  memory::{
    AgentProvenance, Handoff, MemoryFilter, MemoryQuery, MemoryStatus, MemoryStore, RecallRequest, Recalled,
    ScopeFilter, SecretScreen, recall,
  },
  resource::Step,
  run::{ProfileInfo, StepExecutor, StepResult},
  store::{ActivityStore, SecretStore},
  time::Timestamp,
};

use super::{super::run::RunTask, Active, command::problem};
use crate::execution::run::ExecutionError;

/// Matches fetched per record the prompt may get, so ranking has more than the best matches to
/// choose from.
const MATCHES_PER_RECORD: usize = 4;

impl RunTask {
  /// The records the next invocation of `step`, run by `profile`, gets: the task's active records,
  /// the project's pinned ones and those matching the step's and the task's text, within the
  /// project's limits.
  pub(super) fn recall_memory(&self, step: &Step, profile: ProfileInfo<'_>) -> Result<Recalled, RunnerProblem> {
    let failed = |error: orchester_store::StoreError| problem(format!("cannot read task memory: {error}"));
    let settings = self.db.memory_settings().map_err(failed)?;
    let active = |scope| MemoryFilter {
      scope,
      status: Some(MemoryStatus::Active),
      pinned_only: false,
    };
    let task = self.task.id();
    let mut candidates = self.db.memories(&active(ScopeFilter::Task(task))).map_err(failed)?;
    let pinned = MemoryFilter {
      pinned_only: true,
      ..active(ScopeFilter::Project)
    };
    candidates.extend(self.db.memories(&pinned).map_err(failed)?);
    let details = self.task.details();
    let text = [
      step.name.as_str(),
      step.instructions.as_str(),
      details.title.as_str(),
      &details.brief,
    ]
    .join("\n");
    let limit = usize::from(settings.limits.records.get()) * MATCHES_PER_RECORD;
    let matches = self
      .db
      .search_memories(task, &MemoryQuery::from_text(&text), limit)
      .map_err(failed)?;
    let request = RecallRequest {
      task,
      profile: profile.id,
      role: profile.body.role().as_str(),
      limits: settings.limits,
      now: Timestamp::now(),
    };
    Ok(recall(&request, candidates, matches))
  }

  /// Keeps the handoff of `execution`'s result, its agent's `final_message`, as task memory
  /// written by that agent, as the project's Memory setting keeps task memory now.
  pub(super) fn remember_handoff(
    &mut self,
    execution: StepExecutionId,
    active: &Active,
    final_message: Option<&str>,
  ) -> Result<(), ExecutionError> {
    let Some(handoff) = final_message
      .and_then(StepResult::parse)
      .and_then(|result| Handoff::from_result(&result))
    else {
      return Ok(());
    };
    let Some(provenance) = self.provenance(execution) else {
      return Err(ExecutionError::NotAnAgentStep(execution));
    };
    let settings = self.db.memory_settings()?;
    let secrets = self.db.secrets()?;
    let screen = match SecretScreen::load(self.project.id(), &secrets, self.vault.as_ref()) {
      Ok(screen) => screen,
      Err(error) => return self.not_remembered(active, &error),
    };
    let sources = self.final_message_entry(active)?;
    let records = match handoff.records(
      self.task.id(),
      &provenance,
      &sources,
      (&settings, &screen),
      Timestamp::now(),
    ) {
      Ok(records) => records,
      Err(error) => return self.not_remembered(active, &error),
    };
    self.db.add_memories(&records)?;
    Ok(())
  }

  fn not_remembered(&mut self, active: &Active, error: &dyn std::error::Error) -> Result<(), ExecutionError> {
    let problem = RunnerProblem::MemoryNotSaved {
      detail: error.to_string(),
    };
    self.report(&active.invocation, problem)
  }

  /// The agent that works on `execution`: its profile, run, step and execution.
  fn provenance(&self, execution: StepExecutionId) -> Option<AgentProvenance> {
    let step = self.run.execution(execution)?.step().clone();
    let StepExecutor::Agent { profile, .. } = self.run.snapshot().step(&step)?.executor() else {
      return None;
    };
    Some(AgentProvenance {
      profile: profile.resource,
      run: self.run.id(),
      step,
      execution,
    })
  }

  /// The transcript entry of the last message the agent of `active` wrote, its final message.
  fn final_message_entry(&self, active: &Active) -> Result<Vec<u64>, ExecutionError> {
    let entries = self.db.activity(self.run.id(), 0)?;
    Ok(
      entries
        .iter()
        .rev()
        .find(|entry| entry.invocation == active.invocation.id && matches!(entry.event, AgentEvent::Message { .. }))
        .map(|entry| entry.sequence)
        .into_iter()
        .collect(),
    )
  }
}
