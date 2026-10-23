//! Modules in shared projects (ORC-78, `docs/architecture/server.md`, "Modules") against a real `orchester-server` on
//! loopback: Share uploads the project's modules and which module each task is in; the task writer makes and changes
//! modules and moves tasks on the server, and the cache takes what the server confirmed; a module or a task that
//! changed meanwhile is refused or merged as core's rules say; a sync brings other members' modules and moves; joining
//! makes a local module of a server module's name that module; Make local keeps them all; offline nothing is sent.
//!
//! `cargo test -p orchester-workspace --test shared_modules shared_modules_`

#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;
#[path = "shared_modules/tools.rs"]
mod tools;

use chrono::NaiveDate;
use orchester_api::{ModuleCreate, ModuleFields, TaskChanges, TaskPatch};
use orchester_core::{
  id::{IdempotencyKey, ModuleId, TaskId},
  module::{ModuleDetails, ModuleError, ModuleState},
  name::Name,
  store::TaskStore,
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_store::StoreError;
use orchester_workspace::{
  sharing::SharingError,
  tasks::writer::{TaskWriter, WriteError},
};
use server_support::TestServer;
use support::{Other, Person, member, shared};

fn details(name: &str, state: ModuleState) -> ModuleDetails {
  let start = NaiveDate::from_ymd_opt(2026, 10, 1);
  let target = NaiveDate::from_ymd_opt(2026, 10, 31);
  ModuleDetails::new(Name::new(name).expect("name"), "", (start, target), state).expect("details")
}

/// Runs `work` with the project's task writer, on a thread that waits for its requests.
fn with_writer<T>(person: &Person, work: impl FnOnce(&mut TaskWriter) -> T) -> T {
  let _context = person.runtime.enter();
  let mut writer = TaskWriter::open(person.root(), &person.servers).expect("writer");
  work(&mut writer)
}

fn refused_as(result: Result<impl std::fmt::Debug, WriteError>, expected: &ModuleError) {
  match result {
    Err(WriteError::Store(StoreError::Module(error))) => assert_eq!(&error, expected),
    other => panic!("expected {expected:?}, got {other:?}"),
  }
}

/// Oleh, a member of Alice's project, who talks to the server directly.
fn oleh(server: &TestServer, alice: &Person) -> Other {
  let oleh = Other::new(server, "oleh", "Oleh Petrenko");
  member(alice, "oleh");
  oleh
}

#[test]
fn shared_modules_share_uploads_the_modules_and_which_module_each_task_is_in() {
  let server = TestServer::start();
  let code = server.code_for("alice", true);
  let alice = Person::signed_in(
    &server,
    ("alice", "Alice Moreau", &code),
    &["Repository switcher", "Merge queue"],
  );
  let mut db = alice.db();
  let checkout = db
    .create_module(details("Checkout", ModuleState::Open), Timestamp::now())
    .expect("a local module");
  let empty = db
    .create_module(details("Later", ModuleState::Closed), Timestamp::now())
    .expect("a module without tasks");
  let task = db.tasks().expect("tasks").remove(0);
  db.set_task_module(task.id(), Some(checkout.id()), ChangeActor::Person, Timestamp::now())
    .expect("in the module");
  drop(db);
  alice.share(&server);
  let oleh = oleh(&server, &alice);
  let project = alice.server().link().project;
  let read = alice
    .block_on(oleh.client.modules(&oleh.token, project, 0))
    .expect("oleh reads the modules");
  let names: Vec<_> = read
    .modules
    .iter()
    .map(|module| (module.id, module.fields.name.as_str(), module.fields.state))
    .collect();
  assert_eq!(
    names,
    [
      (checkout.id(), "Checkout", ModuleState::Open),
      (empty.id(), "Later", ModuleState::Closed)
    ]
  );
  let moves: Vec<_> = read.changes.iter().map(|change| (change.task, change.to)).collect();
  assert_eq!(
    moves,
    [(task.id(), Some(checkout.id()))],
    "every member's burndown has the task"
  );
  let on_server = alice
    .block_on(oleh.client.tasks(&oleh.token, project))
    .expect("tasks")
    .tasks;
  let modules: Vec<_> = on_server.iter().map(|task| task.content.module).collect();
  assert_eq!(modules, [Some(checkout.id()), None]);
  let db = alice.db();
  assert_eq!(
    db.module_revision(checkout.id()).expect("revision").map(|r| r.get()),
    Some(1)
  );
  assert_eq!(
    db.task(task.id()).expect("read").expect("task").module(),
    Some(checkout.id())
  );
  assert_eq!(
    db.module_changes(checkout.id()).expect("moves").len(),
    1,
    "the move made here stays"
  );
}

#[test]
fn shared_modules_the_writer_makes_changes_and_moves_on_the_server() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = oleh(&server, &alice);
  let project = alice.server().link().project;
  let task = alice.tasks().remove(0);
  let made = with_writer(&alice, |writer| {
    writer.create_module(details("Checkout", ModuleState::Open))
  })
  .expect("made");
  assert_eq!(alice.db().module(made.id()).expect("read"), Some(made.clone()));
  assert!(
    with_writer(&alice, |writer| writer.set_task_module(
      task.id(),
      Some(made.id()),
      ChangeActor::Orchester
    ))
    .expect("moved")
  );
  let cached = alice.db().task(task.id()).expect("read").expect("task");
  assert_eq!(cached.module(), Some(made.id()));
  assert_eq!(
    cached.revision(),
    task.revision().next().expect("next"),
    "under the task's revision"
  );
  let history = alice.db().module_changes(made.id()).expect("moves");
  assert_eq!(history.len(), 1);
  assert_eq!(history[0].actor, ChangeActor::Orchester);
  let closed = details("Checkout", ModuleState::Closed);
  let changed = with_writer(&alice, |writer| {
    writer.update_module(made.id(), made.details(), closed.clone())
  })
  .expect("closed");
  assert_eq!(changed.details(), &closed);
  let read = alice
    .block_on(oleh.client.modules(&oleh.token, project, 0))
    .expect("oleh reads");
  assert_eq!(read.modules[0].fields.state, ModuleState::Closed);
  assert_eq!(read.modules[0].revision.get(), 2);
  assert!(
    !with_writer(&alice, |writer| writer.set_task_module(
      task.id(),
      Some(made.id()),
      ChangeActor::Person
    ))
    .expect("already there"),
  );
  refused_as(
    with_writer(&alice, |writer| {
      writer.create_module(details("checkout", ModuleState::Open))
    }),
    &ModuleError::DuplicateName,
  );
  let missing = ModuleId::generate();
  refused_as(
    with_writer(&alice, |writer| {
      writer.set_task_module(task.id(), Some(missing), ChangeActor::Person)
    }),
    &ModuleError::NotFound(missing),
  );
}

#[test]
fn shared_modules_a_change_meanwhile_is_refused_for_a_module_and_merged_for_a_task() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = oleh(&server, &alice);
  let project = alice.server().link().project;
  let made = with_writer(&alice, |writer| {
    writer.create_module(details("Checkout", ModuleState::Open))
  })
  .expect("made");
  let renamed = ModuleFields {
    name: "Checkout v2".into(),
    ..ModuleFields::from_core(made.details())
  };
  alice
    .block_on(oleh.client.update_module(
      &oleh.token,
      project,
      made.id(),
      &orchester_api::ModulePatch {
        idempotency_key: IdempotencyKey::generate(),
        revision: orchester_core::sharing::TaskRevision::FIRST,
        module: renamed,
      },
    ))
    .expect("oleh renames it");
  refused_as(
    with_writer(&alice, |writer| {
      writer.update_module(made.id(), made.details(), details("Payments", ModuleState::Open))
    }),
    &ModuleError::ChangedMeanwhile,
  );
  let now = alice.db().module(made.id()).expect("read").expect("module");
  assert_eq!(now.name().as_str(), "Checkout v2", "the cache has Oleh's version");
  let task = alice.tasks().remove(1);
  alice
    .block_on(oleh.client.update_task(
      &oleh.token,
      project,
      task.id(),
      &TaskPatch {
        revision: task.revision(),
        changes: TaskChanges {
          brief: Some("Reconnect after sleep.".into()),
          ..TaskChanges::default()
        },
      },
    ))
    .expect("oleh's edit");
  assert!(
    with_writer(&alice, |writer| writer.set_task_module(
      task.id(),
      Some(made.id()),
      ChangeActor::Person
    ))
    .expect("moved on top of Oleh's edit")
  );
  let cached = alice.db().task(task.id()).expect("read").expect("task");
  assert_eq!(cached.module(), Some(made.id()));
  assert_eq!(cached.details().brief, "Reconnect after sleep.");
}

#[test]
fn shared_modules_a_sync_brings_other_members_modules_and_moves() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = oleh(&server, &alice);
  let project = alice.server().link().project;
  let id = ModuleId::generate();
  alice
    .block_on(oleh.client.create_module(
      &oleh.token,
      project,
      &ModuleCreate {
        idempotency_key: IdempotencyKey::generate(),
        id,
        module: ModuleFields::from_core(&details("Billing", ModuleState::Open)),
      },
    ))
    .expect("oleh's module");
  let task = alice.tasks().remove(2);
  alice
    .block_on(oleh.client.update_task(
      &oleh.token,
      project,
      task.id(),
      &TaskPatch {
        revision: task.revision(),
        changes: TaskChanges {
          module: Some(Some(id)),
          ..TaskChanges::default()
        },
      },
    ))
    .expect("oleh moves the task");
  alice.block_on(alice.server().sync()).expect("sync");
  let db = alice.db();
  assert_eq!(db.module(id).expect("read").expect("module").name().as_str(), "Billing");
  assert_eq!(db.task(task.id()).expect("read").expect("task").module(), Some(id));
  let moves = db.module_changes(id).expect("moves");
  assert_eq!(moves.len(), 1, "the burndown has Oleh's move");
  assert_eq!(moves[0].task, task.id());
  alice.block_on(alice.server().sync()).expect("sync again");
  assert_eq!(alice.db().module_changes(id).expect("moves").len(), 1, "kept once");
}

/// Bohdan, a member of Alice's project, with a local project of two tasks, the first in a module `checkout` and the
/// second in a module `Billing`.
fn bohdan_with_modules(server: &TestServer, alice: &Person) -> (Person, [TaskId; 2], [ModuleId; 2]) {
  let code = server.code_for("bohdan", false);
  let bohdan = Person::signed_in(server, ("bohdan", "Bohdan Koval", &code), &["Add a cart", "Invoices"]);
  member(alice, "bohdan");
  let mut db = bohdan.db();
  let tasks = db.tasks().expect("tasks");
  let checkout = db
    .create_module(details("checkout", ModuleState::Open), Timestamp::now())
    .expect("module");
  let billing = db
    .create_module(details("Billing", ModuleState::Open), Timestamp::now())
    .expect("module");
  for (task, module) in [(tasks[0].id(), checkout.id()), (tasks[1].id(), billing.id())] {
    db.set_task_module(task, Some(module), ChangeActor::Person, Timestamp::now())
      .expect("in its module");
  }
  (bohdan, [tasks[0].id(), tasks[1].id()], [checkout.id(), billing.id()])
}

#[test]
fn shared_modules_joining_makes_a_module_of_a_server_module_s_name_that_module_and_make_local_keeps_them() {
  let server = TestServer::start();
  let alice = shared(&server);
  let checkout = with_writer(&alice, |writer| {
    writer.create_module(details("Checkout", ModuleState::Open))
  })
  .expect("Alice's module");
  let (bohdan, tasks, [local_checkout, billing]) = bohdan_with_modules(&server, &alice);
  let review = bohdan
    .block_on(bohdan.servers.join_review_at(&bohdan.entry(), bohdan.root(), "kontur"))
    .expect("review");
  bohdan
    .block_on(bohdan.servers.join(
      &bohdan.entry(),
      bohdan.root(),
      &review,
      (vec![tasks[0]], IdempotencyKey::generate()),
    ))
    .expect("join");
  let db = bohdan.db();
  let modules: Vec<_> = db
    .modules()
    .expect("modules")
    .iter()
    .map(|module| (module.id(), module.name().as_str().to_owned()))
    .collect();
  assert!(modules.contains(&(checkout.id(), "Checkout".to_owned())), "{modules:?}");
  assert!(modules.contains(&(billing, "Billing".to_owned())), "{modules:?}");
  assert!(
    db.module(local_checkout).expect("read").is_none(),
    "Bohdan's checkout is Alice's Checkout"
  );
  assert_eq!(
    db.task(tasks[0]).expect("read").expect("task").module(),
    Some(checkout.id())
  );
  assert_eq!(
    db.task(tasks[1]).expect("read").expect("task").module(),
    Some(billing),
    "the task kept Not shared stays in its module"
  );
  assert!(
    db.module_changes(checkout.id())
      .expect("moves")
      .iter()
      .any(|change| change.task == tasks[0]),
    "the move made before joining names the module it became"
  );
  let oleh = oleh(&server, &alice);
  let on_server = alice
    .block_on(oleh.client.modules(&oleh.token, alice.server().link().project, 0))
    .expect("modules")
    .modules;
  assert_eq!(on_server.len(), 2, "Checkout, and Bohdan's Billing");
  drop(db);
  bohdan.block_on(bohdan.server().make_local()).expect("make local");
  let mut db = bohdan.db();
  assert_eq!(db.module_revision(checkout.id()).expect("revision"), None);
  assert_eq!(
    db.task(tasks[0]).expect("read").expect("task").module(),
    Some(checkout.id())
  );
  db.create_module(details("Local again", ModuleState::Open), Timestamp::now())
    .expect("a local project makes its own modules");
}

#[test]
fn shared_modules_offline_nothing_is_sent() {
  let server = TestServer::start();
  let alice = shared(&server);
  let made = with_writer(&alice, |writer| {
    writer.create_module(details("Checkout", ModuleState::Open))
  })
  .expect("made");
  let task = alice.tasks().remove(0);
  drop(server);
  let first = with_writer(&alice, |writer| {
    writer.create_module(details("Offline", ModuleState::Open))
  });
  assert!(
    matches!(&first, Err(WriteError::Sharing(failure)) if matches!(&failure.source, SharingError::Client(error) if error.is_connection()))
  );
  let second = with_writer(&alice, |writer| {
    writer.set_task_module(task.id(), Some(made.id()), ChangeActor::Person)
  });
  assert!(
    matches!(&second, Err(WriteError::Sharing(failure)) if matches!(failure.source, SharingError::Offline)),
    "refused at once while offline: {second:?}"
  );
  let third = with_writer(&alice, |writer| {
    writer.update_module(made.id(), made.details(), details("Offline", ModuleState::Closed))
  });
  assert!(matches!(&third, Err(WriteError::Sharing(failure)) if matches!(failure.source, SharingError::Offline)));
  let db = alice.db();
  assert_eq!(db.modules().expect("modules"), [made], "the cache is unchanged");
  assert_eq!(db.task(task.id()).expect("read").expect("task").module(), None);
}
