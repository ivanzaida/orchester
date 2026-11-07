//! What a person changes of an Orchestrator conversation in History (ORC-328, ORC-329): *Rename* keeps their title,
//! which wins over the one taken from the first message, a blank one is refused, and *Pin* keeps when it was pinned so
//! History lists the latest pin first; *Archive* lists it under Archived until *Unarchive*, and *Delete* removes it
//! with its messages and the files attached to them, refused while it is busy; all survive the project database being
//! opened again. Run with `cargo test -p orchester-workspace --test orchestrator_history history`.

mod support;

use orchester_core::{
  attachment::{AttachmentName, NewAttachment},
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Chat, Subscription, TurnState, WatchTarget},
  run::{Answerer, TokenUsage},
};
use orchester_store::{ProjectDb, WatchStart};
use orchester_workspace::orchestrator::chat::{archive_in, delete_in, history_in, pin_in, rename_in};
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

/// Keeps a conversation started by `first` at `millis` with a file attached, answered, and what its turn spent; its
/// message's id.
fn answered_with_a_file(fixture: &mut Fixture, first: &str, millis: i64) -> (ChatId, ChatMessageId) {
  let body = MessageBody::new(first).expect("says something");
  let chat = Chat::start(ChatId::generate(), &body, at(millis));
  let message = ChatMessageId::generate();
  let file = NewAttachment::new(
    AttachmentName::new("notes.txt").expect("a name"),
    b"the token expired".to_vec(),
  )
  .expect("a text file");
  fixture
    .db
    .keep_chat_message_with(&chat, (message, &body), (&[file], Answerer::Person), at(millis))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(message, &TurnState::Answered, at(millis + 1))
    .expect("answered");
  let spent = TokenUsage {
    input: 120,
    cached_input: 0,
    output: 9,
  };
  fixture
    .db
    .record_chat_usage(
      (chat.id(), message),
      (AgentAccountId::generate(), spent),
      at(millis + 1),
    )
    .expect("spent");
  (chat.id(), message)
}

#[test]
fn history_archive_lists_the_conversation_under_archived_unpinned_until_unarchived_after_a_restart() {
  let mut fixture = Fixture::new();
  let kept = started(&mut fixture, "Plan the GitHub app permissions", 10);
  let archived = started(&mut fixture, "Daily report · Oct 24", 20);
  pin_in(fixture.repo.path(), archived, Some(at(25))).expect("pinned");
  archive_in(fixture.repo.path(), archived, Some(at(30))).expect("archived");
  let history = history_in(fixture.repo.path()).expect("History");
  let listed: Vec<ChatId> = history.listed().map(|listing| listing.chat.id()).collect();
  assert_eq!(listed, [kept], "History's list leaves it out");
  let under: Vec<_> = history
    .archived()
    .iter()
    .map(|listing| (listing.chat.id(), listing.archived, listing.pinned))
    .collect();
  assert_eq!(
    under,
    [(archived, Some(at(30)), None)],
    "archived, and no longer pinned"
  );
  assert_eq!(
    ProjectDb::open(fixture.repo.path())
      .expect("opened again")
      .latest_chat()
      .expect("read"),
    Some(kept),
    "an agent's wait goes to a conversation History lists"
  );
  archive_in(fixture.repo.path(), archived, None).expect("unarchived");
  let history = history_in(fixture.repo.path()).expect("History");
  assert!(history.archived().is_empty());
  assert_eq!(history.listed().count(), 2, "back under the day it last changed");
}

#[test]
fn history_delete_removes_the_conversation_its_messages_and_their_files_and_keeps_what_it_spent() {
  let mut fixture = Fixture::new();
  let (doomed, message) = answered_with_a_file(&mut fixture, "Why did KON-11 fail?", 10);
  let (other, _) = answered_with_a_file(&mut fixture, "Set up the webhook pipeline", 20);
  let copies: Vec<_> = fixture
    .db
    .chat_attachments(doomed)
    .expect("read")
    .into_values()
    .flatten()
    .map(|attachment| fixture.db.attachment_path(&attachment))
    .collect();
  assert_eq!(copies.len(), 1);
  assert!(copies[0].exists(), "the copy of its file is kept");
  delete_in(fixture.repo.path(), doomed).expect("deleted");
  let reopened = ProjectDb::open(fixture.repo.path()).expect("opened again");
  assert_eq!(reopened.chat(doomed).expect("read"), None);
  assert!(
    reopened.chat_entries(doomed).expect("read").is_empty(),
    "its messages go with it"
  );
  assert!(reopened.chat_attachments(doomed).expect("read").is_empty());
  assert!(!copies[0].exists(), "the copy of the file it alone held is removed");
  assert_eq!(
    reopened.chat_turn_usage(message).expect("read").len(),
    1,
    "what its turns spent stays counted toward the daily budget"
  );
  assert_eq!(
    reopened.chat_attachments(other).expect("read").len(),
    1,
    "another conversation keeps its files"
  );
  let left: Vec<ChatId> = history_in(fixture.repo.path())
    .expect("History")
    .chats
    .iter()
    .map(|listing| listing.chat.id())
    .collect();
  assert_eq!(left, [other]);
}

#[test]
fn history_delete_is_refused_while_the_orchestrator_answers_and_says_so_for_one_already_gone() {
  let mut fixture = Fixture::new();
  let thinking = started(&mut fixture, "Plan next week from the backlog", 10);
  let busy = delete_in(fixture.repo.path(), thinking).expect_err("its turn thinks");
  assert_eq!(busy.key, "orchestrator.conversation.busy");
  assert!(
    fixture.db.chat(thinking).expect("read").is_some(),
    "nothing was deleted"
  );
  let gone = delete_in(fixture.repo.path(), ChatId::generate()).expect_err("not kept");
  assert_eq!(gone.key, "orchestrator.conversation.gone");
  let archived = archive_in(fixture.repo.path(), ChatId::generate(), Some(at(5))).expect_err("not kept");
  assert_eq!(archived.key, "orchestrator.conversation.gone");
}

/// ORC-782 (`i687ei` c): History says what each conversation that leads something leads, read from the subscriptions
/// that lead their targets; one that leads nothing, or whose subscription ended, has no entry.
#[test]
fn history_says_what_each_leading_conversation_leads() {
  let mut fixture = Fixture::new();
  let leading = started(&mut fixture, "Plan the webhook retry work", 10);
  let quiet = started(&mut fixture, "Why did KON-11 fail?", 20);
  let module = |db: &mut ProjectDb, name: &str| {
    let details =
      ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open).expect("details");
    db.create_module(details, at(1)).expect("module").id()
  };
  let webhooks = module(&mut fixture.db, "Webhooks v2");
  let search = module(&mut fixture.db, "Search index");
  for (target, since) in [
    (WatchTarget::Module(webhooks), 30),
    (WatchTarget::Module(search), 31),
    (WatchTarget::WithoutModule, 32),
  ] {
    let started = fixture
      .db
      .start_subscription(&Subscription::new(leading, target, "", at(since)))
      .expect("kept");
    assert_eq!(started, WatchStart::Started);
  }
  let history = history_in(fixture.repo.path()).expect("History");
  let counts = history.leads_of(leading);
  assert_eq!(
    (counts.modules, counts.tasks, counts.without_module, counts.project),
    (2, 0, true, false)
  );
  assert!(history.leads_of(quiet).is_empty());
  assert!(
    !history.leads.contains_key(&quiet),
    "a conversation that leads nothing has no entry"
  );
  fixture
    .db
    .stop_subscription(leading, WatchTarget::WithoutModule, at(40))
    .expect("stopped");
  let counts = history_in(fixture.repo.path()).expect("History").leads_of(leading);
  assert!(!counts.without_module, "an ended subscription no longer counts");
  assert_eq!(counts.modules, 2);
}
