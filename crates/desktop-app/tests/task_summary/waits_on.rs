//! A task whose run waits on another task (ORC-784; 09 `SLuUW`, 10 `z5sQf` and `rlnEZ` f in `Orchestrator watch ·
//! redesign`), on the seed's `orchestrator-waits-on`: the header says what the run waits on and has no primary action,
//! the Summary's last event and the Conversation's last line are the wait's row with *Drop the wait*, *Depends on*
//! names the task not merged yet, and a wait that cannot be met takes the warning's words. Only the person drops a
//! wait: automation's press is refused and says so; the person's ends it, and the row goes.

use lurq::app::events::with_synthetic_input;

use super::page::Page;

/// The branch the seed's waited tasks are to be merged into.
const BASE: &str = "module/webhooks-v2";

#[test]
fn summary_a_run_waiting_on_a_task_says_so_in_the_header_and_its_row() {
  let mut page = Page::seeded("orchestrator-waits-on");
  let limiter = page.key_of("Shared rate limiter for deliveries");
  page.open_titled("Retry failed deliveries");
  let status = page.text("task-status-label");
  assert!(status.starts_with(&format!("Waits on {limiter}")), "{status}");
  assert!(
    page.text("task-run-line").contains(" · since "),
    "the run and since when"
  );
  assert!(!page.has("task-primary"), "the wait's row has what the person can do");
  let row = page.text("task-wait-text");
  assert_eq!(
    row,
    format!(
      "Waits on {limiter} · Shared rate limiter for deliveries. It continues when {limiter} is merged into {BASE}, \
       the run's base branch."
    )
  );
  assert_eq!(page.text("task-wait-drop"), "Drop the wait");
  assert_eq!(
    page.text("task-property-depends-label"),
    format!("{limiter} · not merged")
  );

  page.click("task-tab-conversation");
  page.window.until("the Conversation's wait", |window| {
    window.texts("conversation-wait").is_some()
  });
  assert_eq!(
    page.text("conversation-wait-text"),
    format!("Waits on {limiter} · continues when it is merged into {BASE} · the run's base branch")
  );
  assert_eq!(page.text("conversation-wait-drop"), "Drop the wait");
  let origin = page.text("conversation-wait-origin-text");
  assert!(
    origin.starts_with(&format!(
      "It created {limiter} · Shared rate limiter for deliveries and made this run wait on it · "
    )),
    "where the wait came from: {origin}"
  );
  assert!(!page.has("task-primary"), "nor on the Conversation");
}

#[test]
fn summary_a_wait_on_a_task_the_orchestrator_did_not_create_says_only_the_wait() {
  let mut page = Page::seeded("orchestrator-waits-on");
  let receipts = page.key_of("Delivery receipts");
  page.open_titled("Delivery dashboard");
  page.click("task-tab-conversation");
  page.window.until("the Conversation's wait", |window| {
    window.texts("conversation-wait-origin").is_some()
  });
  let origin = page.text("conversation-wait-origin-text");
  assert!(
    origin.starts_with(&format!("It made this run wait on {receipts} · Delivery receipts · ")),
    "{origin}"
  );
}

#[test]
fn summary_a_wait_that_cannot_be_met_says_why_in_the_warnings_words() {
  let mut page = Page::seeded("orchestrator-waits-on");
  let receipts = page.key_of("Delivery receipts");
  page.open_titled("Delivery dashboard");
  assert_eq!(
    page.text("task-status-label"),
    format!("Waits on {receipts} · cannot be met")
  );
  let row = page.text("task-wait-text");
  assert!(
    row.starts_with(&format!("{receipts} was canceled at ")) && row.contains("so nothing will end this wait"),
    "{row}"
  );
  assert!(
    row.ends_with("«Webhooks v2 and Search index» told you."),
    "the conversation that leads the task: {row}"
  );
  assert_eq!(page.text("task-wait-drop"), "Drop the wait");
}

#[test]
fn summary_only_the_person_drops_the_wait() {
  let mut page = Page::seeded("orchestrator-waits-on");
  page.open_titled("Delivery dashboard");
  with_synthetic_input(|| page.click("task-wait-drop"));
  page.wait("the refusal", |window| window.texts("task-wait-error").is_some());
  assert_eq!(
    page.text("task-wait-error"),
    "Only you can drop a run's wait: a press made by automation was not taken. Nothing was changed."
  );
  assert!(page.has("task-wait"), "the wait still holds the run");

  page.click("task-wait-drop");
  page.wait("the wait dropped", |window| {
    window.texts("task-wait").is_none() && window.texts("task-event").is_some()
  });
  let status = page.text("task-status-label");
  assert!(!status.starts_with("Waits on"), "the run no longer waits: {status}");
}
