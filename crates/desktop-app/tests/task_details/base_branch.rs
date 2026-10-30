//! Properties › Base under Branch (`Base branch · redesign`, 03 `qg3JF`, every state in `PgNtj`): the branch the latest
//! run started from and where it came from, said as the run recorded it when the task's base branch changes after it;
//! before a run, what the next one takes.

use orchester_app::routes::TaskTab;
use orchester_core::{integration::base::TaskBase, run::BranchName, store::RunStore};
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
