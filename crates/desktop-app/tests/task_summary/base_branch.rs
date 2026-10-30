//! A run the runner refused for want of a branch (`Base branch · redesign`, `Refused start` `J18QL` in `PgNtj`): the
//! live task in no module, with no base branch, runs again from a checkout whose HEAD is detached. The header's status
//! line says so with *Set base branch* and *Run again*, no card, and Properties › Base says *No branch*.

use std::process::Command;

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::store::{RunStore, TaskStore};
use orchester_store::ProjectDb;

use crate::{Page, confirming_run_again};

/// Detaches the HEAD of the live scenario's checkout at the commit it is on.
fn detach_head(page: &Page) {
  let repo = page.window.data.dir.path().join("repo-live");
  let status = Command::new("git")
    .arg("-C")
    .arg(&repo)
    .args(["checkout", "--quiet", "--detach"])
    .status()
    .expect("git runs");
  assert!(status.success(), "git checkout --detach");
}

/// The runs of the task titled *Typed errors per module*.
fn runs(page: &Page) -> usize {
  let db = ProjectDb::open(&page.window.data.dir.path().join("repo-live")).expect("the project");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == "Typed errors per module")
    .expect("the task");
  db.runs_of_task(task.id()).expect("runs").len()
}

/// Presses Run again's *Start run* and waits for the refusal in the header.
fn refused(page: &mut Page) {
  page.click("task-run-again-start");
  page.wait("the refusal", |window| window.texts("task-no-branch").is_some());
}

#[test]
fn task_base_branch_a_start_refused_for_want_of_a_branch_says_so_in_the_header() {
  let mut page = Page::live();
  let key = confirming_run_again(&mut page);
  detach_head(&page);
  refused(&mut page);
  assert_eq!(page.text("task-no-branch-title"), "Can't start: no branch checked out");
  assert_eq!(
    page.text("task-no-branch-why"),
    format!("{key} has no base branch and is in no module; set one, or check out a branch")
  );
  assert!(page.text("task-no-branch-set").ends_with("Set base branch"));
  assert!(page.text("task-no-branch-run-again").ends_with("Run again"));
  assert!(!page.has("task-run-again-confirm"), "the confirmation closed");
  assert!(!page.has("task-decision"), "no card");
  assert_eq!(runs(&page), 1, "nothing started");
  // Properties › Base says why too.
  assert_eq!(page.text("task-property-base-branch"), "No branch");
  let why = page.text("task-property-base-source");
  assert!(
    why.starts_with("the checkout is on no branch (detached at ")
      && why.ends_with("Set a base branch for the task or its module, or check out a branch"),
    "{why}"
  );
  // Run again opens the confirmation in its place.
  page.click("task-no-branch-run-again");
  page.wait("the confirmation again", |window| {
    window.texts("task-run-again-confirm").is_some() && window.texts("task-no-branch").is_none()
  });
  assert_ne!(page.text("task-property-base-branch"), "No branch");
  // Set base branch opens Edit task, where the Base branch chip is.
  refused(&mut page);
  page.click("task-no-branch-set");
  page.wait("Edit task", |window| {
    matches!(
      window.navigation().route_now(),
      Route::Project(_, ProjectPage::EditTask(edited)) if edited.to_string() == key
    )
  });
  let services = page.window.launch.services().expect("the session started").clone();
  services.runners.terminate();
}
