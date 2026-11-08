//! The Orchestrator merges a finished run on its own only while the person lets it drive (ORC-69, ORC-909,
//! `docs/design/orchestrator-toolset.md`, decision 5), on the seeded Kontur: in the turn a run's success started by
//! itself, `merge_run` proposes the merge while *Let the Orchestrator drive* is off, and merges it at once with it,
//! kept as the chat's action *on its own*; a run the rule leaves to the person (here: a task that says *Don't merge
//! automatically*) is refused, naming why. The task's conversation names the Orchestrator as the merge's author; a
//! merge that stops at conflicts is refused naming the resolution, which the chat then decides itself.

use orchester_core::{
  id::ChatMessageId,
  orchestrator::{ActionOutcome, AgentWait, Merging, RunEnd, WaitKind},
  run::Run,
  store::MergeStore,
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{app_mcp::ToolError, orchestrator::tools::ChatScope};
use serde_json::{Value, json};

use super::{
  Served,
  kontur::Seeded,
  orchestrator_chat::{actions, allow, chat, chat_call, task_run},
  orchestrator_sent::{entries, open_conversation},
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
  let (task, run) = task_run(&db, READY);
  turn_of_success(&mut db, &own, (&seeded, &run));
  let told = chat_call(&mut served, &own, "merge_run", merge(&run)).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "the switch is off: {told}");
  assert!(db.run_merge(run.id()).expect("read").is_none(), "nothing merged");

  allow(&served, &seeded.repo);
  let merged = chat_call(&mut served, &own, "merge_run", merge(&run)).expect("merged on its own");
  assert_eq!(merged["merge"]["run"], run.id().to_string(), "{merged}");
  assert!(db.run_merge(run.id()).expect("read").is_some(), "the run is merged");
  let kept = actions(&db, &own).pop().expect("kept");
  assert_eq!(kept.outcome, ActionOutcome::Done);
  assert!(kept.on_its_own, "kept as merged by the Orchestrator on its own");

  let again = chat_call(&mut served, &own, "merge_run", merge(&run));
  assert!(
    !matches!(&again, Ok(told) if told.get("merge").is_some()),
    "a run merged is not merged again: {again:?}"
  );

  // The task's conversation attributes the merge to the Orchestrator, on its own (ORC-45's attribution).
  let key = task.key().to_string();
  let read = entries(&mut served, &key);
  let end = read
    .iter()
    .find(|entry| entry["end"] == "merged")
    .unwrap_or_else(|| panic!("the merged end: {read:?}"));
  assert_eq!(end["on_its_own"], true, "{end}");
  assert_eq!(end["author"]["kind"], "orchestrator", "{end}");
  assert_eq!(end["author"]["chat"], own.chat.to_string(), "{end}");
  open_conversation(&mut served, seeded.project, &key);
  served.until("the Orchestrator's row under the end", |served| {
    served
      .tree
      .get_element_by_id("conversation-end-merged-by-row")
      .is_some()
  });
  assert!(
    served
      .tree
      .get_element_by_id("conversation-end-merged-by-open-chat")
      .is_some(),
    "the way back to the chat"
  );
}

#[test]
fn chat_leaves_a_success_its_rule_does_not_let_it_merge_to_the_person() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let own = own_turn(&served, &mut db);
  let (task, run) = task_run(&db, READY);
  turn_of_success(&mut db, &own, (&seeded, &run));
  allow(&served, &seeded.repo);
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

/// The seeded run whose merge conflicts with main (scenario `decisions`).
const CONFLICTING: &str = "ready for review, conflicts with main";

#[test]
fn chat_merge_on_its_own_that_conflicts_names_the_resolution_and_decides_it_itself() {
  let (mut served, seeded) = Served::seeded("decisions");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let own = own_turn(&served, &mut db);
  let (_, run) = super::resolve::latest(&seeded, CONFLICTING);
  turn_of_success(&mut db, &own, (&seeded, &run));
  allow(&served, &seeded.repo);
  let refused = chat_call(&mut served, &own, "merge_run", merge(&run)).expect_err("the merge conflicts");
  let ToolError::Refused { code, reasons } = &refused else {
    panic!("a refusal: {refused:?}");
  };
  assert_eq!(code, "merge_failed");
  assert!(
    reasons
      .iter()
      .any(|reason| reason.key == "orchestrator.tools.mergeConflictsNext"),
    "names resolve_conflicts as the next step: {reasons:?}"
  );
  assert!(db.run_merge(run.id()).expect("read").is_none(), "nothing merged");
  assert!(
    matches!(
      actions(&db, &own).pop().expect("kept").outcome,
      ActionOutcome::Refused { .. }
    ),
    "the refusal is kept"
  );

  let resolve = json!({
    "project": "Kontur", "run": run.number().get().to_string(), "choice": { "kind": "resolve_conflicts" },
  });
  // The chat decides it itself: the tool is called, no card waits (this session has no runner to carry it out).
  let decided = chat_call(&mut served, &own, "continue_run", resolve);
  assert!(
    !matches!(&decided, Ok(told) if told["waits_for_person"] == true),
    "no card for the person: {decided:?}"
  );
  let kept = actions(&db, &own).pop().expect("kept");
  assert_eq!(kept.tool, "continue_run");
  assert_ne!(kept.outcome, ActionOutcome::Waits, "not a proposal");
}
