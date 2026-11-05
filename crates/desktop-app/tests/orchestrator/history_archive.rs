//! Archiving and deleting a conversation in History (ORC-329, `08` `q30JYF`, `08d` `qAfeu`, `08e` `I4xzOf`): *Archive*
//! takes it out of History's list into the Archived view at its foot, where *Unarchive* brings it back; *Delete…* asks
//! in a modal, which Esc cancels, and deleting the conversation shown falls back to a new one; a delete that failed
//! says why in the modal; only a person deletes, an MCP client's press or Enter is refused. Run with `cargo test -p
//! orchester-app --test orchestrator history_archive`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::id::ChatId;

use super::{
  harness::Orchestrator,
  history_rows::{hover, opened},
};

/// Opens the `⋯` menu of `chat`'s row and chooses `choice` in it.
fn choose(screen: &mut Orchestrator, chat: ChatId, choice: &str) {
  hover(screen, &format!("orchestrator-history-{chat}"));
  screen.click(&format!("orchestrator-history-more-{chat}"));
  screen.click(&format!("orchestrator-history-{choice}-{chat}"));
}

fn archived(screen: &Orchestrator, chat: ChatId) -> bool {
  screen
    .db()
    .chats()
    .expect("History")
    .iter()
    .any(|listing| listing.chat.id() == chat && listing.archived.is_some())
}

#[test]
fn history_archive_moves_a_conversation_to_archived_and_unarchive_brings_it_back() {
  let (mut screen, older, newer) = opened();
  assert!(
    !screen.exists("orchestrator-history-archived"),
    "no way to Archived while nothing is archived"
  );
  choose(&mut screen, older, "archive");
  screen.until("the row out of History's list", |screen| {
    !screen.exists(&format!("orchestrator-history-{older}"))
  });
  assert!(archived(&screen, older), "kept in the project's data");
  assert_eq!(screen.text("orchestrator-history-archived"), "Archived 1");
  screen.click("orchestrator-history-archived");
  screen.until("the Archived view", |screen| {
    screen.exists("orchestrator-history-archived-head")
  });
  assert!(!screen.exists(&format!("orchestrator-history-{newer}")));
  let row = format!("orchestrator-history-{older}");
  assert!(
    screen.text(&row).contains("Archived today · 2 messages"),
    "{}",
    screen.text(&row)
  );
  assert!(
    screen
      .text("orchestrator-history-note")
      .starts_with("Archived conversations keep every message.")
  );
  hover(&mut screen, &row);
  screen.click(&format!("orchestrator-history-unarchive-{older}"));
  screen.until("the Archived view empty", |screen| {
    screen.exists("orchestrator-history-empty")
  });
  assert!(!archived(&screen, older));
  screen.click("orchestrator-history-archived-back");
  screen.until("History's list again", |screen| {
    screen.exists(&format!("orchestrator-history-{older}"))
  });
  assert!(!screen.exists("orchestrator-history-archived"));
}

#[test]
fn history_delete_asks_in_a_modal_and_the_open_conversation_falls_back_to_new() {
  let (mut screen, older, newer) = opened();
  choose(&mut screen, newer, "delete");
  screen.until("the question", |screen| screen.exists("orchestrator-history-delete"));
  assert_eq!(
    screen.text("orchestrator-history-delete-title"),
    "Delete “Plan the GitHub app permissions”?"
  );
  assert!(
    screen
      .text("orchestrator-history-delete-messages")
      .starts_with("Its 2 messages and the files attached to them")
  );
  screen.key(("Escape", "Escape"), false);
  screen.until("the question gone", |screen| {
    !screen.exists("orchestrator-history-delete")
  });
  assert!(
    screen.exists("orchestrator-history-panel"),
    "Esc ends the question before it closes History"
  );
  assert!(screen.db().chat(newer).expect("read").is_some(), "nothing deleted");
  choose(&mut screen, newer, "delete");
  screen.until("the question again", |screen| {
    screen.exists("orchestrator-history-delete-confirm")
  });
  screen.click("orchestrator-history-delete-confirm");
  screen.until("the question answered", |screen| {
    !screen.exists("orchestrator-history-delete")
  });
  assert_eq!(screen.db().chat(newer).expect("read"), None);
  assert!(screen.db().chat_entries(newer).expect("read").is_empty());
  assert_eq!(
    screen.route(),
    Route::Project(screen.project, ProjectPage::Orchestrator(OrchestratorChat::New)),
    "the conversation shown was deleted: a new one shows"
  );
  screen.until("the row gone", |screen| {
    !screen.exists(&format!("orchestrator-history-{newer}"))
  });
  assert!(screen.exists(&format!("orchestrator-history-{older}")));
}

#[test]
fn history_delete_that_failed_says_why_in_the_modal() {
  let (mut screen, older, _) = opened();
  choose(&mut screen, older, "delete");
  screen.until("the question", |screen| screen.exists("orchestrator-history-delete"));
  screen.db().delete_chat(older).expect("deleted elsewhere first");
  screen.key(("Enter", "Enter"), false);
  screen.until("why it failed", |screen| {
    screen.exists("orchestrator-history-delete-error")
  });
  assert_eq!(
    screen.text("orchestrator-history-delete-error"),
    "Not deleted: This conversation is no longer kept."
  );
  assert!(screen.exists("orchestrator-history-delete"), "the question stays");
}

#[test]
fn history_delete_is_a_persons_alone() {
  let (mut screen, older, _) = opened();
  choose(&mut screen, older, "delete");
  screen.until("the question", |screen| {
    screen.exists("orchestrator-history-delete-confirm")
  });
  assert!(!screen.exists("orchestrator-history-delete-person-only"));
  with_synthetic_input(|| screen.click("orchestrator-history-delete-confirm"));
  screen.until("the refusal", |screen| {
    screen.exists("orchestrator-history-delete-person-only")
  });
  assert_eq!(
    screen.text("orchestrator-history-delete-person-only"),
    "Only a person deletes a conversation: input from an MCP client is not taken here."
  );
  with_synthetic_input(|| screen.key(("Enter", "Enter"), false));
  // A delete that started would be under way by now: its background work had time to run and was polled.
  std::thread::sleep(std::time::Duration::from_millis(200));
  screen.until("a pass", |_| true);
  assert!(screen.db().chat(older).expect("read").is_some(), "nothing deleted");
  assert!(screen.exists("orchestrator-history-delete"), "the question stays");
  screen.click("orchestrator-history-delete-confirm");
  screen.until("the person's delete", |screen| {
    !screen.exists("orchestrator-history-delete")
  });
  assert_eq!(screen.db().chat(older).expect("read"), None);
}
