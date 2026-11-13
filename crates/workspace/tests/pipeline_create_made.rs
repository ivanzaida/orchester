//! The Pipelines hub's *Create v1* (`Pipelines · v2` P14, PV2-22): the pipeline the person made in the panel is stored
//! as its first version, with the steps and description made, or refused with why and nothing stored (D-032).
//! `cargo test -p orchester-workspace --test pipeline_create_made pipeline_create_made`.

use std::sync::Arc;

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    CommandAction, CommandProgram, Pipeline, PipelineEdit, ResourceKind, Slug, StepKind, Tool, ToolAction,
    VersionNumber,
  },
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{pipelines::create::create_made_in, secrets::Vault, sharing::Servers, workspace::Workspace};
use tempfile::TempDir;

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

/// A local project with a published tool a step can run, and the session's servers, which a local project never asks.
fn project() -> (TempDir, TempDir, Servers, ResourceId) {
  let repo = TempDir::new().expect("a temporary folder");
  std::fs::create_dir(repo.path().join(".git")).expect("a .git folder");
  let mut db = ProjectDb::create(
    repo.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").expect("key"),
      Name::new("Test").expect("name"),
    ),
  )
  .expect("the project database");
  let tool = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("program"),
      vec![],
    )),
    None,
  )
  .expect("a command tool");
  let tool = db
    .create_published(
      Slug::new("tool").expect("slug"),
      Name::new("Tool").expect("name"),
      &tool,
      Timestamp::now(),
    )
    .expect("a published tool")
    .0
    .id();
  let data = TempDir::new().expect("application data");
  let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
  let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
  let servers = Servers::new(workspace, Vault::with(Arc::new(NoVault)), version);
  (repo, data, servers, tool)
}

/// One step that runs `tool`, with `description`.
fn made(tool: ResourceId, description: &str) -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("an empty pipeline"));
  let key = edit.add_step(Name::new("Check").expect("name"), None);
  edit.update(&key, |step| step.kind = StepKind::Tool { tool: Some(tool) });
  edit
    .validate()
    .expect("a valid pipeline")
    .with_description(description.to_owned())
}

fn pipelines(repo: &TempDir) -> usize {
  ProjectDb::open(repo.path())
    .expect("database")
    .resources(ResourceKind::Pipeline)
    .expect("read")
    .len()
}

#[test]
fn pipeline_create_made_stores_what_was_made_as_v1() {
  let (repo, _data, servers, tool) = project();
  let body = made(tool, "Made in the hub");
  let id = create_made_in((repo.path(), &servers), " Made ", &body).expect("created");
  let db = ProjectDb::open(repo.path()).expect("database");
  let latest = db.latest_version::<Pipeline>(id).expect("read").expect("a version");
  assert_eq!(latest.number(), VersionNumber::FIRST);
  assert_eq!(latest.body(), &body, "the steps and description made");
  let resource = db.resource(id).expect("read").expect("the pipeline");
  assert_eq!(resource.name().as_str(), "Made");
  assert_eq!(resource.slug().as_str(), "made");
}

#[test]
fn pipeline_create_made_refuses_a_taken_slug_and_steps_that_cannot_publish_and_stores_nothing() {
  let (repo, _data, servers, tool) = project();
  create_made_in((repo.path(), &servers), "Made", &made(tool, "")).expect("created");

  let taken = create_made_in((repo.path(), &servers), "made", &made(tool, "")).expect_err("the slug is taken");
  assert_eq!(taken.key, "pipelines.create.slugTaken");

  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("an empty pipeline"));
  edit.add_step(Name::new("Plan").expect("name"), None);
  let unset = edit.validate().expect("a valid body");
  let refused = create_made_in((repo.path(), &servers), "Unset", &unset).expect_err("a step has no profile");
  assert!(!refused.key.is_empty());

  let empty = Pipeline::new(vec![]).expect("an empty pipeline");
  create_made_in((repo.path(), &servers), "Empty", &empty).expect_err("no steps cannot publish");
  assert!(
    create_made_in((repo.path(), &servers), "   ", &made(tool, "")).is_err(),
    "no name"
  );
  assert_eq!(pipelines(&repo), 1, "nothing more is stored");
}
