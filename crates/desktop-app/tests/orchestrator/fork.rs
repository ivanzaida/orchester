//! *Fork from here* (ORC-334; ORC-330's 09 `GYAPg` menu and 11 `RT5T7`), end to end with the real runner and
//! `orchester-fake-claude`, which answers `Noted: <prompt>`: a message's *Fork from here* (ORC-706) opens, without a
//! question, a new conversation titled after the original with the messages before the person's message, its words in
//! the composer; the original stays as it was and *Open original ↗* goes back to it. The fork's first answer shows the
//! agent goes on from the copied history only. It works while the Orchestrator answers, from what was kept so far.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::{ChatEvent, EntryKind},
};

use super::harness::{Orchestrator, Setup};

const FIRST: &str = "Remember the word apple.";
const SECOND: &str = "Now remember the word banana too.";

/// Sends `text` and waits until its answer in the newest conversation ended.
fn answered(screen: &mut Orchestrator, text: &str) {
  screen.send(text);
  screen.until("the answer", |screen| {
    let db = screen.db();
    let Some(listing) = db.chats().expect("chats").into_iter().next() else {
      return false;
    };
    let replied = db
      .chat_entries(listing.chat.id())
      .expect("entries")
      .iter()
      .any(|entry| matches!(&entry.kind, EntryKind::Reply { text: reply, .. } if reply.ends_with(text)));
    replied && !screen.exists("orchestrator-thinking")
  });
  for _ in 0..5 {
    screen.window.pass();
  }
}

/// The entries of conversation `chat`, as the project keeps them.
fn entries(screen: &Orchestrator, chat: ChatId) -> Vec<EntryKind> {
  screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .map(|entry| entry.kind)
    .collect()
}

/// The person's message of `chat` that says `text`.
fn message(screen: &Orchestrator, chat: ChatId, text: &str) -> ChatMessageId {
  entries(screen, chat)
    .into_iter()
    .find_map(|kind| match kind {
      EntryKind::Person {
        message, text: said, ..
      } if said.as_str() == text => Some(message),
      _ => None,
    })
    .unwrap_or_else(|| panic!("the message {text:?}"))
}

/// The newest conversation, as History lists it first.
fn newest(screen: &Orchestrator) -> ChatId {
  screen.db().chats().expect("chats")[0].chat.id()
}

/// Shows the actions of the message `id` once it is laid out and presses *Fork from here*.
fn fork_from(screen: &mut Orchestrator, id: &str) {
  let bubble = format!("{id}-bubble");
  screen.until("the message laid out", |screen| {
    screen
      .window
      .tree
      .get_element_by_id_mut(&bubble)
      .and_then(|element| element.bounds())
      .is_some()
  });
  screen.window.hover(&bubble);
  let fork = format!("{id}-fork");
  screen.window.hover(&fork);
  assert_eq!(screen.text(&format!("{fork}-tooltip")), "Fork from here");
  screen.press(&fork);
}

#[test]
fn orchestrator_fork_from_a_message_opens_a_new_conversation_with_what_came_before_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  answered(&mut screen, FIRST);
  answered(&mut screen, SECOND);
  let original = newest(&screen);
  let before = entries(&screen, original);
  let id = format!("orchestrator-message-{}", message(&screen, original, SECOND));

  fork_from(&mut screen, &id);

  // The fork opens as History opens a conversation: its title first, then its messages once they are read.
  screen.until("the fork read", |screen| {
    screen.text("orchestrator-title") == format!("{FIRST} · fork")
      && screen.text("orchestrator-meta") != "· opening from History"
  });
  let fork = newest(&screen);
  assert_ne!(fork, original, "History lists the fork first");
  assert_eq!(
    screen.route(),
    Route::Project(screen.project, ProjectPage::Orchestrator(OrchestratorChat::Chat(fork)))
  );
  assert!(
    screen.text("orchestrator-meta").starts_with("· forked today "),
    "{}",
    screen.text("orchestrator-meta")
  );
  let shown = screen.text("orchestrator-chat");
  assert!(
    shown.contains(&format!("Forked from {FIRST} at ")) && shown.contains("the original stays as it was"),
    "{shown}"
  );
  assert!(
    shown.contains(&format!("Noted: {FIRST}")) && !shown.contains("banana"),
    "the messages before the point, and none after it: {shown}"
  );
  assert!(
    screen.text("orchestrator-input").starts_with(SECOND),
    "the message's words are in the fork's composer: {}",
    screen.text("orchestrator-input")
  );
  assert_eq!(entries(&screen, original), before, "the original stays as it was");
  assert!(matches!(
    entries(&screen, fork).first(),
    Some(EntryKind::Event(ChatEvent::Forked { from, .. })) if *from == original
  ));

  screen.set("orchestrator-input", "");
  answered(&mut screen, "Which words do you know?");
  let told = entries(&screen, fork)
    .into_iter()
    .filter_map(|kind| match kind {
      EntryKind::Reply { text, .. } => Some(text),
      _ => None,
    })
    .next_back()
    .expect("the fork's answer");
  assert!(told.contains(FIRST), "it goes on from the copied history: {told}");
  assert!(!told.contains("banana"), "and knows nothing after the point: {told}");

  let line = screen.db().chat_entries(fork).expect("entries")[0].seq;
  let open = format!("orchestrator-forked-wait-{line}-original");
  screen.until("the fork's line", |screen| screen.exists(&open));
  assert_eq!(
    screen.text(&open),
    "Open original ↗",
    "the fork links back to its original"
  );
  screen.press(&open);
  screen.until("the original", |screen| {
    screen.route()
      == Route::Project(
        screen.project,
        ProjectPage::Orchestrator(OrchestratorChat::Chat(original)),
      )
  });
}

#[test]
fn orchestrator_fork_while_the_orchestrator_answers_takes_what_was_kept_so_far() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  answered(&mut screen, FIRST);
  screen.send("Keep thinking about the queue.");
  screen.until("the answer under way", |screen| screen.exists("orchestrator-thinking"));
  let original = newest(&screen);
  let id = format!("orchestrator-message-{}", message(&screen, original, FIRST));

  fork_from(&mut screen, &id);

  screen.until("the fork", |screen| {
    screen.text("orchestrator-title").ends_with(" · fork")
  });
  let fork = newest(&screen);
  assert_ne!(fork, original);
  assert!(
    screen.text("orchestrator-input").starts_with(FIRST),
    "{}",
    screen.text("orchestrator-input")
  );
  assert!(
    !screen.exists("orchestrator-thinking"),
    "the fork does not answer: only its original does"
  );
  screen.go(OrchestratorChat::Chat(original));
  screen.until("the original's answer", |screen| screen.exists("orchestrator-stop"));
  screen.click("orchestrator-stop");
}
