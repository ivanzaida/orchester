use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_forward_migration_preserves_account_audit_and_is_idempotent() {
  let server = TestServer::start(|config| {
    let db = rusqlite::Connection::open(config.data_dir.join("orchester-server.db")).expect("old database");
    db.execute_batch(include_str!("../../src/db/schema.sql"))
      .expect("v1 schema");
    db.execute(
      "INSERT INTO audit_events(action,at) VALUES ('before_migration',123)",
      [],
    )
    .expect("old audit");
  })
  .await;
  assert_eq!(
    server
      .database()
      .query_row("SELECT schema_version FROM meta", [], |row| row.get::<_, i64>(0))
      .expect("version"),
    2
  );
  assert_eq!(
    server
      .database()
      .query_row(
        "SELECT COUNT(*) FROM audit_events WHERE action='before_migration' AND at=123",
        [],
        |row| row.get::<_, i64>(0)
      )
      .expect("old audit preserved"),
    1
  );
  let owner = server.bootstrap().await;
  project(&server, &owner.token, vec![]).await;
  let config = orchester_server::config::Config {
    data_dir: server.directory.path().into(),
    ..Default::default()
  };
  orchester_server::Server::open(config, server.clock.clone())
    .await
    .expect("reopen version2 without losing data");
  assert_eq!(
    server
      .database()
      .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get::<_, i64>(0))
      .expect("project preserved"),
    1
  );
  server.finish().await;
}
