//! Choosing to lead a target to the end until a window offers it (OA-10): the person's own /watch whose order begins
//! with *lead to the end* has its targets led, within the default ceilings, with what follows as the order; a target
//! the conversation only followed is led from then on; and the watch the agent applies at the person's request never
//! chooses it.
//! `cargo test -p orchester-workspace --test orchestrator_watch_lead watch_lead`.

mod support;

use orchester_core::{
  id::ChatId,
  orchestrator::{
    Ceilings, Chat, ChatEvent, EntryKind, WatchMode, WatchTarget,
    watch::{NamedTarget, WatchCommand},
  },
};
use orchester_workspace::orchestrator::watch::{watch_asked_in, watch_in};
use support::{Fixture, at};

fn everything() -> NamedTarget {
  NamedTarget {
    target: WatchTarget::Project,
    name: None,
  }
}

/// How `chat` takes the whole project, and with which order.
fn taken(fixture: &Fixture, chat: ChatId) -> (WatchMode, Option<String>) {
  let subscriptions = fixture.db.chat_subscriptions(chat).expect("subscriptions");
  assert_eq!(subscriptions.len(), 1, "{subscriptions:?}");
  (subscriptions[0].mode(), subscriptions[0].order().map(str::to_owned))
}

fn leading() -> WatchMode {
  WatchMode::Lead {
    ceilings: Ceilings::default(),
  }
}

#[test]
fn watch_lead_is_chosen_by_the_first_words_of_the_persons_own_order() {
  let fixture = Fixture::new();
  let chat = Chat::named(ChatId::generate(), "Leading", at(10));
  let order = "Lead to the end: merge once the review passed.";
  watch_in(fixture.repo.path(), &chat, &[everything()], order).expect("watched");
  assert_eq!(
    taken(&fixture, chat.id()),
    (leading(), Some("merge once the review passed.".to_owned())),
    "what follows the words is the order"
  );
  // The command is kept as the person wrote it.
  let kept = fixture.db.chat_entries(chat.id()).expect("entries");
  let command = kept.iter().find_map(|entry| match &entry.kind {
    EntryKind::Event(ChatEvent::WatchCommand {
      command: WatchCommand::Watch { order, .. },
    }) => Some(order.clone()),
    _ => None,
  });
  assert_eq!(command.as_deref(), Some(order));
}

#[test]
fn watch_lead_takes_over_a_target_the_conversation_only_followed() {
  let fixture = Fixture::new();
  let chat = Chat::named(ChatId::generate(), "Following", at(10));
  watch_in(fixture.repo.path(), &chat, &[everything()], "Ask before merging.").expect("watched");
  assert_eq!(
    taken(&fixture, chat.id()),
    (WatchMode::Follow, Some("Ask before merging.".to_owned()))
  );
  // An order that only starts alike is an order.
  watch_in(fixture.repo.path(), &chat, &[everything()], "Lead to the endgame of Q4").expect("watched");
  assert_eq!(taken(&fixture, chat.id()).0, WatchMode::Follow);

  watch_in(fixture.repo.path(), &chat, &[everything()], "lead to the end").expect("watched");
  assert_eq!(
    taken(&fixture, chat.id()),
    (leading(), None),
    "led from now on, by the default order"
  );
}

#[test]
fn watch_lead_is_never_chosen_by_the_watch_the_agent_applies() {
  let fixture = Fixture::new();
  let chat = Chat::named(ChatId::generate(), "Asked in words", at(10));
  fixture.db.chat(chat.id()).expect("read");
  watch_asked_in(
    fixture.repo.path(),
    &chat,
    &[everything()],
    "lead to the end: merge everything",
  )
  .expect("watched");
  assert_eq!(
    taken(&fixture, chat.id()),
    (WatchMode::Follow, Some("lead to the end: merge everything".to_owned())),
    "the agent's watch follows, and its order is kept whole"
  );
}
