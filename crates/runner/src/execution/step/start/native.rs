//! How a native agent's invocation starts (`docs/architecture/native-agent.md` §2.2, §5, §6): the API key
//! read from the keychain for this invocation only, the file scope and command policy its permissions
//! give, the shell its commands go to, and its conversation. A new step execution starts a conversation
//! whose system prompt and tools stay the same for every later invocation; a resumed one continues the
//! stored conversation with the person's answer or the resume prompt. While the project lets agents read
//! the repository's own agent files, its `AGENTS.md` joins the system prompt and its skills are listed
//! with Orchester's (`AgentFilesMechanism::OrchesterLoads`).

use std::{collections::BTreeSet, fs, io, num::NonZeroU32, path::Path, sync::Arc};

use orchester_agent::{AnthropicClient, Endpoint, FileTools, WireQuirks};
use orchester_core::{
  agent_files::{AgentFilesMechanism, skills::native::native_skills},
  code_index::RipgrepFallback,
  command::SecretVariable,
  connection::{
    AgentConnection, ApiEndpoint, Effort, NativeModel,
    invocation::{Platform, agent_environment},
    mcp::McpServerProblem,
    stream::{AgentEvent, RunnerProblem},
  },
  id::{ConversationId, InvocationId, StepExecutionId},
  native::{
    command::CommandPolicy,
    scope::{FileScope, ScopeRoots},
    session::{Continuation as Resumed, LoopAction, LoopSettings, NativeLoop},
    store::{NativeConversation, NativeConversationStore},
    tools::ToolSet,
  },
  permission::{RepositoryAccess, capability::EnforcedPermissions},
  resource::{AgentProfile, ModelId},
  run::{InvocationEngine, InvocationPrompt, NativeSetting, ResultDelivery},
  secret::SecretValue,
  store::{ActivityStore, McpServerStore, SettingsStore},
  time::Timestamp,
};
use orchester_exec::ripgrep;
use orchester_store::ProjectDb;

use super::{
  super::{
    super::{codebase_index::IndexTold, connection_check, run::RunTask},
    Continuation, Occasion,
    native::{CommandHost, NativeSession},
    skills::PlacedSkill,
  },
  AgentIndex, AgentStart, Launch, NATIVE_NOT_AVAILABLE, PromptParts, StepAgent, problem, reasoning_effort,
};

/// The response cap a request asks for when the connection lists none for its model; the client lowers
/// it to what the model reports (§3.1).
const DEFAULT_MAX_OUTPUT: u32 = 64_000;

/// What a native invocation's start is made of before its conversation.
struct Prepared {
  /// Whether the step may edit files, which decides a new conversation's tools.
  access: RepositoryAccess,
  scope: FileScope,
  policy: CommandPolicy,
  commands: CommandHost,
  /// The ripgrep `list_files` and `search` run; `None` walks the folders.
  ripgrep: Option<std::path::PathBuf>,
  left_out: Vec<AgentEvent>,
}

impl RunTask {
  /// How invocation `invocation` of `execution`'s native agent starts in `worktree`, continuing as
  /// `continuation` says, with what it gets of the code index (`index`); or why it cannot start.
  pub(super) fn native_start(
    &self,
    execution: StepExecutionId,
    continuation: &Continuation,
    (invocation, worktree): (InvocationId, &Path),
    index: AgentIndex,
    agent: StepAgent<'_>,
  ) -> Result<AgentStart, RunnerProblem> {
    let (step, model, body, connection) = (agent.step, agent.model, agent.body, agent.connection);
    let provider = connection.provider();
    let api = match connection.api() {
      Some(api) if provider.runs_agents() => api,
      _ => return Err(problem(NATIVE_NOT_AVAILABLE.to_owned())),
    };
    let key = self.api_key(api)?;
    let info = agent.info();
    let mut placed = self.place_skills(execution, step, info)?;
    let skill = self.place_code_map_for(&index, execution, &mut placed)?;
    let instructions = self.repository_files(connection, worktree, &mut placed.skills)?;
    let enforced = self.enforced(&step.key, body, provider)?;
    let mut prepared = self.prepare_native(&enforced, worktree, &index, placed.folder.as_deref())?;
    let told = IndexTold::new(&index, skill.as_deref(), provider);
    let recalled = self.recall_memory(step, info)?;
    let parts = PromptParts {
      execution,
      worktree,
      step,
      profile: info,
      continuation,
      placed: &placed,
      code_index: told.prompt(),
      memory: recalled.entries(),
      // A native agent's own prompt names its tools.
      working: None,
    };
    let effort = api.models().effort_for(model, reasoning_effort(body));
    let settings = loop_settings(api, model, effort, &prepared);
    let started = match continuation.session.as_deref() {
      None => self.new_conversation(
        parts,
        (execution, connection),
        settings,
        instructions.as_deref(),
        &prepared,
      )?,
      Some(session) => self.resumed_conversation(parts, (execution, session), settings, &prepared)?,
    };
    let mut left_out = std::mem::take(&mut prepared.left_out);
    left_out.extend(index.problems(provider));
    left_out.extend(self.mcp_left_out(body)?);
    let (session, sources) = self.session(api, Arc::clone(&key), (invocation, prepared), started)?;
    Ok(AgentStart {
      launch: Launch::Native(Box::new(session)),
      prompt: sources,
      left_out,
      code_index: index.used,
      secrets: vec![SecretVariable::new("API key", key)],
      agent: body.role().as_str().to_owned(),
      account: None,
      memory: recalled.injected(),
      permissions: enforced.launch(),
      engine: Some(InvocationEngine {
        endpoint: api.base_url().clone(),
        vendor: connection.vendor(),
        effort,
      }),
    })
  }

  /// The task of invocation `invocation`'s conversation `started`, with the model client of `api`, and
  /// where its prompt came from.
  fn session(
    &self,
    api: &ApiEndpoint,
    key: Arc<SecretValue>,
    (invocation, prepared): (InvocationId, Prepared),
    started: Started,
  ) -> Result<(NativeSession, InvocationPrompt), RunnerProblem> {
    let endpoint = Endpoint::new(api.base_url().clone(), key, WireQuirks::anthropic(api))
      .with_provider_fallback(api.provider_fallback());
    let client = AnthropicClient::new(endpoint).map_err(|error| problem(error.to_string()))?;
    let session = NativeSession {
      root: self.root.clone(),
      conversation: started.conversation,
      invocation,
      client,
      files: Arc::new(FileTools::new(prepared.scope, prepared.ripgrep)),
      commands: Arc::new(prepared.commands),
      native: started.native,
      first: started.first,
    };
    Ok((session, started.sources))
  }

  /// The connection's API key, read from the keychain now; an archived or missing secret, or one the
  /// keychain does not give, keeps the agent from starting.
  fn api_key(&self, api: &ApiEndpoint) -> Result<Arc<SecretValue>, RunnerProblem> {
    connection_check::api_key(&self.db, self.vault.as_ref(), api.key()).map_err(problem)
  }

  /// The file scope, command policy and command host of the agent's permissions, and what it works
  /// without: ripgrep, when it was found neither for the code index nor on its own.
  fn prepare_native(
    &self,
    enforced: &EnforcedPermissions,
    worktree: &Path,
    index: &AgentIndex,
    skills: Option<&Path>,
  ) -> Result<Prepared, RunnerProblem> {
    let folders = self.writable_folders(enforced)?;
    let scope = FileScope::new(&ScopeRoots {
      worktree,
      access: enforced.permissions().repository(),
      code_index: index.folder.as_deref(),
      skills,
      writable_folders: &folders,
    })
    .map_err(|error| problem(error.to_string()))?;
    let policy = CommandPolicy::new(enforced.permissions().commands()).map_err(|error| problem(error.to_string()))?;
    let mut environment = agent_environment(std::env::vars_os());
    self.environment()?.add_to(&mut environment);
    let tools = match &index.tools {
      Some(tools) => Some(tools.clone()),
      None => self.ripgrep_folder()?,
    };
    let mut left_out = Vec::new();
    if tools.is_none() && index.folder.is_none() {
      left_out.push(AgentEvent::RunnerProblem {
        problem: RunnerProblem::RipgrepNotFound {
          fallback: RipgrepFallback::OwnTools,
        },
      });
    }
    if let Some(tools) = &tools {
      prepend_path(&mut environment, tools);
    }
    Ok(Prepared {
      access: enforced.permissions().repository(),
      scope,
      policy,
      commands: CommandHost::new(environment, worktree),
      ripgrep: tools.map(|tools| tools.join(ripgrep::PROGRAM)),
      left_out,
    })
  }

  /// The repository's `AGENTS.md`, and its skills added to `skills`, while the project lets agents read
  /// them; nothing while it does not.
  fn repository_files(
    &self,
    connection: &AgentConnection,
    worktree: &Path,
    skills: &mut Vec<PlacedSkill>,
  ) -> Result<Option<String>, RunnerProblem> {
    let setting = self
      .db
      .repository_agent_files()
      .map_err(|error| problem(error.to_string()))?;
    if setting.mechanism(connection.provider()) != AgentFilesMechanism::OrchesterLoads {
      return Ok(None);
    }
    let found = native_skills(worktree).map_err(|error| problem(error.to_string()))?;
    skills.extend(found.into_iter().map(|skill| PlacedSkill {
      name: skill.name,
      description: skill.description,
      path: skill.path,
      files: Vec::new(),
    }));
    match fs::read_to_string(worktree.join("AGENTS.md")) {
      Ok(text) => Ok(Some(text)),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(problem(format!("cannot read AGENTS.md: {error}"))),
    }
  }

  /// A new conversation with the step's prompt, stored before the agent starts.
  fn new_conversation(
    &self,
    parts: PromptParts<'_>,
    (execution, connection): (StepExecutionId, &AgentConnection),
    mut settings: LoopSettings,
    instructions: Option<&str>,
    prepared: &Prepared,
  ) -> Result<Started, RunnerProblem> {
    let setting = NativeSetting {
      platform: Platform::current(),
      shell: settings.shell,
      repository_instructions: instructions,
    };
    let prompt = self.with_prompt(parts, |input, _| input.assemble_native(setting))?;
    settings.system = prompt.system;
    let conversation = NativeConversation {
      id: settings.conversation,
      run: self.run.id(),
      execution,
      connection: connection.id(),
      wire: orchester_core::connection::WireApi::AnthropicMessages,
      model: settings.model.clone(),
      system: settings.system.clone(),
      access: prepared.access,
      created_at: Timestamp::now(),
    };
    ProjectDb::open(&self.root)
      .and_then(|mut db| db.create_native_conversation(&conversation))
      .map_err(|error| problem(format!("cannot store the conversation: {error}")))?;
    let (native, first) = NativeLoop::start(settings, prepared.scope.clone(), prepared.policy.clone(), prompt.task);
    Ok(Started {
      native,
      first,
      conversation: conversation.id,
      sources: prompt.sources,
    })
  }

  /// The stored conversation `session` continued with the resume prompt, or with the person's answer.
  fn resumed_conversation(
    &self,
    parts: PromptParts<'_>,
    (execution, session): (StepExecutionId, &str),
    mut settings: LoopSettings,
    prepared: &Prepared,
  ) -> Result<Started, RunnerProblem> {
    let id: ConversationId = session
      .parse()
      .map_err(|_| problem(format!("the step's session {session} is no native conversation")))?;
    let stored = self
      .db
      .native_conversation(id)
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem(format!("the native conversation {session} is not stored")))?;
    let items = self.db.native_items(id).map_err(|error| problem(error.to_string()))?;
    let prompt = self.with_prompt(parts, |input, event| input.assemble_for(event, ResultDelivery::Tool))?;
    let resumed = match parts.continuation.occasion {
      Occasion::Answer(_) => Resumed::Answer(prompt.text),
      _ => Resumed::Prompt(prompt.text),
    };
    settings.conversation = id;
    settings.model = stored.model.clone();
    settings.system = stored.system.clone();
    settings.tools = stored.tools();
    let changed = self.changed_files(execution)?;
    let (native, first) = NativeLoop::resume(
      settings,
      prepared.scope.clone(),
      prepared.policy.clone(),
      items,
      resumed,
      changed,
    )
    .map_err(|error| problem(error.to_string()))?;
    Ok(Started {
      native,
      first,
      conversation: id,
      sources: prompt.sources,
    })
  }

  /// The files the agent of `execution` changed in its earlier invocations, as Activity recorded them.
  fn changed_files(&self, execution: StepExecutionId) -> Result<BTreeSet<String>, RunnerProblem> {
    let activity = self
      .db
      .activity(self.run.id(), 0)
      .map_err(|error| problem(error.to_string()))?;
    Ok(
      activity
        .into_iter()
        .filter(|entry| entry.execution == execution)
        .flat_map(|entry| match entry.event {
          AgentEvent::FileChanged { changes, .. } => changes.into_iter().map(|change| change.path).collect(),
          _ => Vec::new(),
        })
        .collect(),
    )
  }
}

/// A native conversation started or resumed.
struct Started {
  native: NativeLoop,
  first: Vec<LoopAction>,
  conversation: ConversationId,
  sources: InvocationPrompt,
}

/// What stays the same for a new conversation on `api`'s `model` with `effort`; its system prompt is
/// filled in once the prompt is assembled, a resumed conversation's from what was stored.
fn loop_settings(api: &ApiEndpoint, model: &ModelId, effort: Option<Effort>, prepared: &Prepared) -> LoopSettings {
  LoopSettings {
    conversation: ConversationId::generate(),
    model: model.clone(),
    effort: effort.map(|effort| effort.as_str().to_owned()),
    max_output: api
      .models()
      .model(model)
      .and_then(NativeModel::max_output)
      .map_or(DEFAULT_MAX_OUTPUT, NonZeroU32::get),
    context_window: api.models().context_window(model),
    system: String::new(),
    tools: ToolSet::for_access(prepared.access),
    shell: prepared.commands.shell(),
  }
}

impl RunTask {
  /// The MCP servers the profile chooses, which a native agent works without until the runner reaches
  /// them for it (§4.9, P4).
  fn mcp_left_out(&self, body: &AgentProfile) -> Result<Vec<AgentEvent>, RunnerProblem> {
    let ids = body.mcp_servers().ids();
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    let servers = self.db.mcp_servers().map_err(|error| problem(error.to_string()))?;
    Ok(
      ids
        .iter()
        .map(|id| AgentEvent::McpServer {
          server: servers
            .iter()
            .find(|server| server.id() == *id)
            .map(|server| server.name().to_string()),
          problem: McpServerProblem::NotSupportedByConnection,
        })
        .collect(),
    )
  }
}

/// `folder` first on the `PATH` of `environment`.
fn prepend_path(environment: &mut Vec<(String, String)>, folder: &Path) {
  let current = environment
    .iter()
    .rev()
    .find(|(name, _)| name.eq_ignore_ascii_case("PATH"))
    .map(|(_, value)| std::ffi::OsString::from(value));
  let mut folders = vec![folder.to_path_buf()];
  folders.extend(current.iter().flat_map(std::env::split_paths));
  match std::env::join_paths(folders) {
    Ok(path) => {
      environment.retain(|(name, _)| !name.eq_ignore_ascii_case("PATH"));
      environment.push(("PATH".to_owned(), path.to_string_lossy().into_owned()));
    }
    Err(error) => tracing::warn!(%error, folder = %folder.display(), "ripgrep's folder cannot go on the agent's PATH"),
  }
}
