//! The agent CLI command of an agent step, for the provider of its profile's connection (Codex or
//! Claude Code): the connection and settings, the worktree it works in, the variables the project
//! adds to its environment (read at each start), the MCP servers the profile chooses (`mcp`), and
//! the prompt assembled for its injection event: for a new session the whole prompt with the
//! instructions injected at start, the outputs of the steps it runs after (and, for Codex, the
//! result schema); for a session that resumes the continue prompt with the instructions injected at
//! resume. A gated step a reviewer sent back is told what should change instead (the plan
//! revision). The extra writable folders its permissions grant are resolved in the repository at
//! each start, and those under `.orchester/` that do not exist yet are created: the CLIs ignore a
//! folder that does not exist. The skills that reach the agent are placed and listed in its prompt, and the
//! project's repository agent files setting is read at each start ([`super::skills`]).

use std::{
  collections::BTreeSet,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::Command,
  connection::{
    AgentConnection, AgentProvider, REASONING_EFFORT,
    invocation::{ClaudeInvocation, ClaudeSession, CodexAgentFiles, CodexInvocation, InvocationError, Platform},
    stream::{AgentEvent, RunnerProblem},
  },
  id::{InvocationId, StepExecutionId},
  permission::{CommandPermission, FolderBounds, capability::EnforcedPermissions},
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
  Continuation, Occasion,
  app_server::AppServerSetup,
  claude_approval::ClaudeApprovals,
  mcp::{McpServers, McpSetup},
  skills::{PlacedSkills, repository_skills},
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
  pub(super) app_server: Option<AppServerSetup>,
  /// Claude Code's permission prompt, for a step that asks a person.
  pub(super) claude_approvals: Option<ClaudeApprovals>,
  pub(super) agent: String,
}

/// What decides the CLI's arguments besides its prompt and MCP servers.
struct Cli<'a> {
  connection: &'a AgentConnection,
  model: &'a ModelId,
  effort: Option<&'a str>,
  enforced: &'a EnforcedPermissions,
  writable_roots: &'a [PathBuf],
  worktree: &'a Path,
  resume: Option<&'a str>,
  environment: &'a CommandEnvironment,
  /// The folder of the skills the agent gets, if any.
  skills: Option<&'a Path>,
  /// The repository's skills Codex disables, when the project has its agent files off; `None` while
  /// they are on.
  repository_skills: Option<&'a [PathBuf]>,
  /// Claude Code's permission prompt configuration, when the step asks a person.
  approvals: Option<&'a Path>,
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
    let placed = self.place_skills(execution, step, info)?;
    let AssembledPrompt { text: prompt, sources } = self.prompt(execution, step, info, continuation, &placed)?;
    // Preflight already refused steps whose permissions the CLI cannot enforce; checked again
    // because an agent never starts with permissions that are not enforced. The run's permissions
    // for the profile, if they were changed for it, else the profile's, narrowed by the step.
    let enforced = self
      .run
      .step_permissions(key, body.permissions())
      .expect("an agent step of the run has permissions")
      .enforced_on(connection.provider(), Platform::current())
      .map_err(|error| problem(error.to_string()))?;
    let writable_roots = self.writable_roots(&enforced)?;
    let environment = self.environment(worktree)?;
    let provider = connection.provider();
    let agent_files = self
      .db
      .repository_agent_files()
      .map_err(|error| problem(error.to_string()))?;
    let repository = match (agent_files.is_on(), provider) {
      (false, AgentProvider::CodexCli) => Some(repository_skills(worktree)?),
      _ => None,
    };
    let McpServers { setup, left_out } = self.mcp_servers(&body, provider, (invocation, worktree, &environment))?;
    let claude_approvals = self.claude_approvals(enforced.permissions(), &setup, (invocation, worktree))?;
    let cli = Cli {
      connection: &connection,
      model,
      effort: body
        .settings()
        .iter()
        .find_map(|(name, value)| (name == REASONING_EFFORT).then_some(value)),
      enforced: &enforced,
      writable_roots: &writable_roots,
      worktree,
      resume: continuation.session.as_deref(),
      environment: &environment,
      skills: placed.folder.as_deref(),
      repository_skills: repository.as_deref(),
      approvals: claude_approvals.as_ref().map(ClaudeApprovals::config),
    };
    let interactive = enforced.permissions().asks_a_person();
    let (command, mcp_file, app_server) = if interactive && provider == AgentProvider::CodexCli {
      let (command, app) = self.app_server_command(&cli, setup, prompt)?;
      (command, None, Some(app))
    } else {
      let (command, file) = self.cli_command(&cli, setup, prompt)?;
      (command, file, None)
    };
    Ok(AgentStart {
      command,
      provider,
      prompt: sources,
      mcp_file,
      mcp_left_out: left_out,
      app_server,
      claude_approvals,
      agent: body.role().as_str().to_owned(),
    })
  }

  fn app_server_command(
    &self,
    cli: &Cli<'_>,
    setup: McpSetup,
    prompt: String,
  ) -> Result<(Command, AppServerSetup), RunnerProblem> {
    let McpSetup::Codex(mcp) = setup else {
      return Err(problem("interactive tool approvals require Codex".to_owned()));
    };
    let expected_servers = mcp.servers().iter().map(ToString::to_string).collect::<BTreeSet<_>>();
    let invocation = CodexInvocation {
      executable: cli.connection.executable(),
      model: cli.model.as_str(),
      reasoning_effort: cli.effort,
      access: cli.enforced.permissions().repository(),
      network: cli.enforced.permissions().network(),
      writable_roots: cli.writable_roots,
      worktree: cli.worktree,
      platform: Platform::current(),
      resume: cli.resume,
      output_schema: None,
      mcp: Some(&mcp),
      agent_files: cli
        .repository_skills
        .map_or(CodexAgentFiles::Loaded, |skills| CodexAgentFiles::LeftOut { skills }),
    };
    let command = invocation
      .app_server_command(std::env::vars_os(), cli.environment)
      .map_err(|error| problem(error.to_string()))?;
    let schema = (cli.resume.is_none()).then(|| {
      serde_json::from_str(orchester_core::run::STEP_RESULT_SCHEMA)
        .expect("the static step result schema is valid JSON")
    });
    Ok((
      command,
      AppServerSetup {
        prompt,
        model: cli.model.as_str().to_owned(),
        effort: cli.effort.map(str::to_owned),
        worktree: cli.worktree.to_path_buf(),
        access: cli.enforced.permissions().repository(),
        command_approvals: *cli.enforced.permissions().commands() == CommandPermission::AskBefore,
        tool_approvals: cli.enforced.permissions().interactive_tool_approvals(),
        network: cli.enforced.permissions().network(),
        writable_roots: cli.writable_roots.to_vec(),
        resume: cli.resume.map(str::to_owned),
        expected_servers,
        output_schema: schema,
        secrets: mcp.secrets().to_vec(),
      },
    ))
  }

  /// The extra writable folders `enforced` grants, resolved in the project's repository, the missing
  /// ones created; or why one cannot be given (it was moved, or now lies somewhere refused).
  fn writable_roots(&self, enforced: &EnforcedPermissions) -> Result<Vec<PathBuf>, RunnerProblem> {
    let Some(folders) = enforced
      .permissions()
      .granted_folders()
      .filter(|folders| !folders.is_empty())
    else {
      return Ok(Vec::new());
    };
    let bounds = FolderBounds::new(&self.root, std::env::home_dir().as_deref())
      .map_err(|error| problem(format!("cannot read the repository {}: {error}", self.root.display())))?;
    let resolved = folders
      .resolve(&bounds)
      .map_err(|(folder, error)| problem(format!("extra writable folder {folder}: {error}")))?;
    resolved
      .into_iter()
      .map(|folder| {
        if folder.is_missing() {
          std::fs::create_dir_all(folder.path()).map_err(|error| {
            problem(format!(
              "cannot create the extra writable folder {}: {error}",
              folder.path().display()
            ))
          })?;
        }
        Ok(folder.path().to_path_buf())
      })
      .collect()
  }

  /// The pinned profile version and the agent connection it runs on.
  pub(super) fn profile_and_connection(
    &self,
    profile: &PinnedVersion,
  ) -> Result<(AgentProfile, AgentConnection), RunnerProblem> {
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
        writable_roots: cli.writable_roots,
        worktree: cli.worktree,
        platform: Platform::current(),
        resume: cli.resume,
        output_schema: Some(&self.schema_path()),
        mcp: Some(mcp),
        agent_files: cli
          .repository_skills
          .map_or(CodexAgentFiles::Loaded, |skills| CodexAgentFiles::LeftOut { skills }),
      }
      .command(prompt, std::env::vars_os(), cli.environment),
      McpSetup::Claude(servers) => ClaudeInvocation {
        executable: cli.connection.executable(),
        model: cli.model.as_str(),
        effort: cli.effort,
        permissions: cli.enforced,
        writable_roots: cli.writable_roots,
        worktree: cli.worktree,
        session: ClaudeSession::continuing(cli.resume),
        mcp: servers
          .as_ref()
          .map(|servers| (servers.file.path(), servers.servers.as_slice())),
        skills: cli.skills,
        approvals: cli.approvals,
      }
      .command(prompt, std::env::vars_os(), cli.environment),
    };
    let command = command.map_err(|error: InvocationError| problem(error.to_string()))?;
    let file = match setup {
      McpSetup::Claude(servers) => servers.map(|servers| servers.file),
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
    placed: &PlacedSkills,
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
    let skills = placed.listings();
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
      skills: &skills,
      dependencies: &dependencies,
      revision: Plan::awaiting_revision(&plans, execution, continuation.last_started).map(|(plan, note)| {
        PlanRevision {
          version: plan.version,
          plan: &plan.text,
          note,
        }
      }),
      messages: (!continuation.messages.is_empty() || matches!(continuation.occasion, Occasion::Recovery(_))).then(
        || DeliveredMessages {
          occasion: continuation.occasion.as_prompt(),
          messages: &continuation.messages,
        },
      ),
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
