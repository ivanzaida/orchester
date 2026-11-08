//! The program path of a team MCP server, kept once per computer in the application database
//! (`docs/design/team-configuration.md`, decision D).
//!
//! `cargo test -p orchester-store --test mcp_server_programs`

use std::path::Path;

use orchester_core::{
  id::{ResourceId, ServerId},
  time::Timestamp,
};
use orchester_store::{AppDb, McpServerProgram, StoreError, StoredServer};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn open() -> (tempfile::TempDir, AppDb) {
  let dir = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  (dir, db)
}

fn add_server(db: &mut AppDb, url: &str) -> ServerId {
  let server = StoredServer {
    id: ServerId::generate(),
    url: url.to_owned(),
    name: "Team".to_owned(),
    plain_http_allowed: false,
    certificate_pin: None,
    added_at: at(1),
    joined_with: None,
    account: None,
  };
  db.add_server(&server, None).expect("add server");
  server.id
}

#[test]
fn mcp_programs_are_set_replaced_read_and_cleared() {
  let (_dir, mut db) = open();
  let server = add_server(&mut db, "https://a.test");
  let resource = ResourceId::generate();
  assert_eq!(db.mcp_server_program(server, resource).expect("read"), None);

  db.set_mcp_server_program(server, resource, Path::new("C:/tools/mcp.exe"), at(5))
    .expect("set");
  let stored = McpServerProgram {
    resource,
    program: "C:/tools/mcp.exe".into(),
    set_at: at(5),
  };
  assert_eq!(db.mcp_server_program(server, resource).expect("read"), Some(stored));

  db.set_mcp_server_program(server, resource, Path::new("D:/mcp"), at(9))
    .expect("replace");
  let replaced = db.mcp_server_program(server, resource).expect("read").expect("set");
  assert_eq!(
    (replaced.program.as_path(), replaced.set_at),
    (Path::new("D:/mcp"), at(9))
  );

  assert!(db.clear_mcp_server_program(server, resource).expect("clear"));
  assert!(!db.clear_mcp_server_program(server, resource).expect("clear again"));
  assert_eq!(db.mcp_server_program(server, resource).expect("read"), None);
}

#[test]
fn mcp_programs_are_per_server_and_listed_for_one_server() {
  let (_dir, mut db) = open();
  let first = add_server(&mut db, "https://a.test");
  let second = add_server(&mut db, "https://b.test");
  let (one, two) = (ResourceId::generate(), ResourceId::generate());
  db.set_mcp_server_program(first, one, Path::new("/a"), at(1))
    .expect("set");
  db.set_mcp_server_program(first, two, Path::new("/b"), at(2))
    .expect("set");
  db.set_mcp_server_program(second, one, Path::new("/c"), at(3))
    .expect("set");

  let mut listed: Vec<_> = db
    .mcp_server_programs(first)
    .expect("list")
    .into_iter()
    .map(|program| (program.resource.to_string(), program.program))
    .collect();
  listed.sort();
  let mut expected = vec![(one.to_string(), "/a".into()), (two.to_string(), "/b".into())];
  expected.sort();
  assert_eq!(listed, expected);
  let other = db.mcp_server_program(second, one).expect("read").expect("set");
  assert_eq!(other.program.as_path(), Path::new("/c"));
  assert_eq!(db.mcp_server_program(second, two).expect("read"), None);
}

#[test]
fn a_program_whose_file_is_gone_stays_stored() {
  let (_dir, mut db) = open();
  let server = add_server(&mut db, "https://a.test");
  let resource = ResourceId::generate();
  let missing = Path::new("Z:/no/such/dir/mcp-server.exe");
  db.set_mcp_server_program(server, resource, missing, at(1))
    .expect("set");
  let stored = db.mcp_server_program(server, resource).expect("read").expect("set");
  assert_eq!(stored.program.as_path(), missing);
  assert!(!stored.program.exists());
}

#[test]
fn a_program_for_an_unknown_server_is_refused() {
  let (_dir, mut db) = open();
  let unknown = ServerId::generate();
  let error = db
    .set_mcp_server_program(unknown, ResourceId::generate(), Path::new("/a"), at(1))
    .expect_err("no such server");
  assert!(matches!(error, StoreError::ServerNotFound(id) if id == unknown));
}

#[test]
fn the_programs_survive_reopening() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  let server = add_server(&mut db, "https://a.test");
  let resource = ResourceId::generate();
  db.set_mcp_server_program(server, resource, Path::new("/a"), at(1))
    .expect("set");
  drop(db);
  let db = AppDb::open(&path).expect("reopen");
  assert!(db.mcp_server_program(server, resource).expect("read").is_some());
}
