//! A shared project has its server project's key (`SharingLink::key`): the first *Share* into a project made on the
//! server under another key gives it to the project, as a join does, and a database shared before that takes it when
//! it opens.
//!
//! `cargo test -p orchester-store --test shared_project_key shared_project_key_`

mod support;

use orchester_core::{
  id::{ProjectId, ServerId},
  project::ProjectKey,
  sharing::MemberRole,
};
use orchester_store::{ProjectDb, ProjectMigration, SharingLink};
use rusqlite::Connection;
use support::{at, copied_at_version, started_run};

fn link(key: &str) -> SharingLink {
  SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new(key).expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(100),
    cursor: 0,
    last_synced_at: at(100),
    removed_at: None,
    address: Some("https://orchester.kontur.lan/teamkit".into()),
  }
}

#[test]
fn shared_project_key_the_first_share_gives_the_project_the_server_projects_key() {
  let (repo, mut db, ..) = started_run();
  assert_eq!(db.project().key().as_str(), "ORC");

  db.start_sharing(&link("TK"), &[], (&[], &Default::default()))
    .expect("shared");

  assert_eq!(db.project().key().as_str(), "TK");
  let read = ProjectDb::open(repo.path()).expect("opened again");
  assert_eq!(read.project().key().as_str(), "TK", "the key is stored");
}

/// The database of `repo` rewound to before the migration, shared under `key` when it names one, and opened again.
fn migrated(repo: &tempfile::TempDir, key: Option<&str>) -> (tempfile::TempDir, ProjectDb) {
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    ProjectDb::version_before(ProjectMigration::SharedProjectKey),
  );
  if let Some(key) = key {
    let connection = Connection::open(old.path().join(".orchester").join("orchester.db")).expect("the old database");
    connection
      .execute(
        "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor,          last_synced_at) VALUES (1, ?1, ?2, ?3, 'owner', 1, 0, 1)",
        [
          ServerId::generate().to_string(),
          ProjectId::generate().to_string(),
          key.to_owned(),
        ],
      )
      .expect("a link");
  }
  let db = ProjectDb::open(old.path()).expect("the migrations apply");
  (old, db)
}

#[test]
fn shared_project_key_a_project_shared_under_another_key_takes_it_when_it_opens() {
  let (repo, db, ..) = started_run();
  drop(db);

  let (_shared, shared) = migrated(&repo, Some("TK"));
  assert_eq!(shared.project().key().as_str(), "TK");
  let (_same, same) = migrated(&repo, Some("ORC"));
  assert_eq!(same.project().key().as_str(), "ORC");
  let (_local, local) = migrated(&repo, None);
  assert_eq!(local.project().key().as_str(), "ORC", "a local project keeps its key");
}
