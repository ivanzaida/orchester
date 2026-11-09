//! Closing, cancelling, reopening and blocking a team task on its page (decision D4, `Task - v2` `jDN0r` rows 14–17: T1
//! `rvvoN`, T2 `o4K8uq`, T3 `g9S4k`, T4 `V26A3`, T11 `VVhcE`), on the QA app's `team-member`, `team-admin`,
//! `team-admin-work` and `team-claims` states with the stand-in team server, which decides each change by the person's
//! claim and role: what the page offers comes from the task's capabilities, a member without the action reads one lock
//! line and has no button, closing ends the claim, and a reopened task is free.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_lifecycle`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{ProjectPage, Route, TaskView},
  test_support::{Window, project_id},
};
use orchester_core::project::TaskKey;

use super::{prepared, team::State};

fn task_route(key: &str) -> Route {
  let key: TaskKey = key.parse().expect("a task key");
  Route::Project(project_id(), ProjectPage::task(key, TaskView::Summary, false))
}

/// The window on the task `key` in `state`, once its claim line shows.
fn on(state: State, key: &str) -> (Window, TeamServer) {
  let (mut window, server) = prepared(state);
  go(&mut window, key);
  (window, server)
}

fn go(window: &mut Window, key: &str) {
  window.navigation().go(task_route(key));
  window.until("the claim line", |window| {
    window.texts("task-team-claim-title").is_some() || window.texts("task-team-closed-title").is_some()
  });
}

fn offered(window: &mut Window, id: &str) -> bool {
  window.texts(id).is_some()
}

/// T1 `rvvoN`: the claim's holder, a member, closes their own task: Mark as done and no Cancel task, and closing ends
/// the claim, which the page says as the task's closed line.
#[test]
fn team_qa_lifecycle_the_holder_marks_their_task_done() {
  let (mut window, _server) = on(State::Member, "KON-12");
  assert!(offered(&mut window, "task-team-done"), "the holder may close it");
  assert!(offered(&mut window, "task-team-block"), "and block it");
  assert!(!offered(&mut window, "task-team-cancel"), "Cancel is an Admin's");

  window.press("task-team-done");
  window.until("the task to be closed", |window| {
    window.texts("task-team-closed-title").is_some()
  });
  assert!(window.text("task-team-closed-title").starts_with("Done · marked by "));
  assert!(
    !offered(&mut window, "task-team-done"),
    "a closed task has no Mark as done"
  );
  assert!(
    !offered(&mut window, "task-reopen"),
    "a member does not reopen it: one lock line says who does"
  );
  assert!(
    window
      .text("task-team-lifecycle-lock-text")
      .starts_with("Only Admins reopen a team task")
  );
}

/// T2 `o4K8uq`, T4 `V26A3`: an Admin cancels a task another person holds, which ends that claim; Reopen offers Back to
/// Todo and Back to Backlog, and the reopened task is free.
#[test]
fn team_qa_lifecycle_an_admin_cancels_and_reopens() {
  let (mut window, _server) = on(State::AdminWork, "KON-13");
  assert_eq!(window.text("task-team-claim-title"), "Claimed by mara");
  assert!(offered(&mut window, "task-team-done") && offered(&mut window, "task-team-cancel"));

  window.press("task-team-cancel");
  window.until("the task to be canceled", |window| {
    window.texts("task-team-closed-title").is_some()
  });
  assert!(window.text("task-team-closed-title").starts_with("Canceled by "));

  window.press("task-reopen");
  window.until("Reopen's choices", |window| window.texts("task-reopen-todo").is_some());
  assert!(offered(&mut window, "task-reopen-backlog"));
  assert_eq!(
    window.text("task-reopen-free"),
    "A reopened task is free: nobody holds it, and nothing runs until someone claims it."
  );
  window.press("task-reopen-todo");
  window.until("the task to be open again", |window| {
    window.texts("task-team-closed-title").is_none()
  });
  window.until("a free task", |window| {
    window
      .texts("task-team-claim-title")
      .is_some_and(|title| title.concat().starts_with("Nobody holds"))
  });
}

/// T3 `g9S4k`: a member without the claim sees the state, one lock line naming who can, and no button.
#[test]
fn team_qa_lifecycle_a_member_without_the_claim_has_no_button_and_one_lock_line() {
  let (mut window, _server) = on(State::Claims, "KON-13");
  for id in [
    "task-team-done",
    "task-team-cancel",
    "task-team-block",
    "task-team-clear-blocked",
    "task-reopen",
  ] {
    assert!(!offered(&mut window, id), "{id} is not offered");
  }
  let line = window.text("task-team-lifecycle-lock-text");
  assert!(
    line.starts_with("Only mara (the claim) and Admins close this task: "),
    "{line}"
  );
}

/// A done task the member did not close: only Admins reopen it, and an Admin sees Reopen.
#[test]
fn team_qa_lifecycle_reopen_is_the_admins() {
  let (mut window, _server) = on(State::Member, "KON-1");
  assert!(window.text("task-team-closed-title").starts_with("Done · marked by "));
  assert!(!offered(&mut window, "task-reopen"));
  assert!(
    window
      .text("task-team-lifecycle-lock-text")
      .starts_with("Only Admins reopen a team task")
  );

  let (mut window, _server) = on(State::Admin, "KON-1");
  assert!(offered(&mut window, "task-reopen"));
}

/// T11 `VVhcE`: the holder marks their task as blocked with a reason and clears it; the claim stays.
#[test]
fn team_qa_lifecycle_the_holder_blocks_and_clears_their_task() {
  let (mut window, _server) = on(State::Member, "KON-12");
  window.press("task-team-block");
  window.until("the reason's question", |window| {
    window.texts("task-block-reason").is_some()
  });
  window.set("task-block-reason", "Waits for the design");
  window.press("task-block-confirm");
  window.until("the task to be blocked", |window| {
    window.texts("task-team-blocked-title").is_some()
  });
  assert!(window.text("task-team-blocked-title").starts_with("Blocked by "));
  assert_eq!(window.text("task-team-blocked-detail"), "Waits for the design");
  assert!(
    window.text("task-team-claim-title").starts_with("Claimed by you"),
    "the claim stays"
  );

  window.press("task-team-clear-blocked");
  window.until("the block to be cleared", |window| {
    window.texts("task-team-blocked-title").is_none()
  });
  assert!(offered(&mut window, "task-team-block"));
}
