//! A pipeline through Orchester's MCP tools and the Orchestrator chat, with pipelines that are always a published
//! version (D-032, D-033): `create_pipeline`, `save_pipeline` and `duplicate_pipeline` publish or refuse with nothing
//! stored, the draft-era names are unknown tools ([`mcp`]).
//! `cargo test -p orchester-workspace --test pipeline_tools pipeline_tools`.

mod mcp;

use std::{fs, sync::Arc};

use orchester_core::{
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, Slug},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::ResourceStore,
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

/// The app's session with one local project, Kontur, whose agent profiles Builder and Reviewer are published.
struct Desktop {
  workspace: Workspace,
  services: app_mcp::Services,
  runtime: Runtime,
  repo: TempDir,
  _data: TempDir,
}

impl Desktop {
  fn new() -> Self {
    let repo = TempDir::new().expect("repository");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
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
    let desktop = Self {
      workspace,
      services,
      runtime,
      repo,
      _data: data,
    };
    desktop.profile("builder", "Builder");
    desktop.profile("reviewer", "Reviewer");
    desktop
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("project database")
  }

  /// A published agent profile named `name`.
  fn profile(&self, slug: &str, name: &str) {
    let profile = AgentProfile::new(
      Name::new(name).expect("name"),
      String::new(),
      PromptText::new("Work.").expect("prompt"),
      ConnectionId::generate(),
      ModelId::new("gpt-5.5").expect("model"),
      ProviderSettings::default(),
    );
    let mut db = self.db();
    let id = db
      .create_resource(
        Slug::new(slug).expect("slug"),
        Name::new(name).expect("name"),
        &profile,
        Timestamp::now(),
      )
      .expect("profile")
      .id();
    db.publish_draft::<AgentProfile>(id, Timestamp::now())
      .expect("published");
  }

  /// Whether a person lets automation widen what agents may do.
  fn lets_automation_widen(&self, allowed: bool) {
    self
      .workspace
      .set_automation_changes_permissions(allowed)
      .expect("the switch is saved");
  }

  /// The app's tool `name` with `arguments` as any MCP client calls it; `None` when the app has no such tool.
  fn tool(&self, name: &str, arguments: Value) -> Option<Result<Value, Value>> {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    let translate = Translate::new(|message| message.key.to_owned());
    let called = self.runtime.block_on(async {
      match app_mcp::call(&self.services, &translate, name, arguments.clone()).await {
        Some(called) => Some(called),
        None => app_mcp::setup::call(&self.services, &translate, name, arguments).await,
      }
    })?;
    Some(called.map_err(|error| error.to_json(|message| message.key.to_owned())["error"].clone()))
  }

  /// The tool `name`'s answer, which must not be a refusal.
  fn done(&self, name: &str, arguments: Value) -> Value {
    self
      .tool(name, arguments)
      .expect("the app has the tool")
      .unwrap_or_else(|error| panic!("{name} refused: {error}"))
  }

  /// The tool `name`'s refusal, which it must give.
  fn refused(&self, name: &str, arguments: Value) -> Value {
    match self.tool(name, arguments).expect("the app has the tool") {
      Ok(answer) => panic!("{name} was not refused: {answer}"),
      Err(error) => error,
    }
  }

  /// The project's pipelines: their names, and how many versions and drafts each has.
  fn pipelines(&self) -> Vec<(String, usize, bool)> {
    let db = self.db();
    db.resources(orchester_core::resource::ResourceKind::Pipeline)
      .expect("pipelines")
      .into_iter()
      .map(|resource| {
        let versions = db.versions::<Pipeline>(resource.id()).expect("versions").len();
        let draft = db.draft::<Pipeline>(resource.id()).expect("draft").is_some();
        (resource.name().as_str().to_owned(), versions, draft)
      })
      .collect()
  }
}

/// A step of `create_pipeline`'s or `save_pipeline`'s `steps`: `key`, run by `profile`, after `after`.
fn step(key: &str, profile: &str, after: &[&str]) -> Value {
  json!({ "key": key, "name": key, "profile": profile, "runs_after": after })
}

/// The reasons of a refusal, as message keys.
fn reasons(error: &Value) -> Vec<String> {
  error["reasons"]
    .as_array()
    .expect("a refusal names its reasons")
    .iter()
    .map(|reason| reason.as_str().expect("a reason is text").to_owned())
    .collect()
}
