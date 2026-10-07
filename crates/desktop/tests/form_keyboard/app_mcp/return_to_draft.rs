//! `return_to_draft`: a queued task its group holds for good goes back to the drafts, as the task
//! list's Return to drafts takes it, and the refusals of a task that is not queued. Run with
//! `cargo test -p orchester-desktop --test form_keyboard app_mcp_return_to_draft`.

use orchester_core::{store::TaskStore, task::TaskStatus};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{agent, ok, refused};
use crate::{harness::Desktop, task_bulk::release::held_group, task_run::has};

#[test]
fn app_mcp_return_to_draft_takes_a_held_task_back_and_refuses_a_draft() {
  let mut desktop = Desktop::with_seeded_project_and_app_mcp(held_group);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  assert!(crate::github::shows(
    &mut desktop,
    "task-row-KON-1",
    "Held · a task failed"
  ));
  let mut client = agent(&desktop);
  let draft = refused(
    &mut client,
    "return_to_draft",
    &json!({"project": "Kontur", "key": "KON-3"}),
  );
  assert_eq!(draft["code"], "not_queued");

  let returned = ok(
    &mut client,
    "return_to_draft",
    &json!({"project": "Kontur", "key": "kon-1"}),
  );
  assert_eq!(returned["task"]["key"], "KON-1");
  assert_eq!(returned["task"]["submission"], "draft");
  let db = ProjectDb::open(desktop.repo()).expect("open");
  let task = db.tasks().expect("tasks")[0].clone();
  assert_eq!(task.status(), TaskStatus::Draft);
  assert!(db.task_group_for_task(task.id()).expect("group").is_none());
  // The task list shows it in place, and the person stays on it.
  desktop.wait_until("the list shows it no longer held", |desktop| {
    !crate::github::shows(desktop, "task-row-KON-1", "Held · a task failed")
  });
  assert!(has(&mut desktop, "tasks-new-task"));

  let again = refused(
    &mut client,
    "return_to_draft",
    &json!({"project": "Kontur", "key": "KON-1"}),
  );
  assert_eq!(again["code"], "not_queued");
}
