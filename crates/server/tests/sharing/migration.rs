use orchester_api::{ErrorCode, JoinedVia, ProjectResponse, Projects, ServerInfo, SessionsResponse, UserResponse};
use rusqlite::params;
use sha2::{Digest, Sha256};

use crate::{
  fixture::*,
  support::{TestServer, credential},
};

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
    3
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
    tls: orchester_server::config::TlsMode::Off,
    ..Default::default()
  };
  orchester_server::Server::open(config, server.clock.clone())
    .await
    .expect("reopen version 3 without losing data");
  assert_eq!(
    server
      .database()
      .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get::<_, i64>(0))
      .expect("project preserved"),
    1
  );
  server.finish().await;
}

const ALICE: &str = "0191c7c0-0000-7000-8000-0000000a11ce";
const BOB: &str = "0191c7c0-0000-7000-8000-000000000b0b";
const CAROL: &str = "0191c7c0-0000-7000-8000-0000000ca201";
const KONTUR: &str = "0191c7c0-0000-7000-8000-00000000c0de";
const USED: i64 = 1_790_999_000_000;

fn token(name: &str) -> String {
  format!("orc_{}", name.repeat(64).chars().take(64).collect::<String>())
}

/// An S1b database: three accounts with passwords, their sessions, and three projects identified by repository.
fn populate(db: &rusqlite::Connection) {
  db.execute_batch(include_str!("../../src/db/schema.sql"))
    .expect("v1 schema");
  db.execute_batch(include_str!("../../src/db/sharing.sql"))
    .expect("v2 schema");
  for (id, name, admin, must_change) in [
    (ALICE, "alice", true, false),
    (BOB, "bob", false, false),
    (CAROL, "carol", false, true),
  ] {
    db.execute(
      "INSERT INTO users VALUES (?1,?2,?3,'$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA',?4,NULL,?5,1,1)",
      params![id, name, name.to_uppercase(), admin, must_change],
    )
    .expect("user");
  }
  for (session, user, initial) in [("a", ALICE, false), ("b", BOB, true), ("c", CAROL, false)] {
    let id = format!("0191c7c0-0000-7000-8000-00000000000{session}");
    db.execute(
      "INSERT INTO sessions VALUES (?1,?2,?3,'Old laptop',1,?4,NULL,?5)",
      params![
        id,
        user,
        Sha256::digest(token(session).as_bytes()).to_vec(),
        USED,
        initial
      ],
    )
    .expect("session");
  }
  for (index, remote) in [
    "github.com/acme/kontur",
    "gitlab.example/other/kontur",
    "host.example/team/admin",
  ]
  .into_iter()
  .enumerate()
  {
    let id = if index == 0 {
      KONTUR.to_owned()
    } else {
      format!("0191c7c0-0000-7000-8000-00000000c0d{index}")
    };
    db.execute(
      "INSERT INTO projects VALUES (?1,'KON','Kontur',?2,4,?3,?4,0)",
      params![id, remote, ALICE, 10 + index as i64],
    )
    .expect("project");
  }
  db.execute(
    "INSERT INTO memberships VALUES (?1,?2,'owner',?2,1,'g1')",
    params![KONTUR, ALICE],
  )
  .expect("owner");
  db.execute(
    "INSERT INTO memberships VALUES (?1,?2,'member',?3,1,'g2')",
    params![KONTUR, BOB, ALICE],
  )
  .expect("member");
}

#[tokio::test]
async fn s1c_migration_keeps_s1a_accounts_and_device_tokens_and_revokes_initial_password_sessions() {
  let server = TestServer::start(|config| {
    populate(&rusqlite::Connection::open(config.data_dir.join("orchester-server.db")).expect("old database"));
  })
  .await;
  assert!(
    !server
      .request("GET", "/server", None, None)
      .await
      .json::<ServerInfo>()
      .needs_bootstrap
  );
  let alice = credential(&token("a"));
  let me: UserResponse = server.request("GET", "/auth/me", Some(&alice), None).await.json();
  assert_eq!(
    (me.user.id.to_string().as_str(), me.user.username.as_str()),
    (ALICE, "alice")
  );
  assert!(me.user.admin && !me.user.cloud);
  let sessions: SessionsResponse = server.request("GET", "/auth/sessions", Some(&alice), None).await.json();
  assert_eq!(sessions.sessions.len(), 1);
  assert_eq!(sessions.sessions[0].via, JoinedVia::Before3);
  for revoked in ["b", "c"] {
    let response = server
      .request("GET", "/auth/me", Some(&credential(&token(revoked))), None)
      .await;
    assert_eq!(response.status, 401, "session {revoked}");
  }
  // Bob keeps his account and membership; an account code gives him a device again.
  let code = server
    .server
    .host_account_code("bob".into())
    .await
    .expect("account code");
  let bob = server.join(&code, None, "new laptop").await;
  assert_eq!(bob.user.id.to_string(), BOB);
  let projects: Projects = server.request("GET", "/projects", Some(&bob.token), None).await.json();
  assert_eq!(projects.projects.len(), 1);
  assert_eq!(projects.projects[0].id.to_string(), KONTUR);
  let unlinked: i64 = server
    .database()
    .query_row("SELECT COUNT(*) FROM users WHERE cloud_subject IS NULL", [], |row| {
      row.get(0)
    })
    .expect("users");
  assert_eq!(unlinked, 3);
  server.finish().await;
}

#[tokio::test]
async fn s1c_projects_made_under_row_10_take_slugs_from_their_repository_names() {
  let server = TestServer::start(|config| {
    populate(&rusqlite::Connection::open(config.data_dir.join("orchester-server.db")).expect("old database"));
  })
  .await;
  let alice = credential(&token("a"));
  let found: ProjectResponse = server
    .request("GET", "/projects?slug=kontur", Some(&alice), None)
    .await
    .json();
  assert_eq!(found.project.id.to_string(), KONTUR);
  assert_eq!(found.project.slug, "kontur");
  assert_eq!(found.project.address, "http://localhost/kontur");
  assert_eq!(
    found.project.remote_hint.expect("hint").as_str(),
    "github.com/acme/kontur"
  );
  assert_eq!(found.project.next_number, 4);
  let slugs: Vec<String> = server
    .database()
    .prepare("SELECT slug FROM projects ORDER BY created_at")
    .expect("slugs")
    .query_map([], |row| row.get(0))
    .expect("rows")
    .collect::<Result<_, _>>()
    .expect("slug list");
  assert_eq!(slugs, ["kontur", "kontur-2", "admin-2"]);
  let other = server
    .request("GET", "/projects?slug=kontur-2", Some(&alice), None)
    .await;
  assert_eq!(other.status, 404);
  assert_eq!(other.json::<orchester_api::ApiError>().error, ErrorCode::NotFound);
  let projects: Projects = server.request("GET", "/projects", Some(&alice), None).await.json();
  assert_eq!(projects.projects.len(), 1);
  server.finish().await;
}
