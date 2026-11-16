//! The Orchestrator's own decisions carry their reason (`docs/plan/orchestrator-autonomy/OA-22-…`), on the seeded
//! Kontur: in a turn nobody asked for, a judgement without a reason is refused `reason_required` before anything else
//! is asked, a turn the person's message started needs none, and what the chat did on its own with a reason is kept
//! as a decision `get_decisions` lists.
//! `cargo test -p orchester-app --test mcp_domain orchestrator_decisions`.

use orchester_core::{orchestrator::decision::Review, time::Timestamp};
use orchester_store::{DecisionsRead, ProjectDb};
use orchester_workspace::orchestrator::{decisions::review_in, tools::ChatScope};
use serde_json::{Value, json};

use super::{
  Served,
  orchestrator_chat::{allow, chat, chat_call, task_run},
};

/// The seeded task whose run waits for a decision after Review used up its retries.
const STUCK: &str = "Cache GitHub API responses";

fn retry(run: &str, reason: Option<Value>) -> Value {
  let mut call = json!({ "project": "Kontur", "run": run, "choice": { "kind": "allow_retry" } });
  if let Some(reason) = reason {
    call["reason"] = reason;
  }
  call
}

#[test]
fn orchestrator_decisions_need_a_reason_only_in_a_turn_nobody_asked_for() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, STUCK);
  allow(&served, &seeded.repo);
  let asked = chat(&served, &mut db);
  let own = ChatScope {
    on_its_own: true,
    ..asked.clone()
  };
  let number = run.number().get().to_string();

  let unreasoned = chat_call(&mut served, &own, "continue_run", retry(&number, None)).expect_err("no reason");
  assert_eq!(unreasoned.code(), "reason_required", "{unreasoned:?}");
  let blank = chat_call(&mut served, &own, "continue_run", retry(&number, Some(json!("  ")))).expect_err("blank");
  assert_eq!(blank.code(), "reason_required", "{blank:?}");
  let mistyped = chat_call(&mut served, &own, "continue_run", retry(&number, Some(json!(7)))).expect_err("a number");
  assert_eq!(mistyped.code(), "invalid_arguments", "{mistyped:?}");

  // With a reason the call reaches the tool, which has no runner in this session; the person's own turn needs none.
  let reason = Some(json!("The review failed on a flaky check."));
  let reasoned = chat_call(&mut served, &own, "continue_run", retry(&number, reason)).expect_err("no runner here");
  assert_ne!(reasoned.code(), "reason_required", "{reasoned:?}");
  let persons = chat_call(&mut served, &asked, "continue_run", retry(&number, None)).expect_err("no runner here");
  assert_ne!(persons.code(), "reason_required", "{persons:?}");

  // Nothing was done, so nothing was decided.
  assert!(db.decisions(&DecisionsRead::default()).expect("read").is_empty());
}

#[test]
fn orchestrator_decisions_are_listed_open_first_and_reviewed_by_the_person() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, STUCK);
  let asked = chat(&served, &mut db);
  let made = |reason: &str, at: i64| orchester_core::orchestrator::decision::NewDecision {
    chat: asked.chat,
    tool: "continue_run".to_owned(),
    task: Some(task.key().to_string()),
    run: Some(run.id()),
    reason: orchester_core::orchestrator::decision::Reason::new(reason).expect("a reason"),
    undo: None,
    at: Timestamp::from_unix_millis(at),
  };
  let first = db.keep_decision(&made("The check is flaky.", 1_000)).expect("kept");
  let second = db
    .keep_decision(&made("The brief names the owner.", 2_000))
    .expect("kept");
  assert!(review_in(&seeded.repo, second, Review::Seen { at: Timestamp::now() }).expect("reviewed"));

  let listed = chat_call(&mut served, &asked, "get_decisions", json!({ "project": "Kontur" })).expect("listed");
  let numbers: Vec<i64> = listed["decisions"]
    .as_array()
    .expect("decisions")
    .iter()
    .map(|decision| decision["number"].as_i64().expect("a number"))
    .collect();
  assert_eq!(numbers, vec![first, second], "the open one first: {listed}");
  assert_eq!(listed["open"], 1, "{listed}");
  let open = &listed["decisions"][0];
  assert_eq!(open["reason"], "The check is flaky.");
  assert_eq!(open["task"], task.key().to_string());
  assert_eq!(open["review"], "open");
  assert_eq!(open["conversation"]["id"], asked.chat.to_string());
  assert_eq!(listed["decisions"][1]["review"], "seen");

  let of_task = json!({ "project": "Kontur", "task": task.key().to_string().to_lowercase(), "open_only": true });
  let narrowed = chat_call(&mut served, &asked, "get_decisions", of_task).expect("listed");
  assert_eq!(narrowed["decisions"].as_array().map(Vec::len), Some(1), "{narrowed}");
  let wrong_day = json!({ "project": "Kontur", "day": "yesterday" });
  let refused = chat_call(&mut served, &asked, "get_decisions", wrong_day).expect_err("no such day");
  assert_eq!(refused.code(), "invalid_arguments", "{refused:?}");
}
