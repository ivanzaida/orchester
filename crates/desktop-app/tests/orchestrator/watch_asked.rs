//! A watch and a stop the person asked the Orchestrator for in words (ORC-893): its `watch` and `unwatch` tools apply
//! them through `watch_asked_in` and `stop_in` with `StopBy::Request`, as these tests do, and the conversation shows
//! the usual subscription and stop lines with the quiet *at your request* after their words, and no command bubble
//! of the person's; a target another conversation leads shows the already-led card, whose *Take it here* stays the
//! person's and gives the subscription the order their words gave.
//! `cargo test -p orchester-app --test orchestrator watch_asked`.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Chat, ChatEvent, EntryKind, TurnState, WatchTarget, watch::NamedTarget},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::watch::{StopBy, StopWhat, stop_in, watch_asked_in, watch_in};

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

/// Module `name`, as a target named as people read it.
fn module(db: &mut ProjectDb, name: &str) -> NamedTarget {
  let details =
    ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open).expect("details");
  NamedTarget {
    target: WatchTarget::Module(db.create_module(details, Timestamp::now()).expect("module").id()),
    name: Some(name.to_owned()),
  }
}

/// The seqs of `chat`'s entries, in order.
fn seqs(screen: &Orchestrator, chat: ChatId) -> Vec<i64> {
  screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .map(|entry| entry.seq)
    .collect()
}

/// Who leads `target`, and with what order.
fn leader(screen: &Orchestrator, target: WatchTarget) -> Option<(ChatId, String)> {
  screen
    .db()
    .active_subscriptions()
    .expect("subscriptions")
    .of(target)
    .map(|led| (led.chat(), led.order().unwrap_or_default().to_owned()))
}

#[test]
fn orchestrator_watch_asked_lines_say_at_your_request_with_no_command_of_the_persons() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let mut db = screen.db();
  let docs = module(&mut db, "Docs");
  let mine = conversation(&mut db, "Follow the docs work");
  drop(db);
  watch_asked_in(
    &screen.root,
    &mine,
    std::slice::from_ref(&docs),
    "Tell me when it merges.",
  )
  .expect("watched");
  stop_in(&screen.root, &mine, &StopWhat::Targets(vec![docs]), StopBy::Request).expect("stopped");
  let commands = screen
    .db()
    .chat_entries(mine.id())
    .expect("entries")
    .into_iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::WatchCommand { .. })))
    .count();
  assert_eq!(commands, 0, "no command of the person's: they asked in words");
  let &[.., leading, stopped] = seqs(&screen, mine.id()).as_slice() else {
    panic!("the subscription line and the stop line");
  };
  let (leading, stopped) = (
    format!("orchestrator-watch-wait-{leading}"),
    format!("orchestrator-watch-wait-{stopped}"),
  );

  screen.go(OrchestratorChat::Chat(mine.id()));
  screen.until("the subscription line", |screen| {
    screen.exists(&format!("{leading}-words"))
  });
  assert!(screen.text(&format!("{leading}-words")).starts_with("Leading Docs"));
  assert_eq!(screen.text(&format!("{leading}-tag")), "at your request");
  assert_eq!(screen.text(&format!("{stopped}-line-words")), "Stopped watching Docs");
  assert_eq!(screen.text(&format!("{stopped}-line-tag")), "at your request");
  assert_eq!(
    screen.text(&format!("{stopped}-line-aside")),
    "— nothing is canceled or paused"
  );
}

#[test]
fn orchestrator_watch_asked_of_a_target_led_elsewhere_shows_the_card_for_the_person() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let mut db = screen.db();
  let billing = module(&mut db, "Billing");
  let theirs = conversation(&mut db, "Billing and invoices");
  let mine = conversation(&mut db, "Webhooks v2");
  drop(db);
  let named = std::slice::from_ref(&billing);
  watch_in(&screen.root, &theirs, named, "").expect("theirs watches it");
  let watched = watch_asked_in(&screen.root, &mine, named, "Merge when green.").expect("applied");
  assert_eq!(watched.led_elsewhere.len(), 1, "not taken: the card's");
  let line = *seqs(&screen, mine.id()).last().expect("the line");
  let card = format!("orchestrator-led-wait-{line}-0");

  screen.go(OrchestratorChat::Chat(mine.id()));
  let take = format!("{card}-take");
  screen.until("the card", |screen| screen.exists(&take));
  assert_eq!(
    screen.text(&format!("{card}-title")),
    "Billing is already led in «Billing and invoices»"
  );
  assert_eq!(leader(&screen, billing.target).map(|(chat, _)| chat), Some(theirs.id()));

  screen.click(&take);
  screen.until("the line in the card's place", |screen| {
    screen.exists(&format!("{card}-words"))
  });
  assert_eq!(
    leader(&screen, billing.target),
    Some((mine.id(), "Merge when green.".to_owned())),
    "the person took it, with the order their words gave"
  );
}
