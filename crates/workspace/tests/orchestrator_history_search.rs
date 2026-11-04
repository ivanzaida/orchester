//! History's search in the words of the conversations (ORC-307): a conversation whose messages hold the text is found,
//! case and accents ignored, with how many places and messages hold it and the line History shows under its title,
//! who said it and the words around the first place cut on one line, at the entry it opens at. Run with
//! `cargo test -p orchester-workspace --test orchestrator_history_search search`.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, TurnState},
  resource::ModelId,
};
use orchester_workspace::{
  orchestrator::search::{Speaker, search_in},
  tasks::timeline::find::Needle,
};
use support::{Fixture, at};

/// Keeps a conversation started by `first` at `millis`, answered with `replies`.
fn kept(fixture: &mut Fixture, first: &str, replies: &[&str], millis: i64) -> ChatId {
  let body = MessageBody::new(first).expect("says something");
  let chat = Chat::start(ChatId::generate(), &body, at(millis));
  let message = ChatMessageId::generate();
  fixture
    .db
    .keep_chat_message(&chat, (message, &body), at(millis))
    .expect("kept");
  let model = ModelId::new("haiku").expect("model");
  for reply in replies {
    fixture
      .db
      .add_chat_reply(chat.id(), (reply, &model), at(millis + 1))
      .expect("reply");
  }
  fixture
    .db
    .end_chat_turn(message, &TurnState::Answered, at(millis + 2))
    .expect("ended");
  chat.id()
}

/// No Markdown: the words as written.
fn as_written(_: &str) -> Option<String> {
  None
}

#[test]
fn search_finds_words_said_only_in_a_reply_with_the_line_around_the_first_place() {
  let mut fixture = Fixture::new();
  let failed = kept(
    &mut fixture,
    "Why did KON-11 fail?",
    &[
      "Run #7 stopped at step 03 Check. The test suite called the GitHub API 412 times in a minute and hit the Rate\n\
       Limit at 11:32.",
      "Waiting for the rate limit to reset takes up to an hour.",
    ],
    10,
  );
  let other = kept(&mut fixture, "Plan the webhook work", &["Webhooks first."], 20);
  let needle = Needle::new("rate limit").expect("a query");
  let found = search_in(fixture.repo.path(), &[failed, other], &needle, &as_written).expect("searched");
  assert_eq!(found.len(), 1, "only the conversation that says it: {found:?}");
  let first = &found[0];
  assert_eq!(first.chat, failed);
  assert_eq!((first.places, first.messages), (2, 2));
  assert_eq!(first.line.who, Speaker::Orchestrator);
  assert_eq!(
    first.line.text, "…a minute and hit the Rate Limit at 11:32.",
    "cut at a word before the place, on one line"
  );
  let marked: Vec<&str> = first
    .line
    .marks
    .iter()
    .map(|range| &first.line.text[range.clone()])
    .collect();
  assert_eq!(marked, ["Rate Limit"], "the place, as written");
}

#[test]
fn search_says_the_person_said_it_and_keeps_a_short_message_whole() {
  let mut fixture = Fixture::new();
  let planned = kept(
    &mut fixture,
    "Keep the search jobs under the rate limit",
    &["Done."],
    10,
  );
  let needle = Needle::new("  RÁTE ").expect("a query");
  let found = search_in(fixture.repo.path(), &[planned], &needle, &as_written).expect("searched");
  assert_eq!(found.len(), 1);
  let line = &found[0].line;
  assert_eq!(line.who, Speaker::Person);
  assert_eq!(line.text, "…search jobs under the rate limit");
  assert_eq!(found[0].messages, 1);
}

#[test]
fn search_finds_nothing_where_no_message_says_it() {
  let mut fixture = Fixture::new();
  let chat = kept(&mut fixture, "Plan the webhook work", &["Webhooks first."], 10);
  let needle = Needle::new("jitter").expect("a query");
  let found = search_in(fixture.repo.path(), &[chat], &needle, &as_written).expect("searched");
  assert!(found.is_empty(), "{found:?}");
}
