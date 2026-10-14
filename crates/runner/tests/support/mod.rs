//! Projects for the execution tests: a git repository with one commit, a signed-in Codex
//! connection running `orchester-fake-codex` (or a Claude Code one running `orchester-fake-claude`),
//! a published pipeline of agent steps and a queued task, and a scheduler whose notifications the
//! test reads.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

pub mod hold;
pub mod native;
pub mod qa;
pub mod tool_wait;
pub mod tools;
pub mod vault;

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  code_index::CodeIndexSetting,
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, REASONING_EFFORT,
    SignIn,
    mcp::{McpServer, McpServerSelection},
    stream::AgentEvent,
  },
  id::{ConnectionId, IdempotencyKey, ProjectId, RunId, TaskId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, StepPermissions},
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, FailureRoute, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind,
  },
  run::{PendingToolApproval, Run, RunStatus},
  secret::Secret,
  store::{
    ActivityStore, ConnectionStore, McpServerStore, ResourceStore, RunStore, SecretStore, SettingsStore, TaskStore,
  },
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_ipc::Event;
use orchester_runner::execution::{Notifier, QaSlots, Scheduler};
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::sync::mpsc;

use self::vault::MemoryVault;

pub const FAKE_CODEX: &str = env!("CARGO_BIN_EXE_orchester-fake-codex");
pub const FAKE_CLAUDE: &str = env!("CARGO_BIN_EXE_orchester-fake-claude");
pub const PATIENCE: Duration = Duration::from_secs(30);

pub fn git(dir: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}: {}",
    output.status,
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

/// A step of a test pipeline graph; what its agent does is in its instructions.
#[derive(Clone, Debug)]
pub struct StepSpec {
  pub key: &'static str,
  pub name: &'static str,
  pub instructions: &'static str,
  pub after: &'static [&'static str],
  /// Where a failure returns to, and how often.
  pub retry: Option<(&'static str, u32)>,
  pub budget: Option<u64>,
  /// Its result waits for a plan decision.
  pub gate: bool,
  /// A tool step running this command tool instead of an agent step.
  pub tool: Option<tools::ToolSpec>,
}

impl StepSpec {
  pub const fn new(
    key: &'static str,
    name: &'static str,
    instructions: &'static str,
    after: &'static [&'static str],
  ) -> Self {
    Self {
      key,
      name,
      instructions,
      after,
      retry: None,
      budget: None,
      gate: false,
      tool: None,
    }
  }

  pub const fn gate(mut self) -> Self {
    self.gate = true;
    self
  }

  /// A tool step running `tool`.
  pub const fn command(mut self, tool: tools::ToolSpec) -> Self {
    self.tool = Some(tool);
    self
  }

  pub const fn retry(mut self, to: &'static str, times: u32) -> Self {
    self.retry = Some((to, times));
    self
  }

  pub const fn budget(mut self, tokens: u64) -> Self {
    self.budget = Some(tokens);
    self
  }
}

/// What the test project runs.
pub struct Setup {
  /// The task's brief, which tells the fake CLI what to do.
  pub brief: &'static str,
  /// `false`: one step, Implement; `true`: Implement, then Review.
  pub review: bool,
  /// The task's token budget and time limit; `None` is no limit.
  pub budget: Option<u64>,
  pub max_duration: Option<Duration>,
  /// A pipeline graph instead of Implement (and Review).
  pub steps: Vec<StepSpec>,
  /// The CLI the Builder profile runs on.
  pub provider: AgentProvider,
  /// The Builder profile's model; `None` for the provider's usual test model.
  pub model: Option<&'static str>,
  /// The Builder profile's permissions; `None` for writing with the provider's usual commands.
  pub permissions: Option<AgentPermissions>,
  /// MCP servers of the project, all chosen by the Builder profile.
  pub mcp: Vec<McpServer>,
  /// Secrets of the project with their values in the keychain.
  pub secrets: Vec<(Secret, &'static str)>,
  /// Off unless a test is about the code index: what an agent is given then depends on where
  /// ripgrep is on the machine.
  pub code_index: CodeIndexSetting,
  /// The model API of a native connection, for [`AgentProvider::AnthropicApi`].
  pub native: Option<native::NativeApi>,
}

impl Setup {
  pub fn new(brief: &'static str) -> Self {
    Self {
      brief,
      review: false,
      steps: Vec::new(),
      provider: AgentProvider::CodexCli,
      model: None,
      permissions: None,
      mcp: Vec::new(),
      secrets: Vec::new(),
      code_index: CodeIndexSetting::Off,
      native: None,
      budget: Some(1_400_000),
      max_duration: Some(Duration::from_secs(30 * 60)),
    }
  }
}

pub struct TestProject {
  pub repo: TempDir,
  pub task: TaskId,
  pub project: ProjectId,
  /// The keychain with the values of the setup's secrets.
  pub vault: Arc<MemoryVault>,
}

pub fn project(setup: &Setup) -> TestProject {
  let repo = tempfile::tempdir().expect("temporary directory");
  project_in(setup, repo)
}

pub fn project_in(setup: &Setup, repo: TempDir) -> TestProject {
  let root = repo.path();
  git(root, &["init", "--quiet", "--initial-branch=main"]);
  git(root, &["config", "user.name", "Orchester Test"]);
  git(root, &["config", "user.email", "test@orchester.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  git(root, &["config", "commit.gpgsign", "false"]);
  fs::write(root.join("README.md"), "hi\n").expect("write readme");
  git(root, &["add", "README.md"]);
  git(root, &["commit", "--quiet", "-m", "initial"]);
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let id = project.id();
  let mut db = ProjectDb::create(root, project).expect("create project");
  let connection = match &setup.native {
    Some(api) => native::connection(api),
    None => agent_connection(setup.provider),
  };
  db.create_connection(&connection).expect("connection");
  let mut values = Vec::new();
  for (secret, value) in &setup.secrets {
    db.create_secret(secret).expect("secret");
    values.push((secret.vault_key(id), *value));
  }
  for server in &setup.mcp {
    db.create_mcp_server(server).expect("MCP server");
  }
  db.save_code_index(setup.code_index).expect("code index setting");
  let task = queued_task(&mut db, connection.id(), setup);
  TestProject {
    repo,
    task,
    project: id,
    vault: MemoryVault::with(values),
  }
}

/// A checked connection of `provider` to its fake CLI.
pub fn agent_connection_of(provider: AgentProvider) -> orchester_core::connection::Connection {
  agent_connection(provider)
}

fn agent_connection(provider: AgentProvider) -> orchester_core::connection::Connection {
  let (title, fake, version) = match provider {
    AgentProvider::CodexCli => ("Codex · Kontur", FAKE_CODEX, "0.139.0"),
    AgentProvider::ClaudeCodeCli => ("Claude · Kontur", FAKE_CLAUDE, "2.1.89"),
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => panic!("a native connection is made from its API"),
  };
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name(title),
    provider,
    PathBuf::from(fake),
    Timestamp::now(),
  )
  .expect("absolute path");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: version.into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("an agent check fits a CLI connection");
  connection.into_connection()
}

fn agent_step(key: &str, title: &str, builder: orchester_core::id::ResourceId) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: name(title),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Change README.md for {{task.id}}.").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn graph_step(db: &mut ProjectDb, spec: &StepSpec, builder: orchester_core::id::ResourceId) -> Step {
  let kind = match &spec.tool {
    Some(tool) => StepKind::Tool {
      tool: Some(tools::publish(db, spec.name, tool)),
    },
    None => StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
  };
  Step {
    key: StepKey::new(spec.key).expect("key"),
    name: name(spec.name),
    kind,
    instructions: PromptText::new(spec.instructions).expect("prompt"),
    depends_on: spec.after.iter().map(|key| StepKey::new(key).expect("key")).collect(),
    approval_gate: spec.gate,
    on_failure: spec.retry.map(|(to, times)| FailureRoute {
      return_to: StepKey::new(to).expect("key"),
      max_retries: std::num::NonZeroU32::new(times).expect("above zero"),
    }),
    budget: spec.budget.map(|tokens| TokenBudget::new(tokens).expect("above zero")),
  }
}

fn queued_task(db: &mut ProjectDb, connection: ConnectionId, setup: &Setup) -> TaskId {
  let now = Timestamp::now();
  let settings =
    ProviderSettings::new(BTreeMap::from([(REASONING_EFFORT.to_owned(), "low".to_owned())])).expect("settings");
  let profile = AgentProfile::new(
    name("Builder"),
    "Implements tasks.".into(),
    PromptText::new("Work carefully.").expect("prompt"),
    connection,
    ModelId::new(setup.model.unwrap_or(match setup.provider {
      AgentProvider::CodexCli => "gpt-5.5",
      AgentProvider::ClaudeCodeCli => "haiku",
      AgentProvider::AnthropicApi => native::MODEL,
      AgentProvider::OpenAiApi => panic!("the runner tests have no OpenAI agent"),
    }))
    .expect("model"),
    settings,
  )
  .with_permissions(
    setup
      .permissions
      .clone()
      .unwrap_or_else(|| AgentPermissions::new(RepositoryAccess::Write, commands(setup.provider), NetworkAccess::Off)),
  )
  .with_mcp_servers(McpServerSelection::new(setup.mcp.iter().map(McpServer::id).collect()).expect("servers"));
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(builder, now).expect("publish profile");
  let mut steps = vec![agent_step("implement", "Implement", builder)];
  if !setup.steps.is_empty() {
    steps = setup.steps.iter().map(|spec| graph_step(db, spec, builder)).collect();
  } else if setup.review {
    let mut review = agent_step("review", "Review", builder);
    review.depends_on = BTreeSet::from([StepKey::new("implement").expect("key")]);
    steps.push(review);
  }
  let pipeline = Pipeline::new(steps).expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, now)
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(delivery, now).expect("publish pipeline");
  let budget = setup.budget.map(|tokens| TokenBudget::new(tokens).expect("above zero"));
  let details = TaskDetails {
    title: name("Say hello"),
    brief: setup.brief.into(),
    acceptance_criteria: vec!["README.md ends with hello".into()],
    pipeline: Some(delivery),
    limits: TaskLimits::new(budget, setup.max_duration).expect("limits"),
  };
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], now)
    .expect("task")[0]
    .clone();
  task
    .queue(orchester_core::resource::VersionNumber::FIRST)
    .expect("complete task");
  db.save_task(&task).expect("save task").saved().expect("saved as read");
  task.id()
}

/// What the Builder may run: Codex confines any command to the worktree; Claude Code and a native
/// agent can only limit commands to a list, here git.
fn commands(provider: AgentProvider) -> CommandPermission {
  match provider {
    AgentProvider::CodexCli => CommandPermission::AnyInWorktree,
    AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi => CommandPermission::Allowed(
      AllowedCommands::new(vec![CommandPrefix::new("git").expect("valid prefix")]).expect("valid list"),
    ),
    AgentProvider::OpenAiApi => panic!("the runner tests have no OpenAI agent"),
  }
}

/// Queues a second task on the project's pipeline, with the same brief as the first.
pub fn queue_another_task(root: &Path) -> TaskId {
  let mut db = ProjectDb::open(root).expect("open");
  let first = db.tasks().expect("tasks").remove(0);
  let mut details = first.details().clone();
  details.title = name("Say hello again");
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("task")[0]
    .clone();
  task
    .queue(first.pipeline_version().expect("first task is pinned"))
    .expect("complete task");
  db.save_task(&task).expect("save task").saved().expect("saved as read");
  task.id()
}

pub fn scheduler(root: &Path) -> (Scheduler, mpsc::UnboundedReceiver<Event>) {
  scheduler_with(root, Arc::new(MemoryVault::default()))
}

/// A scheduler that reads secret values from `vault`. Its QA apps take QA slots of the project's own, in
/// `.orchester/qa-slots/`, never the computer's.
pub fn scheduler_with(root: &Path, vault: Arc<MemoryVault>) -> (Scheduler, mpsc::UnboundedReceiver<Event>) {
  let (sender, events) = mpsc::unbounded_channel();
  let relay = PathBuf::from(env!("CARGO_BIN_EXE_orchester-runner"));
  let slots = QaSlots::in_folder(&root.join(".orchester").join("qa-slots"));
  (
    Scheduler::new(root, Notifier::new(sender), vault, (Some(relay), slots)),
    events,
  )
}

/// Waits for the tasks currently executing runs to close their control receivers.
pub async fn wait_for_idle(scheduler: &Scheduler) {
  tokio::time::timeout(PATIENCE, scheduler.wait_for_current_tasks())
    .await
    .expect("the run tasks end in time");
  assert!(scheduler.active_runs().is_empty(), "the runner has no active runs");
}

/// Whether `run` no longer works on its own: it ended, or it waits for a person's decision.
pub fn settled(run: &Run) -> bool {
  run.status().is_terminal() || matches!(run.status(), RunStatus::WaitingForPerson(_))
}

/// The run once a notification shows it satisfying `done`.
pub async fn wait_for(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  run: RunId,
  done: impl Fn(&Run) -> bool,
) -> Run {
  let wait = async {
    loop {
      let current = ProjectDb::open(root)
        .expect("open")
        .run(run)
        .expect("read")
        .expect("run exists");
      if done(&current) {
        return current;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait)
    .await
    .expect("the run reaches the state in time")
}

/// Waits until the run has a tool approval whose invocation waits for it, and returns it.
pub async fn pending_approval(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  run: RunId,
) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(request) = db.pending_tool_approvals(run).expect("requests").first()
        && db
          .invocations(run)
          .expect("invocations")
          .iter()
          .any(|invocation| invocation.id == request.invocation && invocation.timing.waiting_since().is_some())
      {
        return request.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("an approval arrives")
}

/// Waits until the run's transcript holds `count` agent turns that started.
pub async fn wait_for_turns(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId, count: usize) {
  let db = ProjectDb::open(root).expect("open");
  let started = async {
    while db
      .activity(run, 0)
      .expect("activity")
      .iter()
      .filter(|entry| entry.event == AgentEvent::TurnStarted)
      .count()
      < count
    {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, started)
    .await
    .expect("the agent starts in time");
}

/// What the fake CLI reported on standard error after `prefix` (`"argv: "`, `"pid: "`), once
/// per invocation.
pub fn reported(db: &ProjectDb, run: RunId, prefix: &str) -> Vec<String> {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::Raw { line, .. } => line.strip_prefix(prefix).map(str::to_owned),
      _ => None,
    })
    .collect()
}

/// The parameters of each `thread/start` or `thread/resume` `orchester-fake-codex` was sent, in order.
pub fn threads(db: &ProjectDb, run: RunId) -> Vec<serde_json::Value> {
  reported(db, run, "thread: ")
    .iter()
    .map(|thread| serde_json::from_str(thread).expect("the fake reports JSON"))
    .collect()
}

/// Whether process `pid` still runs.
pub fn is_running(pid: &str) -> bool {
  let output = if cfg!(windows) {
    Command::new("tasklist")
      .args(["/FI", &format!("PID eq {pid}"), "/NH"])
      .output()
  } else {
    Command::new("kill").args(["-0", pid]).output()
  }
  .expect("ask the system about the process");
  if cfg!(windows) {
    String::from_utf8_lossy(&output.stdout).contains(pid)
  } else {
    output.status.success()
  }
}

/// Starts the task's run on a runner of its own and kills that runner once the agent works:
/// its tasks are dropped without recording anything, and its agents end with it, as when the
/// runner process is killed. The run stays recorded as running with an open invocation.
pub async fn killed_mid_run(root: &Path, task: TaskId) -> RunId {
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(2)
    .enable_all()
    .build()
    .expect("a runtime for the killed runner");
  let (scheduler, mut events) = scheduler(root);
  let starting = scheduler.clone();
  let run = runtime
    .spawn(async move { starting.start_run(IdempotencyKey::generate(), task).await })
    .await
    .expect("the runner accepts the request")
    .expect("the run starts");
  wait_for_turns(root, &mut events, run, 1).await;
  runtime.shutdown_background();
  run
}
