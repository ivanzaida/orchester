//! A local MCP server created with the values of the secrets its variables take (`Agents · New MCP server` L4,
//! ORC-843): the server and the values are one create, so a secret store that refuses a value leaves no server behind
//! and says which secret it refused, without the value anywhere in it.
//! Run with `cargo test -p orchester-workspace --test mcp_create create`.

use std::{collections::HashMap, fs};

use orchester_core::{
  id::{ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::{McpServerStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::connections::mcp::{
  Allowed, McpSaveError, McpServerDraft, McpServerText, RefusedField, create_with_values,
};
use parking_lot::Mutex;
use tempfile::TempDir;

/// The value a test types, which no message, error or debug text may hold.
const TYPED: &str = "hunter2-typed-value";

#[derive(Default)]
struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::NotFound)
  }

  fn exists(&self, key: &VaultKey) -> Result<bool, VaultError> {
    Ok(self.0.lock().contains_key(key))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// A secret store that refuses every value, as one that is locked does.
struct RefusingVault;

impl SecretVault for RefusingVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::AccessDenied("refused by the test".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn exists(&self, _: &VaultKey) -> Result<bool, VaultError> {
    Ok(false)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// A project with the secret `DOCS_TOKEN`, and the draft of a server `docs` that takes it.
fn project() -> (TempDir, ProjectDb, Secret, McpServerDraft) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create the project database");
  let token = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("DOCS_TOKEN").expect("valid name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::from_unix_millis(1),
  );
  db.create_secret(&token).expect("secret");
  let program = std::env::current_exe().expect("the test program");
  let text = McpServerText {
    name: "docs".to_owned(),
    program: program.to_string_lossy().into_owned(),
    arguments: "--stdio".to_owned(),
    variables: "DOCS_TOKEN=${secret:DOCS_TOKEN@staging}".to_owned(),
    ..McpServerText::new_server()
  };
  let draft = text.draft(&[], std::slice::from_ref(&token)).expect("valid");
  (repo, db, token, draft)
}

fn typed() -> SecretValue {
  SecretValue::new(TYPED.to_owned()).expect("a value")
}

#[test]
fn create_stores_the_server_and_the_value_of_its_secret() {
  let (repo, db, token, draft) = project();
  let vault = MemoryVault::default();

  let id = create_with_values(
    repo.path(),
    &vault,
    (draft, &Allowed::Anything),
    true,
    &[(token.id(), typed())],
  )
  .expect("created");

  let stored = db.mcp_server(id).expect("read").expect("stored");
  assert!(stored.enabled());
  assert_eq!(stored.name().as_str(), "docs");
  let key = token.vault_key(db.project().id());
  assert_eq!(vault.read(&key).expect("kept").expose(), TYPED);
}

#[test]
fn create_can_leave_the_new_server_off() {
  let (repo, db, _, draft) = project();

  let id = create_with_values(repo.path(), &MemoryVault::default(), (draft, &Allowed::Anything), false, &[])
    .expect("created");

  assert!(!db.mcp_server(id).expect("read").expect("stored").enabled());
}

#[test]
fn create_refused_by_the_secret_store_leaves_no_server_and_names_the_secret_not_the_value() {
  let (repo, db, token, draft) = project();

  let refused = create_with_values(
    repo.path(),
    &RefusingVault,
    (draft, &Allowed::Anything),
    true,
    &[(token.id(), typed())],
  )
  .expect_err("the store refuses");

  assert_eq!(refused.field(), RefusedField::Secret(token.name().clone()));
  let message = refused.message();
  assert_eq!(message.key, "connections.mcp.errors.secretStoreRefused");
  assert!(
    !format!("{refused:?} {refused} {message:?}").contains(TYPED),
    "no value in an error or a message"
  );
  assert!(
    db.mcp_servers().expect("read").is_empty(),
    "a create that was refused made nothing"
  );
}

#[test]
fn create_with_a_value_for_a_secret_the_server_does_not_take_stores_nothing() {
  let (repo, db, _, draft) = project();

  let refused = create_with_values(
    repo.path(),
    &MemoryVault::default(),
    (draft, &Allowed::Anything),
    true,
    &[(SecretId::generate(), typed())],
  )
  .expect_err("no such secret");

  assert!(matches!(refused, McpSaveError::SecretGone(_)), "{refused:?}");
  assert!(db.mcp_servers().expect("read").is_empty());
}
