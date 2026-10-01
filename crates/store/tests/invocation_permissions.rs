//! Launch policies survive later run changes and migration without inventing historical data.

mod support;

use orchester_core::{
  connection::stream::InvocationOutcome,
  id::InvocationId,
  permission::{
    AgentPermissions, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands, RepositoryAccess,
  },
  store::{ActivityStore, RunStore},
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection;
use support::{at, copied_at_version, model, started_run};

fn policy(commands: CommandPermission) -> LaunchPermissions {
  LaunchPermissions::new(
    AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off),
    OutsideCommands::SandboxApproval,
  )
}

#[test]
fn invocation_permissions_are_written_once_and_survive_save_and_reopen() {
  let (repo, mut db, run, execution) = started_run();
  let mut invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("launch");
  let actual = policy(CommandPermission::AskBefore);
  db.save_invocation_permissions(invocation.id, &actual)
    .expect("launch policy");
  assert!(matches!(
    db.save_invocation_permissions(invocation.id, &policy(CommandPermission::AnyInWorktree)),
    Err(StoreError::InvocationPermissionsNotRecordable(id)) if id == invocation.id
  ));
  invocation.permissions = Some(policy(CommandPermission::AnyInWorktree));
  invocation.session = Some("resumed-thread".into());
  invocation.ended_at = Some(at(20));
  invocation.outcome = Some(InvocationOutcome::Completed);
  db.save_invocation(&invocation)
    .expect("progress cannot rewrite launch policy");
  drop(db);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  let stored = db.invocations(run.id()).expect("read").remove(0);
  assert_eq!(stored.permissions, Some(actual));
  assert_eq!(stored.session.as_deref(), Some("resumed-thread"));
  assert_eq!(stored.ended_at, Some(at(20)));
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("raw");
  assert!(raw.execute("UPDATE invocations SET permissions = NULL", []).is_err());
}

/// Launch permissions recorded before what happened to other commands was keep it unknown.
#[test]
fn invocation_permissions_of_schema_one_leave_outside_commands_unknown() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("launch");
  drop(db);
  let legacy = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  let document = serde_json::json!({"schema": 1, "body": legacy}).to_string();
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("raw");
  raw
    .execute(
      "UPDATE invocations SET permissions = ?1 WHERE id = ?2",
      rusqlite::params![document, invocation.id.to_string()],
    )
    .expect("a launch recorded with schema 1");
  drop(raw);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  let stored = db.invocations(run.id()).expect("read").remove(0);
  assert_eq!(stored.permissions, Some(LaunchPermissions::recorded_before(legacy)));
  assert_eq!(stored.permissions.and_then(|launch| launch.outside_commands()), None);
}

#[test]
fn invocation_permissions_refuse_commands_closed_launches_and_missing_ids() {
  let (_repo, mut db, run, execution) = started_run();
  let command = db
    .start_command_invocation(run.id(), execution, at(10))
    .expect("command");
  let mut agent = db
    .start_invocation(run.id(), execution, &model(), at(20))
    .expect("agent");
  agent.ended_at = Some(at(30));
  agent.outcome = Some(InvocationOutcome::Completed);
  db.save_invocation(&agent).expect("closed");
  for id in [command.id, agent.id] {
    assert!(matches!(
      db.save_invocation_permissions(id, &policy(CommandPermission::AskBefore)),
      Err(StoreError::InvocationPermissionsNotRecordable(found)) if found == id
    ));
  }
  let missing = InvocationId::generate();
  assert!(matches!(
    db.save_invocation_permissions(missing, &policy(CommandPermission::AskBefore)),
    Err(StoreError::InvocationNotFound(id)) if id == missing
  ));
  assert!(
    db.invocations(run.id())
      .expect("read")
      .iter()
      .all(|invocation| invocation.permissions.is_none())
  );
}

/// The migrations before the launch permissions migration; migrations are append-only, so this stays.
const BEFORE_INVOCATION_PERMISSIONS: u32 = 29;

#[test]
fn invocation_permissions_migration_preserves_legacy_session_and_unknown_policy() {
  let (repo, mut db, run, execution) = started_run();
  let mut invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("agent");
  invocation.session = Some("legacy-thread".into());
  db.save_invocation(&invocation).expect("session");
  drop(db);
  // The same rows in a database as it was before the launch permissions migration.
  let current = repo.path().join(".orchester").join("orchester.db");
  let repo = copied_at_version(&current, BEFORE_INVOCATION_PERMISSIONS);
  let db = ProjectDb::open(repo.path()).expect("migrate");
  assert_eq!(db.invocations(run.id()).expect("legacy launch"), [invocation]);
  assert_eq!(db.run(run.id()).expect("read run").expect("run"), run);
}
