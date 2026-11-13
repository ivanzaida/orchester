//! The ceilings of work a conversation leads to the end (`docs/plan/orchestrator-autonomy/OA-09-…`), on the seeded
//! Kontur's run that waits for a decision after its retries: in a turn Orchester started by itself the chat's
//! `continue_run`, and a message that would continue the run, are refused `ceiling_reached` once the run had the
//! returns its subscription allows; a task that is only followed, and a turn a person's message started, are not
//! bound.

use orchester_core::{
  orchestrator::{Ceilings, Subscription, WatchMode, WatchTarget},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::tools::ChatScope;
use serde_json::{Value, json};

use super::{
  Served,
  orchestrator_chat::{allow, chat, chat_call, task_run},
};

/// The seeded task whose run waits for a decision after Review used up its retries.
const STUCK: &str = "Cache GitHub API responses";

fn retry(run: &str) -> Value {
  json!({ "project": "Kontur", "run": run, "choice": { "kind": "allow_retry" } })
}

/// `scope`'s conversation takes `target` as `mode` says, from now on.
fn watch(db: &mut ProjectDb, scope: &ChatScope, target: WatchTarget, mode: WatchMode) {
  let subscription = Subscription::new(scope.chat, target, "", Timestamp::now()).in_mode(mode);
  db.take_over_subscription(&subscription)
    .expect("the person watches it here");
}

#[test]
fn ceilings_refuse_a_return_past_what_the_leading_subscription_allows_in_a_turn_nobody_asked_for() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, STUCK);
  allow(&served, &seeded.repo);
  let asked = chat(&served, &mut db);
  let on_its_own = ChatScope {
    on_its_own: true,
    ..asked.clone()
  };
  let number = run.number().get().to_string();
  // Led to the end with no return of the chat's own allowed.
  let none = Ceilings::new((1, 0), (0, 50), None).expect("ceilings");
  watch(
    &mut db,
    &asked,
    WatchTarget::Task(task.id()),
    WatchMode::Lead { ceilings: none },
  );
  let refused =
    chat_call(&mut served, &on_its_own, "continue_run", retry(&number)).expect_err("past the ceiling of returns");
  assert_eq!(refused.code(), "ceiling_reached", "{refused:?}");
  // A message to a run that waits for a decision continues it: the same return by another door.
  let message = json!({ "project": "Kontur", "run": number, "text": "Try the review once more." });
  let refused =
    chat_call(&mut served, &on_its_own, "send_message", message).expect_err("a message that continues the run");
  assert_eq!(refused.code(), "ceiling_reached", "{refused:?}");

  // The person asked for it: the ceilings bind only what nobody asked for. It goes to the runner, which this session
  // has none of.
  let refused = chat_call(&mut served, &asked, "continue_run", retry(&number)).expect_err("no runner here");
  assert_ne!(refused.code(), "ceiling_reached", "{refused:?}");

  // Only followed (the person stopped the leading and watches it again): nothing bounds the chat but the drive
  // switch.
  db.stop_subscription(asked.chat, WatchTarget::Task(task.id()), Timestamp::now())
    .expect("stopped");
  watch(&mut db, &asked, WatchTarget::Task(task.id()), WatchMode::Follow);
  let refused = chat_call(&mut served, &on_its_own, "continue_run", retry(&number)).expect_err("no runner here");
  assert_ne!(refused.code(), "ceiling_reached", "{refused:?}");
}
