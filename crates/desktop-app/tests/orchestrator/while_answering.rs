//! Writing to the Orchestrator while it answers (ORC-92): the composer stays usable, its send button split with the
//! send menu; Ctrl+Enter queues the message after the answer, which shows it queued with *Edit* and *Remove* and sends
//! it once the answer ended; *Send now* hands it to the answer, which takes it; *Stop and send* stops the answer and
//! sends it next; Esc in the message field stops the answer.

use orchester_core::orchestrator::{EntryKind, TurnState};

use super::{
  harness::{Orchestrator, Setup},
  paste::paste_long_text,
};

/// Starts a conversation whose answer runs until it is stopped, or until a message sent now arrives (`waits`).
fn answering(waits: bool) -> Orchestrator {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(orchester_app::routes::OrchestratorChat::New);
  screen.send(if waits {
    "Wait for a message."
  } else {
    "Keep thinking about the queue."
  });
  screen.until("the answer under way", |screen| {
    screen.exists("orchestrator-stop") && screen.exists("orchestrator-send-options")
  });
  screen
}

/// Writes `text` with the focus in the message field.
fn write(screen: &mut Orchestrator, text: &str) {
  screen.set("orchestrator-input", text);
  screen.click("orchestrator-input");
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
}

/// The turns the person's messages stand at, oldest first.
fn turns(screen: &Orchestrator) -> Vec<TurnState> {
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Person { turn, .. } => Some(turn),
      _ => None,
    })
    .collect()
}

#[test]
fn orchestrator_while_answering_queues_a_message_and_sends_it_after_the_answer() {
  let mut screen = answering(false);
  assert!(
    screen.text("orchestrator-keys").contains("Esc"),
    "the keys say Esc stops the answer: {}",
    screen.text("orchestrator-keys")
  );
  write(&mut screen, "Then plan the release.");
  screen.key(("Enter", "Enter"), true);
  // The row shows while the message is being kept; the store has it once kept.
  screen.until("the message queued", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Queued · sent after this answer")
      && turns(screen).len() == 2
  });
  assert!(screen.exists("orchestrator-stop"), "the answer goes on");
  assert_eq!(
    turns(&screen),
    [
      TurnState::Thinking,
      TurnState::Queued(orchester_core::orchestrator::Queued::AfterAnswer)
    ]
  );
  screen.click("orchestrator-stop");
  screen.until("the queued message answered after the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: Then plan the release.")
  });
  assert!(screen.exists("orchestrator-stopped"), "the first answer stopped");
}

#[test]
fn orchestrator_while_answering_takes_back_or_removes_a_queued_message() {
  let mut screen = answering(false);
  write(&mut screen, "Then plan the release.");
  screen.key(("Enter", "Enter"), true);
  screen.until("the message queued", |screen| turns(screen).len() == 2);
  let queued = screen.db().chats().expect("chats").remove(0).chat.id();
  let message = screen
    .db()
    .chat_entries(queued)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Person { message, .. } => Some(message),
      _ => None,
    })
    .next_back()
    .expect("the queued message");
  screen.until("its actions", |screen| {
    screen.exists(&format!("orchestrator-queued-edit-{message}"))
  });
  screen.click(&format!("orchestrator-queued-edit-{message}"));
  screen.until("its words back in the composer", |screen| {
    screen.text("orchestrator-input") == "Then plan the release." && turns(screen).len() == 1
  });
  write(&mut screen, "Never mind.");
  screen.key(("Enter", "Enter"), true);
  screen.until("queued again", |screen| turns(screen).len() == 2);
  let again = screen
    .db()
    .chat_entries(queued)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Person { message, .. } => Some(message),
      _ => None,
    })
    .next_back()
    .expect("the queued message");
  screen.until("its actions", |screen| {
    screen.exists(&format!("orchestrator-queued-remove-{again}"))
  });
  screen.click(&format!("orchestrator-queued-remove-{again}"));
  screen.until("removed", |screen| turns(screen).len() == 1);
  assert!(screen.exists("orchestrator-stop"), "the answer went on");
}

/// The review's case: *Edit* on a queued message of files only takes its files back into the composer, without their
/// names as words, and sending it again keeps the file with the new message.
#[test]
fn orchestrator_while_answering_takes_back_a_queued_message_with_its_files() {
  let mut screen = answering(false);
  paste_long_text(&mut screen);
  screen.key(("Enter", "Enter"), true);
  screen.until("the message queued", |screen| turns(screen).len() == 2);
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let (message, attached) = screen
    .db()
    .chat_attachments(chat)
    .expect("attachments")
    .into_iter()
    .next()
    .expect("the queued message's file");
  assert_eq!(attached.len(), 1);
  screen.until("its actions", |screen| {
    screen.exists(&format!("orchestrator-queued-edit-{message}"))
  });
  screen.click(&format!("orchestrator-queued-edit-{message}"));
  screen.until("its file back in the composer", |screen| {
    turns(screen).len() == 1
      && screen
        .text("orchestrator-composer-attachments")
        .contains("Pasted text.txt")
  });
  assert!(
    !screen.text("orchestrator-input").contains("Pasted text.txt"),
    "no names as words: {}",
    screen.text("orchestrator-input")
  );
  assert!(
    screen.db().chat_attachments(chat).expect("attachments").is_empty(),
    "the removed message keeps no file"
  );
  write(&mut screen, "Read this first.");
  screen.key(("Enter", "Enter"), true);
  screen.until("queued again with its file", |screen| {
    turns(screen).len() == 2 && !screen.db().chat_attachments(chat).expect("attachments").is_empty()
  });
  let db = screen.db();
  let (_, again) = db
    .chat_attachments(chat)
    .expect("attachments")
    .into_iter()
    .next()
    .expect("the file");
  assert_eq!(again[0].name().as_str(), "Pasted text.txt");
  assert!(db.attachment_path(&again[0]).is_file(), "the copy is kept");
}

#[test]
fn orchestrator_while_answering_sends_now_into_the_answer() {
  let mut screen = answering(true);
  write(&mut screen, "Also check the queue.");
  screen.click("orchestrator-send-options");
  screen.until("the send menu", |screen| screen.exists("orchestrator-send-now"));
  assert!(screen.exists("orchestrator-send-after-answer"));
  assert!(screen.exists("orchestrator-stop-and-send"));
  screen.click("orchestrator-send-now");
  screen.until("the answer took it", |screen| {
    let said = screen.text("orchestrator-conversation");
    said.contains("Noted: Also check the queue.") && said.contains("Taken mid-answer")
  });
  assert_eq!(turns(&screen), [TurnState::Answered, TurnState::Joined]);
}

#[test]
fn orchestrator_while_answering_stops_and_sends_and_esc_stops_the_answer() {
  let mut screen = answering(false);
  write(&mut screen, "Stop: what waits for me?");
  screen.click("orchestrator-send-options");
  screen.until("the send menu", |screen| screen.exists("orchestrator-stop-and-send"));
  screen.click("orchestrator-stop-and-send");
  screen.until("stopped and sent next", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: Stop: what waits for me?")
      && screen.exists("orchestrator-stopped")
  });
  screen.until("the composer free", |screen| !screen.exists("orchestrator-stop"));
  screen.send("Keep thinking once more.");
  screen.until("the next answer under way", |screen| screen.exists("orchestrator-stop"));
  write(&mut screen, "");
  screen.key(("Escape", "Escape"), false);
  screen.until("Esc stopped it", |screen| {
    turns(screen).last() == Some(&TurnState::Stopped)
  });
}
