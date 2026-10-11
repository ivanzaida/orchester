//! Illustrative Secrets data, only for the native QA example and its regression tests.
use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  id::{ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, DeniedBehavior, HttpAuth, HttpMethod, HttpRequest, Slug, Tool, ToolAction,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault},
  store::{ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  connections::mcp::{McpServerText, TransportChoice, save_in, secret_reference},
  project_setup::NewProject,
  workspace::{OpenProject, Workspace},
};

use super::vault::QaVault;

type Error = Box<dyn std::error::Error>;

pub fn seed(workspace: &Workspace, repo: &Path, vault: &QaVault, scenario: &str) -> Result<OpenProject, Error> {
  std::fs::create_dir_all(repo.join(".git"))?;
  let open = workspace.create(NewProject {
    project: Project::new(ProjectId::generate(), ProjectKey::new("KON")?, Name::new("Kontur")?),
    repo_root: repo.to_path_buf(),
  })?;
  if scenario == "empty" {
    return Ok(open);
  }
  let mut db = ProjectDb::open(repo)?;
  let mut secrets = Vec::new();
  for (name, kind, environment, rotated) in [
    ("DOCS_API_KEY", SecretKind::ApiKey, Environment::Development, false),
    ("SENTRY_TOKEN", SecretKind::AccessToken, Environment::Development, false),
    (
      "STAGING_DB_URL",
      SecretKind::DatabaseConnection,
      Environment::Staging,
      true,
    ),
    (
      "STAGING_DEPLOY_TOKEN",
      SecretKind::DeploymentToken,
      Environment::Staging,
      true,
    ),
    (
      "RELEASE_SIGNING_PASSWORD",
      SecretKind::Password,
      Environment::Staging,
      false,
    ),
  ] {
    let secret = Secret::from_stored(
      SecretId::generate(),
      NewSecret {
        name: SecretName::new(name)?,
        kind,
        environment,
      },
      Timestamp::from_unix_millis(1_789_862_400_000),
      rotated.then_some(Timestamp::from_unix_millis(1_790_467_200_000)),
      None,
    );
    db.create_secret(&secret)?;
    vault.store(
      &secret.vault_key(open.project.id()),
      &SecretValue::new("native-qa-only-credential".into())?,
    )?;
    secrets.push(secret);
  }
  if scenario == "list" {
    references(&mut db, repo, &secrets)?;
  }
  Ok(open)
}

fn references(db: &mut ProjectDb, repo: &Path, secrets: &[Secret]) -> Result<(), Error> {
  tool(db, &secrets[0], "search-docs", "Search docs")?;
  tool(db, &secrets[2], "query-staging", "Query staging")?;
  tool(db, &secrets[3], "deploy-staging", "Deploy staging")?;
  let docs = McpServerText {
    name: "kontur-docs".into(),
    program: std::env::current_exe()?.to_string_lossy().into_owned(),
    arguments: "--qa-mcp-placeholder".into(),
    variables: format!("DOCS_API_KEY={}", secret_reference(&secrets[0])),
    ..McpServerText::new_server()
  };
  save_in(
    repo,
    None,
    docs
      .draft(&[], secrets)
      .map_err(|_| std::io::Error::other("invalid QA docs server"))?,
  )
  .map_err(|_| std::io::Error::other("QA docs server was not saved"))?;
  let sentry = McpServerText {
    name: "sentry".into(),
    transport: TransportChoice::Http,
    url: "https://mcp.example.test/sentry".into(),
    bearer: Some(secrets[1].id()),
    ..McpServerText::new_server()
  };
  save_in(
    repo,
    None,
    sentry
      .draft(&[], secrets)
      .map_err(|_| std::io::Error::other("invalid QA sentry server"))?,
  )
  .map_err(|_| std::io::Error::other("QA sentry server was not saved"))?;
  Ok(())
}

fn tool(db: &mut ProjectDb, secret: &Secret, slug: &str, name: &str) -> Result<(), Error> {
  let request = HttpRequest::new(
    HttpMethod::Get,
    "https://qa.example.test/".into(),
    "qa.example.test".into(),
    HttpAuth::Bearer { secret: secret.id() },
    BTreeMap::new(),
    BTreeMap::new(),
  )?;
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Development")?,
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )?;
  db.create_resource(
    Slug::new(slug)?,
    Name::new(name)?,
    &Tool::new("", ToolAction::Http(request), Some(policy))?,
    Timestamp::now(),
  )?;
  Ok(())
}

/// The store has no API for corruption; damage only this example's known credential kind.
pub fn read_failure(repo: &Path, failed: bool) -> Result<(), rusqlite::Error> {
  let connection = rusqlite::Connection::open(repo.join(".orchester/orchester.db"))?;
  connection.pragma_update(None, "ignore_check_constraints", true)?;
  connection.execute(
    "UPDATE secrets SET kind = ?1 WHERE name = 'DOCS_API_KEY'",
    [if failed { "qa-invalid-kind" } else { "api_key" }],
  )?;
  Ok(())
}
