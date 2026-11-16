//! A conversation that leads a module of a team project (`docs/plan/orchestrator-autonomy/OA-28-…`): the module is
//! read whole from the team server, so a task the server holds in it that never ran on this computer is among what
//! the conversation leads. It counts in where the module stands (`get_standing`), with the server's Blocked mark as
//! *parked by the person*; `/watching` and `get_watches` answer for the project; and the module's watch does not end
//! while such a task is open.

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Ceilings, Chat, Subscription, TurnState, WatchMode, WatchTarget, standing::TargetStanding},
  time::Timestamp,
};
use orchester_workspace::orchestrator::{
  standing::{Led, standing_in},
  watching::WatchReads,
};
use serde_json::json;

use crate::{
  Checkout,
  tools::server,
  watch_ends::{module_watch, refreshed},
};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A conversation of the project that leads `target` to the end.
fn leading(checkout: &Checkout, target: WatchTarget) -> ChatId {
  let mut db = checkout.db();
  let chat = Chat::named(ChatId::generate(), "Leads Webhooks", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Lead it to the end").expect("text");
  db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  db.start_subscription(&Subscription::new(chat.id(), target, "", at(13)).in_mode(lead))
    .expect("leading");
  chat.id()
}

/// Where the one subscription of the project stands.
fn stands(checkout: &Checkout) -> Led {
  let mut all = standing_in(checkout.root()).expect("standing");
  assert_eq!(all.len(), 1, "{all:?}");
  all.remove(0)
}

fn named(led: &Led) -> Vec<(&str, &str)> {
  led
    .standing
    .tasks
    .iter()
    .map(|(task, standing)| (task.as_str(), standing.name()))
    .collect()
}

#[test]
fn team_mcp_a_led_module_has_the_team_task_that_never_ran_here_to_start() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let target = module_watch(&checkout, server.module);
  let chat = leading(&checkout, target);
  assert!(
    checkout.db().team_task_ids().expect("read").is_empty(),
    "no team task ran on this computer"
  );

  // The module is read from the server though nothing of it ran here.
  assert!(refreshed(&checkout).polling);
  let led = stands(&checkout);
  assert_eq!(led.name.as_deref(), Some("Webhooks"));
  assert_eq!(named(&led), [("KON-7", "ready_to_start")]);
  assert_eq!(
    led.standing.target,
    TargetStanding::CanStart {
      tasks: vec!["KON-7".to_owned()],
      slot_free: true
    }
  );

  // `/watching` counts it among what the conversation leads, and `get_watches` answers for a team project.
  let reads = WatchReads::read(&checkout.db(), checkout.root()).expect("reads");
  let listing = reads.watching(chat);
  assert_eq!(listing.subscriptions.len(), 1, "{listing:?}");
  assert!(
    listing.subscriptions[0].rest.is_some(),
    "the task with nothing going on is counted: {listing:?}"
  );
  let watches = checkout
    .call("get_watches", &json!({"project": "Kontur"}))
    .expect("answered");
  assert_eq!(watches["conversations"].as_array().map(Vec::len), Some(1), "{watches}");

  // The server holds the task Blocked: it is the person's, and the module is stuck.
  seen.lock().task_fields = json!({ "workflow": "todo", "blocked_reason": "Waits for the vendor" });
  refreshed(&checkout);
  let led = stands(&checkout);
  assert_eq!(named(&led), [("KON-7", "parked_by_person")]);
  assert_eq!(led.standing.target, TargetStanding::Stuck { budget_used: false });

  // Done on the server, the module is done.
  seen.lock().task_fields = json!({ "workflow": "done" });
  refreshed(&checkout);
  assert_eq!(stands(&checkout).standing.target, TargetStanding::Done);
}
