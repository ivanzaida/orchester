use orchester_core::store::SecretStore;
use orchester_workspace::connections::{
  decision::{check_draft_in, save_with_key_in},
  native::KeySecret,
};

use super::*;

fn new_key() -> KeySecret {
  KeySecret::New(NewSecret {
    name: SecretName::new("DECISION_KEY").expect("name"),
    kind: SecretKind::ApiKey,
    environment: Environment::Development,
  })
}

#[tokio::test]
async fn decision_connections_edit_checks_unsaved_fields_without_storing_connection_or_secret() {
  let (repo, db) = project();
  let mut original = draft("http://127.0.0.1:1/v1");
  let id = save_in(repo.path(), &original, Writer::Person).expect("store");
  let (url, server) = server("200 OK", TYPESAFE, "", true).await;
  original.id = Some(id);
  original.base_url = DecisionServerUrl::new(&url).expect("url");
  let key = new_key();
  let checked = check_draft_in(
    repo.path(),
    &Vault { available: false },
    &original,
    Some(&key),
    Some(SecretValue::new(TEST_KEY.to_owned()).expect("synthetic")),
    Writer::Person,
  )
  .await
  .expect("preview");
  server.await.expect("stand-in");
  assert_eq!(checked.models(), &[model("jev-latest")]);
  assert_eq!(
    checked.last_check().expect("check").result,
    DecisionCheckResult::Connected
  );
  assert!(db.secrets().expect("secrets").is_empty());
  let stored = db.decision_connection(id).expect("read").expect("original");
  assert_eq!(stored.base_url().as_str(), "http://127.0.0.1:1/v1");
  assert!(stored.last_check().is_none());
}

#[test]
fn decision_connections_edit_hosted_changes_are_person_only_before_secret_creation() {
  let (repo, db) = project();
  let mut edit = draft("https://api.typesafe.ai/v1");
  let id = save_in(repo.path(), &edit, Writer::Person).expect("hosted");
  edit.id = Some(id);
  edit.base_url = DecisionServerUrl::new("http://127.0.0.1:8000/v1").expect("local");
  assert!(matches!(
    save_in(repo.path(), &edit, Writer::Automation),
    Err(DecisionError::PersonOnly)
  ));
  assert!(
    save_with_key_in(
      repo.path(),
      &Vault { available: true },
      &edit,
      (
        Some(&new_key()),
        Some(SecretValue::new(TEST_KEY.to_owned()).expect("synthetic"))
      ),
      Writer::Automation,
      None
    )
    .is_err()
  );
  assert!(db.secrets().expect("secrets").is_empty());
  save_with_key_in(
    repo.path(),
    &Vault { available: true },
    &edit,
    (
      Some(&new_key()),
      Some(SecretValue::new(TEST_KEY.to_owned()).expect("synthetic")),
    ),
    Writer::Person,
    None,
  )
  .expect("person saves");
  assert_eq!(db.secrets().expect("secrets").len(), 1);
  assert!(
    db.decision_connection(id)
      .expect("read")
      .expect("stored")
      .key()
      .is_some()
  );
}

#[test]
fn decision_connections_edit_secret_creation_rolls_back_on_connection_failure() {
  let (repo, db) = project();
  let original = draft("http://127.0.0.1:1/v1");
  save_in(repo.path(), &original, Writer::Person).expect("original");
  assert!(
    save_with_key_in(
      repo.path(),
      &Vault { available: true },
      &original,
      (
        Some(&new_key()),
        Some(SecretValue::new(TEST_KEY.to_owned()).expect("synthetic"))
      ),
      Writer::Person,
      None
    )
    .is_err()
  );
  assert!(db.secrets().expect("active secrets").is_empty());
}

#[tokio::test]
async fn decision_connections_edit_automation_cannot_send_a_key_to_an_unsaved_hosted_url() {
  let (repo, db) = project();
  let mut edit = draft("http://127.0.0.1:1/v1");
  let id = save_in(repo.path(), &edit, Writer::Person).expect("original");
  edit.id = Some(id);
  edit.base_url = DecisionServerUrl::new("https://example.test/v1").expect("hosted");
  let result = check_draft_in(
    repo.path(),
    &Vault { available: true },
    &edit,
    Some(&new_key()),
    Some(SecretValue::new(TEST_KEY.to_owned()).expect("synthetic")),
    Writer::Automation,
  )
  .await;
  assert!(result.is_err(), "no unsaved hosted check is authorized");
  assert!(db.secrets().expect("secrets").is_empty());
  assert_eq!(
    db.decision_connection(id)
      .expect("read")
      .expect("stored")
      .base_url()
      .as_str(),
    "http://127.0.0.1:1/v1"
  );
}

#[test]
fn decision_connections_edit_rejects_stale_form_without_creating_secret_or_changing_new_settings() {
  let (repo, mut db) = project();
  let mut form = draft("http://127.0.0.1:1/v1");
  let id = save_in(repo.path(), &form, Writer::Person).expect("original");
  let expected = db.decision_connection(id).expect("read").expect("original");
  form.id = Some(id);
  form.name = Name::new("My rename").expect("name");
  let mut concurrent = expected.clone();
  concurrent.set_server(DecisionServerUrl::new("http://127.0.0.1:2/v1").expect("url"), None);
  concurrent.record_models(vec![model("new-model")]);
  db.save_decision_connection(&concurrent).expect("outside edit");
  let result = save_with_key_in(
    repo.path(),
    &Vault { available: true },
    &form,
    (
      Some(&new_key()),
      Some(SecretValue::new(TEST_KEY.to_owned()).expect("synthetic")),
    ),
    Writer::Person,
    Some(&expected),
  );
  assert_eq!(result, Err(DecisionError::EditStale.message()));
  assert_eq!(db.decision_connection(id).expect("read"), Some(concurrent.clone()));
  assert!(db.secrets().expect("secrets").is_empty());
  form.base_url = concurrent.base_url().clone();
  save_with_key_in(
    repo.path(),
    &Vault { available: true },
    &form,
    (None, None),
    Writer::Person,
    Some(&concurrent),
  )
  .expect("fresh edit saves");
  let saved = db.decision_connection(id).expect("read").expect("saved");
  assert_eq!(saved.name(), &form.name);
  assert_eq!(saved.models(), concurrent.models());
}
