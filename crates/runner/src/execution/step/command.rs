//! The agent CLI command of an agent step, for the provider of its profile's connection (Codex or
//! Claude Code): the connection and settings, the worktree it works in, the assembled prompt with the
//! outputs of the steps it runs after (and, for Codex, the result schema) for a new session, or the
//! continue prompt for a session that resumes.

use std::path::Path;

use orchester_core::{
  command::Command,
  connection::{
    AgentConnection, AgentProvider, REASONING_EFFORT,
    invocation::{ClaudeInvocation, ClaudeSession, CodexInvocation, InvocationError, Platform},
    stream::RunnerProblem,
  },
  id::{ResourceId, StepExecutionId},
  resource::{AgentProfile, Step, StepKind},
  run::{CONTINUE_PROMPT, DependencyOutput, PipelineInfo, PromptAssemblyError, PromptInput, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore},
};

use super::super::run::RunTask;

fn problem(detail: String) -> RunnerProblem {
  RunnerProblem::CouldNotStart { detail }
}

impl RunTask {
  /// The command that runs `execution`'s agent and the CLI it starts, continuing session `resume`
  /// when given, or why it cannot start.
  pub(super) fn command(
    &self,
    execution: StepExecutionId,
    resume: Option<&str>,
    worktree: &Path,
  ) -> Result<(Command, AgentProvider), RunnerProblem> {
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
    let connection =
      AgentConnection::try_from(connection).map_err(|_| problem("the connection is not an agent CLI".to_owned()))?;
    let prompt = match resume {
      Some(_) => CONTINUE_PROMPT.to_owned(),
      None => self.prompt(execution, step, profile.resource, &body)?,
    };
    let effort = body
      .settings()
      .iter()
      .find_map(|(name, value)| (name == REASONING_EFFORT).then_some(value));
    let provider = connection.provider();
    let command = match provider {
      AgentProvider::CodexCli => CodexInvocation {
        executable: connection.executable(),
        model: model.as_str(),
        reasoning_effort: effort,
        access: *repository,
        worktree,
        platform: Platform::current(),
        resume,
        output_schema: Some(&self.schema_path()),
      }
      .command(prompt, std::env::vars_os()),
      AgentProvider::ClaudeCodeCli => ClaudeInvocation {
        executable: connection.executable(),
        model: model.as_str(),
        effort,
        access: *repository,
        worktree,
        session: ClaudeSession::continuing(resume),
      }
      .command(prompt, std::env::vars_os()),
    };
    command
      .map(|command| (command, provider))
      .map_err(|error: InvocationError| problem(error.to_string()))
  }

  /// The prompt of a new session: role, step and task, with the run's variables filled in, then the
  /// outputs of the passed executions of the steps it depends on.
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
    let outputs = self
      .db
      .step_outputs(self.run.id())
      .map_err(|error| problem(error.to_string()))?;
    let dependencies: Vec<DependencyOutput<'_>> = self
      .run
      .snapshot()
      .body()
      .steps()
      .iter()
      .filter(|dependency| step.depends_on.contains(&dependency.key))
      .filter_map(|dependency| {
        let passed = self.run.latest_execution(&dependency.key)?;
        let attempt = self
          .run
          .attempts()
          .iter()
          .find(|attempt| attempt.execution(passed.id()).is_some())?;
        let text = outputs
          .iter()
          .find(|output| output.execution == passed.id())
          .map_or("", |output| output.text.as_str());
        Some(DependencyOutput {
          step: dependency,
          attempt: attempt.number(),
          text,
        })
      })
      .collect();
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
      dependencies: &dependencies,
    }
    .assemble()
    .map_err(
      |PromptAssemblyError::UnavailableVariable { variable, .. }| RunnerProblem::UnavailableVariable {
        variable: variable.name().to_owned(),
      },
    )
  }
}
