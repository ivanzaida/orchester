use std::fs;

use orchester_core::{
  id::SecretId,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  client::{Client, value},
  domain_support::Fixture,
  integration_support::{PRIVATE_VALUE, Page, PrivateVault, contains},
};

#[test]
fn local_policy_legacy_sensitive_filename_is_refused_before_job_and_never_reaches_tree_or_response() {
  let fixture = Fixture::new(false);
  let filename = format!("notes-{PRIVATE_VALUE}.txt");
  fs::write(fixture.root().join(&filename), "ordinary private fixture bytes").expect("file");
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.create_secret(&Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("PRIVATE_FIXTURE").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  ))
  .expect("private metadata");
  let mut page = Page::new(fixture);
  page.ready("local-read-error");
  assert!(!contains(
    page.window.tree.get_element_by_id("app-window").expect("window"),
    PRIVATE_VALUE
  ));
  let client = Client::new(&mut page, false);
  let args = serde_json::json!({"project":db.project().id().to_string(),"run":page.run().to_string()});
  let result = client.call(&mut page, "merge_run", &args);
  assert_eq!(value(&result)["code"], "integration_preflight_failed");
  assert!(!result.to_string().contains(PRIVATE_VALUE));
  let policy = orchester_workspace::tasks::local_integration::LegacyPolicy::new(
    page.root(),
    page.run(),
    orchester_workspace::secrets::Vault::with(std::sync::Arc::new(PrivateVault)),
  )
  .expect("private scope");
  let safe = policy.safe_message(
    orchester_workspace::messages::Message::new("runs.merge.failure.refused").with("detail", PRIVATE_VALUE.to_owned()),
  );
  assert_eq!(safe.key, "localIntegration.sensitiveOutcome");
  assert!(!format!("{safe:?}").contains(PRIVATE_VALUE));
  assert_eq!(
    fs::read(page.root().join(filename)).expect("unchanged"),
    b"ordinary private fixture bytes"
  );
}
#[test]
fn local_policy_sensitive_recorded_base_branch_is_refused_before_legacy_job_metadata() {
  use orchester_core::{
    id::IdempotencyKey,
    run::{BranchName, NewRun},
    store::RunStore,
  };
  let fixture = Fixture::new(false);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let original = db.run(fixture.run).expect("original").expect("exists");
  let created = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: original.task(),
        snapshot: original.snapshot().clone(),
        branch: BranchName::new("private-new-run").expect("branch"),
        base: original.base().clone(),
        base_branch: Some(BranchName::new(PRIVATE_VALUE).expect("private fixture branch")),
        base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
        budget: None,
        max_duration: None,
      },
      Timestamp::now(),
    )
    .expect("new private metadata fixture");
  db.create_secret(&Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("PRIVATE_FIXTURE").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  ))
  .expect("private known-value policy");
  let policy = orchester_workspace::tasks::local_integration::LegacyPolicy::new(
    fixture.root(),
    created.id(),
    orchester_workspace::secrets::Vault::with(std::sync::Arc::new(PrivateVault)),
  )
  .expect("owned private policy");
  let refusal = policy.preflight().expect_err("unsafe branch cannot name an operation");
  assert!(matches!(
    refusal,
    orchester_workspace::tasks::local_integration::IntegrationError::SensitiveMetadata
  ));
  assert!(!format!("{refusal:?}").contains(PRIVATE_VALUE));
  assert!(!format!("{refusal}").contains(PRIVATE_VALUE));
  assert_eq!(
    db.run(fixture.run).expect("original").expect("exists").snapshot(),
    original.snapshot()
  );
}

#[test]
fn local_policy_merge_run_checks_metadata_with_the_sessions_vault() {
  let fixture = Fixture::new(false);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.create_secret(&Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("PRIVATE_FIXTURE").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  ))
  .expect("private metadata");
  let mut page = Page::new(fixture);
  page.ready("local-review-preserve");
  let client = Client::new(&mut page, false);
  let args = serde_json::json!({"project":db.project().id().to_string(),"run":page.run().to_string()});
  // The session's vault knows the secret's value and nothing names it; the system keychain knows no such
  // secret, so a merge checked against it would be refused as unverifiable.
  let result = client.call(&mut page, "merge_run", &args);
  assert_eq!(value(&result)["merge"]["kind"], "fast_forward", "{result}");
}
