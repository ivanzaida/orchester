//! `list_service_proposals` and `save_service_proposals` through Orchester's MCP server, and saving through the
//! workspace: the chosen proposals become drafts of new services and stacks, never versions; services wait for the
//! person's switch, stacks do not; a name the project has already is kept.

use std::sync::Arc;

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::ResourceKind,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  service::{Service, Stack},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  agent_testing::import::{Choice, propose_in, save_in},
  app_mcp::{self, ToolError, Translate},
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::kontur;

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// The app's session with Kontur, the fixture checkout as a local project, whose tools are called as the app's MCP
/// server calls them.
struct Desktop {
  workspace: Workspace,
  services: app_mcp::Services,
  runtime: Runtime,
  repo: TempDir,
  _data: TempDir,
}

impl Desktop {
  fn new() -> Self {
    let repo = kontur();
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    ProjectDb::create(repo.path(), project).expect("project");
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    workspace.open(repo.path()).expect("register the project");
    let runtime = Runtime::new().expect("runtime");
    let runners = Runners::new(
      repo.path().join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(NoVault)), version);
    let services = app_mcp::Services::new(
      workspace.clone(),
      runners,
      servers,
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    Self {
      workspace,
      services,
      runtime,
      repo,
      _data: data,
    }
  }

  fn call(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
    let translate = Translate::new(|message| message.key.to_owned());
    let Value::Object(args) = args.clone() else {
      panic!("arguments are an object");
    };
    self
      .runtime
      .block_on(app_mcp::setup::call(&self.services, &translate, name, args))
      .expect("a setup tool")
  }

  fn ok(&self, name: &str, args: &Value) -> Value {
    self
      .call(name, args)
      .unwrap_or_else(|error| panic!("{name} refused: {}", error.to_json(|message| message.key.to_owned())))
  }

  fn refused(&self, name: &str, args: &Value) -> Value {
    match self.call(name, args) {
      Ok(value) => panic!("{name} did what it should refuse: {value}"),
      Err(error) => error.to_json(|message| message.key.to_owned())["error"].clone(),
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("the project database")
  }

  /// Each service and stack of the project with whether it has a draft and a version.
  fn stored(&self) -> Vec<(ResourceKind, String, bool, bool)> {
    let db = self.db();
    let mut stored = Vec::new();
    for kind in [ResourceKind::Service, ResourceKind::Stack] {
      for resource in db.resources(kind).expect("resources") {
        let (draft, version) = match kind {
          ResourceKind::Service => (
            db.draft::<Service>(resource.id()).expect("draft").is_some(),
            db.latest_version::<Service>(resource.id()).expect("version").is_some(),
          ),
          _ => (
            db.draft::<Stack>(resource.id()).expect("draft").is_some(),
            db.latest_version::<Stack>(resource.id()).expect("version").is_some(),
          ),
        };
        stored.push((kind, resource.slug().as_str().to_owned(), draft, version));
      }
    }
    stored
  }
}

fn save(services: &[&str], stacks: &[&str]) -> Value {
  json!({ "project": "Kontur", "services": services, "stacks": stacks })
}

#[test]
fn service_import_lists_proposals_with_sources_and_unmapped_keys() {
  let desktop = Desktop::new();
  let listed = desktop.ok("list_service_proposals", &json!({ "project": "Kontur" }));
  let proposals = listed["proposals"].as_array().expect("a list");
  let api = proposals
    .iter()
    .find(|proposal| proposal["name"] == "api")
    .expect("api is proposed");
  assert_eq!(api["kind"], "service");
  assert_eq!(
    api["source"],
    json!({ "file": "deploy/cloud/compose.yaml", "key": "services.api" })
  );
  assert_eq!(api["definition"]["program"], "ghcr.io/kontur/api:latest");
  assert_eq!(api["definition"]["run_data"]["kind"], "database_container");
  assert_eq!(api["definition"]["readiness"]["path"], "/healthz");
  assert_eq!(api["unmapped"][0]["key"], "services.api.build");
  assert_eq!(api["unmapped"][0]["reason"], "services.import.unmapped.build");
  assert_eq!(api["exists"], false);
  let cloud = proposals
    .iter()
    .find(|proposal| proposal["name"] == "cloud")
    .expect("cloud is proposed");
  assert_eq!(cloud["kind"], "stack");
  assert_eq!(cloud["services"], json!(["postgres-2", "api"]));
  assert_eq!(listed["skipped"][0]["source"]["key"], "services.web");
  assert_eq!(listed["skipped"][0]["reason"], "services.import.skipped.builds");
  assert_eq!(listed["unreadable"], json!([]));
  assert_eq!(listed["truncated"], false);
}

#[test]
fn service_import_saves_services_only_under_the_persons_switch() {
  let desktop = Desktop::new();
  let refused = desktop.refused("save_service_proposals", &save(&["api", "kontur-dev"], &["cloud"]));
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert_eq!(
    refused["reasons"],
    json!(["appMcp.errors.personOnly", "appMcp.services.widens.imported"])
  );
  assert!(desktop.stored().is_empty(), "nothing was saved");

  let stacks = desktop.ok("save_service_proposals", &save(&[], &["cloud"]));
  assert_eq!(
    stacks["saved"],
    json!([{ "kind": "stack", "name": "cloud", "id": stacks["saved"][0]["id"] }])
  );

  desktop
    .workspace
    .set_automation_changes_permissions(true)
    .expect("a person turns it on");
  let saved = desktop.ok("save_service_proposals", &save(&["api", "kontur-dev"], &["cloud"]));
  let names: Vec<&str> = saved["saved"]
    .as_array()
    .expect("a list")
    .iter()
    .filter_map(|saved| saved["name"].as_str())
    .collect();
  assert_eq!(names, ["api", "kontur-dev"]);
  assert_eq!(saved["taken"], json!([{ "kind": "stack", "name": "cloud" }]));
  assert_eq!(
    desktop.stored(),
    [
      (ResourceKind::Service, "api".to_owned(), true, false),
      (ResourceKind::Service, "kontur-dev".to_owned(), true, false),
      (ResourceKind::Stack, "cloud".to_owned(), true, false),
    ],
    "drafts only: nothing is published"
  );
  let listed = desktop.ok("list_service_proposals", &json!({ "project": "Kontur" }));
  let exists: Vec<&str> = listed["proposals"]
    .as_array()
    .expect("a list")
    .iter()
    .filter(|proposal| proposal["exists"] == true)
    .filter_map(|proposal| proposal["name"].as_str())
    .collect();
  assert_eq!(exists, ["api", "cloud", "kontur-dev"]);
}

#[test]
fn service_import_refuses_a_name_it_does_not_propose() {
  let desktop = Desktop::new();
  let refused = desktop.refused("save_service_proposals", &save(&[], &["cloud", "full"]));
  assert_eq!(refused["code"], "not_proposed", "{refused}");
  assert_eq!(refused["reasons"], json!(["services.import.errors.notProposed"]));
  let as_stack = desktop.refused("save_service_proposals", &save(&[], &["api"]));
  assert_eq!(as_stack["code"], "not_proposed", "a service's name is not a stack's");
  assert!(desktop.stored().is_empty(), "nothing was saved");
}

#[test]
fn service_import_saves_drafts_through_the_workspace() {
  let desktop = Desktop::new();
  let root = desktop.repo.path();
  let proposals = propose_in(root).expect("the checkout is read");
  let choice = Choice {
    services: vec!["postgres-2".to_owned(), "api".to_owned(), "migrate".to_owned()],
    stacks: vec!["cloud".to_owned()],
  };
  let chosen = proposals.chosen(&choice).expect("each is proposed");
  let imported = save_in(root, &chosen, Timestamp::now()).expect("saved");
  let saved: Vec<&str> = imported.saved.iter().map(|saved| saved.name.as_str()).collect();
  assert_eq!(
    saved,
    ["postgres-2", "api", "cloud", "migrate"],
    "in the order they are proposed"
  );
  assert!(imported.taken.is_empty());
  let db = desktop.db();
  let api = imported.saved.iter().find(|saved| saved.name == "api").expect("api");
  let draft = db.draft::<Service>(api.id).expect("draft").expect("a draft");
  let proposed = proposals
    .proposals
    .iter()
    .find(|proposal| proposal.draft.name() == "api")
    .expect("api");
  assert_eq!(
    orchester_workspace::agent_testing::import::Draft::Service(Box::new(draft.body().clone())),
    proposed.draft,
    "the draft is the proposal"
  );
  assert!(db.latest_version::<Service>(api.id).expect("version").is_none());

  let again = save_in(root, &chosen, Timestamp::now()).expect("saved");
  assert!(again.saved.is_empty());
  assert_eq!(again.taken.len(), 4, "a name the project has is kept");
}
