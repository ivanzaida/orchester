//! The chat's start and its `watch` of a team task nothing on this computer defines: the team server holds it, it
//! never ran here and no conversation watches it or its module, so neither a run's record nor the server's last
//! reading names it (a task the chat made a moment ago stands so). Both ask the server for it: the start goes on to
//! `work.start`, and the watch is kept and read with the next reading.

use orchester_core::{
  connection::AgentProvider,
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, WatchTarget},
  time::Timestamp,
};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{ToolError, Translate},
  orchestrator::{
    team_status::TeamReading,
    tools::{self, ChatScope},
  },
  runner::Runners,
};
use serde_json::{Value, json};

use crate::{Checkout, team_cache, tools::server, watch_ends::refreshed};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A conversation of the project's chat in a turn the person's message started.
fn asked(checkout: &Checkout) -> ChatScope {
  let mut db = checkout.db();
  let chat = Chat::named(ChatId::generate(), "Start it and watch", at(10));
  let text = MessageBody::new("Start KON-7 and watch it").expect("text");
  db.keep_chat_message(&chat, (ChatMessageId::generate(), &text), at(10))
    .expect("kept");
  ChatScope {
    project: db.project().id(),
    name: "Kontur".to_owned(),
    root: checkout.root().to_path_buf(),
    chat: chat.id(),
    on_its_own: false,
  }
}

/// The chat's own tool `name`, called with `args` in `scope`.
fn chat_call(checkout: &Checkout, scope: &ChatScope, name: &str, args: &Value) -> Result<Value, ToolError> {
  team_cache::keep_cached_team(&checkout.workspace, checkout.root());
  let runners = Runners::new(
    checkout.root().join("no-runner"),
    checkout.runtime.handle().clone(),
    Activity::new(),
    orchester_ipc::SessionEnd::Clean,
  );
  let translate = Translate::new(|message| message.key.to_owned());
  let Value::Object(args) = args.clone() else {
    panic!("arguments are an object");
  };
  checkout.runtime.block_on(tools::call(
    (&checkout.services, &translate, &runners),
    scope,
    name,
    args,
  ))
}

#[test]
fn team_mcp_the_chat_starts_a_team_task_that_never_ran_here_and_nothing_watches() {
  let (server, _seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    crate::ready::team_ready(db, &config, AgentProvider::CodexCli);
  });
  checkout.db().set_orchestrator_drives(true).expect("drives");
  let scope = asked(&checkout);
  assert!(
    checkout.db().team_task_ids().expect("read").is_empty(),
    "nothing of the task is on this computer"
  );
  assert_eq!(
    TeamReading::of(checkout.root()),
    TeamReading::default(),
    "nothing is watched"
  );

  // The start reaches the team server as the app's own `run_task` does: with no runner here it is accepted, kept and
  // waits for the runner.
  let refused = chat_call(&checkout, &scope, "run_task", &json!({"key": "KON-7"})).expect_err("no runner here");
  assert_eq!(refused.code(), "runner_unavailable", "{refused:?}");
  assert_eq!(server.state().works_made, 1, "work.start made the task's work");

  // By its server id it is found the same way.
  let by_id = chat_call(&checkout, &scope, "run_task", &json!({"key": server.task.to_string()}));
  assert_ne!(by_id.expect_err("no runner here").code(), "task_not_found");
}

#[test]
fn team_mcp_the_chat_starts_no_team_task_the_server_holds_blocked() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  checkout.db().set_orchestrator_drives(true).expect("drives");
  let scope = asked(&checkout);
  seen.lock().task_fields = json!({ "workflow": "todo", "blocked_reason": "Waits for the vendor" });

  let refused = chat_call(&checkout, &scope, "run_task", &json!({"key": "KON-7"})).expect_err("blocked");

  assert_eq!(refused.code(), "task_blocked", "{refused:?}");
  assert_eq!(server.state().works_made, 0, "nothing started");
}

#[test]
fn team_mcp_the_chat_watches_a_team_task_that_never_ran_here() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let scope = asked(&checkout);

  let told = chat_call(&checkout, &scope, "watch", &json!({"targets": ["kon-7"]})).expect("watched");

  assert_eq!(told["watch"]["started"], json!(["KON-7"]), "{told}");
  assert_eq!(told["watch"]["gone"], json!([]), "{told}");
  // The server's task is known here by its identity, so the subscription is read with the next reading.
  assert_eq!(checkout.db().team_task_ids().expect("read"), [server.task]);
  let led = checkout.db().active_subscriptions().expect("subscriptions");
  assert_eq!(led.leading(server.task, None), Some(scope.chat));
  assert_eq!(led.as_slice()[0].target(), WatchTarget::Task(server.task));
  assert!(refreshed(&checkout).polling, "the watch needs the server's answer");
  assert_ne!(
    TeamReading::of(checkout.root()),
    TeamReading::default(),
    "the reading saw it"
  );

  // Asked again, it is here already; a name the server has no task for is still nothing of the project.
  let again = chat_call(&checkout, &scope, "watch", &json!({"targets": ["KON-7"]})).expect("answered");
  assert_eq!(again["watch"]["already_here"], json!(["KON-7"]), "{again}");
  let unknown = chat_call(&checkout, &scope, "watch", &json!({"targets": ["KON-99"]})).expect_err("unknown");
  assert_eq!(unknown.code(), "target_not_found", "{unknown:?}");
}
