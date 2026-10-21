//! Modules of a project's tasks (ORC-48): made with distinct names and valid dates, changed in place over the details
//! read (not over a change made since), and each move of a task into, between and out of modules kept in order; a
//! task reads its module back.

mod support;

use orchester_core::{
  id::{IdempotencyKey, ModuleId},
  module::{ModuleDetails, ModuleError, ModuleState},
  store::TaskStore,
  task::organization::ChangeActor,
};
use orchester_store::StoreError;
use support::{at, name, started_run};

fn details(title: &str, state: ModuleState) -> ModuleDetails {
  ModuleDetails::new(name(title), " Payments ", (None, None), state).expect("details")
}

#[test]
fn modules_are_made_with_distinct_names_and_changed_in_place() {
  let (_repo, mut db, ..) = started_run();
  let checkout = db
    .create_module(details("Checkout", ModuleState::Open), at(10))
    .expect("module");
  assert!(matches!(
    db.create_module(details(" checkout", ModuleState::Open), at(11)),
    Err(StoreError::Module(ModuleError::DuplicateName))
  ));
  let billing = db
    .create_module(details("Billing", ModuleState::Open), at(12))
    .expect("module");
  assert_eq!(
    db.modules()
      .expect("modules")
      .iter()
      .map(|module| module.id())
      .collect::<Vec<_>>(),
    [checkout.id(), billing.id()]
  );
  let closed = db
    .update_module(
      checkout.id(),
      checkout.details(),
      details("Checkout v2", ModuleState::Closed),
    )
    .expect("update");
  assert_eq!(db.module(checkout.id()).expect("read"), Some(closed.clone()));
  assert_eq!(closed.details().description(), "Payments");
  assert!(matches!(
    db.update_module(
      checkout.id(),
      checkout.details(),
      details("Checkout v3", ModuleState::Open)
    ),
    Err(StoreError::Module(ModuleError::ChangedMeanwhile))
  ));
  assert_eq!(
    db.module(checkout.id()).expect("read"),
    Some(closed),
    "a change over details read before another one is refused"
  );
  assert!(matches!(
    db.update_module(
      billing.id(),
      billing.details(),
      details("CHECKOUT V2", ModuleState::Open)
    ),
    Err(StoreError::Module(ModuleError::DuplicateName))
  ));
  assert!(matches!(
    db.update_module(
      ModuleId::generate(),
      billing.details(),
      details("Other", ModuleState::Open)
    ),
    Err(StoreError::Module(ModuleError::NotFound(_)))
  ));
}

#[test]
fn modules_keep_every_move_of_a_task() {
  let (_repo, mut db, run, _) = started_run();
  let task = run.task();
  let first = db
    .create_module(details("First", ModuleState::Open), at(10))
    .expect("module");
  let second = db
    .create_module(details("Second", ModuleState::Open), at(10))
    .expect("module");
  assert!(
    db.set_task_module(task, Some(first.id()), ChangeActor::Person, at(20))
      .expect("in")
  );
  assert!(
    !db
      .set_task_module(task, Some(first.id()), ChangeActor::Person, at(21))
      .expect("again")
  );
  assert_eq!(db.task(task).expect("read").expect("task").module(), Some(first.id()));
  assert!(
    db.set_task_module(task, Some(second.id()), ChangeActor::Orchester, at(22))
      .expect("moved")
  );
  assert!(
    db.set_task_module(task, None, ChangeActor::Person, at(23))
      .expect("out")
  );
  assert_eq!(db.task(task).expect("read").expect("task").module(), None);
  let moves: Vec<_> = db
    .module_changes(first.id())
    .expect("history")
    .iter()
    .map(|change| (change.from, change.to, change.actor, change.at))
    .collect();
  assert_eq!(
    moves,
    [
      (None, Some(first.id()), ChangeActor::Person, at(20)),
      (Some(first.id()), Some(second.id()), ChangeActor::Orchester, at(22)),
    ]
  );
  assert_eq!(db.module_changes(second.id()).expect("history").len(), 2);
  assert!(matches!(
    db.set_task_module(task, Some(ModuleId::generate()), ChangeActor::Person, at(24)),
    Err(StoreError::Module(ModuleError::NotFound(_)))
  ));
  let details = db.task(task).expect("read").expect("task").details().clone();
  let created = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(25))
    .expect("task");
  assert_eq!(created[0].module(), None, "a new task is in no module");
}
