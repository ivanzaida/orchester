//! Properties › Base under Branch (`Base branch · redesign`, 03 `qg3JF`, every state in `PgNtj`): the branch the latest
//! run started from and where it came from, said as the run recorded it when the task's base branch changes after it;
//! before a run, what the next one takes.

use orchester_app::routes::TaskTab;
use orchester_core::{
  id::IdempotencyKey,
  integration::base::{BaseChoice, RunBaseSource, TaskBase},
  module::{ModuleDetails, ModuleState},
  name::Name,
  revision::Revision,
  run::{BranchName, NewRun},
  store::RunStore,
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::Page;

/// Sets the base branch of task `key` as a person would on Edit task, and has the window read the project again.
fn set_base(page: &mut Page, key: &str, base: &TaskBase) {
  let task = page.task(key);
  ProjectDb::open(&page.repo)
    .expect("the project")
    .set_task_base(task.id(), task.revision(), base)
    .expect("the base branch is stored")
    .saved()
    .expect("saved as read");
  page.changed_outside();
}

#[test]
fn task_base_branch_property_says_what_the_run_recorded_and_what_changed_since() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  page.open(&key, TaskTab::Summary, "task-property-base");
  let run = ProjectDb::open(&page.repo)
    .expect("the project")
    .runs_of_task(page.task(&key).id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .map(|run| run.number().get())
    .max()
    .expect("a run");
  assert_eq!(page.text("task-property-base-key"), "Base");
  assert_eq!(page.text("task-property-base-branch"), "main");
  assert_eq!(
    page.text("task-property-base-source"),
    format!("checked out when run #{run} started")
  );
  let staging = BranchName::new("staging").expect("a branch name");
  set_base(&mut page, &key, &TaskBase::Branch(staging));
  let kept = format!("run #{run} keeps it · the task now says staging; the next run starts from staging");
  page.window.until("the change since the run is said", |window| {
    window
      .texts("task-property-base-source")
      .is_some_and(|texts| texts.join("") == kept)
  });
  assert_eq!(page.text("task-property-base-branch"), "main", "the run keeps its own");
}

#[test]
fn task_base_branch_property_before_a_run_says_what_the_next_one_takes() {
  let mut page = Page::seeded("task-states");
  let key = page.key("draft, ready to queue");
  page.open(&key, TaskTab::Summary, "task-property-base");
  assert_eq!(page.text("task-property-base-branch"), "Checked-out branch");
  assert_eq!(
    page.text("task-property-base-source"),
    "the next run takes the branch checked out when it starts"
  );
  set_base(
    &mut page,
    &key,
    &TaskBase::Branch(BranchName::new("release/2.0").expect("a branch name")),
  );
  page.window.until("the task's own base branch", |window| {
    window
      .texts("task-property-base-branch")
      .is_some_and(|texts| texts.join("") == "release/2.0 · this task's")
  });
  assert_eq!(page.text("task-property-base-source"), "the next run starts from it");
}

/// A run that took its module's base branch names the module it came from (`PgNtj`, *from module*), and keeps it
/// while the task's next run would take the same.
#[test]
fn task_base_branch_property_says_a_run_took_its_modules_branch() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  page.open(&key, TaskTab::Summary, "task-property-base");
  let task = page.task(&key);
  let mut db = ProjectDb::open(&page.repo).expect("the project");
  let details = ModuleDetails::new(
    Name::new("Webhooks v2").expect("a name"),
    "",
    (None, None),
    ModuleState::Open,
  )
  .expect("details");
  let module = db.create_module(details, Timestamp::now()).expect("module");
  let release = BranchName::new("release/2.0").expect("a branch name");
  db.set_module_base(module.id(), module.base(), BaseChoice::Branch(release.clone()))
    .expect("the module's base branch");
  db.set_task_module(task.id(), Some(module.id()), ChangeActor::Person, Timestamp::now())
    .expect("moved into the module");
  let latest = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .max_by_key(|run| run.number().get())
    .expect("a run");
  let new = NewRun {
    task: task.id(),
    snapshot: latest.snapshot().clone(),
    branch: BranchName::new("orchester/module-base-run").expect("a branch name"),
    base: Revision::new(&"b".repeat(40)).expect("a revision"),
    base_branch: Some(release),
    base_source: RunBaseSource::Module { module: module.id() },
    budget: latest.budget(),
    max_duration: latest.max_duration(),
  };
  db.create_run(IdempotencyKey::generate(), new, Timestamp::now())
    .expect("the run");
  page.changed_outside();
  page.window.until("the run's module named", |window| {
    window
      .texts("task-property-base-source")
      .is_some_and(|texts| texts.join("") == "from module Webhooks v2")
  });
  assert_eq!(page.text("task-property-base-branch"), "release/2.0");
}
