//! The conversation follows its end (`ui::chat::follow`, shared with a task's Conversation): another conversation
//! chosen in History opens at the end of its newest turn, also after the person scrolled up in the one before; a reply
//! that grows the newest turn keeps a chat at its end there, and leaves a chat the person scrolled up where it was,
//! with *Jump to latest · N new*, which goes to the end; sending a message goes to the end too.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{id::ChatId, resource::ModelId, time::Timestamp};

use super::{
  harness::Orchestrator,
  scroll::{long_conversation, long_message, long_turns, newest_foot_gap, newest_top, settle, wheel},
};

/// The conversation of `screen`'s project other than `not`; the first one with `None`.
fn other_chat(screen: &Orchestrator, not: Option<ChatId>) -> ChatId {
  screen
    .db()
    .chats()
    .expect("chats")
    .into_iter()
    .map(|listing| listing.chat.id())
    .find(|chat| Some(*chat) != not)
    .expect("the conversation")
}

/// Keeps a long reply of the Orchestrator's, tagged `tag`, in `chat`, as its agent does while it answers.
fn reply(screen: &mut Orchestrator, chat: ChatId, tag: &str) {
  let model = ModelId::new("haiku").expect("model");
  screen
    .db()
    .add_chat_reply(chat, (&long_message(tag), &model), Timestamp::now())
    .expect("kept");
  let root = screen.root.clone();
  screen
    .window
    .launch
    .services()
    .expect("the session started")
    .runners
    .chat_changed(&root);
  let shown = format!("{tag} paragraph 1:");
  screen.until("the reply", |screen| {
    screen.text("orchestrator-conversation").contains(&shown)
  });
  settle(screen);
}

/// Asserts the chat sits at the very end of its newest turn and follows it.
fn at_end(screen: &mut Orchestrator, when: &str) {
  let gap = newest_foot_gap(screen);
  assert!(
    gap.abs() < 1.0,
    "{when}: the chat is at its newest turn's end, {gap} under it"
  );
  assert!(!screen.exists("conversation-jump"), "{when}: the chat follows its end");
}

/// Opens conversation `chat` and waits until it shows `answer`.
fn open(screen: &mut Orchestrator, chat: ChatId, answer: &str) {
  screen.go(OrchestratorChat::Chat(chat));
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains(answer)
  });
  settle(screen);
}

#[test]
fn orchestrator_follow_switching_conversations_opens_each_at_its_end() {
  let mut screen = long_conversation(2);
  let first = other_chat(&screen, None);
  screen.go(OrchestratorChat::New);
  long_turns(&mut screen, "Other", 2);
  let second = other_chat(&screen, Some(first));
  at_end(&mut screen, "after the answer");
  wheel(&mut screen, 600.0);
  assert!(
    screen.exists("conversation-jump"),
    "scrolled up, the chat no longer follows"
  );
  open(&mut screen, first, "Noted: Turn 2 paragraph 1");
  at_end(&mut screen, "the first conversation, chosen after a scroll up");
  open(&mut screen, second, "Noted: Other 2 paragraph 1");
  at_end(&mut screen, "the second conversation again");
}

#[test]
fn orchestrator_follow_a_growing_reply_keeps_the_end_in_view() {
  let mut screen = long_conversation(2);
  let chat = other_chat(&screen, None);
  at_end(&mut screen, "after the answer");
  reply(&mut screen, chat, "More");
  at_end(&mut screen, "after a long reply grew the newest turn");
}

#[test]
fn orchestrator_follow_scrolled_up_stays_and_jump_to_latest_goes_to_the_end() {
  let mut screen = long_conversation(2);
  let chat = other_chat(&screen, None);
  wheel(&mut screen, 600.0);
  settle(&mut screen);
  let before = newest_top(&mut screen);
  reply(&mut screen, chat, "Later");
  let after = newest_top(&mut screen);
  assert!(
    (after - before).abs() < 1.0,
    "a reply leaves the scrolled chat where it was: {before} then {after}"
  );
  let jump = screen.text("conversation-jump");
  assert!(
    jump.contains("Jump to latest · 1 new"),
    "the pill counts the reply: {jump}"
  );
  screen.click("conversation-jump");
  settle(&mut screen);
  at_end(&mut screen, "after Jump to latest");
}

#[test]
fn orchestrator_follow_sending_after_a_scroll_up_goes_to_the_end() {
  let mut screen = long_conversation(2);
  wheel(&mut screen, 600.0);
  assert!(
    screen.exists("conversation-jump"),
    "scrolled up, the chat no longer follows"
  );
  long_turns(&mut screen, "Sent", 1);
  at_end(&mut screen, "after a message sent from up the chat");
}
