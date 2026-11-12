//! Edit and *Rewind to here* on an earlier message (ORC-333; ORC-330's 09 `GYAPg`, 09b `QJb0b`, 09c `z7sgJ`, 10
//! `Zm7kq`), end to end with the real runner and `orchester-fake-claude`, which answers `Noted: <prompt>`: the
//! person's message's actions (icons under its bubble, ORC-706) offer *Edit*, Copy and *Rewind to here*, which rewinds
//! to before it after the drawn question;
//! the conversation then says where it went back to, the message's words are back in the composer, and the next answer
//! shows the agent no longer knows what was dropped. An edit asks before it drops what follows, then sends the edited
//! words in the message's place. Neither works while the Orchestrator answers.

use lurq::app::events::with_synthetic_input;
use orchester_app::{routes::OrchestratorChat, ui::chat::toolbar::HELD_CLASS};
use orchester_core::{
  id::ChatMessageId,
  orchestrator::{ChatEvent, EntryKind},
};

use super::harness::{Orchestrator, Setup};

const FIRST: &str = "Remember the word apple.";
const SECOND: &str = "Now remember the word banana too.";

/// Sends `text` and waits until its answer ended.
fn answered(screen: &mut Orchestrator, text: &str) {
  screen.send(text);
  screen.until("the answer", |screen| {
    let db = screen.db();
    let Some(listing) = db.chats().expect("chats").into_iter().next() else {
      return false;
    };
    let entries = db.chat_entries(listing.chat.id()).expect("entries");
    let said = entries
      .iter()
      .any(|entry| matches!(&entry.kind, EntryKind::Person { text: kept, .. } if kept.as_str() == text));
    let replied = entries
      .iter()
      .any(|entry| matches!(&entry.kind, EntryKind::Reply { text: reply, .. } if reply.ends_with(text)));
    said && replied && !screen.exists("orchestrator-thinking")
  });
  for _ in 0..5 {
    screen.window.pass();
  }
}

/// A conversation of two turns, [`FIRST`] and [`SECOND`], both answered.
fn two_turns() -> Orchestrator {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  answered(&mut screen, FIRST);
  answered(&mut screen, SECOND);
  screen
}

/// The entries of the conversation, as the project keeps them.
fn entries(screen: &Orchestrator) -> Vec<EntryKind> {
  let db = screen.db();
  let chat = db.chats().expect("chats")[0].chat.id();
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .map(|entry| entry.kind)
    .collect()
}

/// The person's message that says `text`.
fn message(screen: &Orchestrator, text: &str) -> ChatMessageId {
  entries(screen)
    .into_iter()
    .find_map(|kind| match kind {
      EntryKind::Person {
        message, text: said, ..
      } if said.as_str() == text => Some(message),
      _ => None,
    })
    .unwrap_or_else(|| panic!("the message {text:?}"))
}

/// Moves the pointer onto the bubble of the message `id` once it is laid out, which shows its toolbar.
fn hover(screen: &mut Orchestrator, id: &str) {
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
}

/// What the Orchestrator answered, in order.
fn replies(screen: &Orchestrator) -> Vec<String> {
  entries(screen)
    .into_iter()
    .filter_map(|kind| match kind {
      EntryKind::Reply { text, .. } => Some(text),
      _ => None,
    })
    .collect()
}

#[test]
fn orchestrator_rewind_to_before_a_message_drops_it_and_the_next_answer_does_not_know_it() {
  let mut screen = two_turns();
  let id = format!("orchestrator-message-{}", message(&screen, SECOND));
  hover(&mut screen, &id);
  screen.press(&format!("{id}-rewind"));
  screen.until("the question", |screen| screen.exists("orchestrator-rewind-confirm"));
  assert!(
    screen
      .text("orchestrator-rewind-title")
      .ends_with("and drop 2 messages?"),
    "{}",
    screen.text("orchestrator-rewind-title")
  );
  assert!(
    screen
      .text("orchestrator-rewind-goes")
      .contains("1 from you and 1 from the Orchestrator"),
    "{}",
    screen.text("orchestrator-rewind-goes")
  );
  screen.press("orchestrator-rewind-confirm");
  screen.until("the rewound line", |screen| {
    !screen.exists("orchestrator-rewind") && screen.text("screen-orchestrator").contains("2 messages dropped")
  });
  assert!(
    screen.text("orchestrator-input").starts_with(SECOND),
    "its words are back in the composer: {}",
    screen.text("orchestrator-input")
  );
  assert!(
    entries(&screen)
      .iter()
      .any(|kind| matches!(kind, EntryKind::Event(ChatEvent::Rewound { dropped: 2, .. }))),
    "the conversation keeps where it went back to"
  );

  screen.set("orchestrator-input", "");
  answered(&mut screen, "Which words do you know?");
  let replies = replies(&screen);
  assert_eq!(replies.len(), 2, "{replies:?}");
  let told = &replies[1];
  assert!(told.contains(FIRST), "it knows what was kept: {told}");
  assert!(!told.contains("banana"), "and not what was dropped: {told}");
}

#[test]
fn orchestrator_edit_an_earlier_message_asks_then_sends_it_in_its_place() {
  let mut screen = two_turns();
  let id = format!("orchestrator-message-{}", message(&screen, FIRST));
  hover(&mut screen, &id);
  screen.press(&format!("{id}-edit"));
  assert!(screen.exists(&format!("{id}-input")), "the bubble is a field");
  assert_eq!(
    screen.text(&format!("{id}-drops")),
    "Sending drops the 3 messages after this one"
  );
  assert_eq!(
    screen.text("orchestrator-edit-drop-line-label"),
    "These 3 messages go when you send"
  );
  screen.set(&format!("{id}-input"), "Remember the word cherry.");
  screen.press(&format!("{id}-send"));
  assert_eq!(
    screen.text("orchestrator-rewind-title"),
    "Send the edit and drop 3 messages?"
  );
  screen.press("orchestrator-rewind-confirm");
  screen.until("the edit's answer", |screen| {
    replies(screen)
      .iter()
      .any(|reply| reply.ends_with("Remember the word cherry."))
  });
  let said: Vec<String> = entries(&screen)
    .into_iter()
    .filter_map(|kind| match kind {
      EntryKind::Person { text, .. } => Some(text.as_str().to_owned()),
      _ => None,
    })
    .collect();
  assert_eq!(said, ["Remember the word cherry."], "the edit took the message's place");
  let told = replies(&screen).pop().expect("the answer");
  assert!(!told.contains("apple") && !told.contains("banana"), "{told}");
}

#[test]
fn orchestrator_edit_cancels_with_esc_and_changes_nothing() {
  let mut screen = two_turns();
  let id = format!("orchestrator-message-{}", message(&screen, FIRST));
  let before = entries(&screen);
  hover(&mut screen, &id);
  screen.press(&format!("{id}-edit"));
  screen.key(("Escape", "Escape"), false);
  assert!(!screen.exists(&format!("{id}-input")), "Esc ends the edit");
  assert!(screen.exists(&format!("{id}-bubble")));
  assert_eq!(entries(&screen), before);
}

#[test]
fn orchestrator_rewind_is_a_persons_alone() {
  let mut screen = two_turns();
  let id = format!("orchestrator-message-{}", message(&screen, SECOND));
  let before = entries(&screen);
  hover(&mut screen, &id);
  screen.press(&format!("{id}-rewind"));
  screen.until("the question", |screen| screen.exists("orchestrator-rewind-confirm"));
  assert!(screen.window.refused_press().is_none());
  with_synthetic_input(|| screen.click("orchestrator-rewind-confirm"));
  screen.until("the refusal", |screen| screen.window.refused_press().is_some());
  assert!(
    !screen.exists("orchestrator-rewind-person-only"),
    "nothing in the window says it"
  );
  with_synthetic_input(|| screen.key(("Enter", "Enter"), false));
  assert!(screen.window.refused_press().is_some(), "Enter is refused too");
  // A rewind that started would be under way by now: its background work had time to run and was polled.
  std::thread::sleep(std::time::Duration::from_millis(200));
  screen.until("a pass", |_| true);
  assert_eq!(entries(&screen), before, "nothing dropped");
  assert!(screen.exists("orchestrator-rewind"), "the question stays");
  screen.click("orchestrator-rewind-confirm");
  screen.until("the person's rewind", |screen| {
    !screen.exists("orchestrator-rewind") && screen.text("screen-orchestrator").contains("2 messages dropped")
  });
}

#[test]
fn orchestrator_rewind_that_failed_changes_nothing_and_says_why() {
  let mut screen = two_turns();
  let id = format!("orchestrator-message-{}", message(&screen, SECOND));
  hover(&mut screen, &id);
  screen.press(&format!("{id}-rewind"));
  screen.until("the question", |screen| screen.exists("orchestrator-rewind-confirm"));
  let chat = screen.db().chats().expect("chats")[0].chat.id();
  screen.db().delete_chat(chat).expect("deleted elsewhere first");
  screen.click("orchestrator-rewind-confirm");
  screen.until("why it failed", |screen| screen.exists("orchestrator-rewind-error"));
  assert!(
    screen
      .text("orchestrator-rewind-error")
      .starts_with("Nothing was dropped: "),
    "{}",
    screen.text("orchestrator-rewind-error")
  );
  assert!(screen.exists("orchestrator-rewind"), "the question stays");
}

#[test]
fn orchestrator_edit_and_rewind_wait_while_the_orchestrator_answers() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("Keep thinking about the queue.");
  screen.until("the answer under way", |screen| screen.exists("orchestrator-thinking"));
  let id = format!(
    "orchestrator-message-{}",
    message(&screen, "Keep thinking about the queue.")
  );
  screen.until("the message's toolbar", |screen| screen.exists(&format!("{id}-edit")));
  hover(&mut screen, &id);
  let held = |screen: &mut Orchestrator, part: &str| {
    screen
      .window
      .tree
      .get_element_by_id(&format!("{id}-{part}"))
      .is_some_and(|element| element.has_class(HELD_CLASS))
  };
  assert!(held(&mut screen, "edit"), "Edit waits for the answer");
  assert!(held(&mut screen, "rewind"), "Rewind to here waits for the answer");
  assert!(
    !held(&mut screen, "fork") && !held(&mut screen, "copy"),
    "Fork from here (ORC-334) and Copy still work"
  );
  for part in ["edit", "rewind"] {
    screen.window.hover(&format!("{id}-{part}"));
    assert_eq!(
      screen.text(&format!("{id}-{part}-tooltip")),
      "Stop the answer to edit or rewind",
      "{part} says why"
    );
  }
  screen.press(&format!("{id}-edit"));
  assert!(!screen.exists(&format!("{id}-input")), "a press edits nothing");
  let before = entries(&screen);
  screen.press(&format!("{id}-rewind"));
  assert!(!screen.exists("orchestrator-rewind"), "and asks nothing");
  // The answer goes on meanwhile: what was kept stays, and nothing was rewound.
  let after = entries(&screen);
  assert!(after.starts_with(&before), "{before:?} then {after:?}");
  assert!(
    !after
      .iter()
      .any(|kind| matches!(kind, EntryKind::Event(ChatEvent::Rewound { .. }))),
    "{after:?}"
  );
  screen.click("orchestrator-stop");
}
