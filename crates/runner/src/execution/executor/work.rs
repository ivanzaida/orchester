//! How an executing run describes its current work on the [`super::board::Board`]: task, step,
//! agent role, provider and model, read from the run's pinned configuration.

use orchester_core::{
  connection::ConnectionKind,
  id::StepExecutionId,
  resource::AgentProfile,
  run::StepExecutor,
  store::{ConnectionStore, ResourceStore},
};
use orchester_ipc::{RunWork, WorkState};

use super::run::{ExecutionError, RunTask};

impl RunTask {
  /// Tells the board which ready agent steps of the run wait for an agent slot, after [`RunTask::start_ready`] said
  /// whether anything could not start (`waiting_for_slot`). A free slot means something else held it back (a busy
  /// workspace, no command slot); a run waiting for its workspace would not start with more slots either, and a tool
  /// step waits for a command slot, which more agent slots would not give it.
  pub(super) fn report_slot_waits(&self, waiting_for_slot: bool) {
    let waiting = if waiting_for_slot && !self.waits_for_workspace && !self.board.has_free_slot() {
      let ready = self.run.executions_to_start(usize::MAX).into_iter();
      ready.filter(|execution| !self.is_tool_step(*execution)).collect()
    } else {
      Vec::new()
    };
    self.board.waiting_for_slot(self.run.id(), waiting);
  }

  /// The work of `execution`, an agent step of the run; `None` when its agent profile version
  /// or connection is gone, which fails the step when it starts.
  pub(super) fn work(&self, execution: StepExecutionId) -> Result<Option<RunWork>, ExecutionError> {
    let key = self
      .run
      .execution(execution)
      .ok_or(ExecutionError::NotAnAgentStep(execution))?
      .step();
    let step = self
      .run
      .snapshot()
      .body()
      .step(key)
      .ok_or(ExecutionError::NotAnAgentStep(execution))?;
    let Some(StepExecutor::Agent { profile, model, .. }) = self.run.snapshot().step(key).map(|step| step.executor())
    else {
      return Err(ExecutionError::NotAnAgentStep(execution));
    };
    let Some(version) = self.db.version::<AgentProfile>(profile.resource, profile.number)? else {
      return Ok(None);
    };
    let connection = self.db.connection(version.body().connection())?;
    let Some(ConnectionKind::Agent(provider)) = connection.as_ref().map(|connection| connection.kind()) else {
      return Ok(None);
    };
    Ok(Some(RunWork {
      run: self.run.id(),
      task: self.task.id(),
      task_key: self.task.key().to_string(),
      task_title: self.task.details().title.as_str().to_owned(),
      step: step.name.as_str().to_owned(),
      agent: version.body().role().as_str().to_owned(),
      provider: *provider,
      model: model.as_str().to_owned(),
      state: WorkState::Working,
    }))
  }
}
