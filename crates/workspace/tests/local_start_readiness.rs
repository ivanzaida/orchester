//! A local `run_task` on a pipeline that is not ready on this computer (ORC-827): it refuses `not_ready_here` with a
//! message per missing part, queues, claims and starts nothing, and counts no attempt; once the part is there the same
//! task starts as it did before (here it reaches the runner, which these tests do not have).
//!
//! `cargo test -p orchester-workspace --test local_start_readiness local_start`

#[path = "local_start_readiness/follows.rs"]
mod follows;

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  },
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId},
  name::Name,
  orchestrator::proposal::StartBasis,
  permission::{AgentPermissions, StepPermissions},
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, CommandProgram, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step,
    StepKey, StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ConnectionStore, McpServerStore, ResourceStore, RunStore, TaskStore},
  task::{
    TaskDetails, TaskLimits, TaskStatus,
    organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, Translate},
  readiness::NotStarted,
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::{OpenProject, Workspace},
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
  workspace: Workspace,
  services: app_mcp::Services,
  /// What a message or a Fix CI follows through, as the window has them.
  follow: (Runners, Servers, OpenProject),
  runtime: Runtime,
  published: Published,
  _data: TempDir,
}

/// The pipeline the draft runs, published, with the snapshot a run of it holds.
struct Published {
  pipeline: ResourceId,
  snapshot: RunSnapshot,
}

impl Kontur {
  fn new(program: &Path) -> Self {
    Self::with_brief(program, "Back off with jitter.")
  }

  fn with_brief(program: &Path, brief: &str) -> Self {
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
    let published = published_pipeline(&mut db, root, program);
    let pipeline = published.pipeline;
    let details = TaskDetails {
      title: name("Retry with jitter"),
      brief: brief.to_owned(),
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
    let open = workspace.open(root).expect("register the project");
    let runtime = Runtime::new().expect("runtime");
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let servers = Servers::new(workspace.clone(), Vault::keychain(), version);
    let runners = Runners::new(
      root.join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let follow = (runners.clone(), servers.clone(), open);
    let services = app_mcp::Services::new(
      workspace.clone(),
      runners,
      servers,
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    Self {
      dir,
      workspace,
      services,
      follow,
      runtime,
      published,
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

  fn task(&self) -> orchester_core::task::Task {
    self.db().tasks().expect("tasks").into_iter().next().expect("the task")
  }

  /// What the project holds of the task: its status, its revision and how many runs it has.
  fn standing(&self) -> (TaskStatus, u64, usize) {
    let (db, task) = (self.db(), self.task());
    (
      task.status(),
      task.revision().get(),
      db.runs_of_task(task.id()).expect("runs").len(),
    )
  }

  /// The task as Run queues it before its first run.
  fn queue(&self) {
    let mut db = self.db();
    let mut task = self.task();
    task
      .queue(self.published.snapshot.pipeline().number)
      .expect("a complete draft queues");
    db.save_task(&task).expect("save").saved().expect("no conflict");
  }

  /// The task queued, with a first run that started and was cancelled: it runs again.
  fn end_a_run(&self) {
    let mut run = self.start_a_run();
    let events = run.cancel(Timestamp::from_unix_millis(30)).expect("cancelled");
    self.db().append_events(&run, &events).expect("recorded");
  }

  /// The task queued, with a first run at its step: the task holds an active run.
  fn start_a_run(&self) -> orchester_core::run::Run {
    self.queue();
    let (mut db, at) = (self.db(), Timestamp::from_unix_millis);
    let new = NewRun {
      task: self.task().id(),
      snapshot: self.published.snapshot.clone(),
      branch: BranchName::new("orchester/orc-run-1").expect("branch"),
      base: Revision::new(&"a".repeat(40)).expect("revision"),
      base_branch: None,
      base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
      budget: None,
      max_duration: None,
    };
    let mut run = db
      .create_run(self.task().id().first_run_key(), new, at(10))
      .expect("run");
    let events = run.start(at(20)).expect("start");
    db.append_events(&run, &events).expect("recorded");
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.start_step(execution, at(21)).expect("start step");
    db.append_events(&run, &events).expect("recorded");
    run
  }

  /// A second, unfinished task that the first one waits for.
  fn block_on_another_task(&self) {
    let mut db = self.db();
    let details = TaskDetails {
      title: name("Blocker"),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: TaskLimits::NONE,
    };
    let blocker = db
      .create_tasks(
        IdempotencyKey::generate(),
        vec![details],
        Timestamp::from_unix_millis(8),
      )
      .expect("the blocker")
      .remove(0);
    let waits = TaskOrganization::new(
      WorkflowStatus::Backlog,
      Priority::None,
      None,
      vec![BlockingTask {
        task: blocker.id(),
        gate: DependencyGate::Merge,
      }],
      Vec::new(),
    )
    .expect("organization");
    db.set_task_organization(
      self.task().id(),
      &waits,
      ChangeActor::Person,
      Timestamp::from_unix_millis(9),
    )
    .expect("blocked");
  }

  /// What the task would run if it were proposed now.
  fn basis(&self) -> StartBasis {
    StartBasis {
      revision: self.task().revision().get(),
      pipeline: Some(self.published.pipeline),
      version: Some(self.published.snapshot.pipeline().number.get()),
    }
  }

  fn run_proposed(&self) -> Value {
    let render = |message: &orchester_workspace::messages::Message| message.key.to_owned();
    self
      .runtime
      .block_on(app_mcp::run_proposed(
        &self.services,
        ("Kontur".to_owned(), "KON-1".to_owned()),
        self.basis(),
      ))
      .expect_err("this computer has no runner, so a start never succeeds here")
      .to_json(render)
  }

  /// `refused` is `not_ready_here` naming the server `knowledge`, and the project is as it was: nothing queued, no run.
  fn assert_refused_and_untouched(&self, refused: &Value, before: (TaskStatus, u64, usize)) {
    assert_eq!(refused["error"]["code"], "not_ready_here", "{refused}");
    assert_eq!(
      refused["error"]["reasons"],
      json!(["notReadyHere.refused", "notReadyHere.localMcpProgramNotFound"])
    );
    let parts = refused["error"]["parts"].as_array().expect("the parts, structured");
    assert_eq!(parts.len(), 1, "{refused}");
    assert_eq!(parts[0]["name"], "knowledge", "{refused}");
    assert_eq!(self.standing(), before, "nothing was queued, claimed or started");
  }
}

/// A checked Codex connection kept in `db`, and the model its catalog offers.
fn checked_connection(db: &mut ProjectDb, root: &Path) -> (AgentConnection, ModelId) {
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
  (connection, model)
}

/// A pipeline of one agent step on a checked Codex connection, whose profile chooses a local MCP server that
/// starts `program`.
fn published_pipeline(db: &mut ProjectDb, root: &Path, program: &Path) -> Published {
  let at = Timestamp::from_unix_millis;
  let (connection, model) = checked_connection(db, root);
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
  let profile_version = db.publish_draft::<AgentProfile>(profile_id, at(5)).expect("published");
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
  let version = db.publish_draft::<Pipeline>(resource.id(), at(7)).expect("published");
  let profiles = BTreeMap::from([(profile_id, profile_version)]);
  let snapshot = RunSnapshot::build(
    resource.id(),
    &version,
    PublishedVersions {
      profiles: &profiles,
      tools: &BTreeMap::new(),
      instructions: &BTreeMap::new(),
      skills: &BTreeMap::new(),
    },
  )
  .expect("snapshot");
  Published {
    pipeline: resource.id(),
    snapshot,
  }
}

/// Kontur whose server program is gone, and the folder that holds the program, which the test may put there.
fn not_ready() -> (Kontur, PathBuf, TempDir) {
  let outside = tempfile::tempdir().expect("a folder outside the repository");
  let program = outside.path().join("knowledge.sh");
  (Kontur::new(&program), program, outside)
}

#[test]
fn local_start_a_program_that_is_gone_refuses_not_ready_here_and_starts_nothing() {
  let (kontur, program, _outside) = not_ready();
  let before = (TaskStatus::Draft, kontur.task().revision().get(), 0);

  kontur.assert_refused_and_untouched(&kontur.run_task(), before);
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

#[test]
fn local_start_a_queued_draft_is_refused_and_stays_as_it_was() {
  let (kontur, _program, _outside) = not_ready();
  kontur.queue();
  let before = kontur.standing();
  assert_eq!(before.0, TaskStatus::Queued);

  kontur.assert_refused_and_untouched(&kontur.run_task(), before);
}

#[test]
fn local_start_run_again_is_refused_and_makes_no_run() {
  let (kontur, _program, _outside) = not_ready();
  kontur.end_a_run();
  let before = kontur.standing();
  assert_eq!(before.2, 1, "the run that ended");

  kontur.assert_refused_and_untouched(&kontur.run_task(), before);
}

#[test]
fn local_start_a_proposed_start_is_refused_and_starts_nothing() {
  let (kontur, _program, _outside) = not_ready();
  let before = kontur.standing();
  kontur.assert_refused_and_untouched(&kontur.run_proposed(), before);

  kontur.queue();
  let queued = kontur.standing();
  kontur.assert_refused_and_untouched(&kontur.run_proposed(), queued);
}

#[test]
fn local_start_a_proposed_run_again_is_refused_and_makes_no_run() {
  let (kontur, _program, _outside) = not_ready();
  kontur.end_a_run();
  let before = kontur.standing();

  kontur.assert_refused_and_untouched(&kontur.run_proposed(), before);
}

#[test]
fn local_start_a_blocked_task_still_answers_task_blocked() {
  let (kontur, _program, _outside) = not_ready();
  kontur.end_a_run();
  kontur.block_on_another_task();

  assert_eq!(kontur.run_task()["error"]["code"], "task_blocked");
}

#[test]
fn local_start_a_task_with_a_run_at_work_still_answers_run_active() {
  let (kontur, _program, _outside) = not_ready();
  kontur.start_a_run();

  assert_eq!(kontur.run_task()["error"]["code"], "run_active");
}

#[test]
fn local_start_an_incomplete_draft_still_answers_task_incomplete() {
  let outside = tempfile::tempdir().expect("a folder outside the repository");
  let kontur = Kontur::with_brief(&outside.path().join("knowledge.sh"), "");

  assert_eq!(kontur.run_task()["error"]["code"], "task_incomplete");
}

#[test]
fn local_start_the_window_guard_names_the_parts_and_lets_a_ready_task_through() {
  let (kontur, program, _outside) = not_ready();
  let task = kontur.task().id();
  let workspace = &kontur.workspace;
  let guard = || {
    kontur
      .runtime
      .block_on(workspace.ready_to_start(kontur.dir.path(), &Vault::keychain(), task))
  };

  let Err(NotStarted::NotReadyHere(refusal)) = guard() else {
    panic!("a program that is gone refuses the start");
  };
  assert_eq!(refusal.pipeline(), "Quick fix");
  assert_eq!(refusal.number().map(|number| number.get()), Some(1));
  assert_eq!(refusal.parts().len(), 1);
  assert_eq!(refusal.short().key, "notReadyHere.refusedShort");
  assert_eq!(
    kontur.standing(),
    (TaskStatus::Draft, kontur.task().revision().get(), 0),
    "the check queues, claims and starts nothing"
  );

  fs::write(&program, "#!/bin/sh\n").expect("the program appears");
  assert_eq!(guard(), Ok(()), "ready, the start goes on");
}

#[test]
fn local_start_the_window_guard_by_pipeline_leaves_a_task_without_one_to_the_preflight() {
  let (kontur, _program, _outside) = not_ready();
  let workspace = &kontur.workspace;
  let ready = |pipeline| {
    kontur
      .runtime
      .block_on(workspace.ready_to_start_pipeline(kontur.dir.path(), &Vault::keychain(), pipeline))
  };

  assert_eq!(ready(None), Ok(()), "no pipeline is the preflight's to refuse");
  assert!(matches!(
    ready(Some(kontur.published.pipeline)),
    Err(NotStarted::NotReadyHere(_))
  ));
}
