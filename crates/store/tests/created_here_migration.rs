//! Migration 160, a version marks that its resource was created here as its first version: a team project's database
//! before it, with versions waiting for the server, opens with every row kept and none marked (a resource published
//! from a draft, or made before the mark, is never taken back whole), refuses a mark other than 0 or 1, and marks the
//! versions created from then on.
//! `cargo test -p orchester-store --test created_here_migration created_here_migration`.

mod support;

use std::{
  collections::{BTreeSet, HashMap},
  fs,
};

use orchester_core::{
  id::{ProjectId, ServerId},
  project::{Project, ProjectKey},
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, Slug},
  sharing::MemberRole,
};
use orchester_store::{ProjectDb, SharingLink};
use rusqlite::Connection;
use support::{at, copied_at_version, name};

/// The schema before `shared_publishes` took the mark `created`. Migrations are append-only, so a later one still
/// applies on open.
const BEFORE_CREATED_HERE: u32 = 159;

fn instruction(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("valid instruction")
}

/// A team project in which the person is an owner, with two versions created here and waiting for the server.
fn team_project() -> (tempfile::TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("project");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(100),
    cursor: 0,
    last_synced_at: at(100),
    removed_at: None,
    address: Some("https://orchester.kontur.lan/kontur".into()),
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("shared");
  for (slug, text) in [("rule", "Be brief."), ("tone", "Be kind.")] {
    db.create_published(Slug::new(slug).expect("slug"), name(slug), &instruction(text), at(200))
      .expect("created here");
  }
  (repo, db)
}

#[test]
fn created_here_migration_keeps_every_waiting_version_unmarked_and_marks_the_new_ones() {
  let (repo, db) = team_project();
  let before = db.waiting_publishes().expect("the outbox");
  assert_eq!(before.len(), 2);
  assert!(before.iter().all(|waiting| waiting.created), "created here, so marked");
  drop(db);

  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_CREATED_HERE);
  let old_file = old.path().join(".orchester").join("orchester.db");
  {
    let raw = Connection::open(&old_file).expect("the old database");
    let has_mark: bool = raw
      .query_row(
        "SELECT EXISTS (SELECT 1 FROM pragma_table_info('shared_publishes') WHERE name = 'created')",
        [],
        |row| row.get(0),
      )
      .expect("columns");
    assert!(!has_mark, "before the migration a version has no mark");
    let rows: i64 = raw
      .query_row("SELECT COUNT(*) FROM shared_publishes", [], |row| row.get(0))
      .expect("count");
    assert_eq!(rows, 2, "the rows waited before the migration");
    raw.close().expect("closed");
  }

  let mut migrated = ProjectDb::open(old.path()).expect("migrate the existing project");
  let kept = migrated.waiting_publishes().expect("the outbox");
  assert_eq!(kept.len(), 2, "every row is kept");
  assert!(
    kept.iter().all(|waiting| !waiting.created),
    "none is marked: they were not created as v1 here"
  );
  let ids: BTreeSet<_> = kept.iter().map(|waiting| waiting.resource).collect();
  assert_eq!(ids, before.iter().map(|waiting| waiting.resource).collect());

  migrated
    .create_published(
      Slug::new("new").expect("slug"),
      name("New"),
      &instruction("New."),
      at(300),
    )
    .expect("created after the migration");
  let marked: Vec<_> = migrated
    .waiting_publishes()
    .expect("the outbox")
    .into_iter()
    .filter(|waiting| waiting.created)
    .collect();
  assert_eq!(marked.len(), 1, "only the one created after the migration");
  drop(migrated);

  let rebuilt = Connection::open(&old_file).expect("the migrated database");
  let refused = rebuilt.execute("UPDATE shared_publishes SET created = 2", []);
  assert!(refused.is_err(), "the mark is 0 or 1");
  rebuilt
    .execute("UPDATE shared_publishes SET created = 1 WHERE created = 0", [])
    .expect("1 is a mark");
}
