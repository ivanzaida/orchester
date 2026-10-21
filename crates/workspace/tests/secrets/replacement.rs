use std::{
  sync::{atomic::Ordering, mpsc},
  time::Duration,
};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, EndpointPreset},
  id::ConnectionId,
  resource::Draft,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::{ConnectionStore, McpServerStore},
};
use orchester_workspace::secrets::data::{ReplacementOutcome, ReplacementPermission, replace_allowed_in};

use super::*;

fn native(key: SecretId) -> AgentConnection {
  AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Native API").expect("name"),
    AgentProvider::AnthropicApi,
    ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new("https://api.anthropic.com").expect("url"),
      key,
    ),
    Timestamp::from_unix_millis(2),
  )
  .expect("native connection")
}

fn http_tool(key: SecretId) -> Tool {
  let request = HttpRequest::new(
    HttpMethod::Post,
    "https://deploy.example.test/v1".into(),
    "deploy.example.test".into(),
    HttpAuth::Bearer { secret: key },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .expect("request");
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Staging").expect("environment"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("policy");
  Tool::new("", ToolAction::Http(request), Some(policy)).expect("tool")
}

fn replace(repo: &TempDir, vault: &dyn SecretVault, id: SecretId) -> ReplacementOutcome {
  replace_allowed_in(
    repo.path(),
    vault,
    id,
    &SecretValue::new("fixture-only-replacement".into()).expect("fixture"),
    ReplacementPermission::UnusedOnly,
  )
  .expect("authorization")
}

#[test]
fn secrets_replacement_native_key_requires_person_without_mutation() {
  let (repo, mut db) = project_db();
  let active = secret("API_KEY");
  db.create_secret(&active).expect("metadata");
  db.create_connection(&native(active.id())).expect("native binding");
  let vault = CountingVault::default();
  assert_eq!(replace(&repo, &vault, active.id()), ReplacementOutcome::NeedsPerson);
  assert_eq!(vault.0.load(Ordering::SeqCst), 0);
  assert_eq!(
    db.secret(active.id()).expect("metadata").expect("secret").rotated_at(),
    None
  );

  // This tests the domain permission with an in-memory vault; it does not drive a protected UI answer.
  assert_eq!(
    replace_allowed_in(
      repo.path(),
      &vault,
      active.id(),
      &SecretValue::new("fixture-only-replacement".into()).expect("fixture"),
      ReplacementPermission::PersonConfirmed,
    )
    .expect("confirmed"),
    ReplacementOutcome::Replaced
  );
  assert_eq!(vault.0.load(Ordering::SeqCst), 1);
  assert!(
    db.secret(active.id())
      .expect("metadata")
      .expect("secret")
      .rotated_at()
      .is_some()
  );
}

#[test]
fn secrets_replacement_retained_versions_require_person_even_with_unbound_draft() {
  let (repo, mut db) = project_db();
  let active = secret("OLD_KEY");
  let other = secret("OTHER_KEY");
  db.create_secret(&active).expect("metadata");
  db.create_secret(&other).expect("metadata");
  let resource = db
    .create_resource(
      Slug::new("deploy").expect("slug"),
      Name::new("Deploy").expect("name"),
      &http_tool(active.id()),
      Timestamp::from_unix_millis(2),
    )
    .expect("tool")
    .id();
  let first = db
    .publish_draft::<Tool>(resource, Timestamp::from_unix_millis(3))
    .expect("publish");
  db.save_draft(
    resource,
    &Draft::new(
      http_tool(other.id()),
      Some(first.number()),
      Timestamp::from_unix_millis(4),
    ),
  )
  .expect("draft");
  let vault = CountingVault::default();
  assert_eq!(replace(&repo, &vault, active.id()), ReplacementOutcome::NeedsPerson);
  db.publish_draft::<Tool>(resource, Timestamp::from_unix_millis(5))
    .expect("publish other key");
  assert_eq!(replace(&repo, &vault, active.id()), ReplacementOutcome::NeedsPerson);
  assert_eq!(vault.0.load(Ordering::SeqCst), 0);
  assert_eq!(
    db.secret(active.id()).expect("metadata").expect("secret").rotated_at(),
    None
  );
}

struct PausedVault {
  entered: mpsc::Sender<()>,
  release: parking_lot::Mutex<mpsc::Receiver<()>>,
}

impl SecretVault for PausedVault {
  fn store(&self, _key: &VaultKey, _value: &SecretValue) -> Result<(), VaultError> {
    self.entered.send(()).expect("notify vault entry");
    self
      .release
      .lock()
      .recv_timeout(Duration::from_secs(20))
      .expect("release vault barrier");
    Ok(())
  }

  fn read(&self, _key: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _key: &VaultKey) -> Result<(), VaultError> {
    panic!("replacement must not delete");
  }
}

fn concurrent_binding(write: impl Fn(&mut ProjectDb, &Secret) -> Result<(), orchester_store::StoreError>) {
  give_up_on_held_writes_soon();
  let (repo, mut second_db) = project_db();
  let active = secret("CONCURRENT_KEY");
  second_db.create_secret(&active).expect("metadata");
  let (entered_send, entered_recv) = mpsc::channel();
  let (release_send, release_recv) = mpsc::channel();
  let vault = PausedVault {
    entered: entered_send,
    release: parking_lot::Mutex::new(release_recv),
  };
  std::thread::scope(|scope| {
    let replacement = scope.spawn(|| replace(&repo, &vault, active.id()));
    entered_recv
      .recv_timeout(Duration::from_secs(10))
      .expect("replacement reached vault");
    // The second handle attempts a real write while credential mutation is paused. Its wait for the
    // lock must run out, rather than committing a binding before the vault is released.
    let result = write(&mut second_db, &active);
    release_send.send(()).expect("release replacement");
    assert_eq!(
      replacement.join().expect("replacement thread"),
      ReplacementOutcome::Replaced
    );
    assert!(
      matches!(result, Err(orchester_store::StoreError::Sqlite(
      rusqlite::Error::SqliteFailure(error, _)
    )) if error.code == rusqlite::ErrorCode::DatabaseBusy),
      "binding must not commit during rotation"
    );
  });
  write(&mut second_db, &active).expect("binding succeeds after rotation commits");
  let rotated = second_db
    .secret(active.id())
    .expect("metadata")
    .expect("secret")
    .rotated_at();
  assert!(rotated.is_some());
  let count = CountingVault::default();
  assert_eq!(replace(&repo, &count, active.id()), ReplacementOutcome::NeedsPerson);
  assert_eq!(count.0.load(Ordering::SeqCst), 0);
  assert_eq!(
    second_db
      .secret(active.id())
      .expect("metadata")
      .expect("secret")
      .rotated_at(),
    rotated
  );
}

#[test]
fn secrets_replacement_serializes_concurrent_mcp_binding_from_second_db() {
  concurrent_binding(|db, secret| {
    let server = McpServerText {
      name: "remote".into(),
      transport: TransportChoice::Http,
      url: "https://mcp.example.test".into(),
      bearer: Some(secret.id()),
      ..McpServerText::new_server()
    }
    .draft(&[], std::slice::from_ref(secret))
    .expect("server draft");
    let server = orchester_core::connection::mcp::McpServer::new(
      ConnectionId::generate(),
      server.name,
      server.transport,
      server.environment,
      Timestamp::from_unix_millis(2),
    )
    .expect("server");
    db.create_mcp_server(&server)
  });
}

#[test]
fn secrets_replacement_serializes_concurrent_native_binding_from_second_db() {
  concurrent_binding(|db, secret| db.create_connection(&native(secret.id())));
}

#[test]
fn secrets_replacement_serializes_concurrent_tool_binding_from_second_db() {
  concurrent_binding(|db, secret| {
    db.create_resource(
      Slug::new("deploy").expect("slug"),
      Name::new("Deploy").expect("name"),
      &http_tool(secret.id()),
      Timestamp::from_unix_millis(2),
    )
    .map(|_| ())
  });
}
