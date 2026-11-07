//! A watch badge sent with a message in a new conversation (ORC-890, P2 `WbHSt` in `Proposal · watch composer v2`
//! `LRg3R`): the watch and the message both keep the conversation they start, and whichever writes it first, it is
//! kept as the send keeps it, with its *Plan only* switch and named after the message.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::ChatId,
  orchestrator::{EntryKind, TurnMode, WatchTarget},
};

use super::{
  harness::Orchestrator,
  watch_module::{BADGE, PICKER, badge_words, focus_field, kontur_with, messages, open_picker, row},
};

const INPUT: &str = "orchestrator-input";
const ORDER: &str = "Ask before merging.";

/// The modes of the person's messages in conversation `chat`, in order.
fn modes(screen: &Orchestrator, chat: ChatId) -> Vec<TurnMode> {
  screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Person { mode, .. } => Some(mode),
      _ => None,
    })
    .collect()
}

#[test]
fn orchestrator_watch_new_chat_with_plan_only_keeps_the_switch_and_the_title() {
  let mut kontur = kontur_with(|_| {});
  let (screen, webhooks) = (&mut kontur.screen, kontur.webhooks);
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-plan-only");
  screen.until("the mode line", |screen| screen.exists("orchestrator-plan-mode"));

  open_picker(screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  screen.key(("Enter", "Enter"), false);
  screen.until("the badge, the picker closed", |screen| {
    !screen.exists(PICKER) && badge_words(screen, BADGE) == "Watch · Webhooks v2"
  });
  focus_field(screen);
  screen.set(INPUT, ORDER);
  screen.key(("Enter", "Enter"), false);

  let led = |screen: &mut Orchestrator| {
    let subscriptions = screen.db().active_subscriptions().expect("read");
    subscriptions.of(WatchTarget::Module(webhooks)).map(|led| led.chat())
  };
  screen.until("the subscription", |screen| led(screen).is_some());
  let chat = led(screen).expect("led");
  assert_ne!(chat, kontur.here, "the new conversation leads it");
  screen.until("the message sent", |screen| {
    messages(screen, chat).iter().any(|text| text == ORDER)
  });
  let kept = screen.db().chat(chat).expect("read").expect("kept");
  assert!(kept.plan_only(), "the conversation keeps the Plan only switch");
  assert_eq!(kept.title(), ORDER, "named after the message");
  assert_eq!(modes(screen, chat), [TurnMode::Plans], "the message plans");
}
