//! The chat's tools that name a task, for a team task nothing on this computer defines: it never ran here and no
//! conversation watches it, so neither a run's record nor the team server's last reading names it. Each asks the team
//! server for it, as the chat's `run_task` and `watch` do ([`super::chat_task`]): `park_task` parks it, the start in a
//! turn Orchester started finds it under what leads it, and `wait_on_task` waits on it.

use orchester_core::{
  connection::AgentProvider,
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, TaskId},
  orchestrator::{Ceilings, Chat, Subscription, TurnState, WatchMode, WatchTarget},
  run::WaitReason,
  time::Timestamp,
};
use orchester_workspace::orchestrator::tools::ChatScope;
use serde_json::json;

use crate::{
  Checkout,
  chat_task::{asked, chat_call},
  team_runs::started,
  tools::server,
};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

#[test]
fn team_mcp_the_chat_parks_a_team_task_that_never_ran_here_and_nothing_watches() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let scope = asked(&checkout);
  let park = json!({
    "task": "KON-7", "needs": "decision", "why": "The vendor has not answered.", "to_do": "Ask the vendor."
  });

  let told = chat_call(&checkout, &scope, "park_task", &park).expect("parked");

  assert_eq!(told["parked"]["task"], "KON-7", "{told}");
  let parking = checkout.db().parked_task(server.task).expect("read");
  assert_eq!(parking.map(|parking| parking.by), Some(scope.chat));
}

/// A conversation that leads the whole project to the end, in a turn Orchester started.
fn leading_the_project(checkout: &Checkout) -> ChatScope {
  let mut db = checkout.db();
  let chat = Chat::named(ChatId::generate(), "Leads Kontur", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Lead it to the end").expect("text");
  db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  db.set_orchestrator_drives(true).expect("drives");
  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  db.start_subscription(&Subscription::new(chat.id(), WatchTarget::Project, "", at(13)).in_mode(lead))
    .expect("leading");
  ChatScope {
    project: db.project().id(),
    name: "Kontur".to_owned(),
    root: checkout.root().to_path_buf(),
    chat: chat.id(),
    on_its_own: true,
  }
}

#[test]
fn team_mcp_the_chat_starts_a_team_task_it_leads_through_the_project_in_a_turn_it_started() {
  let (server, _seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    crate::ready::team_ready(db, &config, AgentProvider::CodexCli);
  });
  let scope = leading_the_project(&checkout);

  // The whole project's subscription reads no team task from the server, and the task never ran here: the start asks
  // the server, finds the task led here, and goes on to `work.start`.
  let refused = chat_call(&checkout, &scope, "run_task", &json!({"key": "KON-7"})).expect_err("no runner here");

  assert_eq!(refused.code(), "runner_unavailable", "{refused:?}");
  assert_eq!(server.state().works_made, 1);
}

#[test]
fn team_mcp_a_team_task_the_chat_creates_comes_back_to_the_conversation_that_watches() {
  let (server, seen) = server();
  seen.lock().takes_creates = true;
  let checkout = crate::created::admin(&server);
  let scope = asked(&checkout);
  checkout
    .db()
    .start_subscription(&Subscription::new(scope.chat, WatchTarget::WithoutModule, "", at(11)))
    .expect("watches");

  let told = chat_call(&checkout, &scope, "create_task", &json!({"title": "Say hello"})).expect("created");

  // The task is in a module no conversation watches: it becomes this conversation's task subscription.
  assert_eq!(told["watch"]["kind"], "watched_here", "{told}");
  let led = checkout.db().active_subscriptions().expect("subscriptions");
  assert_eq!(led.leading(server.task, Some(server.module)), Some(scope.chat));
}

#[test]
fn team_mcp_the_chat_makes_a_team_run_wait_on_a_team_task_that_never_ran_here() {
  let team = started();
  let other = TaskId::generate();
  team.seen.lock().more_tasks = vec![json!({ "id": other, "number": 8, "key": "KON-8", "title": "Rate limiter" })];
  team.step_into(|run, execution, now| run.wait_step(execution, WaitReason::Message, now).expect("waits"));
  let scope = asked(&team.checkout);

  let told = chat_call(
    &team.checkout,
    &scope,
    "wait_on_task",
    &json!({"run": team.run.to_string(), "task": "KON-8"}),
  )
  .expect("waits");

  assert_eq!(told["wait"]["task"], "KON-7", "{told}");
  assert_eq!(told["wait"]["waits_on"], "KON-8", "{told}");
  let kept = team.checkout.db().run_wait(team.run).expect("read").expect("kept");
  assert_eq!(kept.wait.waited(), other);
  // A name the server has no task for is still nothing of the project.
  let unknown = chat_call(
    &team.checkout,
    &scope,
    "wait_on_task",
    &json!({"run": team.run.to_string(), "task": "KON-99"}),
  )
  .expect_err("unknown");
  assert_eq!(unknown.code(), "task_not_found", "{unknown:?}");
}
