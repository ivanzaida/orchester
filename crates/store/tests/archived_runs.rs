//! A run does not start on what a local project archived (ORC-842): the store refuses it in the transaction that
//! records the run, naming what is archived, whatever the preflight saw before; an archived MCP server is in no list
//! the app reads, and restoring either lets a run start again.
//! `cargo test -p orchester-store --test archived_runs archived`.

mod support;

use std::{collections::BTreeMap, fs};

use orchester_core::{
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId},
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, CommandProgram, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind, Version,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{McpServerStore, ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
};
use orchester_store::{ProjectDb, RunStoreError};
use support::{at, model, name};
use tempfile::TempDir;

/// A project with the profile Builder, which chooses the server docs, a pipeline that runs it, and a new run of it.
struct Fixture {
  _repo: TempDir,
  db: ProjectDb,
  builder: ResourceId,
  docs: ConnectionId,
  new: NewRun,
}

fn project() -> Fixture {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let docs = docs_server(&mut db);
  let (builder, profiles) = builder_choosing(&mut db, docs);
  let new = new_run(&mut db, builder, &profiles);
  Fixture {
    _repo: repo,
    db,
    builder,
    docs,
    new,
  }
}

/// The server docs, stored.
fn docs_server(db: &mut ProjectDb) -> ConnectionId {
  let docs = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("docs").expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("docs-mcp").expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::default(),
    at(1),
  )
  .expect("server");
  db.create_mcp_server(&docs).expect("stored");
  docs.id()
}

/// The profile Builder, published, which chooses `docs`.
fn builder_choosing(
  db: &mut ProjectDb,
  docs: ConnectionId,
) -> (ResourceId, BTreeMap<ResourceId, Version<AgentProfile>>) {
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    model(),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(vec![docs]).expect("selection"));
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
    .expect("profile")
    .id();
  let published = db.publish_draft::<AgentProfile>(builder, at(3)).expect("publish");
  (builder, BTreeMap::from([(builder, published)]))
}

/// A run of a task, of a pipeline whose one step runs `builder`.
fn new_run(db: &mut ProjectDb, builder: ResourceId, profiles: &BTreeMap<ResourceId, Version<AgentProfile>>) -> NewRun {
  let details = TaskDetails {
    title: name("Say hello"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::new(None, None).expect("limits"),
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(1))
    .expect("task")[0]
    .id();
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let (delivery, version) = db
    .published_pipeline_for_test("delivery", "Delivery", &pipeline, at(4))
    .expect("publish pipeline");
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &BTreeMap::new(),
  };
  NewRun {
    task,
    snapshot: RunSnapshot::build(delivery, &version, published).expect("snapshot"),
    branch: BranchName::new("orchester/orc-1-run-1").expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    base_branch: None,
    base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
    budget: None,
    max_duration: None,
  }
}

fn start(project: &mut Fixture) -> Result<(), RunStoreError> {
  let new = NewRun {
    branch: BranchName::new(&format!("orchester/orc-1-{}", IdempotencyKey::generate())).expect("branch"),
    ..project.new.clone()
  };
  project.db.create_run(IdempotencyKey::generate(), new, at(10)).map(drop)
}

#[test]
fn archived_mcp_server_is_in_no_list_the_app_reads_but_all_of_them() {
  let mut project = project();
  assert_eq!(project.db.mcp_servers().expect("read").len(), 1);

  project.db.archive_mcp_server(project.docs, at(20)).expect("archived");

  assert!(
    project.db.mcp_servers().expect("read").is_empty(),
    "pickers, tools and runs see none"
  );
  assert_eq!(
    project.db.all_mcp_servers().expect("read").len(),
    1,
    "what restoring puts back is still there"
  );
  assert!(
    project.db.mcp_server(project.docs).expect("read").is_some(),
    "by its id too"
  );
  project.db.restore_mcp_server(project.docs).expect("restored");
  assert_eq!(project.db.mcp_servers().expect("read").len(), 1);
}

#[test]
fn archived_server_chosen_by_a_pinned_profile_refuses_the_run_until_it_is_restored() {
  let mut project = project();
  start(&mut project).expect("a run starts on what is not archived");

  project.db.archive_mcp_server(project.docs, at(20)).expect("archived");
  let refused = start(&mut project).expect_err("the server is archived");
  assert!(
    matches!(&refused, RunStoreError::UsesArchived(name) if name == "docs"),
    "{refused:?}"
  );

  project.db.restore_mcp_server(project.docs).expect("restored");
  start(&mut project).expect("restored, it runs again");
}

#[test]
fn archived_profile_pinned_by_the_run_refuses_it_until_it_is_restored() {
  let mut project = project();
  project
    .db
    .archive_local_resource(project.builder, at(20))
    .expect("archived");

  let refused = start(&mut project).expect_err("the profile is archived");
  assert!(
    matches!(&refused, RunStoreError::UsesArchived(name) if name == "Builder"),
    "{refused:?}"
  );

  project.db.restore_local_resource(project.builder).expect("restored");
  start(&mut project).expect("restored, it runs again");
}
