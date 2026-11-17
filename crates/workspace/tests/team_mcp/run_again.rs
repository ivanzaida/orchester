//! A message that runs a team task again (`send_message` to its run that ended, OA-29): the message is kept on this
//! computer for the task's next run, which starts through the team server as `run_task` starts it, and the runner gives
//! it to that run's agents. A start refused before the message is kept keeps nothing; the ceilings of a subscription
//! that leads the task to the end bound the chat's message as they bound a local one (OA-09).

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Ceilings, Chat, Subscription, TurnState, WatchMode},
  run::InterruptCause,
  store::{ConversationStore, RunStore},
  time::Timestamp,
};
use orchester_workspace::orchestrator::tools::ChatScope;
use serde_json::json;

use crate::{
  chat_task::chat_call,
  team_runs::{Started, started},
  watch_ends::module_watch,
};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A team task whose one run was canceled, and whose work the team server holds as ended, as the run's report of its
/// end leaves it.
fn canceled() -> Started {
  let team = started();
  team.settle(|run, now| run.cancel(now).expect("canceled"));
  for work in &mut team.server.state().works {
    "cancelled".clone_into(&mut work.state);
  }
  team
}

/// The messages of the run's task that wait for its next run: kept for no run yet.
fn kept_for_next_run(team: &Started) -> usize {
  let db = team.checkout.db();
  let task = db.run(team.run).expect("read").expect("run").task();
  db.messages(task)
    .expect("messages")
    .iter()
    .filter(|message| message.run().is_none())
    .count()
}

#[test]
fn team_mcp_a_message_runs_a_team_task_again_through_the_team_server() {
  let team = canceled();
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Try again, smaller."});

  let told = team.checkout.call("send_message", &message).expect("sent");

  assert_eq!(told["action"], "run_again", "{told}");
  assert_eq!(
    team.server.state().works_made,
    2,
    "the next run's work started through the team server: {told}"
  );
  assert_eq!(
    kept_for_next_run(&team),
    1,
    "the message waits for the next run's agents"
  );
  // This session has no runner: the accepted start waits for it, and the message with it.
  assert_eq!(told["outcome"], "queued", "{told}");
}

#[test]
fn team_mcp_a_message_to_an_interrupted_team_run_without_its_worktree_waits_for_the_tasks_next_run() {
  let team = started();
  team.settle(|run, now| run.interrupt(InterruptCause::RunnerLost, now).expect("interrupted"));
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Carry on."});

  // The fixture's run has no worktree, so the composer runs the task again with the message. The team server still
  // holds the run's work as running and refuses a second one: the message waits for the next run, with why.
  let told = team.checkout.call("send_message", &message).expect("sent");

  assert_eq!(told["action"], "run_again", "{told}");
  assert_eq!(told["outcome"], "queued", "{told}");
  assert_eq!(told["reasons"], json!(["appMcp.errors.teamRefused"]), "{told}");
  assert_eq!(team.server.state().works_made, 1);
  assert_eq!(kept_for_next_run(&team), 1);
}

#[test]
fn team_mcp_a_message_whose_team_start_is_refused_is_not_kept() {
  let team = canceled();
  team.seen.lock().task_fields = json!({ "pipeline": null });
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Try again."});

  assert_eq!(team.checkout.refused("send_message", &message), "task_incomplete");
  assert_eq!(team.messages(), 0, "nothing was kept");
  assert_eq!(
    team.server.state().works_made,
    1,
    "nothing started beyond the first work"
  );
}

/// A conversation that leads the team task's module to the end under `ceilings`, in a turn Orchester started.
fn leading(team: &Started, ceilings: Ceilings) -> ChatScope {
  let mut db = team.checkout.db();
  let chat = Chat::named(ChatId::generate(), "Leads Webhooks", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Lead it to the end").expect("text");
  db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  db.set_orchestrator_drives(true).expect("drives");
  drop(db);
  let target = module_watch(&team.checkout, team.server.module);
  let lead = WatchMode::Lead { ceilings };
  team
    .checkout
    .db()
    .start_subscription(&Subscription::new(chat.id(), target, "", at(13)).in_mode(lead))
    .expect("leading");
  ChatScope {
    project: team.checkout.db().project().id(),
    name: "Kontur".to_owned(),
    root: team.checkout.root().to_path_buf(),
    chat: chat.id(),
    on_its_own: true,
  }
}

#[test]
fn team_mcp_the_ceiling_of_runs_bounds_the_chats_message_that_runs_a_team_task_again() {
  let team = canceled();
  let one_run = Ceilings::new((1, 4), (1, 50), None).expect("ceilings");
  let scope = leading(&team, one_run);
  let message = json!({"run": team.run.to_string(), "text": "Try again."});

  let refused = chat_call(&team.checkout, &scope, "send_message", &message).expect_err("past the ceiling");

  assert_eq!(refused.code(), "ceiling_reached", "{refused:?}");
  assert_eq!(team.messages(), 0);
  assert_eq!(team.server.state().works_made, 1);
}

#[test]
fn team_mcp_the_chat_runs_a_team_task_again_with_a_message_within_its_ceilings() {
  let team = canceled();
  let scope = leading(&team, Ceilings::default());
  let message = json!({"run": team.run.to_string(), "text": "Try again."});

  let told = chat_call(&team.checkout, &scope, "send_message", &message).expect("sent");

  assert_eq!(told["action"], "run_again", "{told}");
  assert_eq!(team.server.state().works_made, 2);
  assert_eq!(kept_for_next_run(&team), 1);
}
