use orchester_core::id::SecretId;

use super::*;

fn add_key(repo: &TempDir, db: &mut ProjectDb) -> SecretId {
  secret::add(
    db,
    &Vault { available: true },
    db_project_id(repo.path()),
    NewSecret {
      name: SecretName::new("CHECK_KEY").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::NEW_SECRET,
    },
    &SecretValue::new(TEST_KEY.to_owned()).expect("synthetic value"),
    Timestamp::now(),
  )
  .expect("key metadata")
  .id()
}

#[tokio::test]
async fn decision_connections_unavailable_and_archived_keys_fail_closed() {
  let (repo, mut db) = project();
  let mut value = draft("http://localhost:8000/v1");
  value.key = Some(SecretId::generate());
  assert!(matches!(
    save_in(repo.path(), &value, Writer::Automation),
    Err(DecisionError::KeyUnavailable)
  ));
  let key = add_key(&repo, &mut db);
  value.key = Some(key);
  let id = save_in(repo.path(), &value, Writer::Automation).expect("save");
  let checked = check_in(repo.path(), &Vault { available: false }, id, None)
    .await
    .expect("record missing key");
  assert_eq!(
    checked.last_check().expect("check").result,
    DecisionCheckResult::KeyRefused
  );
  secret::archive(
    &mut db,
    &Vault { available: true },
    db_project_id(repo.path()),
    key,
    Timestamp::now(),
  )
  .expect("archive");
  assert!(matches!(
    save_in(repo.path(), &value, Writer::Automation),
    Err(DecisionError::KeyUnavailable)
  ));
  let checked = check_in(repo.path(), &Vault { available: true }, id, None)
    .await
    .expect("record archived key");
  assert_eq!(
    checked.last_check().expect("check").result,
    DecisionCheckResult::KeyRefused
  );
  assert_eq!(ProjectConnections::load(&db).expect("load").archived_keys.len(), 1);
}

#[tokio::test]
async fn decision_connections_credential_echoes_never_enter_models() {
  let (repo, mut db) = project();
  let key = add_key(&repo, &mut db);
  let mut value = draft("http://localhost:8000/v1");
  value.key = Some(key);
  let id = save_in(repo.path(), &value, Writer::Automation).expect("save");
  value.id = Some(id);
  let escaped = TEST_KEY
    .chars()
    .map(|letter| format!("\\u{:04x}", u32::from(letter)))
    .collect::<String>();
  for token in [TEST_KEY.to_owned(), escaped] {
    let body = format!(r#"{{"models":[{{"name":"{token}","description":"","release_date":""}}]}}"#);
    let (url, task) = server("200 OK", &body, "", true).await;
    value.base_url = DecisionServerUrl::new(&url).expect("url");
    save_in(repo.path(), &value, Writer::Automation).expect("edit");
    let checked = check_in(repo.path(), &Vault { available: true }, id, None)
      .await
      .expect("check");
    task.await.expect("server complete");
    assert_eq!(
      checked.last_check().expect("check").result,
      DecisionCheckResult::NotSystemOneApi
    );
    assert!(checked.models().is_empty());
    assert!(!format!("{checked:?}").contains(TEST_KEY));
  }
}

#[tokio::test]
async fn decision_connections_oversized_and_ambiguous_responses_fail_closed() {
  let (repo, _) = project();
  for body in [
    " ".repeat(1024 * 1024 + 1),
    r#"{"models":[{"name":"valid","description":"","release_date":""}],"data":[{"id":"other"}]}"#.to_owned(),
  ] {
    let (url, task) = server("200 OK", &body, "", false).await;
    let id = save_in(repo.path(), &draft(&url), Writer::Automation).expect("save");
    let checked = check_in(repo.path(), &Vault { available: false }, id, None)
      .await
      .expect("check");
    task.await.expect("server complete");
    assert_eq!(
      checked.last_check().expect("check").result,
      DecisionCheckResult::NotSystemOneApi
    );
    delete_in(repo.path(), id).expect("remove");
  }
}
