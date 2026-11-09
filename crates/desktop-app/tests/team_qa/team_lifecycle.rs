//! Closing, cancelling, reopening and blocking a team task on its page (decision D4, `Task - v2` `jDN0r` rows 14–17: T1
//! `rvvoN`, T2 `o4K8uq`, T3 `g9S4k`, T4 `V26A3`, T11 `VVhcE`), on the QA app's `team-member`, `team-admin`,
//! `team-admin-work` and `team-claims` states with the stand-in team server, which decides each change by the person's
//! claim and role: what the page offers comes from the task's capabilities, a member without the action reads one lock
//! line and has no button, closing ends the claim, and a reopened task is free. The Tasks list's row menus and Mark as
//! blocked's dialog are in [`super::team_lifecycle_menus`].
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

/// The task `key` in `state`, once its claim line or its closed card shows.
pub(super) fn on(state: State, key: &str) -> (Window, TeamServer) {
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

pub(super) fn offered(window: &mut Window, id: &str) -> bool {
  window.texts(id).is_some()
}

/// Where the element `id` is laid out: its left and its top.
pub(super) fn at(window: &mut Window, id: &str) -> (f32, f32) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y)
}

/// Where the element `id` is laid out: its left, its width.
fn bounds(window: &mut Window, id: &str) -> (f32, f32, f32) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y, bounds.width)
}

/// T1 `rvvoN`: the claim's holder, a member, closes their own task: Mark as done and no Cancel task, and closing ends
/// the claim, which the page says as the task's closed card, which names no person or time the app does not know.
#[test]
fn team_qa_lifecycle_the_holder_marks_their_task_done() {
  let (mut window, _server) = on(State::Member, "KON-12");
  assert!(offered(&mut window, "task-team-done"), "the holder may close it");
  assert!(offered(&mut window, "task-team-block"), "and block it");
  assert!(!offered(&mut window, "task-team-cancel"), "Cancel is an Admin's");
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only Admins cancel or reopen a team task: dana-k (Admin) and olha (Server admin).",
    "the one line under the actions"
  );
  assert!(
    !offered(&mut window, "task-team-read-only"),
    "the page's own line steps aside"
  );
  let (done, block) = (at(&mut window, "task-team-done"), at(&mut window, "task-team-block"));
  assert!(done.0 < block.0, "Mark as done first, Mark as blocked after it");

  window.press("task-team-done");
  window.until("the task to be closed", |window| {
    window.texts("task-team-closed-title").is_some()
  });
  assert_eq!(window.text("task-team-closed-title"), "Done");
  assert_eq!(window.text("task-team-closed-badge"), "Done");
  assert_eq!(
    window.text("task-status-label"),
    "Done",
    "the header says it too, not a draft with no runs"
  );
  assert_eq!(
    window.text("task-team-closed-detail"),
    "Marking it done ended its claim."
  );
  assert!(
    !offered(&mut window, "task-team-done") && !offered(&mut window, "task-team-block"),
    "a closed task has no Mark as done and no Mark as blocked"
  );
  assert!(
    !offered(&mut window, "task-reopen"),
    "a member does not reopen it: one lock line says who does"
  );
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only Admins reopen a team task: dana-k (Admin) and olha (Server admin)."
  );
}

/// T2 `o4K8uq`, T4 `V26A3`: an Admin cancels a task another person holds, which ends that claim; Reopen offers Back to
/// Todo and Back to Backlog under it, and the reopened task is free.
#[test]
fn team_qa_lifecycle_an_admin_cancels_and_reopens() {
  let (mut window, _server) = on(State::AdminWork, "KON-13");
  assert_eq!(window.text("task-team-claim-title"), "Claimed by mara");
  let detail = window.text("task-team-claim-detail");
  assert!(
    detail.contains("As an Admin you can close it for them; either way their claim ends."),
    "{detail}"
  );
  assert!(offered(&mut window, "task-team-done") && offered(&mut window, "task-team-cancel"));
  assert!(
    !offered(&mut window, "task-team-lifecycle-lock"),
    "an Admin has nothing to be told"
  );
  let (done, cancel, block) = (
    at(&mut window, "task-team-done"),
    at(&mut window, "task-team-cancel"),
    at(&mut window, "task-team-block"),
  );
  assert!(
    done.0 < cancel.0 && cancel.0 < block.0,
    "Done, Cancel task, Mark as blocked"
  );

  window.press("task-team-cancel");
  window.until("the task to be canceled", |window| {
    window.texts("task-team-closed-title").is_some()
  });
  assert_eq!(window.text("task-team-closed-title"), "Canceled");
  assert_eq!(window.text("task-team-closed-badge"), "Canceled");
  let (detail, reopen) = (
    at(&mut window, "task-team-closed-detail"),
    at(&mut window, "task-reopen"),
  );
  assert!(reopen.1 > detail.1, "Reopen sits below the words");

  window.press("task-reopen");
  window.until("Reopen's choices", |window| window.texts("task-reopen-todo").is_some());
  assert!(offered(&mut window, "task-reopen-backlog"));
  let menu = at(&mut window, "task-reopen-menu");
  assert!(
    (menu.0 - reopen.0).abs() < 1.0,
    "the menu starts where the button does: {menu:?} {reopen:?}"
  );
  assert_eq!(
    window.text("task-reopen-free"),
    "A reopened task is free: nobody holds it, and nothing runs until someone claims it."
  );
  assert_eq!(window.text("task-reopen-todo-hint"), "ready to claim");
  let (panel, hint) = (
    bounds(&mut window, "task-reopen-menu"),
    bounds(&mut window, "task-reopen-todo-hint"),
  );
  assert!(
    ((panel.0 + panel.2 - 12.0) - (hint.0 + hint.2)).abs() < 1.5,
    "the hint ends 12 inside the menu's right (T4 `V26A3`): {panel:?} {hint:?}"
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
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only mara (the claim) and Admins close this task: dana-k and olha."
  );
  assert!(
    !offered(&mut window, "task-team-read-only"),
    "exactly one line: the page's own read-only line is not shown with it"
  );
}

/// A done task the member did not close: only Admins reopen it, and an Admin sees Reopen and no lock line. Neither is
/// offered Mark as blocked on it.
#[test]
fn team_qa_lifecycle_reopen_is_the_admins() {
  let (mut window, _server) = on(State::Member, "KON-1");
  assert_eq!(window.text("task-team-closed-title"), "Done");
  assert!(!offered(&mut window, "task-reopen"));
  assert!(
    !offered(&mut window, "task-team-block"),
    "a Done task is not offered Mark as blocked"
  );
  assert!(
    window
      .text("task-team-lifecycle-lock-text")
      .starts_with("Only Admins reopen a team task")
  );

  let (mut window, _server) = on(State::Admin, "KON-1");
  assert!(offered(&mut window, "task-reopen"));
  assert!(!offered(&mut window, "task-team-block"), "nor an Admin");
  assert!(!offered(&mut window, "task-team-lifecycle-lock"));
}

/// T11f `vyQJx`: a member reads a Canceled task: its card, and the one line saying that only Admins reopen it.
#[test]
fn team_qa_lifecycle_a_member_reads_a_canceled_task() {
  let (mut window, _server) = on(State::Claims, "KON-15");
  assert_eq!(window.text("task-team-closed-title"), "Canceled");
  assert_eq!(window.text("task-team-closed-detail"), "Canceling it ended its claim.");
  assert_eq!(window.text("task-status-label"), "Canceled", "and so does the header");
  assert!(
    !offered(&mut window, "task-run-line"),
    "a task that never ran has no run line"
  );
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only Admins reopen a team task: dana-k (Admin) and olha (Server admin)."
  );
  assert!(!offered(&mut window, "task-reopen"));
}

/// T11 `VVhcE`: the holder marks their task as blocked with a reason and clears it; the claim stays, and the blocked
/// state is one card, with the reason under its title and no second banner over the tabs.
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
  assert_eq!(window.text("task-team-blocked-title"), "Blocked");
  assert_eq!(window.text("task-team-blocked-reason"), "Waits for the design");
  assert_eq!(
    window.text("task-team-blocked-detail"),
    "Nothing runs while it is blocked. Clear it when work can go on; your claim stays."
  );
  assert!(
    !offered(&mut window, "task-blocked"),
    "the reason is told once, not again in a banner over the tabs"
  );
  assert!(
    window.text("task-team-claim-title").starts_with("Claimed by you"),
    "the claim stays"
  );
  let (clear, done) = (
    at(&mut window, "task-team-clear-blocked"),
    at(&mut window, "task-team-done"),
  );
  assert!(clear.0 < done.0, "Clear blocked first, Mark as done after it");

  window.press("task-team-clear-blocked");
  window.until("the block to be cleared", |window| {
    window.texts("task-team-blocked-title").is_none()
  });
  assert!(offered(&mut window, "task-team-block"));
}

/// T11c `dTn0v`: a member without the claim reads a blocked task: the reason, what blocked means, no button and one
/// line saying who clears it.
#[test]
fn team_qa_lifecycle_a_member_without_the_claim_reads_a_blocked_task() {
  let (mut window, _server) = on(State::Claims, "KON-16");
  assert_eq!(window.text("task-team-blocked-title"), "Blocked");
  assert_eq!(window.text("task-team-blocked-reason"), "Waiting for the staging key");
  assert_eq!(
    window.text("task-team-blocked-detail"),
    "Nothing runs while it is blocked."
  );
  assert!(!offered(&mut window, "task-team-clear-blocked") && !offered(&mut window, "task-team-done"));
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only mara (the claim) and Admins clear it: dana-k and olha."
  );
}

/// T11b `x0xP1`: an Admin reads a blocked task someone else holds: Clear blocked is the primary action, then Cancel
/// task, and no line.
#[test]
fn team_qa_lifecycle_an_admin_reads_a_blocked_task_of_someone_else() {
  let (mut window, _server) = on(State::AdminWork, "KON-21");
  assert_eq!(window.text("task-team-blocked-reason"), "Waiting for the staging key");
  assert_eq!(
    window.text("task-team-blocked-detail"),
    "As an Admin you can clear it for olha or close the task; closing ends the claim."
  );
  let (clear, cancel) = (
    at(&mut window, "task-team-clear-blocked"),
    at(&mut window, "task-team-cancel"),
  );
  assert!(clear.0 < cancel.0, "Clear blocked, then Cancel task");
  assert!(!offered(&mut window, "task-team-lifecycle-lock"));
}

/// T1 `rvvoN`, T11d `dTn0v`: in `team-claims` the person holds KON-12's claim on this computer and nothing runs, so its
/// card offers Run and Mark as done and one line saying who cancels and reopens; Mark as done closes it and ends the
/// claim. (`team-member` has KON-12's claim on another computer, which is another card.)
#[test]
fn team_qa_lifecycle_the_holder_here_with_nothing_running_marks_their_task_done() {
  let (mut window, _server) = on(State::Claims, "KON-12");
  assert!(window.text("task-team-claim-title").starts_with("Claimed by you"));
  assert!(
    offered(&mut window, "task-run"),
    "nothing runs here, so the holder may run it"
  );
  assert!(offered(&mut window, "task-team-done") && offered(&mut window, "task-team-block"));
  assert!(!offered(&mut window, "task-team-cancel"), "Cancel is an Admin's");
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only Admins cancel or reopen a team task: dana-k (Admin) and olha (Server admin)."
  );
  window.press("task-team-done");
  window.until("the task to be closed", |window| {
    window.texts("task-team-closed-title").is_some()
  });
  assert_eq!(window.text("task-team-closed-title"), "Done");
  assert_eq!(window.text("task-status-label"), "Done");
  assert_eq!(
    window.text("task-team-closed-detail"),
    "Marking it done ended its claim."
  );
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only Admins reopen a team task: dana-k (Admin) and olha (Server admin)."
  );
}
