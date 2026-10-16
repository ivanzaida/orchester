use std::sync::Arc;

use orchester_api::{
  Credential,
  identity::{PrivateKey, SigningKey},
};
use orchester_cloud::{Cloud, LATEST_SCHEMA, error::CloudError, mail::SmtpMailer};
use serde_json::json;

use crate::support::{
  PASSWORD, RecordingMailer, TestClock, TestCloud, config, key_encryption_key, postgres, relay::Relay, remember,
};

pub async fn migrations_apply_once_and_a_newer_database_is_refused() {
  let database = postgres().create_database().await;
  let key = key_encryption_key();
  let open = || {
    Cloud::open(
      config(&database, &key, Vec::new()),
      Arc::new(TestClock::at(1_791_000_000_000)),
      Arc::new(RecordingMailer::default()),
    )
  };
  let (first, second) = tokio::join!(open(), open());
  assert!(first.is_ok() && second.is_ok(), "two instances start together");
  let admin = postgres().database(&database).await;
  let row = admin
    .query_one("SELECT count(*), max(version) FROM schema_migrations", &[])
    .await
    .expect("migrations");
  assert_eq!(
    (row.get::<_, i64>(0), row.get::<_, i32>(1)),
    (migration_files(), LATEST_SCHEMA),
    "every migration once, the newest last"
  );
  admin
    .execute(
      "INSERT INTO schema_migrations (version, applied_at) VALUES ($1, now())",
      &[&(LATEST_SCHEMA + 1)],
    )
    .await
    .expect("a newer schema");
  match open().await {
    Err(CloudError::SchemaTooNew(version)) => assert_eq!(version, LATEST_SCHEMA + 1),
    other => panic!("a newer database must be refused, got {:?}", other.map(|_| ())),
  }
  drop(admin);
  postgres().drop_database(&database).await;
}

/// The migrations the binary carries: their numbers may leave a gap for a branch not merged yet (9).
fn migration_files() -> i64 {
  let folder = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations")).expect("the migrations");
  let files = folder.filter(|entry| {
    entry
      .as_ref()
      .is_ok_and(|entry| entry.path().extension().is_some_and(|extension| extension == "sql"))
  });
  i64::try_from(files.count()).expect("a few migrations")
}

/// Security review 2, 3: a database linked under migration 3 may hold a server whose id is not its key's. Migration 4
/// removes it, so the key that id belongs to can link, and keeps the servers whose id is their own.
pub async fn migration_4_removes_servers_whose_id_is_not_their_keys() {
  let database = postgres().create_database().await;
  let admin = postgres().database(&database).await;
  admin
    .batch_execute(concat!(
      include_str!("../../migrations/0001_accounts.sql"),
      include_str!("../../migrations/0002_pending_sign_up.sql"),
      include_str!("../../migrations/0003_servers_invites.sql"),
      "CREATE TABLE schema_migrations (version integer PRIMARY KEY, applied_at timestamptz NOT NULL);
       INSERT INTO schema_migrations VALUES (1, now()), (2, now()), (3, now());
       INSERT INTO accounts VALUES ('0199f0a0-0000-7000-8000-000000000001', 'ivan@example.test', 'Ivan', 'x', now(),
         now(), now());"
    ))
    .await
    .expect("a database at migration 3");
  let key = || {
    SigningKey::new(&PrivateKey::generate().expect("key"))
      .expect("signing key")
      .public_key()
  };
  let (own, squatter) = (key(), key());
  let own_id = own.server_id().as_uuid();
  let foreign_id = uuid::Uuid::new_v4();
  for (id, public_key) in [(own_id, own.encoded()), (foreign_id, squatter.encoded())] {
    admin
      .execute(
        "INSERT INTO servers (id, owner, name, address, public_key, linked_at)
         VALUES ($1, '0199f0a0-0000-7000-8000-000000000001', 'Kontur team', 'https://kontur.test', $2, now())",
        &[&id, &public_key],
      )
      .await
      .expect("a migration 3 server");
  }
  Cloud::open(
    config(&database, &key_encryption_key(), Vec::new()),
    Arc::new(TestClock::at(1_791_000_000_000)),
    Arc::new(RecordingMailer::default()),
  )
  .await
  .expect("migration 4 applies");
  let left: Vec<uuid::Uuid> = admin
    .query("SELECT id FROM servers", &[])
    .await
    .expect("servers")
    .iter()
    .map(|row| row.get(0))
    .collect();
  assert_eq!(left, [own_id]);
  drop(admin);
  postgres().drop_database(&database).await;
}

pub async fn smtp_mailer_sends_through_the_relay() {
  let (relay, address) = Relay::start().await;
  let url = Credential::new(format!("smtp://{address}"));
  let mailer = SmtpMailer::new(&url, "Orchester <no-reply@cloud.example.test>").expect("mailer");
  let cloud = TestCloud::start_with_mailer(Arc::new(mailer)).await;
  assert_eq!(cloud.sign_up("relay@example.test", PASSWORD).await.status, 202);
  let message = relay.first().await;
  assert!(message.contains("To: relay@example.test"));
  assert!(message.contains("Subject: Confirm your Orchester account"));
  assert!(message.contains("https://cloud.example.test/confirm/"));
  cloud.finish().await;
}

pub async fn requests_follow_the_contract() {
  let cloud = TestCloud::start().await;
  remember("leaked secret");
  let json_type = [("Content-Type", "application/json")];
  let malformed = cloud
    .raw(
      "POST",
      "/api/v1/auth/sessions",
      &json_type,
      br#"{"email": "a@example.test", "password": "leaked secret"#,
    )
    .await;
  assert_eq!((malformed.status, malformed.error()), (400, "bad_request".into()));
  assert!(!String::from_utf8_lossy(&malformed.body).contains("leaked secret"));
  let unknown_field = cloud
    .request(
      "POST",
      "/auth/reset",
      None,
      Some(json!({"email": "a@example.test", "admin": true})),
    )
    .await;
  assert_eq!(unknown_field.status, 400);
  let large = vec![b' '; 1024 * 1024 + 1];
  let too_large = cloud.raw("POST", "/api/v1/auth/signup", &json_type, &large).await;
  assert_eq!((too_large.status, too_large.error()), (413, "payload_too_large".into()));
  let missing = cloud.request("GET", "/nothing-here", None, None).await;
  assert_eq!((missing.status, missing.error()), (404, "not_found".into()));
  let method = cloud.request("GET", "/auth/signup", None, None).await;
  assert_eq!((method.status, method.error()), (405, "method_not_allowed".into()));
  let untyped = cloud
    .raw("POST", "/api/v1/auth/reset", &[], br#"{"email": "a@example.test"}"#)
    .await;
  assert_eq!(untyped.status, 415, "a JSON body needs its content type");
  let anonymous = cloud.request("GET", "/me", None, None).await;
  assert_eq!((anonymous.status, anonymous.error()), (401, "unauthorized".into()));
  let wrong_scheme = cloud
    .raw("GET", "/api/v1/me", &[("Authorization", "Basic b3JjYzp4")], b"")
    .await;
  assert_eq!(wrong_scheme.status, 401);
  assert_eq!(
    missing.headers.get("cache-control").map(String::as_str),
    Some("no-store")
  );
  let invalid_email = cloud.sign_up("not-an-email", PASSWORD).await;
  assert_eq!((invalid_email.status, invalid_email.error()), (422, "invalid".into()));
  let short = cloud.sign_up("short@example.test", "too short").await;
  assert_eq!((short.status, short.error()), (422, "invalid".into()));
  cloud.finish().await;
}

/// Review 1, L3 and L6: a database connection that may fall back to plain text, or plain text to a host outside the
/// local network, and a key-encryption key that is not strict hex are refused at start.
pub async fn unsafe_configuration_is_refused() {
  let database = postgres().create_database().await;
  let key = key_encryption_key();
  let url = postgres().url(&database);
  let open = |change: &dyn Fn(&mut orchester_cloud::config::Config)| {
    let mut config = config(&database, &key, Vec::new());
    change(&mut config);
    Cloud::open(
      config,
      Arc::new(TestClock::at(1_791_000_000_000)),
      Arc::new(RecordingMailer::default()),
    )
  };
  let without_mode = url.replace("?sslmode=disable", "");
  let refused = [
    open(&|config| config.database_url = Credential::new(without_mode.clone())).await,
    open(&|config| config.database_url = Credential::new(url.replace("disable", "prefer"))).await,
    open(&|config| {
      config.database_url = Credential::new("postgres://u:p@db.example.com:5432/d?sslmode=disable".into());
    })
    .await,
    open(&|config| config.key_encryption_key = Credential::new("+f".repeat(32))).await,
    open(&|config| {
      // Review 2: `hostaddr` decides where the connection goes, whatever `host` says.
      config.database_url = Credential::new("postgres://u:p@localhost/d?sslmode=disable&hostaddr=203.0.113.5".into());
    })
    .await,
  ];
  for (index, result) in refused.into_iter().enumerate() {
    assert!(
      matches!(result, Err(CloudError::Configuration(_))),
      "configuration {index} must be refused, got {:?}",
      result.map(|_| ())
    );
  }
  assert!(
    open(&|_| {}).await.is_ok(),
    "loopback with sslmode=disable is the tests' own setup"
  );
  postgres().drop_database(&database).await;
}
