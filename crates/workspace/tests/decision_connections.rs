//! Documentation-derived stand-ins, never a real inference request or a real credential.
//! TypeSafe fixture: https://docs.typesafe.ai/api/list-models (read 2026-11-01).
//! Laya fixture: https://api.laya.studio/v1/models (public GET, read 2026-11-01);
//! https://laya.studio/v1/openapi declares that endpoint public.

#[path = "decision_connections/edit.rs"]
mod edit;
#[path = "decision_connections/security.rs"]
mod security;
#[path = "decision_connections/tools.rs"]
mod tools;
use std::fs;

use orchester_core::{
  connection::{DecisionCheck, DecisionCheckResult, DecisionServerUrl},
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::ModelId,
  secret::{self, Environment, NewSecret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use orchester_workspace::connections::{
  data::ProjectConnections,
  decision::{DecisionDraft, DecisionError, Writer, check_in, delete_in, record_checked_in, save_in},
};
use tempfile::TempDir;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
  task::JoinHandle,
};

const TYPESAFE: &str =
  r#"{"models":[{"name":"jev-latest","description":"General-purpose system one model.","release_date":"2026-09-15"}]}"#;
const LAYA: &str = r#"{"object":"list","routing":"auto","data":[{"id":"english","repo":"convaiinnovations/laya","backbone":"ModernBERT-large","context_tokens":512,"languages":"English"}]}"#;
// A synthetic credential used only inside the stand-in's checks; never included in assertion diagnostics.
const TEST_KEY: &str = "stand-in-only-credential";

struct Vault {
  available: bool,
}

impl SecretVault for Vault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Ok(())
  }
  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    if self.available {
      Ok(SecretValue::new(TEST_KEY.to_owned()).expect("synthetic value"))
    } else {
      Err(VaultError::NotFound)
    }
  }
  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

fn project() -> (TempDir, ProjectDb) {
  let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/decision-connections/tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let repo = tempfile::tempdir_in(scratch).expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("git folder");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("DEC").expect("key"),
    Name::new("Decision checks").expect("name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("database");
  (repo, db)
}

fn draft(url: &str) -> DecisionDraft {
  DecisionDraft {
    id: None,
    name: Name::new("Decision server").expect("name"),
    base_url: DecisionServerUrl::new(url).expect("url"),
    key: None,
  }
}

fn model(value: &str) -> ModelId {
  ModelId::new(value).expect("model")
}

async fn server(status: &str, body: &str, headers: &str, with_key: bool) -> (String, JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("stand-in listener");
  let address = listener.local_addr().expect("address");
  let response = format!(
    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n{body}",
    body.len()
  );
  let task = tokio::spawn(async move {
    let (mut stream, _) = listener.accept().await.expect("check connected");
    let mut bytes = Vec::new();
    loop {
      let mut chunk = [0; 4096];
      let count = stream.read(&mut chunk).await.expect("read request");
      assert!(count > 0);
      bytes.extend_from_slice(&chunk[..count]);
      if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
        break;
      }
    }
    let request = String::from_utf8(bytes).expect("HTTP bytes");
    let (head, body) = request.split_once("\r\n\r\n").expect("headers end");
    // No assertion prints the credential-bearing request.
    assert!(head.starts_with("GET /v1/models HTTP/1.1\r\n"));
    assert!(body.is_empty());
    assert!(!head.to_ascii_lowercase().contains("transfer-encoding"));
    assert!(!head.to_ascii_lowercase().contains("content-type"));
    assert_eq!(head.contains(&format!("Bearer {TEST_KEY}")), with_key);
    stream.write_all(response.as_bytes()).await.expect("answer");
  });
  (format!("http://{address}/v1"), task)
}

#[test]
fn decision_connections_add_edit_list_delete_and_person_only() {
  let (repo, db) = project();
  let mut value = draft("http://127.0.0.1:8000/v1");
  let id = save_in(repo.path(), &value, Writer::Automation).expect("automation adds local");
  let listed = ProjectConnections::load(&db).expect("list");
  assert_eq!(listed.decisions.len(), 1);
  assert_eq!(listed.decisions[0].id(), id);
  assert!(
    listed.connections.is_empty(),
    "decision servers are not agent connections"
  );
  value.id = Some(id);
  value.base_url = DecisionServerUrl::new("https://hosted.example/v1").expect("url");
  assert!(matches!(
    save_in(repo.path(), &value, Writer::Automation),
    Err(DecisionError::PersonOnly)
  ));
  assert!(
    db.decision_connection(id)
      .expect("read")
      .expect("saved")
      .is_on_this_computer()
  );
  save_in(repo.path(), &value, Writer::Person).expect("person edits to hosted");
  value.id = None;
  value.name = Name::new("Hosted").expect("name");
  assert!(matches!(
    save_in(repo.path(), &value, Writer::Automation),
    Err(DecisionError::PersonOnly)
  ));
  let hosted = save_in(repo.path(), &value, Writer::Person).expect("person adds hosted");
  delete_in(repo.path(), hosted).expect("delete hosted");
  delete_in(repo.path(), id).expect("delete local");
  assert!(ProjectConnections::load(&db).expect("list").decisions.is_empty());
}

#[tokio::test]
async fn decision_connections_check_typesafe_models_with_key_and_no_request_data() {
  let (repo, mut db) = project();
  let (url, task) = server("200 OK", TYPESAFE, "", true).await;
  let mut value = draft(&url);
  let vault = Vault { available: true };
  let added = secret::add(
    &mut db,
    &vault,
    db_project_id(repo.path()),
    NewSecret {
      name: SecretName::new("DECISION_KEY").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::NEW_SECRET,
    },
    &SecretValue::new(TEST_KEY.to_owned()).expect("synthetic value"),
    Timestamp::now(),
  )
  .expect("secret metadata");
  value.key = Some(added.id());
  let id = save_in(repo.path(), &value, Writer::Automation).expect("save");
  let started = Timestamp::now();
  let checked = check_in(repo.path(), &vault, id, Some(&model("jev-latest")))
    .await
    .expect("check");
  task.await.expect("stand-in completed");
  assert_eq!(checked.models(), [model("jev-latest")]);
  let check = checked.last_check().expect("recorded");
  assert_eq!(check.result, DecisionCheckResult::Connected);
  assert!(check.at >= started);
  assert!(!format!("{:?}", ProjectConnections::load(&db).expect("read")).contains(TEST_KEY));
  assert_eq!(db.decision_connection(id).expect("read").expect("stored"), checked);
}

fn db_project_id(root: &std::path::Path) -> ProjectId {
  ProjectDb::open(root).expect("open").project().id()
}

#[tokio::test]
async fn decision_connections_check_states_and_public_laya_catalog() {
  let (repo, db) = project();
  let mut value = draft("http://localhost:8000/v1");
  let id = save_in(repo.path(), &value, Writer::Automation).expect("save");
  value.id = Some(id);
  for (status, body, selected, expected) in [
    ("200 OK", LAYA, None, DecisionCheckResult::Connected),
    (
      "200 OK",
      TYPESAFE,
      Some(model("retired-model")),
      DecisionCheckResult::ModelGone {
        model: model("retired-model"),
      },
    ),
    (
      "200 OK",
      r#"{"models":[]}"#,
      Some(model("jev-latest")),
      DecisionCheckResult::ModelGone {
        model: model("jev-latest"),
      },
    ),
    ("401 Unauthorized", "refused", None, DecisionCheckResult::KeyRefused),
    ("403 Forbidden", "refused", None, DecisionCheckResult::KeyRefused),
    ("503 Unavailable", "offline", None, DecisionCheckResult::NothingAnswers),
    (
      "404 Not Found",
      "not system one",
      None,
      DecisionCheckResult::NotSystemOneApi,
    ),
    (
      "200 OK",
      r#"{"object":"list","data":[{"id":"gpt-5","object":"model","owned_by":"openai"}]}"#,
      None,
      DecisionCheckResult::NotSystemOneApi,
    ),
    ("200 OK", "invalid JSON", None, DecisionCheckResult::NotSystemOneApi),
    (
      "200 OK",
      r#"{"models":[{"name":"bad id","description":"","release_date":""}]}"#,
      None,
      DecisionCheckResult::NotSystemOneApi,
    ),
  ] {
    let (url, task) = server(status, body, "", false).await;
    value.base_url = DecisionServerUrl::new(&url).expect("url");
    save_in(repo.path(), &value, Writer::Automation).expect("edit");
    let checked = check_in(repo.path(), &Vault { available: false }, id, selected.as_ref())
      .await
      .expect("check");
    task.await.expect("server complete");
    assert_eq!(checked.last_check().expect("check").result, expected);
    if body == LAYA {
      assert_eq!(checked.models(), [model("english")]);
    }
    assert_eq!(db.decision_connection(id).expect("read").expect("stored"), checked);
  }
}

#[tokio::test]
async fn decision_connections_nothing_answers_and_redirects_are_not_followed() {
  let (repo, _) = project();
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("port");
  let unused = format!("http://{}/v1", listener.local_addr().expect("address"));
  drop(listener);
  let mut value = draft(&unused);
  let id = save_in(repo.path(), &value, Writer::Automation).expect("save");
  let checked = check_in(repo.path(), &Vault { available: false }, id, None)
    .await
    .expect("check");
  assert_eq!(
    checked.last_check().expect("check").result,
    DecisionCheckResult::NothingAnswers
  );
  let target = TcpListener::bind("127.0.0.1:0").await.expect("redirect target");
  let headers = format!(
    "Location: http://{}/v1/models\r\n",
    target.local_addr().expect("address")
  );
  let (url, task) = server("302 Found", "", &headers, false).await;
  value.id = Some(id);
  value.base_url = DecisionServerUrl::new(&url).expect("url");
  save_in(repo.path(), &value, Writer::Automation).expect("edit");
  let checked = check_in(repo.path(), &Vault { available: false }, id, None)
    .await
    .expect("check");
  task.await.expect("server complete");
  assert_eq!(
    checked.last_check().expect("check").result,
    DecisionCheckResult::NotSystemOneApi
  );
  assert!(
    tokio::time::timeout(std::time::Duration::from_millis(50), target.accept())
      .await
      .is_err()
  );
}

#[test]
fn decision_connections_keep_renames_refuse_stale_checks_and_used_deletion() {
  let (repo, mut db) = project();
  let mut value = draft("http://localhost:8000/v1");
  let id = save_in(repo.path(), &value, Writer::Automation).expect("save");
  let before = db.decision_connection(id).expect("read").expect("saved");
  value.id = Some(id);
  value.name = Name::new("Renamed").expect("name");
  save_in(repo.path(), &value, Writer::Automation).expect("rename");
  let check = DecisionCheck {
    at: Timestamp::now(),
    result: DecisionCheckResult::Connected,
  };
  let checked =
    record_checked_in(repo.path(), &before, check.clone(), Some(&[model("jev-latest")]), None).expect("record");
  assert_eq!(checked.name(), &value.name);
  save_in(repo.path(), &value, Writer::Automation).expect("rename retains check");
  assert_eq!(
    db.decision_connection(id).expect("read").expect("saved").last_check(),
    Some(&check)
  );
  value.base_url = DecisionServerUrl::new("http://localhost:9000/v1").expect("url");
  save_in(repo.path(), &value, Writer::Automation).expect("change server");
  assert!(matches!(
    record_checked_in(repo.path(), &before, check, Some(&[]), None),
    Err(DecisionError::CheckStale)
  ));
  let changed = db.decision_connection(id).expect("read").expect("saved");
  assert!(changed.models().is_empty());
  assert!(changed.last_check().is_none());
  db.save_decision_reviewer_connection(Some(id)).expect("choose");
  assert!(
    matches!(delete_in(repo.path(), id), Err(DecisionError::Store(StoreError::DecisionConnectionInUse(found))) if found == id)
  );
}
