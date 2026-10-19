//! What the Orchestrator chat does not do itself (`docs/design/orchestrator-toolset.md`, decision 5), on the seeded
//! Kontur: in a turn a wait or a run's end started by itself, `merge_run` and `run_task` only propose, even with *Let
//! the Orchestrator merge runs* and auto mode on; and `resume_run` refuses a run stopped at a limit or interrupted,
//! which only the person resumes.

use orchester_core::{
  id::{ChatMessageId, RunId},
  orchestrator::{ActionOutcome, AgentWait, RunEnd, WaitKind},
  run::{InterruptCause, RunLimit, review::AutoModeSettings},
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{app_mcp::ToolError, orchestrator::tools::ChatScope, workspace::OrchestratorConsent};
use serde_json::json;

use super::{
  Served,
  orchestrator_chat::{actions, chat, chat_call, task_run},
};

/// What the seed's run that succeeded and waits to be merged belongs to.
const READY: &str = "Show the signed-in account";

/// A turn of `scope`'s conversation, which the end of `run` started by itself, thinks.
fn turn_on_its_own(db: &mut ProjectDb, scope: &ChatScope, run: RunId) {
  let chat = db.chat(scope.chat).expect("read").expect("the conversation");
  let ended = AgentWait {
    kind: WaitKind::Ended(RunEnd::Succeeded),
    task: "KON-0".to_owned(),
    title: READY.to_owned(),
    run,
    number: 1,
    step: None,
    place: None,
    others: 0,
    agent: None,
    link: String::new(),
    own: false,
    automatic: true,
  };
  db.keep_agent_wait(&chat, ("end:test", &ended), Timestamp::now())
    .expect("kept");
  let seq = db.automatic_waits_without_turn().expect("read")[0].seq;
  assert!(
    db.start_wait_turn((scope.chat, seq), ChatMessageId::generate())
      .expect("started")
  );
}

fn refusal(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    other => panic!("a refusal: {other:?}"),
  }
}

#[test]
fn chat_turn_on_its_own_only_proposes_a_merge_or_a_start() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, READY);
  let scope = chat(&served, &mut db);
  served
    .services()
    .workspace
    .set_orchestrator_allows(OrchestratorConsent::MergeRuns, true)
    .expect("the person turned it on");
  db.update_auto_mode_settings(|current| AutoModeSettings {
    new_runs: true,
    reviewer: current.reviewer,
  })
  .expect("auto mode on");
  turn_on_its_own(&mut db, &scope, run.id());
  let merge = json!({ "project": "Kontur", "run": run.id().to_string() });
  let told = chat_call(&mut served, &scope, "merge_run", merge).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert!(db.run_merge(run.id()).expect("read").is_none(), "nothing merged");
  assert_eq!(actions(&db, &scope).pop().expect("it").outcome, ActionOutcome::Waits);
  // The seed's draft that is complete enough to run.
  let draft = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == "Drop the legacy sync worker")
    .expect("the ready draft");
  let start = json!({ "project": "Kontur", "key": draft.key().to_string() });
  let told = chat_call(&mut served, &scope, "run_task", start).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "even in auto mode: {told}");
  assert!(
    actions(&db, &scope)
      .pop()
      .is_some_and(|action| action.outcome.is_proposed()),
    "kept as a proposal"
  );
}

#[test]
fn chat_resumes_no_run_stopped_at_a_limit_or_interrupted() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let (_, mut interrupted) = task_run(&db, "Sync contacts with HubSpot");
  let events = interrupted
    .interrupt(InterruptCause::Crashed, Timestamp::now())
    .expect("interrupted");
  db.append_events(&interrupted, &events).expect("recorded");
  let resume = |run: &orchester_core::run::Run| json!({ "project": "Kontur", "run": run.id().to_string() });
  let refused = chat_call(&mut served, &scope, "resume_run", resume(&interrupted)).expect_err("the person's");
  assert_eq!(refusal(&refused), "person_only");
  let (_, mut limited) = task_run(&db, "Plan the GitHub app permissions");
  let events = limited
    .stop_at_limit(RunLimit::DailyBudget, Timestamp::now())
    .expect("stopped");
  db.append_events(&limited, &events).expect("recorded");
  let refused = chat_call(&mut served, &scope, "resume_run", resume(&limited)).expect_err("the person's");
  assert_eq!(refusal(&refused), "person_only");
  assert!(
    actions(&db, &scope)
      .iter()
      .all(|action| matches!(&action.outcome, ActionOutcome::Refused { code, .. } if code == "person_only")),
    "each refusal is kept"
  );
}
