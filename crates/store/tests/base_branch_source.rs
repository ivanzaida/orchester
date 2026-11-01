//! Base branches of tasks and modules in the project database (`Base branch · redesign`, `ri5ru`): a local task's
//! is versioned with its revision, a shared task's stays outside it; a module's is kept, a stale change refused; a run
//! records where its base came from and keeps it whatever its task and module say later; and the project's base
//! branch moves to its open modules that had none, its runs migrating without losing their branch, and the move is
//! forgotten once General said so.
//! `cargo test -p orchester-store --test base_branch_source base_branch_source`.

mod support;

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, ModuleId, ProjectId, ServerId},
  integration::base::{BaseChoice, RunBaseSource, TaskBase},
  module::{ModuleDetails, ModuleError, ModuleState},
  project::ProjectKey,
  revision::Revision,
  run::{BranchName, NewRun, Run},
  sharing::MemberRole,
  store::{RunStore, TaskStore},
  task::{TaskUpdate, organization::ChangeActor},
};
use orchester_store::{CachedTask, ProjectDb, SharingLink, StoreError};
use tempfile::TempDir;

use self::support::{at, copied_at_version, name, started_run};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("valid branch")
}

fn module(db: &mut ProjectDb, title: &str, state: ModuleState) -> ModuleId {
  let details = ModuleDetails::new(name(title), "", (None, None), state).expect("details");
  db.create_module(details, at(2)).expect("module").id()
}

/// Another run of `first`'s task, from `base_branch` taken from `source`.
fn run_from(db: &mut ProjectDb, first: &Run, (number, base_branch, source): (u32, Option<&str>, RunBaseSource)) -> Run {
  let new = NewRun {
    task: first.task(),
    snapshot: first.snapshot().clone(),
    branch: branch(&format!("orchester/orc-1-run-{number}")),
    base: Revision::new(&"b".repeat(40)).expect("revision"),
    base_branch: base_branch.map(branch),
    base_source: source,
    budget: first.budget(),
    max_duration: first.max_duration(),
  };
  db.create_run(IdempotencyKey::generate(), new, at(20)).expect("run")
}

fn schema_version(database: &Path) -> u32 {
  rusqlite::Connection::open(database)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("user_version")
}

#[test]
fn base_branch_source_a_local_tasks_base_is_versioned_with_its_revision() {
  let (repo, mut db, run, _) = started_run();
  let read = db.task(run.task()).expect("read").expect("task");
  assert_eq!(read.base(), &TaskBase::Inherit, "a task inherits by default");
  let staging = TaskBase::Branch(branch("staging"));
  let saved = db
    .set_task_base(read.id(), read.revision(), &staging)
    .expect("set")
    .saved()
    .expect("saved on the revision read");
  assert_eq!(saved.base(), &staging);
  assert_eq!(saved.revision(), read.revision().next().expect("raised"));
  let TaskUpdate::Conflict { current } = db
    .set_task_base(read.id(), read.revision(), &TaskBase::CheckedOut)
    .expect("set")
  else {
    panic!("the task changed after it was read");
  };
  assert_eq!(current, saved);
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.task(read.id()).expect("read"), Some(saved));
  let for_run = reopened.task_for_run(read.id(), run.id()).expect("read").expect("task");
  assert_eq!(for_run.base(), &staging);
  assert_eq!(
    reopened.run(run.id()).expect("read").expect("run").base_source(),
    RunBaseSource::CheckedOut,
    "the run that started keeps its record"
  );
}

#[test]
fn base_branch_source_a_modules_base_is_kept_and_a_stale_change_is_refused() {
  let (repo, mut db, _run, _) = started_run();
  let webhooks = module(&mut db, "Webhooks v2", ModuleState::Open);
  let read = db.module(webhooks).expect("read").expect("module");
  assert_eq!(
    read.base(),
    &BaseChoice::CheckedOut,
    "a module takes the checked-out branch by default"
  );
  let release = BaseChoice::Branch(branch("release/2.0"));
  let saved = db.set_module_base(webhooks, read.base(), release.clone()).expect("set");
  assert_eq!(saved.base(), &release);
  assert!(matches!(
    db.set_module_base(webhooks, &BaseChoice::CheckedOut, BaseChoice::CheckedOut),
    Err(StoreError::Module(ModuleError::ChangedMeanwhile))
  ));
  let details = saved.details().clone();
  let renamed = ModuleDetails::new(name("Webhooks"), "", (None, None), details.state()).expect("details");
  let updated = db.update_module(webhooks, &details, renamed).expect("renamed");
  assert_eq!(updated.base(), &release, "an edit of its details keeps its base branch");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(
    reopened.module(webhooks).expect("read").expect("module").base(),
    &release
  );
  assert!(matches!(
    db.set_module_base(ModuleId::generate(), &BaseChoice::CheckedOut, release),
    Err(StoreError::Module(ModuleError::NotFound(_)))
  ));
}

#[test]
fn base_branch_source_a_run_records_where_its_base_came_from_and_keeps_it() {
  let (repo, mut db, first, _) = started_run();
  let webhooks = module(&mut db, "Webhooks v2", ModuleState::Open);
  db.set_task_module(first.task(), Some(webhooks), ChangeActor::Person, at(3))
    .expect("moved");
  let sources = [
    (2, Some("release/2.0"), RunBaseSource::Module { module: webhooks }),
    (3, Some("staging"), RunBaseSource::Task),
    (4, None, RunBaseSource::CheckedOut),
  ];
  let runs: Vec<Run> = sources
    .iter()
    .map(|&source| run_from(&mut db, &first, source))
    .collect();
  let task = db.task(first.task()).expect("read").expect("task");
  db.set_task_base(task.id(), task.revision(), &TaskBase::CheckedOut)
    .expect("set")
    .saved()
    .expect("saved on the revision read");
  db.set_module_base(webhooks, &BaseChoice::CheckedOut, BaseChoice::Branch(branch("main")))
    .expect("set");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  for (run, (_, base_branch, source)) in runs.iter().zip(sources) {
    let read = reopened.run(run.id()).expect("read").expect("run");
    assert_eq!(read.base_source(), source);
    assert_eq!(read.base_branch().map(BranchName::as_str), base_branch);
    assert_eq!(
      &read, run,
      "a change of the task's or module's base leaves the run as it started"
    );
  }
}

#[test]
fn base_branch_source_a_shared_tasks_base_stays_outside_its_revision() {
  let (_repo, mut db, run, _) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let details = task.details();
  let cached = CachedTask {
    id: task.id(),
    number: task.key().number(),
    revision: task.revision(),
    title: details.title.clone(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: None,
    limits: details.limits,
    status: task.status(),
    organization: db.task_organization(task.id()).expect("organization"),
    module: None,
    former_key: None,
    created_by: "Alice Moreau".into(),
    created_at: task.created_at(),
    updated_by: "Alice Moreau".into(),
    updated_at: at(100),
    claim: None,
    run: None,
  };
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("ORC").expect("key"),
    role: Some(MemberRole::Admin),
    joined_at: at(100),
    cursor: 7,
    last_synced_at: at(100),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (std::slice::from_ref(&cached), &Default::default()))
    .expect("share");
  let shared = db.task(task.id()).expect("read").expect("task");
  let staging = TaskBase::Branch(branch("staging"));
  let saved = db
    .set_task_base(shared.id(), shared.revision(), &staging)
    .expect("set")
    .saved()
    .expect("saved");
  assert_eq!(saved.revision(), shared.revision(), "the server's revision stays");
  assert_eq!(saved.base(), &staging);
  let newer = CachedTask {
    revision: shared.revision().next().expect("raised"),
    title: name("Say hello twice"),
    ..cached
  };
  db.cache_shared_tasks(std::slice::from_ref(&newer), None)
    .expect("cached");
  let read = db.task(task.id()).expect("read").expect("task");
  assert_eq!(read.revision(), newer.revision);
  assert_eq!(
    read.base(),
    &staging,
    "a change from the server keeps this computer's base branch"
  );
}

/// The migrations appended after the base sources' (the server's admins, *Open my draft on vN*, the worktree
/// removals' not-removed state, the disk guard): the schema before it is this many and one more under the latest.
const AFTER_BASE_SOURCES: u32 = 4;

/// A project whose base branch was `set` (none: the checked-out branch) before tasks and modules had theirs, with
/// open modules Webhooks and Billing, closed Legacy, and runs from a detached HEAD, `staging` and `main`, opened at
/// the latest schema: its repository, which holds the database, its modules and its runs as they read now.
fn migrated(set: Option<&str>) -> (TempDir, ProjectDb, [ModuleId; 3], Vec<Run>) {
  let (current, mut db, first, _) = started_run();
  let modules = [
    module(&mut db, "Webhooks v2", ModuleState::Open),
    module(&mut db, "Billing", ModuleState::Open),
    module(&mut db, "Legacy", ModuleState::Closed),
  ];
  let mut runs = vec![first.clone()];
  runs.push(run_from(
    &mut db,
    &first,
    (2, Some("staging"), RunBaseSource::CheckedOut),
  ));
  runs.push(run_from(&mut db, &first, (3, Some("main"), RunBaseSource::CheckedOut)));
  drop(db);
  let database = current.path().join(".orchester").join("orchester.db");
  let latest = schema_version(&database);
  let old = copied_at_version(&database, latest - AFTER_BASE_SOURCES - 1);
  let old_database = old.path().join(".orchester").join("orchester.db");
  if let Some(set) = set {
    rusqlite::Connection::open(&old_database)
      .expect("open")
      .execute("INSERT INTO project_base_branch (id, branch) VALUES (1, ?1)", [set])
      .expect("the project's base branch");
  }
  let db = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&old_database), latest);
  let runs = runs
    .iter()
    .map(|run| db.run(run.id()).expect("read").expect("run"))
    .collect();
  (old, db, modules, runs)
}

#[test]
fn base_branch_source_the_projects_base_branch_moves_to_its_open_modules() {
  let (_repo, db, [webhooks, billing, legacy], runs) = migrated(Some("staging"));
  let staging = BaseChoice::Branch(branch("staging"));
  let base = |id| db.module(id).expect("read").expect("module").base().clone();
  assert_eq!((base(webhooks), base(billing)), (staging.clone(), staging));
  assert_eq!(base(legacy), BaseChoice::CheckedOut, "a closed module is not changed");
  let moved = db.base_branch_move().expect("read").expect("moved");
  assert_eq!(moved.branch, branch("staging"));
  let mut took = moved.modules.clone();
  took.sort();
  let mut open = vec![webhooks, billing];
  open.sort();
  assert_eq!(took, open);
  assert!(moved.at.unix_millis() > 0);
  let records: Vec<_> = runs
    .iter()
    .map(|run| (run.base_branch().map(BranchName::as_str), run.base_source()))
    .collect();
  assert_eq!(
    records,
    [
      (None, RunBaseSource::CheckedOut),
      (Some("staging"), RunBaseSource::Project),
      (Some("main"), RunBaseSource::CheckedOut),
    ],
    "each run keeps its branch; the one that took the project's is recorded so"
  );
}

#[test]
fn base_branch_source_the_move_is_forgotten_once_said_and_the_modules_keep_it() {
  let (_repo, mut db, [webhooks, ..], _) = migrated(Some("staging"));
  db.forget_base_branch_move().expect("forget");
  assert_eq!(db.base_branch_move().expect("read"), None);
  assert_eq!(
    db.module(webhooks).expect("read").expect("module").base(),
    &BaseChoice::Branch(branch("staging")),
    "a module keeps the branch it took"
  );
  db.forget_base_branch_move().expect("forgetting again changes nothing");
}

#[test]
fn base_branch_source_a_project_without_a_base_branch_moves_nothing() {
  let (_repo, db, modules, runs) = migrated(None);
  assert_eq!(db.base_branch_move().expect("read"), None);
  for id in modules {
    assert_eq!(
      db.module(id).expect("read").expect("module").base(),
      &BaseChoice::CheckedOut
    );
  }
  assert!(runs.iter().all(|run| run.base_source() == RunBaseSource::CheckedOut));
  assert_eq!(
    runs[1].base_branch().map(BranchName::as_str),
    Some("staging"),
    "its branch is kept"
  );
}
