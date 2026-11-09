//! The card of a target another conversation leads (ORC-778; 05 `EIjDH` in `Orchestrator watch · redesign`): a /watch
//! that named it shows the card under its line, which only a person answers — automation's press is refused, visibly,
//! and moves nothing. *Take it here* moves the subscription here; *Open that conversation* opens it and moves nothing.
//! Either way the card settles into a line in its place.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat, ChatEvent, EntryKind, TurnState, WatchTarget,
    watch::{NamedTarget, WatchLine},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::watch::watch_in;

use super::harness::{Orchestrator, Setup};

/// A conversation the person started with `text`, answered.
fn conversation(db: &mut ProjectDb, text: &str) -> Chat {
  let (chat, at) = (ChatId::generate(), Timestamp::now());
  let body = MessageBody::new(text).expect("text");
  let first = ChatMessageId::generate();
  db.keep_chat_message(&Chat::start(chat, &body, at), (first, &body), at)
    .expect("kept");
  db.end_chat_turn(first, &TurnState::Answered, at).expect("ended");
  db.chat(chat).expect("read").expect("the conversation")
}

/// Module *Billing*, led by *Billing and invoices*, which a /watch in *Webhooks v2* named too: that conversation, this
/// one and the module.
pub(super) fn led(screen: &Orchestrator) -> (Chat, Chat, WatchTarget) {
  let mut db = screen.db();
  let details =
    ModuleDetails::new(Name::new("Billing").expect("name"), "", (None, None), ModuleState::Open).expect("details");
  let billing = WatchTarget::Module(db.create_module(details, Timestamp::now()).expect("module").id());
  let theirs = conversation(&mut db, "Billing and invoices");
  let mine = conversation(&mut db, "Webhooks v2");
  drop(db);
  let named = [NamedTarget {
    target: billing,
    name: Some("Billing".to_owned()),
  }];
  watch_in(&screen.root, &theirs, &named, "").expect("theirs watches it");
  let watched = watch_in(&screen.root, &mine, &named, "Ask me before merging.").expect("the command applied");
  assert_eq!(watched.led_elsewhere.len(), 1, "the card's target");
  (theirs, mine, billing)
}

/// The conversation that leads `target`.
pub(super) fn leader(screen: &Orchestrator, target: WatchTarget) -> Option<ChatId> {
  let db = screen.db();
  db.active_subscriptions()
    .expect("subscriptions")
    .of(target)
    .map(|led| led.chat())
}

/// The watch lines `chat` keeps, in order.
fn lines(screen: &Orchestrator, chat: ChatId) -> Vec<WatchLine> {
  screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Event(ChatEvent::Watch { line }) => Some(line),
      _ => None,
    })
    .collect()
}

/// The id of the first card of `chat`'s /watch line.
pub(super) fn card(screen: &Orchestrator, chat: ChatId) -> String {
  let seq = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::Watch { .. })).then_some(entry.seq))
    .expect("the /watch line");
  format!("orchestrator-led-wait-{seq}-0")
}

#[test]
fn orchestrator_led_card_takes_it_here_for_a_person_only() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let (theirs, mine, billing) = led(&screen);
  screen.go(OrchestratorChat::Chat(mine.id()));
  let card = card(&screen, mine.id());
  let take = format!("{card}-take");
  screen.until("the card", |screen| screen.exists(&take));
  assert_eq!(
    screen.text(&format!("{card}-title")),
    "Billing is already led in «Billing and invoices»"
  );
  assert!(
    screen.text(&format!("{card}-note")).starts_with("Since "),
    "since when it is led there"
  );

  with_synthetic_input(|| screen.click(&take));
  screen.until("the refusal", |screen| screen.exists(&format!("{card}-person-only")));
  assert_eq!(
    leader(&screen, billing),
    Some(theirs.id()),
    "automation's press moves nothing"
  );

  screen.click(&take);
  let words = format!("{card}-words");
  screen.until("the line in the card's place", |screen| screen.exists(&words));
  assert_eq!(leader(&screen, billing), Some(mine.id()), "the subscription moved here");
  assert_eq!(screen.text(&words), "Billing is now led here");
  assert!(!screen.exists(&take), "the card settled");
  assert!(
    matches!(
      lines(&screen, theirs.id()).last(),
      Some(WatchLine::NowLedIn { taken_over: true, .. })
    ),
    "that conversation got its line"
  );
}

#[test]
fn orchestrator_led_card_opens_that_conversation_and_moves_nothing() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let (theirs, mine, billing) = led(&screen);
  screen.go(OrchestratorChat::Chat(mine.id()));
  let card = card(&screen, mine.id());
  let open = format!("{card}-open");
  screen.until("the card", |screen| screen.exists(&open));

  with_synthetic_input(|| screen.click(&open));
  screen.until("the refusal", |screen| screen.exists(&format!("{card}-person-only")));
  assert_eq!(
    screen.route(),
    Route::Project(
      screen.project,
      ProjectPage::Orchestrator(OrchestratorChat::Chat(mine.id()))
    ),
    "automation's press opens nothing"
  );

  screen.click(&open);
  assert_eq!(
    screen.route(),
    Route::Project(
      screen.project,
      ProjectPage::Orchestrator(OrchestratorChat::Chat(theirs.id()))
    ),
    "that conversation opens"
  );
  screen.until("the choice kept", |screen| {
    matches!(lines(screen, mine.id()).last(), Some(WatchLine::LeftThere { .. }))
  });
  assert_eq!(leader(&screen, billing), Some(theirs.id()), "nothing moved");

  screen.go(OrchestratorChat::Chat(mine.id()));
  let words = format!("{card}-words");
  screen.until("the line in the card's place", |screen| screen.exists(&words));
  assert_eq!(screen.text(&words), "Billing stays led in «Billing and invoices»");
  assert!(!screen.exists(&open), "the card settled");
  assert!(
    screen.exists(&format!("{card}-there")),
    "the line opens that conversation"
  );
}
