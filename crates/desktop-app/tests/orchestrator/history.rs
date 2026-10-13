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

use super::{
  harness::{Orchestrator, Setup},
  support::project_id,
};

/// A conversation started with `text` at `at`, answered with `reply`.
fn kept(screen: &Orchestrator, text: &str, at: Timestamp, reply: &str) -> ChatId {
  let mut db = screen.db();
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let body = MessageBody::new(text).expect("text");
  db.keep_chat_message(&Chat::start(chat, &body, at), (message, &body), at)
    .expect("kept");
  db.add_chat_reply(chat, (reply, &ModelId::new("haiku").expect("model")), at)
    .expect("reply");
  db.end_chat_turn(message, &TurnState::Answered).expect("ended");
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
    screen.text("orchestrator-title") == "Why did KON-12 go back?"
  });
  assert!(screen.text("orchestrator-conversation").contains("Its review failed."));
  screen.click("orchestrator-history");
  screen.until("History", |screen| screen.exists("orchestrator-history-panel"));
  let listed = screen.text("orchestrator-history-panel");
  assert!(listed.contains("TODAY") && listed.contains("EARLIER"), "{listed}");
  assert!(
    screen
      .text(&format!("orchestrator-history-{newer}"))
      .contains("This conversation · 2 messages"),
    "{listed}"
  );
  screen.set("orchestrator-history-search", "auth");
  screen.until("the search", |screen| {
    !screen.exists(&format!("orchestrator-history-{newer}"))
  });
  screen.click(&format!("orchestrator-history-{older}"));
  screen.until("the older conversation", |screen| {
    screen.text("orchestrator-title") == "Order of the auth work"
  });
  assert_eq!(
    screen.route(),
    Route::Project(project_id(), ProjectPage::Orchestrator(OrchestratorChat::Chat(older)))
  );
  assert!(
    screen.exists("orchestrator-history-panel"),
    "History stays open while another conversation opens"
  );
  screen.click("orchestrator-input");
  screen.key(("Escape", "Escape"), false);
  assert!(!screen.exists("orchestrator-history-panel"), "Esc closes History");
}
