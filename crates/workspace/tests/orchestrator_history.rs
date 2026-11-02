//! What a person changes of an Orchestrator conversation in History (ORC-328): *Rename* keeps their title, which wins
//! over the one taken from the first message, a blank one is refused, and *Pin* keeps when it was pinned so History
//! lists the latest pin first; both survive the project database being opened again. Run with
//! `cargo test -p orchester-workspace --test orchestrator_history history`.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::Chat,
};
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::chat::{history_in, pin_in, rename_in};
use support::{Fixture, at};

/// Keeps a conversation started by `first` at `millis`.
fn started(fixture: &mut Fixture, first: &str, millis: i64) -> ChatId {
  let body = MessageBody::new(first).expect("says something");
  let chat = Chat::start(ChatId::generate(), &body, at(millis));
  fixture
    .db
    .keep_chat_message(&chat, (ChatMessageId::generate(), &body), at(millis))
    .expect("kept");
  chat.id()
}

fn title_in(fixture: &Fixture, chat: ChatId) -> String {
  let history = history_in(fixture.repo.path()).expect("History");
  let listing = history
    .chats
    .iter()
    .find(|listing| listing.chat.id() == chat)
    .expect("listed");
  listing.chat.title().to_owned()
}

#[test]
fn history_rename_keeps_the_persons_title_over_the_first_message_after_a_restart() {
  let mut fixture = Fixture::new();
  let chat = started(&mut fixture, "Why did KON-11 fail?", 10);
  rename_in(fixture.repo.path(), chat, "  KON-11 failure: expired GitHub token \n").expect("renamed");
  assert_eq!(title_in(&fixture, chat), "KON-11 failure: expired GitHub token");
  let reopened = ProjectDb::open(fixture.repo.path()).expect("opened again");
  assert_eq!(
    reopened.chat(chat).expect("read").expect("kept").title(),
    "KON-11 failure: expired GitHub token",
    "the conversation itself is titled so, as its header shows it"
  );
}

#[test]
fn history_rename_refuses_a_blank_title_and_keeps_the_one_before() {
  let mut fixture = Fixture::new();
  let chat = started(&mut fixture, "Plan the webhook work", 10);
  let refused = rename_in(fixture.repo.path(), chat, " \n\t").expect_err("blank");
  assert_eq!(refused.key, "orchestrator.conversation.blankTitle");
  assert_eq!(title_in(&fixture, chat), "Plan the webhook work");
}

#[test]
fn history_rename_and_pin_of_a_conversation_the_project_no_longer_keeps_say_so() {
  let fixture = Fixture::new();
  let gone = ChatId::generate();
  let renamed = rename_in(fixture.repo.path(), gone, "Anything").expect_err("not kept");
  assert_eq!(renamed.key, "orchestrator.conversation.gone");
  let pinned = pin_in(fixture.repo.path(), gone, Some(at(5))).expect_err("not kept");
  assert_eq!(pinned.key, "orchestrator.conversation.gone");
}

#[test]
fn history_pin_keeps_when_it_was_pinned_until_it_is_unpinned() {
  let mut fixture = Fixture::new();
  let first = started(&mut fixture, "Release checklist for v0.4", 10);
  let second = started(&mut fixture, "Plan the GitHub app permissions", 20);
  pin_in(fixture.repo.path(), first, Some(at(30))).expect("pinned");
  pin_in(fixture.repo.path(), second, Some(at(40))).expect("pinned");
  let pins = |fixture: &Fixture| {
    history_in(fixture.repo.path())
      .expect("History")
      .chats
      .iter()
      .map(|listing| (listing.chat.id(), listing.pinned))
      .collect::<Vec<_>>()
  };
  assert_eq!(pins(&fixture), [(second, Some(at(40))), (first, Some(at(30)))]);
  pin_in(fixture.repo.path(), second, None).expect("unpinned");
  assert_eq!(pins(&fixture), [(second, None), (first, Some(at(30)))]);
}
