//! The context meter in the composer's bar (ORC-308): unknown until the session's CLI reports a reading, then plenty,
//! near full and full as each turn's reading arrives, and its popover with the model and the tokens used of the window,
//! following the next turn while it is open.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{connection::context_use::ContextUse, id::ChatId, time::Timestamp};

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

const METER: &str = "orchestrator-context";

/// Keeps `reading` as `chat`'s session's and tells the chat a turn changed it, as the runner does.
fn turn(screen: &Orchestrator, chat: ChatId, reading: &ContextUse) {
  screen.db().set_chat_context(chat, reading).expect("kept");
  let services = screen.window.launch.services().expect("the session started");
  services.runners.chat_changed(&screen.root);
}

#[test]
fn orchestrator_context_meter_follows_each_turn() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let chat = kept(&screen, "What runs?", Timestamp::now(), "Nothing yet.");
  screen.go(OrchestratorChat::Chat(chat));
  screen.until("the meter", |screen| screen.exists(METER));
  assert!(screen.exists(&format!("{METER}-ring")), "a ring");
  assert!(
    !screen.exists(&format!("{METER}-share")),
    "unknown until the CLI reports a reading"
  );
  let turns = [
    (ContextUse::reported(76_000, 200_000), "38%"),
    (ContextUse::reported(172_000, 200_000), "86%"),
    (ContextUse::reported(200_000, 200_000), "Full"),
  ];
  for (reading, share) in turns {
    turn(&screen, chat, &reading);
    screen.until(&format!("the meter at {share}"), |screen| {
      screen.text(&format!("{METER}-share")) == share
    });
  }
  screen.press(METER);
  screen.until("the popover", |screen| screen.exists(&format!("{METER}-popover")));
  assert_eq!(screen.text(&format!("{METER}-popover-model")), "haiku");
  assert_eq!(screen.text(&format!("{METER}-popover-badge-label")), "Full");
  assert_eq!(screen.text(&format!("{METER}-popover-tokens")), "200k of 200k tokens");
  turn(&screen, chat, &ContextUse::new(108_000, 128_000, true));
  screen.until("the open popover follows the next turn", |screen| {
    screen.text(&format!("{METER}-popover-tokens")) == "≈ 108k of 128k tokens"
  });
  assert!(
    screen.exists(&format!("{METER}-popover-note")),
    "an estimate says it is one"
  );
}
