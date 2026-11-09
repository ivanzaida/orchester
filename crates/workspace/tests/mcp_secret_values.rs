//! A local MCP server saved with the new value of a secret its variables take (`Agents · MCP server · editing` L8,
//! `save refused` L12, ORC-842): the value goes to the secret store with the server's settings as one save, so a store
//! that refuses leaves the server as it was and says which secret it refused, without the value anywhere in it.
//! Run with `cargo test -p orchester-workspace --test mcp_secret_values secret`.

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
  Allowed, McpSaveError, McpServerText, RefusedField, save_in, store_with_values,
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

/// A project with the server `docs`, which takes `DOCS_TOKEN` in its variable, and that secret.
fn project() -> (
  TempDir,
  ProjectDb,
  orchester_core::id::ConnectionId,
  Secret,
  McpServerText,
) {
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
  let id = save_in(repo.path(), None, draft).expect("saved");
  (repo, db, id, token, text)
}

#[test]
fn secret_value_is_kept_with_the_settings_in_one_save() {
  let (repo, db, id, token, text) = project();
  let vault = MemoryVault::default();
  let changed = McpServerText {
    arguments: "--stdio\n--port\n3845".to_owned(),
    ..text
  };
  let draft = changed.draft(&[], std::slice::from_ref(&token)).expect("valid");
  let value = SecretValue::new(TYPED.to_owned()).expect("a value");

  store_with_values(
    repo.path(),
    &vault,
    (id, draft, &Allowed::Anything),
    &[(token.id(), value)],
  )
  .expect("saved");

  let key = token.vault_key(db.project().id());
  assert_eq!(vault.read(&key).expect("kept").expose(), TYPED);
  let stored = db.mcp_server(id).expect("read").expect("stored");
  assert_eq!(
    McpServerText::of(&stored, std::slice::from_ref(&token)).arguments,
    "--stdio\n--port\n3845"
  );
}

#[test]
fn secret_store_refusal_leaves_the_server_as_it_was_and_names_the_secret_not_the_value() {
  let (repo, db, id, token, text) = project();
  let changed = McpServerText {
    arguments: "--stdio\n--port\n3845".to_owned(),
    ..text
  };
  let draft = changed.draft(&[], std::slice::from_ref(&token)).expect("valid");
  let value = SecretValue::new(TYPED.to_owned()).expect("a value");

  let refused = store_with_values(
    repo.path(),
    &RefusingVault,
    (id, draft, &Allowed::Anything),
    &[(token.id(), value)],
  )
  .expect_err("the store refuses");

  assert_eq!(refused.field(), RefusedField::Secret(token.name().clone()));
  let message = refused.message();
  assert_eq!(message.key, "connections.mcp.errors.secretStoreRefused");
  assert_eq!(message.args, [("secret", "DOCS_TOKEN".to_owned())]);
  assert!(
    !format!("{refused:?} {refused} {message:?}").contains(TYPED),
    "no value in an error or a message"
  );
  let stored = db.mcp_server(id).expect("read").expect("stored");
  assert_eq!(
    McpServerText::of(&stored, std::slice::from_ref(&token)).arguments,
    "--stdio",
    "not even the other fields were saved"
  );
}

#[test]
fn secret_of_an_unknown_id_is_refused_as_gone_and_changes_nothing() {
  let (repo, db, id, token, text) = project();
  let draft = text.draft(&[], std::slice::from_ref(&token)).expect("valid");
  let value = SecretValue::new(TYPED.to_owned()).expect("a value");

  let refused = store_with_values(
    repo.path(),
    &MemoryVault::default(),
    (id, draft, &Allowed::Anything),
    &[(SecretId::generate(), value)],
  )
  .expect_err("no such secret");

  assert!(matches!(refused, McpSaveError::SecretGone(_)), "{refused:?}");
  assert!(db.mcp_server(id).expect("read").is_some());
}
