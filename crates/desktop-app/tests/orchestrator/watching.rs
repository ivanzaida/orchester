//! `/watching` (ORC-775, `06 · /watching` `Ff7AD`): *What I watch* runs while the conversation leads something; it
//! keeps the person's command with no agent turn, and its row shows the command's chip and Orchester's answer from the
//! live state, grouped per subscription, whose *Stop watching* ends one at once.
//! `cargo test -p orchester-app --test orchestrator watching`.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::ChatId,
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat, ChatEvent, EntryKind, WatchTarget,
    watch::{NamedTarget, WatchCommand},
  },
  time::Timestamp,
};
use orchester_workspace::orchestrator::watch::watch_in;

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

const INPUT: &str = "orchestrator-input";
const WATCHING: &str = "orchestrator-composer-slash-watching";

/// The Orchestrator on a conversation that watches the module *Webhooks v2* with an order of its own.
fn watching_a_module() -> (Orchestrator, ChatId) {
  let mut screen = Orchestrator::new(Setup::Ready);
  let chat = kept(&screen, "Webhooks work", Timestamp::now(), "Noted.");
  let details = ModuleDetails::new(
    Name::new("Webhooks v2").expect("name"),
    "",
    (None, None),
    ModuleState::Open,
  )
  .expect("details");
  let module = screen
    .db()
    .create_module(details, Timestamp::now())
    .expect("module")
    .id();
  let target = NamedTarget {
    target: WatchTarget::Module(module),
    name: Some("Webhooks v2".to_owned()),
  };
  watch_in(
    &screen.root,
    &Chat::named(chat, "", Timestamp::now()),
    &[target],
    "Answer agents from the brief",
  )
  .expect("watched");
  screen.go(OrchestratorChat::Latest);
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Noted.")
  });
  (screen, chat)
}

/// The kept entries of `chat`.
fn entries(screen: &Orchestrator, chat: ChatId) -> Vec<EntryKind> {
  screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .map(|entry| entry.kind)
    .collect()
}

#[test]
fn orchestrator_watching_answers_from_the_live_state_and_stops_watching_from_it() {
  let (mut screen, chat) = watching_a_module();
  let before = entries(&screen, chat).len();
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  // The menu filters by a command's label, *What I watch*.
  screen.set(INPUT, "/what");
  screen.until("What I watch to run", |screen| {
    screen.exists(WATCHING) && screen.text(WATCHING).contains("Enter")
  });
  assert!(
    screen.text(WATCHING).contains("What this conversation leads"),
    "{}",
    screen.text(WATCHING)
  );
  screen.key(("Enter", "Enter"), false);
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("This conversation leads 1 module. Nothing waits for you.")
  });

  let kept = entries(&screen, chat);
  assert_eq!(kept.len(), before + 1, "the command alone: no turn of its own");
  assert_eq!(
    kept.last(),
    Some(&EntryKind::Event(ChatEvent::WatchCommand {
      command: WatchCommand::Watching,
    }))
  );
  let seq = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .last()
    .expect("the command")
    .seq;
  let row = format!("orchestrator-watching-wait-{seq}");
  let shown = screen.text(&row);
  for said in [
    "/watching",
    "Applied",
    "Orchester",
    "the live state, no agent turn",
    "Webhooks v2",
    "module · since",
    "“Answer agents from the brief”",
    "Stop watching",
  ] {
    assert!(shown.contains(said), "{said}: {shown}");
  }

  screen.click(&format!("{row}-answer-subscription-0-stop"));
  screen.until("the subscription to end", |screen| {
    screen
      .db()
      .active_subscriptions()
      .expect("subscriptions")
      .of_chat(chat)
      .next()
      .is_none()
  });
  screen.until("the answer to follow the live state", |screen| {
    screen.text(&row).contains("This conversation watches nothing yet.")
  });
}
