//! The person's report: a long text pasted into the Orchestrator's composer of a long conversation became
//! *Pasted text.txt* and was sent. The sent message shows the file's chip alone, at the message's end, its preview
//! closed until the chip is pressed, and the chat stays at the end of the new turn, following it; a wheel over the
//! opened text scrolls that text and leaves the chat following, without *Jump to latest*.

use lurq::app::events::{ClipboardContent, ScrollPhase};

use super::{
  harness::Orchestrator,
  scroll::{long_conversation, newest_foot_gap, settle},
};

/// A pasted text of about 3 KB, past the long-paste limit.
fn long_text() -> String {
  (1..=40)
    .map(|line| format!("Line {line}: auto mode holds harmless, read-only commands for a person instead."))
    .collect::<Vec<_>>()
    .join("\n")
}

/// Pastes [`long_text`] into the composer and waits for its chip.
fn paste_long_text(screen: &mut Orchestrator) {
  screen.click("orchestrator-input");
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
  assert!(screen.window.tree.paste(ClipboardContent::text(long_text())));
  screen.until("the pasted text's chip", |screen| {
    screen
      .text("orchestrator-composer-attachments")
      .contains("Pasted text.txt")
  });
}

/// The sent message's element id and its file chip's, once the conversation keeps the file.
fn sent_message(screen: &Orchestrator) -> Option<(String, String)> {
  let db = screen.db();
  let chat = db.chats().ok()?.into_iter().next()?.chat.id();
  let (message, attached) = db.chat_attachments(chat).ok()?.into_iter().next()?;
  let id = format!("orchestrator-message-{message}");
  let chip = format!("{id}-attachment-{}", attached.first()?.id());
  Some((id, chip))
}

/// A long conversation whose newest message is the pasted text alone, answered: the message's id and its chip's.
fn sent_pasted_text() -> (Orchestrator, String, String) {
  let mut screen = long_conversation(2);
  paste_long_text(&mut screen);
  screen.send("");
  let mut sent = None;
  screen.until("the sent file", |screen| {
    sent = sent_message(screen);
    sent.is_some()
  });
  let (message, chip) = sent.expect("the sent message");
  screen.until("the answer", |screen| {
    screen.exists(&chip) && !screen.exists("orchestrator-thinking")
  });
  settle(&mut screen);
  (screen, message, chip)
}

/// Asserts the chat sits at the very end of its newest turn and follows it.
fn at_end(screen: &mut Orchestrator, when: &str) {
  let gap = newest_foot_gap(screen);
  assert!(
    gap.abs() < 1.0,
    "{when}: the chat is at the new turn's end, {gap} under it"
  );
  assert!(!screen.exists("conversation-jump"), "{when}: the chat follows its end");
}

/// Turns the wheel over the middle of `id`: up (`delta` above 0) scrolls toward the start.
fn wheel_over(screen: &mut Orchestrator, id: &str, delta: f32) {
  let frame = screen.bounds(id);
  let (x, y) = (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
  let tree = &mut screen.window.tree;
  tree.mouse_move(x, y);
  tree.scroll(x, y, 0.0, delta, ScrollPhase::Scroll);
  screen.window.pass();
  screen.window.pass();
}

#[test]
fn orchestrator_paste_a_sent_text_shows_its_chip_alone_at_the_messages_end() {
  let (mut screen, message, chip) = sent_pasted_text();
  assert!(
    screen.text(&chip).contains("Pasted text.txt"),
    "the sent message shows its file"
  );
  assert!(
    !screen.exists(&format!("{message}-attachment-preview")),
    "the file's preview stays closed until its chip is pressed"
  );
  let (message_box, chip_box) = (screen.bounds(&message), screen.bounds(&chip));
  let (message_end, chip_end) = (message_box.x + message_box.width, chip_box.x + chip_box.width);
  assert!(
    (message_end - chip_end).abs() < 1.0,
    "the chip sits at the message's end, as its words do: it ends at {chip_end}, the message at {message_end}"
  );
  at_end(&mut screen, "after the pasted text was sent");
}

#[test]
fn orchestrator_paste_a_wheel_over_the_opened_text_leaves_the_chat_following() {
  let (mut screen, message, chip) = sent_pasted_text();
  screen.click(&chip);
  let text = format!("{message}-attachment-preview-text-box");
  screen.until("the opened text", |screen| screen.exists(&text));
  settle(&mut screen);
  at_end(&mut screen, "the preview opened");
  // Down into the text, then back up: the text scrolls both ways and the chat stays where it is.
  wheel_over(&mut screen, &text, -120.0);
  wheel_over(&mut screen, &text, 60.0);
  settle(&mut screen);
  at_end(&mut screen, "after a wheel over the opened text");
}
