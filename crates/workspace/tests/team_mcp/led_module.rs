//! A conversation that leads a module of a team project (`docs/plan/orchestrator-autonomy/OA-28-…`): the module is
//! read whole from the team server, so a task the server holds in it that never ran on this computer is among what
//! the conversation leads. It counts in where the module stands (`get_standing`), with the server's Blocked mark as
//! *parked by the person*; `/watching` and `get_watches` answer for the project; and the module's watch does not end
//! while such a task is open. Once its every task is done, work merged into its branch here waits, as a local module's,
//! for its acceptance (OA-32) and then its delivery, which the conversation asks of the team server and an Admin or
//! the team's automatic merge decides.

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, TaskId},
  integration::{MergeKind, RunMerge},
  module::delivery::{DeliveryStep, PersonsDelivery},
  orchestrator::{Ceilings, Chat, Subscription, TurnState, WatchMode, WatchTarget, standing::TargetStanding},
  revision::Revision,
  run::BranchName,
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::orchestrator::{
  leaving::{AppSet, Weak, leaving_of},
  standing::{Led, standing_in},
  watching::WatchReads,
};
use serde_json::json;

use crate::{
  Checkout, git,
  team_runs::{Started, started},
  team_server::MODULE_BRANCH,
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

#[test]
fn team_mcp_a_led_module_merges_into_its_own_branch_whatever_the_checkout_holds() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let target = module_watch(&checkout, server.module);
  let chat = leading(&checkout, target);
  std::fs::write(checkout.root().join("notes.txt"), "not committed").expect("a file of the person's");

  let app = AppSet {
    drives: true,
    keeps_awake: true,
  };
  let told = |chat, target| -> Vec<&'static str> {
    let left = leaving_of((checkout.root(), &Programs::processes()), (chat, target), app)
      .expect("read")
      .expect("it leads the target");
    left.weak.iter().map(Weak::name).collect()
  };
  let module = told(chat, target);
  assert!(
    !module.contains(&"checkout_has_changes"),
    "a team module's work never merges into the checked-out branch: {module:?}"
  );
  // A team task in no module merges into the branch the checkout has, as a local one does.
  let rest = leading(&checkout, WatchTarget::WithoutModule);
  let loose = told(rest, WatchTarget::WithoutModule);
  assert!(loose.contains(&"checkout_has_changes"), "{loose:?}");
}

#[test]
fn team_mcp_a_led_module_whose_work_is_merged_waits_for_its_delivery_until_the_team_delivers_it() {
  let Started {
    checkout,
    server,
    run,
    seen,
  } = started();
  let target = module_watch(&checkout, server.module);
  leading(&checkout, target);
  let module = json!({"project": "Kontur", "module": "Webhooks"});
  seen.lock().task_fields = json!({ "workflow": "done" });
  refreshed(&checkout);
  assert_eq!(
    stands(&checkout).standing.target,
    TargetStanding::Done,
    "nothing was merged into the module's branch here"
  );

  // The run's work reached the module's branch on this computer: the module is done once that branch is delivered.
  let mut db = checkout.db();
  let mut ended = db.run(run).expect("read").expect("the run");
  let events = ended.cancel(Timestamp::now()).expect("canceled");
  db.append_events(&ended, &events).expect("recorded");
  db.save_run_merge(&RunMerge {
    run,
    into: BranchName::new(MODULE_BRANCH).expect("a branch"),
    revision: Revision::new(&git(checkout.root(), &["rev-parse", MODULE_BRANCH])).expect("a revision"),
    kind: MergeKind::FastForward,
    merged_at: Timestamp::now(),
  })
  .expect("the merge");
  drop(db);
  // As a local module: nothing looked at the whole of it without an acceptance pipeline, so its delivery is the
  // person's.
  assert_eq!(
    stands(&checkout).standing.target,
    TargetStanding::ToDeliver(DeliveryStep::Person(PersonsDelivery::NoAcceptance))
  );
  let choose = json!({"project": "Kontur", "module": "Webhooks", "acceptance_pipeline": "delivery",
    "deliver_when_accepted": true});
  checkout.call("update_module", &choose).expect("kept here");
  assert_eq!(
    stands(&checkout).standing.target,
    TargetStanding::ToDeliver(DeliveryStep::Accept)
  );

  // Its acceptance, a task the server holds in the module, is still to run: the module is not done.
  let acceptance = TaskId::generate();
  checkout
    .db()
    .keep_module_acceptance(server.module, acceptance, Timestamp::now())
    .expect("kept");
  let listed = |workflow: &str| json!({"id": acceptance, "number": 8, "key": "KON-8", "workflow": workflow});
  seen.lock().more_tasks = vec![listed("todo")];
  refreshed(&checkout);
  let target = stands(&checkout).standing.target;
  assert!(!matches!(target, TargetStanding::ToDeliver(_)), "{target:?}");

  // Done on the server, it passed: the delivery is due.
  seen.lock().more_tasks = vec![listed("done")];
  refreshed(&checkout);
  assert_eq!(
    stands(&checkout).standing.target,
    TargetStanding::ToDeliver(DeliveryStep::Deliver)
  );

  // Asked for, the team's server keeps it waiting for an Admin: that is the person's, and nothing is asked again.
  assert_eq!(checkout.refused("deliver_module", &module), "delivery_awaiting_admin");
  assert_eq!(
    stands(&checkout).standing.target,
    TargetStanding::ToDeliver(DeliveryStep::Person(PersonsDelivery::TeamDecides))
  );

  // The team's automatic merge lets the next request through: delivered, the module is done.
  server.state().automatic_merge = true;
  checkout.call("deliver_module", &module).expect("delivered");
  assert_eq!(stands(&checkout).standing.target, TargetStanding::Done);
}
