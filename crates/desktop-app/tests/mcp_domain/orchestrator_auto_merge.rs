//! The Orchestrator merges a finished run on its own only behind the person's switches (ORC-69,
//! `docs/design/orchestrator-toolset.md`, decision 5), on the seeded Kontur: in the turn a run's success started by
//! itself, `merge_run` of that run proposes the merge while *Let the Orchestrator merge finished runs on its own* is
//! off, and merges it at once with it, *merge runs* and *answer agents* on, kept as the chat's action *on its own*;
//! it merges once per end, only the run whose success started the turn, and a run the rule leaves to the person by
//! then (here: a task that says *Don't merge automatically*) is refused, naming why.

use orchester_core::{
  id::ChatMessageId,
  orchestrator::{ActionOutcome, AgentWait, Merging, RunEnd, WaitKind},
  run::Run,
  store::MergeStore,
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{app_mcp::ToolError, orchestrator::tools::ChatScope, workspace::OrchestratorConsent};
use serde_json::{Value, json};

use super::{
  Served,
  kontur::Seeded,
  orchestrator_chat::{actions, allow, chat, chat_call, task_run},
};

/// A seeded task whose latest run succeeded and is ready to merge.
const READY: &str = "Show the signed-in account";

fn merge(run: &Run) -> Value {
  json!({ "project": "Kontur", "run": run.id().to_string() })
}

/// The turn `run`'s success started by itself in `scope`'s conversation, to merge it on its own, thinking now.
fn turn_of_success(db: &mut ProjectDb, scope: &ChatScope, (seeded, run): (&Seeded, &Run)) {
  let wait = AgentWait {
    kind: WaitKind::Ended(RunEnd::Succeeded),
    task: "KON".to_owned(),
    title: "A success".to_owned(),
    run: run.id(),
    number: run.number().get(),
    step: None,
    place: None,
    others: 0,
    agent: None,
    link: format!("/project/{}/tasks", seeded.project),
    own: false,
    automatic: true,
    continuing: None,
    merging: Some(Merging::Merges),
  };
  let conversation = db.chat(scope.chat).expect("read").expect("the conversation");
  let key = format!("end:{}:test", run.id());
  db.keep_agent_wait(&conversation, (&key, &wait), Timestamp::now())
    .expect("kept");
  let seq = db
    .automatic_waits_without_turn()
    .expect("waits")
    .into_iter()
    .find(|kept| kept.key == key)
    .expect("it waits for its turn")
    .seq;
  assert!(
    db.start_wait_turn((scope.chat, seq), ChatMessageId::generate())
      .expect("started")
  );
}

/// The turn's scope: the chat's conversation, a turn nobody asked for.
fn own_turn(served: &Served, db: &mut ProjectDb) -> ChatScope {
  ChatScope {
    on_its_own: true,
    ..chat(served, db)
  }
}

#[test]
fn chat_merges_the_run_whose_success_started_the_turn_on_its_own_once() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let own = own_turn(&served, &mut db);
  let (_, run) = task_run(&db, READY);
  turn_of_success(&mut db, &own, (&seeded, &run));
  allow(&served, OrchestratorConsent::MergeRuns);
  allow(&served, OrchestratorConsent::AnswerAgents);
  let told = chat_call(&mut served, &own, "merge_run", merge(&run)).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "the switch is off: {told}");
  assert!(db.run_merge(run.id()).expect("read").is_none(), "nothing merged");

  allow(&served, OrchestratorConsent::MergeFinished);
  let merged = chat_call(&mut served, &own, "merge_run", merge(&run)).expect("merged on its own");
  assert_eq!(merged["merge"]["run"], run.id().to_string(), "{merged}");
  assert!(db.run_merge(run.id()).expect("read").is_some(), "the run is merged");
  let kept = actions(&db, &own).pop().expect("kept");
  assert_eq!(kept.outcome, ActionOutcome::Done);
  assert!(kept.on_its_own, "kept as merged by the Orchestrator on its own");

  let again = chat_call(&mut served, &own, "merge_run", merge(&run));
  assert!(
    !matches!(&again, Ok(told) if told.get("merge").is_some()),
    "one automatic merge per end: {again:?}"
  );
  assert!(!actions(&db, &own).pop().expect("kept").on_its_own);
}

#[test]
fn chat_leaves_a_success_its_rule_does_not_let_it_merge_to_the_person() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let own = own_turn(&served, &mut db);
  let (task, run) = task_run(&db, READY);
  turn_of_success(&mut db, &own, (&seeded, &run));
  for consent in [
    OrchestratorConsent::MergeRuns,
    OrchestratorConsent::AnswerAgents,
    OrchestratorConsent::MergeFinished,
  ] {
    allow(&served, consent);
  }
  db.update_task_organization(task.id(), ChangeActor::Person, Timestamp::now(), |value| {
    value.with_no_automatic_merge(true)
  })
  .expect("the person opted the task out");
  let refused = chat_call(&mut served, &own, "merge_run", merge(&run)).expect_err("left to the person");
  match refused {
    ToolError::Refused { code, reasons } => {
      assert_eq!(code, "left_to_person");
      assert_eq!(reasons[0].key, "orchestrator.tools.mergeLeft.optedOut");
    }
    other => panic!("a refusal: {other:?}"),
  }
  assert!(db.run_merge(run.id()).expect("read").is_none(), "nothing merged");
}
