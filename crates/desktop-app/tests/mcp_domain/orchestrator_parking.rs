//! Parking a task for the person (`docs/plan/orchestrator-autonomy/OA-15-…`), on the seeded Kontur's run that waits
//! for a decision: in a turn nobody asked for the chat parks only a task its conversation leads; a parked task reads
//! so in `get_standing` with what the person is to do, is started by nobody, and its parking is a decision the person
//! reviews; given back, it is the chat's again.
//! `cargo test -p orchester-app --test mcp_domain orchestrator_parking`.

use orchester_core::{
  orchestrator::{Ceilings, Subscription, WatchMode, WatchTarget, decision::Undo, parking::Needs},
  time::Timestamp,
};
use orchester_store::{DecisionsRead, ProjectDb};
use orchester_workspace::orchestrator::{parking::give_back_in, tools::ChatScope};
use serde_json::{Value, json};

use super::{
  Served,
  orchestrator_chat::{allow, chat, chat_call, task_run},
};

/// The seeded task whose run waits for a decision after Review used up its retries.
const STUCK: &str = "Cache GitHub API responses";

fn park(task: &str) -> Value {
  json!({
    "project": "Kontur",
    "task": task,
    "needs": "decision",
    "why": "The review asks which cache to drop, and the brief does not say.",
    "to_do": "Say in the task's conversation which cache goes.",
  })
}

#[test]
fn orchestrator_parking_leaves_a_led_task_to_the_person_until_it_is_given_back() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, STUCK);
  let key = task.key().to_string();
  allow(&served, &seeded.repo);
  let asked = chat(&served, &mut db);
  let own = ChatScope {
    on_its_own: true,
    ..asked.clone()
  };

  // Nobody asked, and the conversation does not lead the task: it is not the chat's to park.
  let refused = chat_call(&mut served, &own, "park_task", park(&key)).expect_err("not led");
  assert_eq!(refused.code(), "not_led", "{refused:?}");
  assert_eq!(db.parked_task(task.id()).expect("read"), None);

  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  let leading = Subscription::new(asked.chat, WatchTarget::Task(task.id()), "", Timestamp::now()).in_mode(lead);
  db.take_over_subscription(&leading)
    .expect("the person has it led to the end");
  let mut wordless = park(&key);
  wordless["to_do"] = json!("   ");
  let refused = chat_call(&mut served, &own, "park_task", wordless).expect_err("says nothing");
  assert_eq!(refused.code(), "invalid_arguments", "{refused:?}");

  let parked = chat_call(&mut served, &own, "park_task", park(&key)).expect("parked");
  assert_eq!(parked["parked"]["task"], key, "{parked}");
  assert_eq!(parked["parked"]["run"], run.number().get(), "{parked}");
  let kept = db.parked_task(task.id()).expect("read").expect("parked");
  assert_eq!(
    (kept.needs, kept.run, kept.by),
    (Needs::Decision, Some(run.id()), asked.chat)
  );
  // Parked on its own: a decision the person reviews, undone by giving the task back.
  let decisions = db.decisions(&DecisionsRead::default()).expect("read");
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].made.tool, "park_task");
  assert_eq!(decisions[0].made.task.as_deref(), Some(key.as_str()));
  assert_eq!(decisions[0].made.undo, Some(Undo::GiveBack));

  let standing = chat_call(&mut served, &asked, "get_standing", json!({ "project": "Kontur" })).expect("read");
  let led = &standing["subscriptions"][0];
  assert_eq!(led["leads_to_the_end"], true, "{standing}");
  assert_eq!(led["standing"], "stuck", "all that is left is parked: {standing}");
  assert_eq!(led["tasks"][0]["standing"], "parked", "{standing}");
  assert_eq!(
    led["tasks"][0]["parked"]["to_do"],
    "Say in the task's conversation which cache goes."
  );

  // Nothing starts a parked task.
  let start = json!({ "project": "Kontur", "key": key });
  let refused = chat_call(&mut served, &own, "run_task", start.clone()).expect_err("parked");
  assert_eq!(refused.code(), "parked", "{refused:?}");

  // Given back, it is the chat's again: a task whose run did not fail is still not run again from here.
  assert!(give_back_in(&seeded.repo, task.id()).expect("given back"));
  let refused = chat_call(&mut served, &own, "run_task", start).expect_err("it has a run");
  assert_eq!(refused.code(), "already_ran", "{refused:?}");
}
