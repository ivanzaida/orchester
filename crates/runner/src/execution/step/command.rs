//! The Codex command of an agent step: its profile's connection and settings, the assembled prompt
//! for a new session, or the continue prompt for a session that resumes.

use orchester_core::{
  command::Command,
  connection::{
    REASONING_EFFORT,
    invocation::{CodexInvocation, Platform},
    stream::RunnerProblem,
  },
  id::{ResourceId, StepExecutionId},
  resource::{AgentProfile, Step, StepKind},
  run::{CONTINUE_PROMPT, PipelineInfo, PromptAssemblyError, PromptInput, StepExecutor},
  store::{ConnectionStore, ResourceStore},
};

use super::super::run::RunTask;

fn problem(detail: String) -> RunnerProblem {
  RunnerProblem::CouldNotStart { detail }
}

impl RunTask {
  /// The command that runs `execution`'s agent, continuing session `resume` when given, or why it
  /// cannot start.
  pub(super) fn command(&self, execution: StepExecutionId, resume: Option<&str>) -> Result<Command, RunnerProblem> {
    let key = self
      .run
      .execution(execution)
      .expect("the execution's model was found")
      .step();
    let step = self
      .run
      .snapshot()
      .body()
      .step(key)
      .expect("executions name snapshot steps");
    let (StepKind::Agent { repository, .. }, Some(StepExecutor::Agent { profile, model, .. })) =
      (&step.kind, self.run.snapshot().step(key).map(|step| step.executor()))
    else {
      return Err(problem(format!("step `{key}` is not an agent step")));
    };
    let body: AgentProfile = self
      .db
      .version::<AgentProfile>(profile.resource, profile.number)
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem(format!("agent profile version {} is missing", profile.number)))?
      .body()
      .clone();
    let connection = self
      .db
      .connection(body.connection())
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem("the agent profile's connection no longer exists".to_owned()))?;
    let prompt = match resume {
      Some(_) => CONTINUE_PROMPT.to_owned(),
      None => self.prompt(execution, step, profile.resource, &body)?,
    };
    let effort = body
      .settings()
      .iter()
      .find_map(|(name, value)| (name == REASONING_EFFORT).then_some(value));
    CodexInvocation {
      executable: connection.executable(),
      model: model.as_str(),
      reasoning_effort: effort,
      access: *repository,
      worktree: &self.worktree,
      platform: Platform::current(),
      resume,
    }
    .command(prompt, std::env::vars_os())
    .map_err(|error| problem(error.to_string()))
  }

  /// The prompt of a new session: role, step and task, with the run's variables filled in.
  fn prompt(
    &self,
    execution: StepExecutionId,
    step: &Step,
    profile_id: ResourceId,
    profile: &AgentProfile,
  ) -> Result<String, RunnerProblem> {
    let pipeline = self.run.snapshot().pipeline();
    let pipeline_name = self
      .db
      .resource(pipeline.resource)
      .map_err(|error| problem(error.to_string()))?
      .map(|resource| resource.name().as_str().to_owned())
      .ok_or_else(|| problem("the run's pipeline no longer exists".to_owned()))?;
    PromptInput {
      project: &self.project,
      task: &self.task,
      run: &self.run,
      execution,
      pipeline: PipelineInfo {
        id: pipeline.resource,
        name: &pipeline_name,
        version: pipeline.number,
      },
      step,
      profile_id,
      profile,
    }
    .assemble()
    .map_err(
      |PromptAssemblyError::UnavailableVariable { variable, .. }| RunnerProblem::UnavailableVariable {
        variable: variable.name().to_owned(),
      },
    )
  }
}
