//! The agent CLI command of an agent step, for the provider of its profile's connection (Codex or
//! Claude Code): the connection and settings, the worktree it works in, the variables the project
//! adds to its environment (read at each start), the MCP servers the profile chooses (`mcp`), and
//! the prompt assembled for its injection event: for a new session the whole prompt with the
//! instructions injected at start, the outputs of the steps it runs after (and, for Codex, the
//! result schema); for a session that resumes the continue prompt with the instructions injected at
//! resume. A gated step a reviewer sent back is told what should change instead (the plan
//! revision).

use std::path::Path;

use orchester_core::{
  command::Command,
  connection::{
    AgentConnection, AgentProvider, REASONING_EFFORT,
    invocation::{ClaudeInvocation, ClaudeSession, CodexInvocation, InvocationError, Platform},
    stream::{AgentEvent, RunnerProblem},
  },
  id::{InvocationId, StepExecutionId},
  permission::capability::EnforcedPermissions,
  resource::{AgentProfile, CommandEnvironment, InjectionEvent, Instruction, ModelId, Step, StepKind},
  run::{
    AssembledPrompt, DeliveredMessages, DependencyOutput, InstructionSource, InstructionVersion, InvocationPrompt,
    PinnedVersion, PipelineInfo, Plan, PlanRevision, ProfileInfo, PromptAssemblyError, PromptInput, Run, RunContext,
    StepExecutor, StepOutput,
  },
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore, SettingsStore},
};

use super::{
  super::run::RunTask,
  Continuation,
  mcp::{McpServers, McpSetup},
};
use crate::private_file::PrivateFile;

fn problem(detail: String) -> RunnerProblem {
  RunnerProblem::CouldNotStart { detail }
}

/// How an agent invocation starts.
pub(super) struct AgentStart {
  pub(super) command: Command,
  pub(super) provider: AgentProvider,
  pub(super) prompt: InvocationPrompt,
  /// Claude Code's MCP configuration file, removed when this is dropped: once the CLI has ended.
  pub(super) mcp_file: Option<PrivateFile>,
  /// An event for each MCP server the agent works without.
  pub(super) mcp_left_out: Vec<AgentEvent>,
}

/// What decides the CLI's arguments besides its prompt and MCP servers.
struct Cli<'a> {
  connection: &'a AgentConnection,
  model: &'a ModelId,
  effort: Option<&'a str>,
  enforced: &'a EnforcedPermissions,
  worktree: &'a Path,
  resume: Option<&'a str>,
  environment: &'a CommandEnvironment,
}

impl RunTask {
  /// How invocation `invocation` of `execution`'s agent starts: its command, the CLI it starts,
  /// what its prompt is made of and its MCP servers, continuing as `continuation` says, or why it
  /// cannot start.
  pub(super) fn command(
    &self,
    execution: StepExecutionId,
    continuation: &Continuation,
    (invocation, worktree): (InvocationId, &Path),
  ) -> Result<AgentStart, RunnerProblem> {
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
    let (body, connection) = self.profile_and_connection(profile)?;
    let info = ProfileInfo {
      id: profile.resource,
      version: profile.number,
      body: &body,
    };
    let AssembledPrompt { text: prompt, sources } = self.prompt(execution, step, info, continuation)?;
    // Preflight already refused steps whose permissions the CLI cannot enforce; checked again
    // because an agent never starts with permissions that are not enforced. The run's permissions
    // for the profile, if they were changed for it, else the profile's, narrowed by the step.
    let enforced = self
      .run
      .step_permissions(key, body.permissions())
      .expect("an agent step of the run has permissions")
      .enforced_on(connection.provider(), Platform::current())
      .map_err(|error| problem(error.to_string()))?;
    let environment = self
      .db
      .project_environment()
      .map_err(|error| problem(error.to_string()))?;
    let provider = connection.provider();
    let McpServers { setup, left_out } = self.mcp_servers(&body, provider, (invocation, worktree, &environment))?;
    let cli = Cli {
      connection: &connection,
      model,
      effort: body
        .settings()
        .iter()
        .find_map(|(name, value)| (name == REASONING_EFFORT).then_some(value)),
      enforced: &enforced,
      worktree,
      resume: continuation.session.as_deref(),
      environment: &environment,
    };
    let (command, mcp_file) = self.cli_command(&cli, setup, prompt)?;
    Ok(AgentStart {
      command,
      provider,
      prompt: sources,
      mcp_file,
      mcp_left_out: left_out,
    })
  }

  /// The pinned profile version and the agent connection it runs on.
  fn profile_and_connection(&self, profile: &PinnedVersion) -> Result<(AgentProfile, AgentConnection), RunnerProblem> {
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
    Ok((body, connection))
  }

  /// The CLI's command with `prompt` on its standard input and the MCP servers of `setup`, and the
  /// configuration file it reads them from, if any.
  fn cli_command(
    &self,
    cli: &Cli<'_>,
    setup: McpSetup,
    prompt: String,
  ) -> Result<(Command, Option<PrivateFile>), RunnerProblem> {
    let command = match &setup {
      McpSetup::Codex(mcp) => CodexInvocation {
        executable: cli.connection.executable(),
        model: cli.model.as_str(),
        reasoning_effort: cli.effort,
        access: cli.enforced.permissions().repository(),
        network: cli.enforced.permissions().network(),
        worktree: cli.worktree,
        platform: Platform::current(),
        resume: cli.resume,
        output_schema: Some(&self.schema_path()),
        mcp: Some(mcp),
      }
      .command(prompt, std::env::vars_os(), cli.environment),
      McpSetup::Claude(file) => ClaudeInvocation {
        executable: cli.connection.executable(),
        model: cli.model.as_str(),
        effort: cli.effort,
        permissions: cli.enforced,
        worktree: cli.worktree,
        session: ClaudeSession::continuing(cli.resume),
        mcp: file.as_ref().map(|(file, servers)| (file.path(), servers.as_slice())),
      }
      .command(prompt, std::env::vars_os(), cli.environment),
    };
    let command = command.map_err(|error: InvocationError| problem(error.to_string()))?;
    let file = match setup {
      McpSetup::Claude(file) => file.map(|(file, _)| file),
      McpSetup::Codex(_) => None,
    };
    Ok((command, file))
  }

  /// The prompt of `execution`'s next invocation: role, instructions, step and task with the run's
  /// variables filled in, then the outputs of the passed executions of the steps it depends on (a
  /// gated one's approved plan), and what a reviewer asked to change in its own plan, if the
  /// invocation revises it.
  fn prompt(
    &self,
    execution: StepExecutionId,
    step: &Step,
    profile: ProfileInfo<'_>,
    continuation: &Continuation,
  ) -> Result<AssembledPrompt, RunnerProblem> {
    let event = if continuation.session.is_some() {
      InjectionEvent::Resume
    } else {
      InjectionEvent::Start
    };
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
    let plans = self
      .db
      .plans(self.run.id())
      .map_err(|error| problem(error.to_string()))?;
    let dependencies = dependencies(&self.run, step, &outputs, &plans);
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
      profile,
      instructions: &sources,
      dependencies: &dependencies,
      revision: Plan::awaiting_revision(&plans, execution, continuation.last_started).map(|(plan, note)| {
        PlanRevision {
          version: plan.version,
          plan: &plan.text,
          note,
        }
      }),
      messages: (!continuation.messages.is_empty()).then(|| DeliveredMessages {
        occasion: continuation.occasion.as_prompt(),
        messages: &continuation.messages,
      }),
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

/// The outputs of the passed executions of the steps `step` depends on, in pipeline order: a gated
/// step's is the plan that was approved, its latest; any other's what it left for the steps after
/// it.
fn dependencies<'a>(
  run: &'a Run,
  step: &Step,
  outputs: &'a [StepOutput],
  plans: &'a [Plan],
) -> Vec<DependencyOutput<'a>> {
  run
    .snapshot()
    .body()
    .steps()
    .iter()
    .filter(|dependency| step.depends_on.contains(&dependency.key))
    .filter_map(|dependency| {
      let passed = run.latest_execution(&dependency.key)?;
      let attempt = run
        .attempts()
        .iter()
        .find(|attempt| attempt.execution(passed.id()).is_some())?;
      let text = Plan::output_of(passed.id(), outputs, plans).unwrap_or_default();
      Some(DependencyOutput {
        step: dependency,
        attempt: attempt.number(),
        text,
      })
    })
    .collect()
}
