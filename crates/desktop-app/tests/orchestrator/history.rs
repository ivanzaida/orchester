//! History (`kPs8W`): the project's conversations by the day they last changed, the one shown marked, a search by
//! title, and opening one; the Orchestrator opens on the latest.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, TurnState},
  resource::ModelId,
  time::Timestamp,
};

use super::harness::{Orchestrator, Setup};

/// A conversation started with `text` at `at`, answered with `reply`.
pub(super) fn kept(screen: &Orchestrator, text: &str, at: Timestamp, reply: &str) -> ChatId {
  let mut db = screen.db();
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let body = MessageBody::new(text).expect("text");
  db.keep_chat_message(&Chat::start(chat, &body, at), (message, &body), at)
    .expect("kept");
  db.add_chat_reply(chat, (reply, &ModelId::new("haiku").expect("model")), at)
    .expect("reply");
  db.end_chat_turn(message, &TurnState::Answered, at).expect("ended");
  chat
}

#[test]
fn orchestrator_history_lists_conversations_by_day_and_opens_one() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let week_ago = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 7 * 24 * 3_600_000);
  let older = kept(&screen, "Order of the auth work", week_ago, "Auth first.");
  let newer = kept(
    &screen,
    "Why did KON-12 go back?",
    Timestamp::now(),
    "Its review failed.",
  );
  screen.go(OrchestratorChat::Latest);
  screen.until("the latest conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Its review failed.")
  });
  assert_eq!(screen.text("orchestrator-title"), "Why did KON-12 go back?");
  screen.click("orchestrator-history");
  screen.until("History", |screen| screen.exists("orchestrator-history-panel"));
  let listed = screen.text("orchestrator-history-panel");
  assert!(listed.contains("TODAY") && listed.contains("EARLIER"), "{listed}");
  assert!(
    screen
      .text(&format!("orchestrator-history-{newer}"))
      .ends_with(" · 2 messages"),
    "when and how many messages: {listed}"
  );
  screen.set("orchestrator-history-search", "auth");
  screen.until("the search", |screen| {
    !screen.exists(&format!("orchestrator-history-{newer}"))
  });
  screen.click(&format!("orchestrator-history-{older}"));
  screen.until("the older conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Auth first.")
  });
  assert_eq!(
    screen.route(),
    Route::Project(screen.project, ProjectPage::Orchestrator(OrchestratorChat::Chat(older)))
  );
  assert!(
    screen.exists("orchestrator-history-panel"),
    "History stays open while another conversation opens"
  );
  screen.click("orchestrator-input");
  screen.key(("Escape", "Escape"), false);
  assert!(!screen.exists("orchestrator-history-panel"), "Esc closes History");
}

#[test]
fn orchestrator_opening_a_conversation_never_looks_like_a_new_one() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let older = kept(&screen, "Order of the auth work", Timestamp::now(), "Auth first.");
  kept(
    &screen,
    "Why did KON-12 go back?",
    Timestamp::now(),
    "Its review failed.",
  );
  screen.go(OrchestratorChat::Latest);
  screen.until("the latest conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Its review failed.")
  });
  screen.click("orchestrator-history");
  screen.until("History", |screen| screen.exists("orchestrator-history-panel"));
  let hold = screen
    .window
    .launch
    .services()
    .expect("the session started")
    .hold
    .clone();
  hold.set(true);
  screen.click(&format!("orchestrator-history-{older}"));
  // Its turns are bones named *Loading conversation…* (ORC-110).
  screen.until("the conversation loading", |screen| {
    screen.exists("orchestrator-loading")
  });
  assert_eq!(screen.text("orchestrator-title"), "Order of the auth work");
  assert_eq!(screen.text("orchestrator-meta"), "· opening from History");
  hold.set(false);
  screen.until("the older conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Auth first.")
  });
}
