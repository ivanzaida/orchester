use std::fs;

use orchester_core::{
  id::SecretId,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::{Preview, ShownSnapshot, Snapshot, read};

use super::{Fixture, LocalPath, git};

pub(super) struct Vault(pub(super) Option<&'static str>);
impl SecretVault for Vault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Unsupported)
  }
  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    self
      .0
      .map(|value| SecretValue::new(value.to_owned()).expect("test value"))
      .ok_or(VaultError::NotFound)
  }
  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Err(VaultError::Unsupported)
  }
}

#[test]
fn local_integration_previews_hide_sensitive_names_known_values_and_json_escaped_values() {
  let fixture = Fixture::new(false);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.create_secret(&Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("FIXTURE_SECRET").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  ))
  .expect("metadata");
  let value = "fixture credential\nwith \"quotes\"";
  let vault = Vault(Some(value));
  for (filename, bytes) in [
    (".env", "ordinary environment content".to_owned()),
    ("notes.txt", value.to_owned()),
    ("encoded.json", serde_json::to_string(value).expect("escaped")),
  ] {
    fs::write(fixture.root().join(filename), bytes).expect("fixture");
    let path = LocalPath::new(filename).expect("path");
    let view = read(fixture.root(), fixture.run, Some(&untracked(&path)), &vault).expect("read");
    assert_eq!(view.diff, Preview::Sensitive);
    assert!(view.editor_policy.sensitive(value));
    assert!(!view.editor_policy.sensitive("ordinary reviewed result"));
    assert!(!format!("{:?}", view.editor_policy).contains(value));
  }
  git(fixture.root(), &["add", ".env"]);
  let session = fixture.open();
  let tree = session
    .selected_commit_tree(&[LocalPath::new(".env").expect("path")])
    .expect("tree");
  assert_eq!(
    session.selected_commit_preview(&tree, &vault).expect("preview"),
    Preview::Sensitive
  );
  assert!(matches!(
    session.selected_commit_preview(&tree, &Vault(None)),
    Err(orchester_workspace::tasks::local_integration::IntegrationError::SensitiveMetadata)
  ));
}
#[test]
fn local_integration_checked_review_detects_external_changes_in_the_task_checkout() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("fresh check");
  let exact = session.operation().checked().expect("checked").clone();
  drop(session);
  assert!(
    read(fixture.root(), fixture.run, None, &Vault(None))
      .expect("view")
      .checked_current
  );
  fs::write(fixture.task_root.join("task.txt"), "external task edit\n").expect("edit");
  assert!(
    !read(fixture.root(), fixture.run, None, &Vault(None))
      .expect("stale view")
      .checked_current
  );
  let mut session = fixture.open();
  assert!(session.approve_and_merge(&exact).is_err());
}

/// The untracked snapshot of `path`, as the review shows a new file.
pub(super) fn untracked(path: &LocalPath) -> ShownSnapshot {
  ShownSnapshot {
    path: path.clone(),
    snapshot: Snapshot::Untracked,
  }
}
