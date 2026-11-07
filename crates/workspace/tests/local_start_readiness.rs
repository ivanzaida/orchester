//! A local `run_task` on a pipeline that is not ready on this computer (ORC-827): it refuses `not_ready_here` with a
//! message per missing part, queues, claims and starts nothing, and counts no attempt; once the part is there the same
//! task starts as it did before (here it reaches the runner, which these tests do not have).
//!
//! `cargo test -p orchester-workspace --test local_start_readiness local_start`

use std::{
  collections::BTreeSet,
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  },
  id::{ConnectionId, IdempotencyKey, ProjectId},
  name::Name,
  permission::{AgentPermissions, StepPermissions},
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, CommandProgram, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step,
    StepKey, StepKind,
  },
  store::{ConnectionStore, McpServerStore, ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits, TaskStatus},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, Translate},
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::runtime::Runtime;

fn name(text: &str) -> Name {
  Name::new(text).expect("name")
}

fn git(root: &Path, args: &[&str]) {
  let status = Command::new("git")
    .args(args)
    .current_dir(root)
    .status()
    .expect("git runs");
  assert!(status.success(), "git {args:?}");
}

/// A project named Kontur whose one draft runs a pipeline of one agent step, on a profile that chooses a local MCP
/// server starting `program`.
struct Kontur {
  dir: TempDir,
  services: app_mcp::Services,
  runtime: Runtime,
  _data: TempDir,
}

impl Kontur {
  fn new(program: &Path) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-827-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("local-start-")
      .tempdir_in(&scratch)
      .expect("repo");
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Local start test"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
    fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
    fs::write(root.join("README.md"), "Kontur\n").expect("readme");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(root, project).expect("project");
    let pipeline = published_pipeline(&mut db, root, program);
    let details = TaskDetails {
      title: name("Retry with jitter"),
      brief: "Back off with jitter.".to_owned(),
      acceptance_criteria: vec!["Retries are spread out.".to_owned()],
      pipeline: Some(pipeline),
      limits: TaskLimits::NONE,
    };
    db.create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
      .expect("the draft");
    drop(db);
    let data = tempfile::Builder::new()
      .prefix("local-start-data-")
      .tempdir_in(&scratch)
      .expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    workspace.open(root).expect("register the project");
    let runtime = Runtime::new().expect("runtime");
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let servers = Servers::new(workspace.clone(), Vault::keychain(), version);
    let runners = Runners::new(
      root.join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let services = app_mcp::Services::new(
      workspace,
      runners,
      servers,
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    Self {
      dir,
      services,
      runtime,
      _data: data,
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.dir.path()).expect("open")
  }

  fn run_task(&self) -> Value {
    let render = |message: &orchester_workspace::messages::Message| message.key.to_owned();
    let translate = Translate::new(render);
    let Value::Object(args) = json!({ "project": "Kontur", "key": "KON-1" }) else {
      unreachable!("an object");
    };
    self
      .runtime
      .block_on(app_mcp::call(&self.services, &translate, "run_task", args))
      .expect("a tool")
      .expect_err("this computer has no runner, so a start never succeeds here")
      .to_json(render)
  }

  /// What the project holds of the task: its status and how many runs it has.
  fn standing(&self) -> (TaskStatus, usize) {
    let db = self.db();
    let task = db.tasks().expect("tasks").into_iter().next().expect("the task");
    (task.status(), db.runs_of_task(task.id()).expect("runs").len())
  }
}

/// A pipeline of one agent step on a checked Codex connection, whose profile chooses a local MCP server that
/// starts `program`.
fn published_pipeline(db: &mut ProjectDb, root: &Path, program: &Path) -> orchester_core::id::ResourceId {
  let at = Timestamp::from_unix_millis;
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Codex"),
    AgentProvider::CodexCli,
    root.join("codex"),
    at(1),
  )
  .expect("connection");
  let model = ModelId::new("sonnet").expect("model");
  connection
    .record_check(ConnectionCheck {
      at: at(2),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(
          CatalogSource::Provider,
          vec![CatalogModel::new(model.clone(), Vec::new())],
        ),
      },
    })
    .expect("check");
  db.create_connection(&connection).expect("a connection");
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("knowledge").expect("server name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(program.to_str().expect("utf-8")).expect("program"),
      vec![],
      None,
    )),
    McpEnvironment::default(),
    at(3),
  )
  .expect("server");
  db.create_mcp_server(&server).expect("stored");
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    connection.id(),
    model,
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::before_permissions())
  .with_mcp_servers(McpServerSelection::new(vec![server.id()]).expect("one server"));
  let profile_id = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(4))
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(profile_id, at(5)).expect("published");
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("build").expect("key"),
    name: name("Build"),
    kind: StepKind::Agent {
      profile: Some(profile_id),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("pipeline");
  let resource = db
    .create_resource(
      Slug::new("quick-fix").expect("slug"),
      name("Quick fix"),
      &pipeline,
      at(6),
    )
    .expect("pipeline");
  db.publish_draft::<Pipeline>(resource.id(), at(7)).expect("published");
  resource.id()
}

#[test]
fn local_start_a_program_that_is_gone_refuses_not_ready_here_and_starts_nothing() {
  let outside = tempfile::tempdir().expect("a folder outside the repository");
  let program = outside.path().join("knowledge.sh");
  let kontur = Kontur::new(&program);

  let refused = kontur.run_task();
  assert_eq!(refused["error"]["code"], "not_ready_here", "{refused}");
  assert_eq!(
    refused["error"]["reasons"],
    json!(["runs.notReadyHere.lead", "runs.notReadyHere.localProgram.notFound"])
  );
  assert_eq!(
    kontur.standing(),
    (TaskStatus::Draft, 0),
    "the draft was not queued and no run was made"
  );
  assert_eq!(
    kontur.run_task()["error"]["code"],
    "not_ready_here",
    "asked again, still refused"
  );

  fs::write(&program, "#!/bin/sh\n").expect("the program appears");
  let started = kontur.run_task();
  assert_eq!(
    started["error"]["code"], "runner_unavailable",
    "ready, the start goes on to the runner as before: {started}"
  );
  assert_eq!(kontur.standing().0, TaskStatus::Queued);
}
