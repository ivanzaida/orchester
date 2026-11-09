//! A local MCP server saved with the new value of a secret its variables take (`Agents · MCP server · editing` L8,
//! `save refused` L12, ORC-842): the value goes to the secret store with the server's settings as one save, so a store
//! that refuses leaves the server as it was and says which secret it refused, without the value anywhere in it.
//! Run with `cargo test -p orchester-workspace --test mcp_secret_values secret`.

use std::{collections::HashMap, fs, path::PathBuf};

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

fn second_secret(db: &mut ProjectDb) -> Secret {
  let other = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("OTHER_TOKEN").expect("valid name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::from_unix_millis(2),
  );
  db.create_secret(&other).expect("secret");
  other
}

/// The server `both` taking both secrets, and the text it was saved from.
fn taking_both(
  repo: &TempDir,
  db: &mut ProjectDb,
  token: &Secret,
) -> (orchester_core::id::ConnectionId, Secret, McpServerText) {
  let other = second_secret(db);
  let text = McpServerText {
    name: "both".to_owned(),
    program: std::env::current_exe()
      .expect("the test program")
      .to_string_lossy()
      .into_owned(),
    arguments: "--stdio".to_owned(),
    variables: "DOCS_TOKEN=${secret:DOCS_TOKEN@staging}\nOTHER_TOKEN=${secret:OTHER_TOKEN@staging}".to_owned(),
    ..McpServerText::new_server()
  };
  let secrets = [token.clone(), other.clone()];
  let id = save_in(repo.path(), None, text.draft(&[], &secrets).expect("valid")).expect("saved");
  (id, other, text)
}

/// A secret store that keeps every value but one, and runs `after` once it refused that one.
struct RefusingOne {
  refused: VaultKey,
  kept: MemoryVault,
  after: Box<dyn Fn() + Send + Sync>,
}

impl SecretVault for RefusingOne {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    if *key == self.refused {
      (self.after)();
      return Err(VaultError::AccessDenied("refused by the test".to_owned()));
    }
    self.kept.store(key, value)
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    self.kept.read(key)
  }

  fn exists(&self, key: &VaultKey) -> Result<bool, VaultError> {
    self.kept.exists(key)
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.kept.delete(key)
  }
}

#[test]
fn secret_store_refusing_the_second_value_names_it_and_restores_the_server() {
  let (repo, mut db, _, token, _) = project();
  let (id, other, both) = taking_both(&repo, &mut db, &token);
  let vault = RefusingOne {
    refused: other.vault_key(db.project().id()),
    kept: MemoryVault::default(),
    after: Box::new(|| {}),
  };
  let changed = McpServerText {
    arguments: "--stdio\n--port\n3845".to_owned(),
    ..both
  };
  let draft = changed.draft(&[], &[token.clone(), other.clone()]).expect("valid");
  let first = SecretValue::new(TYPED.to_owned()).expect("a value");
  let second = SecretValue::new("another-typed-value".to_owned()).expect("a value");

  let refused = store_with_values(
    repo.path(),
    &vault,
    (id, draft, &Allowed::Anything),
    &[(token.id(), first), (other.id(), second)],
  )
  .expect_err("the second value is refused");

  assert_eq!(
    refused.field(),
    RefusedField::Secret(other.name().clone()),
    "the second one is named"
  );
  let stored = db.mcp_server(id).expect("read").expect("stored");
  assert_eq!(
    McpServerText::of(&stored, &[token.clone(), other]).arguments,
    "--stdio",
    "the server is as it was"
  );
  assert_eq!(
    vault.read(&token.vault_key(db.project().id())).expect("kept").expose(),
    TYPED,
    "the first value stays replaced: the store cannot say what it held"
  );
}

#[test]
fn secret_store_refusal_and_a_failed_restore_are_both_reported() {
  let (repo, mut db, _, token, _) = project();
  let (id, other, both) = taking_both(&repo, &mut db, &token);
  let root: PathBuf = repo.path().to_owned();
  let vault = RefusingOne {
    refused: other.vault_key(db.project().id()),
    kept: MemoryVault::default(),
    after: Box::new(move || {
      // Another window took the server's old name while the value was refused, so it cannot be put back.
      let plain = McpServerText {
        name: "both".to_owned(),
        program: std::env::current_exe()
          .expect("the test program")
          .to_string_lossy()
          .into_owned(),
        ..McpServerText::new_server()
      };
      save_in(&root, None, plain.draft(&[], &[]).expect("valid")).expect("another server");
    }),
  };
  let renamed = McpServerText {
    name: "both-renamed".to_owned(),
    ..both
  };
  let draft = renamed.draft(&[], &[token, other.clone()]).expect("valid");
  let value = SecretValue::new(TYPED.to_owned()).expect("a value");

  let refused = store_with_values(
    repo.path(),
    &vault,
    (id, draft, &Allowed::Anything),
    &[(other.id(), value)],
  )
  .expect_err("refused, and not restored");

  assert!(matches!(refused, McpSaveError::NotRestored { .. }), "{refused:?}");
  assert_eq!(
    refused.field(),
    RefusedField::Secret(other.name().clone()),
    "the refused secret is still marked"
  );
  let message = refused.message();
  assert_eq!(message.key, "connections.mcp.errors.notRestored");
  assert_eq!(message.args, [("secret", "OTHER_TOKEN".to_owned())]);
  let said = format!("{refused} {refused:?}");
  assert!(
    said.contains("refused") && said.contains("put back"),
    "both are in it: {said}"
  );
  assert!(!said.contains(TYPED));
}

#[test]
fn secret_the_server_does_not_take_is_refused_before_anything_is_stored() {
  let (repo, mut db, id, token, text) = project();
  let stranger = second_secret(&mut db);
  let changed = McpServerText {
    arguments: "--stdio\n--port\n3845".to_owned(),
    ..text
  };
  let draft = changed.draft(&[], std::slice::from_ref(&token)).expect("valid");
  let vault = MemoryVault::default();
  let value = SecretValue::new(TYPED.to_owned()).expect("a value");

  let refused = store_with_values(
    repo.path(),
    &vault,
    (id, draft, &Allowed::Anything),
    &[(stranger.id(), value)],
  )
  .expect_err("a secret of the project that the server does not take");

  assert!(matches!(refused, McpSaveError::SecretNotTaken(_)), "{refused:?}");
  assert_eq!(refused.message().key, "connections.mcp.errors.secretNotTaken");
  let stored = db.mcp_server(id).expect("read").expect("stored");
  assert_eq!(
    McpServerText::of(&stored, &[token]).arguments,
    "--stdio",
    "nothing was stored"
  );
  assert!(
    vault.read(&stranger.vault_key(db.project().id())).is_err(),
    "no value was kept"
  );
}
