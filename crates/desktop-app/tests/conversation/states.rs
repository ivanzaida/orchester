//! What the Conversation tab shows instead of a timeline, on `task-states`: a queued run that has not
//! started (its steps), a task that never ran, a run that cannot be read, the transcript being read while
//! reads are held, and a transcript that cannot be read with *Try again*; and a card that waits for a
//! decision (C3's), saying answering in the chat is not available yet.

use rusqlite::Connection;

use super::{all_text, click, key_of, open, seeded};

#[test]
fn timeline_a_queued_task_says_nothing_ran_yet() {
  let (mut window, seeded) = seeded("task-states");
  let key = key_of(&seeded, "Localize the invoice emails");
  open(&mut window, &seeded, &key, "conversation-empty");
  let text = window.texts("conversation-empty-text").expect("why").join(" ");
  assert!(text.contains("is queued: nothing has run yet"), "{text}");
  assert!(
    window
      .texts("conversation-run-state")
      .is_some_and(|texts| texts.contains(&"Queued".to_owned())),
    "the chip says Queued"
  );
}

#[test]
fn timeline_a_task_without_a_run_says_it_has_not_run() {
  let (mut window, seeded) = seeded("task-states");
  let key = key_of(&seeded, "Docs: connecting GitHub");
  open(&mut window, &seeded, &key, "conversation-no-run");
  assert_eq!(
    window.texts("conversation-no-run-title"),
    Some(vec!["This task has not run yet".to_owned()])
  );
  assert!(
    window.tree.get_element_by_id("conversation-run-state").is_none(),
    "no run, no state to show"
  );
}

#[test]
fn timeline_an_unreadable_run_shows_nothing_it_did() {
  let (mut window, seeded) = seeded("task-states");
  let key = key_of(&seeded, "Typed errors per module");
  open(&mut window, &seeded, &key, "conversation-unreadable");
  let text = all_text(&mut window);
  assert!(text.contains("could not be read"), "{text}");
  assert!(
    window.tree.get_element_by_id("conversation-run-state").is_none(),
    "where an unreadable run stands is unknown: no state chip"
  );
  assert!(text.contains("Summary lists the task’s runs"), "{text}");
}

#[test]
fn timeline_shows_loading_while_reads_are_held() {
  let (mut window, seeded) = seeded("task-states");
  // The project's tasks are read first; only the transcript's reads are held then.
  let first = key_of(&seeded, "Docs: connecting GitHub");
  open(&mut window, &seeded, &first, "conversation-no-run");
  let hold = window.launch.services().expect("a started session").hold.clone();
  hold.set(true);
  let key = key_of(&seeded, "Paginate the events API");
  open(&mut window, &seeded, &key, "conversation-loading");
  let title = window.texts("conversation-loading-title").expect("a title").join(" ");
  assert!(title.starts_with("Loading activity for Run "), "{title}");
  hold.set(false);
  window.until("the timeline", |window| {
    window.tree.get_element_by_id("conversation-chat").is_some()
  });
}

#[test]
fn timeline_a_transcript_that_cannot_be_read_offers_try_again() {
  let (mut window, seeded) = seeded("task-states");
  let key = key_of(&seeded, "Paginate the events API");
  let database = seeded.repo.join(".orchester").join("orchester.db");
  let damaged = Connection::open(&database).expect("the database file opens");
  drop_append_only(&damaged);
  damaged
    .execute("UPDATE agent_events SET payload = '{'", [])
    .expect("the events are damaged");
  open(&mut window, &seeded, &key, "conversation-failed");
  let title = window.texts("conversation-failed-title").expect("a title").join(" ");
  assert!(title.starts_with("Couldn’t load the activity of Run "), "{title}");
  assert!(window.tree.get_element_by_id("conversation-open-log").is_some());
  // Repaired, Try again reads it.
  damaged
    .execute("DELETE FROM agent_events", [])
    .expect("the damaged events are removed");
  click(&mut window, "conversation-retry");
  window.until("the chat", |window| {
    window.tree.get_element_by_id("conversation-failed").is_none()
  });
}

#[test]
fn timeline_a_card_that_waits_says_answering_here_is_not_available_yet() {
  let (mut window, seeded) = seeded("task-states");
  let key = key_of(&seeded, "Add serde to the webhook payloads");
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the request's card", |window| {
    all_text(window).contains("Answering in the chat is not available in this version yet.")
  });
  let text = all_text(&mut window);
  assert!(text.contains("asks to run a command"), "{text}");
  assert!(text.contains("Waiting for approval"), "{text}");
}

#[test]
fn timeline_a_tool_steps_result_card_names_the_tool_step() {
  let (mut window, seeded) = seeded("task-states");
  let key = key_of(&seeded, "Export invoices as PDF");
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the Run checks result", |window| {
    all_text(window).contains("Run checks failed")
  });
  let text = all_text(&mut window);
  assert!(text.contains("04 Run checks · Tool"), "{text}");
}

#[test]
fn timeline_a_transcript_that_cannot_be_read_again_shows_as_last_read() {
  let (mut window, seeded) = seeded("task-states");
  let key = key_of(&seeded, "Paginate the events API");
  open(&mut window, &seeded, &key, "conversation-chat");
  let damaged = Connection::open(seeded.repo.join(".orchester").join("orchester.db")).expect("the database file opens");
  drop_append_only(&damaged);
  damaged
    .execute("UPDATE agent_events SET payload = '{'", [])
    .expect("the events are damaged");
  // A change of the run reads the transcript again.
  window
    .projects()
    .feed(seeded.project)
    .changes
    .update(|count| *count += 1);
  window.until("the notice", |window| {
    window.tree.get_element_by_id("conversation-stale").is_some()
  });
  assert!(
    window.tree.get_element_by_id("conversation-chat").is_some(),
    "the chat shows as last read"
  );
  assert!(all_text(&mut window).contains("Conversation shown as last read"));
  damaged
    .execute("DELETE FROM agent_events", [])
    .expect("the damaged events are removed");
  click(&mut window, "conversation-stale-retry");
  window.until("the notice gone", |window| {
    window.tree.get_element_by_id("conversation-stale").is_none()
  });
}

/// Lets the transcript be damaged from outside Orchester, which its append-only triggers do not stop.
fn drop_append_only(database: &Connection) {
  let triggers: Vec<String> = database
    .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'agent_events'")
    .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
    .expect("the triggers are listed");
  for trigger in triggers {
    database
      .execute(&format!("DROP TRIGGER \"{trigger}\""), [])
      .expect("the trigger is dropped");
  }
}
