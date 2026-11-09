//! Who closed a team task and who blocked it, on its page (decision D4, `Task - v2` `jDN0r`: T4 `V26A3`, T5 `eJKwy`,
//! T6 `Xwa1j`, T11a–f `ujJ6U`, `Tydl9`, `x0xP1`, `dTn0v`, `b3t84e`, `vyQJx`), on the QA app's `team-closed-admin` and
//! `team-closed-member` states: each task was closed or blocked through the stand-in team server's own records, and
//! its card names the person and the time, the module whose delivery closed it, or its run. A task Done with no
//! record, as an older server leaves it, keeps the plain word.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_closed_by`

use super::{
  team::State,
  team_lifecycle::{assert_stamped, offered, on},
};

#[test]
fn team_qa_closed_by_a_person_is_named_with_the_time() {
  let (mut window, _server) = on(State::ClosedAdmin, "KON-9");
  assert_stamped(&window.text("task-team-closed-title"), "Done · marked by you at ");
  assert_eq!(window.text("task-team-closed-badge"), "Done");
  assert_eq!(
    window.text("task-team-closed-detail"),
    "Marking it done ended its claim.",
    "the claim ended with it: whether it was the closer's is not in the record"
  );
  assert!(offered(&mut window, "task-reopen"), "an Admin reopens it");

  let (mut window, _server) = on(State::ClosedAdmin, "KON-8");
  assert_stamped(&window.text("task-team-closed-title"), "Done · marked by mara at ");
  assert_eq!(window.text("task-team-closed-badge"), "Done");
}

#[test]
fn team_qa_closed_by_a_canceled_task_names_who_canceled_it() {
  let (mut window, _server) = on(State::ClosedAdmin, "KON-15");
  assert_stamped(&window.text("task-team-closed-title"), "Canceled by dana-k at ");
  assert_eq!(window.text("task-team-closed-badge"), "Canceled");
  assert_eq!(window.text("task-team-closed-detail"), "Canceling it ended its claim.");

  let (mut window, _server) = on(State::ClosedMember, "KON-15");
  assert_stamped(&window.text("task-team-closed-title"), "Canceled by dana-k at ");
  assert!(!offered(&mut window, "task-reopen"), "a member does not reopen it");
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only Admins reopen a team task: dana-k (Admin) and olha (Server admin)."
  );
}

#[test]
fn team_qa_closed_by_a_delivery_names_its_module() {
  let (mut window, _server) = on(State::ClosedAdmin, "KON-19");
  assert_eq!(
    window.text("task-team-closed-title"),
    "Done · delivered with module Webhooks v2"
  );
  assert_eq!(window.text("task-team-closed-badge"), "Done");
  let detail = window.text("task-team-closed-detail");
  assert!(
    detail.starts_with("The team server accepted its module's delivery at ")
      && detail.ends_with("so the task closed by itself; the claim ended with it."),
    "{detail}"
  );
  assert!(offered(&mut window, "task-reopen"), "an Admin reopens it");
}

#[test]
fn team_qa_closed_by_its_run_says_the_run_succeeded() {
  let (mut window, _server) = on(State::ClosedMember, "KON-20");
  assert_eq!(window.text("task-team-closed-title"), "Done · its run succeeded");
  assert_eq!(window.text("task-team-closed-badge"), "Done");
  let detail = window.text("task-team-closed-detail");
  assert!(
    detail.starts_with("Its run delivers no changes to merge, so the task closed when it succeeded, at "),
    "{detail}"
  );
  assert!(!offered(&mut window, "task-reopen"), "a member sees no Reopen");
  assert_eq!(
    window.text("task-team-lifecycle-lock-text"),
    "Only Admins reopen a team task: dana-k (Admin) and olha (Server admin)."
  );
}

#[test]
fn team_qa_closed_by_a_blocked_task_names_who_blocked_it_and_keeps_its_reason() {
  let (mut window, _server) = on(State::ClosedMember, "KON-21");
  assert_stamped(&window.text("task-team-blocked-title"), "Blocked by you at ");
  assert_eq!(window.text("task-team-blocked-badge"), "Blocked");
  assert_eq!(window.text("task-team-blocked-reason"), "Waiting for the staging key");

  let (mut window, _server) = on(State::ClosedMember, "KON-16");
  assert_stamped(&window.text("task-team-blocked-title"), "Blocked by mara at ");
  assert_eq!(window.text("task-team-blocked-reason"), "Waiting for the staging key");
  assert_eq!(
    window.text("task-team-blocked-detail"),
    "Nothing runs while it is blocked."
  );

  let (mut window, _server) = on(State::ClosedAdmin, "KON-16");
  assert_stamped(&window.text("task-team-blocked-title"), "Blocked by mara at ");
}

#[test]
fn team_qa_closed_by_a_task_with_no_record_keeps_the_plain_words() {
  let (mut window, _server) = on(State::ClosedAdmin, "KON-1");
  assert_eq!(window.text("task-team-closed-title"), "Done");
  assert_eq!(window.text("task-team-closed-badge"), "Done");
  assert_eq!(
    window.text("task-team-closed-detail"),
    "Marking it done ended its claim."
  );
}
