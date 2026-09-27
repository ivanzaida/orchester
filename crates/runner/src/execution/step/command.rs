//! The agent CLI command of an agent step, for the provider of its profile's connection (Codex or
//! Claude Code): the connection and settings, the worktree it works in, and the prompt assembled for
//! its injection event: for a new session the whole prompt with the instructions injected at start,
//! the outputs of the steps it runs after (and, for Codex, the result schema); for a session that
//! resumes the continue prompt with the instructions injected at resume.

use std::path::Path;

use orchester_core::{
  command::Command,
  connection::{
    AgentConnection, AgentProvider, REASONING_EFFORT,
    invocation::{ClaudeInvocation, ClaudeSession, CodexInvocation, InvocationError, Platform},
    stream::RunnerProblem,
  },
  id::StepExecutionId,
  resource::{AgentProfile, InjectionEvent, Instruction, Step, StepKind},
  run::{
    AssembledPrompt, DependencyOutput, InstructionSource, InstructionVersion, InvocationPrompt, PinnedVersion,
    PipelineInfo, ProfileInfo, PromptAssemblyError, PromptInput, RunContext, StepExecutor,
  },
  store::{ActivityStore, ConnectionStore, ResourceStore},
};

use super::super::run::RunTask;

fn problem(detail: String) -> RunnerProblem {
  RunnerProblem::CouldNotStart { detail }
}

impl RunTask {
  /// The command that runs `execution`'s agent, the CLI it starts and what its prompt is made of,
  /// continuing session `resume` when given, or why it cannot start.
  pub(super) fn command(
    &self,
    execution: StepExecutionId,
    resume: Option<&str>,
    worktree: &Path,
  ) -> Result<(Command, AgentProvider, InvocationPrompt), RunnerProblem> {
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
    let (StepKind::Agent { .. }, Some(StepExecutor::Agent { profile, model, .. })) =
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
    let event = if resume.is_some() {
      InjectionEvent::Resume
    } else {
      InjectionEvent::Start
    };
    let AssembledPrompt { text: prompt, sources } = self.prompt(execution, step, *profile, &body, event)?;
    // Preflight already refused steps whose permissions the CLI cannot enforce; checked again
    // because an agent never starts with permissions that are not enforced. The run's permissions
    // for the profile, if they were changed for it, else the profile's, narrowed by the step.
    let enforced = self
      .run
      .step_permissions(key, body.permissions())
      .expect("an agent step of the run has permissions")
      .enforced_on(connection.provider(), Platform::current())
      .map_err(|error| problem(error.to_string()))?;
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
        access: enforced.permissions().repository(),
        network: enforced.permissions().network(),
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
        permissions: &enforced,
        worktree,
        session: ClaudeSession::continuing(resume),
      }
      .command(prompt, std::env::vars_os()),
    };
    command
      .map(|command| (command, provider, sources))
      .map_err(|error: InvocationError| problem(error.to_string()))
  }

  /// The prompt at `event`: role, instructions, step and task with the run's variables filled in,
  /// then the outputs of the passed executions of the steps it depends on.
  fn prompt(
    &self,
    execution: StepExecutionId,
    step: &Step,
    profile: PinnedVersion,
    body: &AgentProfile,
    event: InjectionEvent,
  ) -> Result<AssembledPrompt, RunnerProblem> {
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
    let instructions = self.pinned_instructions()?;
    let sources: Vec<InstructionSource<'_>> = instructions
      .iter()
      .map(|(pin, name, body)| InstructionSource {
        resource: pin.resource,
        name,
        version: InstructionVersion::Published { number: pin.number },
        body,
      })
      .collect();
    let attempt = self
      .run
      .attempts()
      .iter()
      .find(|attempt| attempt.execution(execution).is_some())
      .map(|attempt| attempt.number());
    PromptInput {
      project: &self.project,
      task: &self.task,
      run: Some(RunContext {
        run: &self.run,
        attempt,
      }),
      pipeline: PipelineInfo {
        id: pipeline.resource,
        name: &pipeline_name,
        version: pipeline.number,
      },
      step,
      profile: ProfileInfo {
        id: profile.resource,
        version: profile.number,
        body,
      },
      instructions: &sources,
      dependencies: &dependencies,
    }
    .assemble(event)
    .map_err(
      |PromptAssemblyError::UnavailableVariable { variable, .. }| RunnerProblem::UnavailableVariable {
        variable: variable.name().to_owned(),
      },
    )
  }

  /// The instruction versions the run pinned, with their names, in library (slug) order.
  fn pinned_instructions(&self) -> Result<Vec<(PinnedVersion, String, Instruction)>, RunnerProblem> {
    let mut pinned = Vec::new();
    for pin in self.run.snapshot().instructions() {
      let resource = self
        .db
        .resource(pin.resource)
        .map_err(|error| problem(error.to_string()))?
        .ok_or_else(|| problem(format!("instruction {} no longer exists", pin.resource)))?;
      let version = self
        .db
        .version::<Instruction>(pin.resource, pin.number)
        .map_err(|error| problem(error.to_string()))?
        .ok_or_else(|| problem(format!("instruction version {} is missing", pin.number)))?;
      pinned.push((
        resource.slug().clone(),
        (*pin, resource.name().as_str().to_owned(), version.body().clone()),
      ));
    }
    pinned.sort_by(|(left, _), (right, _)| left.cmp(right));
    Ok(pinned.into_iter().map(|(_, instruction)| instruction).collect())
  }
}
