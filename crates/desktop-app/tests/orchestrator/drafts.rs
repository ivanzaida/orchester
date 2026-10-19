//! The composer's drafts for as long as the app runs: one through another screen and back, each conversation's own
//! and a new one's, and what sending leaves of them: a message sent and kept empties its draft and no other, a failed
//! send keeps it, and one whose send the person left before it ended is kept once when sent again.

use std::time::{Duration, Instant};

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route, TaskList};
use orchester_core::{orchestrator::EntryKind, time::Timestamp};
use rusqlite::Connection;

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

/// What an existing conversation's empty field shows.
const PLACEHOLDER: &str = "Message Orchestrator…";

/// Leaves the Orchestrator for the Tasks list.
fn leave(screen: &mut Orchestrator) {
  let project = screen.project;
  screen
    .window
    .navigation()
    .go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
  screen.until("the Tasks list", |screen| {
    screen.exists("screen-tasks") && !screen.exists("screen-orchestrator")
  });
}

/// Opens `chat` and waits until its field shows `shown`.
fn open(screen: &mut Orchestrator, chat: OrchestratorChat, shown: &str) {
  screen.go(chat);
  screen.until(&format!("the field to show {shown}"), |screen| {
    screen.text("orchestrator-input") == shown
  });
}

#[test]
fn orchestrator_draft_stays_through_another_screen_and_back() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.set("orchestrator-input", "Plan the webhook work");
  leave(&mut screen);
  open(&mut screen, OrchestratorChat::New, "Plan the webhook work");
}

#[test]
fn orchestrator_each_conversation_keeps_its_own_draft() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let older = kept(&screen, "Order of the auth work", Timestamp::now(), "Auth first.");
  let newer = kept(
    &screen,
    "Why did KON-12 go back?",
    Timestamp::now(),
    "Its review failed.",
  );
  open(&mut screen, OrchestratorChat::Chat(older), PLACEHOLDER);
  screen.set("orchestrator-input", "For the auth work");
  open(&mut screen, OrchestratorChat::Chat(newer), PLACEHOLDER);
  screen.set("orchestrator-input", "For KON-12");
  screen.go(OrchestratorChat::New);
  screen.until("a new conversation's empty field", |screen| {
    screen.text("orchestrator-input").starts_with("Describe what you want")
  });
  screen.set("orchestrator-input", "Something new");
  open(&mut screen, OrchestratorChat::Chat(older), "For the auth work");
  open(&mut screen, OrchestratorChat::Latest, "For KON-12");
  leave(&mut screen);
  open(&mut screen, OrchestratorChat::New, "Something new");
  open(&mut screen, OrchestratorChat::Chat(newer), "For KON-12");
}

#[test]
fn orchestrator_a_message_sent_and_kept_empties_only_its_own_draft() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let chat = kept(&screen, "Order of the auth work", Timestamp::now(), "Auth first.");
  screen.go(OrchestratorChat::New);
  screen.set("orchestrator-input", "A later idea");
  open(&mut screen, OrchestratorChat::Chat(chat), PLACEHOLDER);
  screen.send("What is running?");
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: What is running?")
      && !screen.exists("orchestrator-thinking")
  });
  assert_eq!(screen.text("orchestrator-input"), PLACEHOLDER, "the draft is done with");
  leave(&mut screen);
  open(&mut screen, OrchestratorChat::Chat(chat), PLACEHOLDER);
  open(&mut screen, OrchestratorChat::New, "A later idea");
}

#[test]
fn orchestrator_a_new_conversation_sent_and_kept_leaves_no_draft_behind() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: What is running?")
      && !screen.exists("orchestrator-thinking")
  });
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  leave(&mut screen);
  open(&mut screen, OrchestratorChat::Chat(chat), PLACEHOLDER);
  screen.go(OrchestratorChat::New);
  screen.until("a new conversation's empty field", |screen| {
    screen.text("orchestrator-input").starts_with("Describe what you want")
  });
}

#[test]
fn orchestrator_a_failed_send_keeps_the_draft_through_another_screen() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  screen.until("the failed send", |screen| screen.exists("orchestrator-send-problem"));
  assert_eq!(screen.text("orchestrator-input"), "What is running?");
  leave(&mut screen);
  open(&mut screen, OrchestratorChat::New, "What is running?");
}

#[test]
fn orchestrator_a_send_left_unfinished_is_kept_once_when_sent_again() {
  let mut screen = Orchestrator::new(Setup::Ready);
  // The runner is up, so the next send goes straight on to keep its message.
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  until_answered(&mut screen, "What is running?");
  screen.go(OrchestratorChat::New);
  // The store waits for another writer while the person leaves the Orchestrator.
  let holder = Connection::open(screen.root.join(".orchester").join("orchester.db")).expect("the database opens");
  holder.execute_batch("BEGIN EXCLUSIVE").expect("the database is held");
  screen.send("Plan the webhook work");
  let started = Instant::now();
  screen.until("the send to wait for the store", |_| {
    started.elapsed() > Duration::from_millis(500)
  });
  leave(&mut screen);
  holder.execute_batch("ROLLBACK").expect("the database is let go");
  open(&mut screen, OrchestratorChat::New, "Plan the webhook work");
  screen.click("orchestrator-input");
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
  screen.key(("Enter", "Enter"), true);
  until_answered(&mut screen, "Plan the webhook work");
  assert_eq!(screen.text("orchestrator-input"), PLACEHOLDER, "the draft is done with");
  assert_eq!(messages(&screen, "Plan the webhook work"), 1, "kept once");
}

/// Waits until the conversation shows the answer to `text` and the turn ended.
fn until_answered(screen: &mut Orchestrator, text: &str) {
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains(&format!("Noted: {text}"))
      && !screen.exists("orchestrator-thinking")
  });
}

/// How many of the person's messages in all conversations say `text`.
fn messages(screen: &Orchestrator, text: &str) -> usize {
  let db = screen.db();
  let chats = db.chats().expect("chats");
  chats
    .iter()
    .flat_map(|listing| db.chat_entries(listing.chat.id()).expect("entries"))
    .filter(|entry| matches!(&entry.kind, EntryKind::Person { text: said, .. } if said.as_str() == text))
    .count()
}
