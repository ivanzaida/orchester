use std::fs;

use orchester_core::{
  id::SecretId,
  integration::local::{LocalOperation, LocalPath},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::SecretStore,
  time::Timestamp,
};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::{IntegrationError, read};

use super::{
  privacy::{Vault, untracked},
  support::Fixture,
};

#[test]
fn local_integration_known_secret_metadata_is_refused_and_debug_preserves_actual_private_identity() {
  let fixture = Fixture::new(false);
  let private = "private-known-metadata-value";
  let name = format!("notes-{private}.txt");
  fs::write(fixture.root().join(&name), "ordinary original bytes").expect("private fixture");
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.create_secret(&Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("FIXTURE").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  ))
  .expect("private metadata");
  let path = LocalPath::new(&name).expect("path");
  let scratch =
    orchester_exec::local_git::prepare_scratch(fixture.root(), &[".orchester", "privacy-test"]).expect("scratch");
  let git = LocalGit::new(fixture.root(), &scratch).expect("git");
  let mut checkout = git.snapshot().expect("snapshot");
  assert!(checkout.files.iter().any(|file| file.path == path));
  checkout.branch = private.into();
  checkout.root = format!("/private/{private}");
  checkout.git_dir = format!("/private/{private}/.git");
  let operation = LocalOperation::new(fixture.run, checkout.clone()).expect("operation");
  assert!(!format!("{path:?} {checkout:?} {operation:?}").contains(private));
  assert_eq!(path.as_str(), name);
  assert!(matches!(
    read(
      fixture.root(),
      fixture.run,
      Some(&untracked(&path)),
      &Vault(Some(private))
    ),
    Err(IntegrationError::SensitiveMetadata)
  ));
  assert_eq!(
    fs::read(fixture.root().join(&name)).expect("unchanged original"),
    b"ordinary original bytes"
  );
}
