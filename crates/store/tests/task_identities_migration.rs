//! The migration to task identities (team authority P3): a populated project database from before it opens with every
//! row it held, each task with a local identity, its base branch kept, every reference whole and its tasks behaving as
//! before; a shared task's identity names its server project.
mod support;

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{IdempotencyKey, ProjectId, ServerId},
  integration::base::TaskBase,
  run::BranchName,
  sharing::TaskRevision,
  source::SourcePath,
  store::{ConversationStore, RunStore, TaskStore},
  task::identity::{TaskOwner, TeamTaskRef},
};
use orchester_store::ProjectDb;
use rusqlite::Connection;
use support::{at, copied_at_version, started_run};

/// The migrations this change appends: task identities and run inputs.
const ADDED: u32 = 2;

fn database(repo: &Path) -> std::path::PathBuf {
  repo.join(".orchester").join("orchester.db")
}

fn schema_version(connection: &Connection) -> u32 {
  connection
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

/// The number of rows of each table of `connection`.
fn row_counts(connection: &Connection) -> BTreeMap<String, i64> {
  let tables: Vec<String> = connection
    .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
    .expect("tables")
    .query_map([], |row| row.get(0))
    .expect("tables")
    .collect::<Result<_, _>>()
    .expect("tables");
  tables
    .into_iter()
    .map(|table| {
      let count = connection
        .query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |row| row.get(0))
        .expect("count");
      (table, count)
    })
    .collect()
}

#[test]
fn task_identities_migration_keeps_every_row_of_a_local_project() {
  let (repo, mut db, run, _) = started_run();
  let task = run.task();
  let message = Message::from_person(
    task,
    Some(run.id()),
    Recipient::Orchestrator,
    MessageBody::new("Use the new API.").expect("a message"),
    at(12),
  );
  db.send_message(IdempotencyKey::generate(), &message).expect("sent");
  let sources = vec![SourcePath::new(env!("CARGO_MANIFEST_DIR")).expect("an absolute path")];
  db.set_task_sources(task, &sources).expect("sources");
  let other = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![db.task(task).expect("read").expect("task").details().clone()],
      at(13),
    )
    .expect("another task")
    .remove(0);
  let latest = schema_version(&Connection::open(database(repo.path())).expect("open"));
  drop(db);

  let old = copied_at_version(&database(repo.path()), latest - ADDED);
  let before = {
    let connection = Connection::open(database(old.path())).expect("open");
    connection
      .execute(
        "UPDATE tasks SET base_choice = 'branch', base_branch = 'staging' WHERE id = ?1",
        [task.to_string()],
      )
      .expect("the task's own base branch");
    row_counts(&connection)
  };
  let mut db = ProjectDb::open(old.path()).expect("migrate");
  let connection = Connection::open(database(old.path())).expect("open");
  assert_eq!(schema_version(&connection), latest);
  let after = row_counts(&connection);
  for (table, count) in &before {
    assert_eq!(after.get(table), Some(count), "{table} keeps its rows");
  }
  assert_eq!(after["task_identities"], before["tasks"]);
  assert_eq!(after["task_bases"], 1);
  let broken: i64 = connection
    .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))
    .expect("check");
  assert_eq!(broken, 0);

  let read = db.task(task).expect("read").expect("task");
  assert_eq!(
    read.base(),
    &TaskBase::Branch(BranchName::new("staging").expect("branch"))
  );
  assert_eq!(
    db.task(other.id()).expect("read").expect("task").base(),
    &TaskBase::Inherit
  );
  for id in [task, other.id()] {
    let identity = db.task_identity(id).expect("read").expect("identity");
    assert_eq!(identity.owner, TaskOwner::Local);
  }
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
  assert_eq!(db.messages(task).expect("messages"), [message]);
  assert_eq!(db.task_sources(task).expect("sources"), sources);

  let saved = db
    .set_task_base(task, read.revision(), &TaskBase::Inherit)
    .expect("set")
    .saved()
    .expect("a local task's base changes with its revision");
  assert_eq!(saved.base(), &TaskBase::Inherit);
  assert!(saved.revision().get() > read.revision().get());
  db.update_task(other.id(), other.revision(), other.details().clone())
    .expect("edit")
    .saved()
    .expect("a migrated task changes here");
  db.delete_task(other.id(), at(20))
    .expect("a migrated task is deleted as before");
  assert_eq!(db.task_identity(other.id()).expect("read"), None);
}

#[test]
fn task_identities_migration_gives_shared_tasks_their_server_identity() {
  let (repo, mut db, run, _) = started_run();
  let local = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![db.task(run.task()).expect("read").expect("task").details().clone()],
      at(13),
    )
    .expect("another task")
    .remove(0);
  let latest = schema_version(&Connection::open(database(repo.path())).expect("open"));
  drop(db);
  let old = copied_at_version(&database(repo.path()), latest - ADDED);
  let (server, project) = (ServerId::generate(), ProjectId::generate());
  {
    let connection = Connection::open(database(old.path())).expect("open");
    connection
      .execute(
        "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
         last_synced_at) VALUES (1, ?1, ?2, 'ORC', 'member', 1, 0, 1)",
        [server.to_string(), project.to_string()],
      )
      .expect("a link");
    connection
      .execute(
        "UPDATE tasks SET sharing = 'shared', revision = 4 WHERE id = ?1",
        [run.task().to_string()],
      )
      .expect("a shared task");
  }
  let db = ProjectDb::open(old.path()).expect("migrate");
  let shared = db.task_identity(run.task()).expect("read").expect("identity");
  assert_eq!(
    shared.owner,
    TaskOwner::Team {
      task: TeamTaskRef {
        server,
        project,
        task: run.task(),
      },
      recorded_revision: TaskRevision::try_from(4).expect("revision"),
    }
  );
  let kept = db.task_identity(local.id()).expect("read").expect("identity");
  assert_eq!(kept.owner, TaskOwner::Local);
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
}
