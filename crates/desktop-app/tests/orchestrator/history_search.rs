//! History's search in the messages (ORC-307, `13 · History · search · match in messages only` `SZ6QY`): a word said
//! only in a reply finds its conversation, the row shows the line around it under its title with how many matches,
//! the line under the field says how many conversations hold it, and opening the result shows the turn that says it,
//! not the conversation's end. Run with `cargo test -p orchester-app --test orchestrator history_search`.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, TurnState},
  resource::ModelId,
  time::Timestamp,
};

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
  scroll::{long_message, settle},
};

/// A conversation of `turns` long turns answered at `at`, the first reply saying `said` too: its id and the first
/// message's.
fn long_with(screen: &Orchestrator, said: &str, turns: usize, at: Timestamp) -> (ChatId, ChatMessageId) {
  let mut db = screen.db();
  let model = ModelId::new("haiku").expect("model");
  let first = MessageBody::new("Plan the retry work").expect("text");
  let chat = Chat::start(ChatId::generate(), &first, at);
  let mut opening = None;
  for turn in 1..=turns {
    let message = ChatMessageId::generate();
    let body = MessageBody::new(&long_message(&format!("Ask {turn}"))).expect("text");
    let body = if turn == 1 { first.clone() } else { body };
    db.keep_chat_message(&chat, (message, &body), at).expect("kept");
    let reply = long_message(&format!("Answer {turn}"));
    let reply = if turn == 1 {
      format!("{reply}\n\nRetries back off with {said}.")
    } else {
      reply
    };
    db.add_chat_reply(chat.id(), (&reply, &model), at).expect("reply");
    db.end_chat_turn(message, &TurnState::Answered, at).expect("ended");
    opening.get_or_insert(message);
  }
  (chat.id(), opening.expect("a first message"))
}

/// Whether element `id` shows in the chat's view.
fn in_view(screen: &mut Orchestrator, id: &str) -> bool {
  if !screen.exists(id) {
    return false;
  }
  let (shown, view) = (screen.bounds(id), screen.bounds("orchestrator-scroll"));
  shown.y < view.y + view.height && shown.y + shown.height > view.y
}

#[test]
fn history_search_finds_a_word_said_only_in_a_message_and_opens_at_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (found, first) = long_with(&screen, "full jitter", 6, Timestamp::now());
  let other = kept(
    &screen,
    "Set up the webhook pipeline",
    Timestamp::now(),
    "Webhooks first.",
  );
  screen.go(OrchestratorChat::Chat(other));
  screen.until("the other conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Webhooks first.")
  });
  screen.click("orchestrator-history");
  screen.until("History", |screen| screen.exists("orchestrator-history-panel"));
  screen.set("orchestrator-history-search", "JITTER");
  let row = format!("orchestrator-history-{found}");
  screen.until("the message search", |screen| {
    screen.exists(&format!("orchestrator-history-line-{found}"))
  });
  assert!(
    !screen.exists(&format!("orchestrator-history-{other}")),
    "a conversation that never says it is not listed"
  );
  assert_eq!(
    screen.text(&format!("orchestrator-history-title-{found}")),
    "Plan the retry work",
    "the title stays as it is"
  );
  let line = screen.text(&format!("orchestrator-history-said-{found}"));
  assert!(line.contains("full jitter"), "the words around the match: {line}");
  assert!(line.starts_with('…'), "cut before the match: {line}");
  assert!(
    screen
      .text(&format!("orchestrator-history-line-{found}"))
      .starts_with("Orchestrator:"),
    "who said it"
  );
  assert!(
    screen
      .text(&format!("orchestrator-history-meta-{found}"))
      .ends_with(" · 1 match"),
    "{}",
    screen.text(&row)
  );
  screen.until("the search's end", |screen| {
    screen.text("orchestrator-history-status") == "Found in 1 conversation · titles and messages"
  });
  screen.click(&row);
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("full jitter")
  });
  settle(&mut screen);
  assert!(
    in_view(&mut screen, &format!("orchestrator-turn-{first}")),
    "it opens at the turn that says it, not at its end"
  );
  assert!(
    screen.exists("orchestrator-history-panel"),
    "History stays open on the result"
  );
}

#[test]
fn history_search_says_how_many_it_searched_when_nothing_holds_the_words() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let chat = kept(
    &screen,
    "Set up the webhook pipeline",
    Timestamp::now(),
    "Webhooks first.",
  );
  screen.go(OrchestratorChat::Chat(chat));
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Webhooks first.")
  });
  screen.click("orchestrator-history");
  screen.until("History", |screen| screen.exists("orchestrator-history-panel"));
  screen.set("orchestrator-history-search", "jitter");
  screen.until("the search's end", |screen| {
    screen.text("orchestrator-history-status") == "Searched 1 conversation · titles and messages"
  });
  assert!(screen.exists("orchestrator-history-empty"));
  screen.set("orchestrator-history-search", "webhook");
  screen.until("the matches in the messages", |screen| {
    screen.exists(&format!("orchestrator-history-line-{chat}"))
  });
  let meta = screen.text(&format!("orchestrator-history-meta-{chat}"));
  assert!(meta.ends_with(" · title and 2 messages"), "title and messages: {meta}");
  assert!(
    screen
      .text(&format!("orchestrator-history-line-{chat}"))
      .starts_with("You:"),
    "the first match is in the person's message"
  );
}
