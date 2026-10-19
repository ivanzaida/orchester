//! What the Orchestrator chat does not do itself (`docs/design/orchestrator-toolset.md`, decision 5), on the seeded
//! Kontur: in a turn a wait or a run's end started by itself, `merge_run` and `run_task` only propose, even with *Let
//! the Orchestrator merge runs* and auto mode on; `resume_run` refuses a run stopped at a limit or interrupted,
//! which only the person resumes; and a hold or a release of new starts that would change nothing is refused.

use orchester_core::{
  id::IdempotencyKey,
  orchestrator::ActionOutcome,
  resource::Pipeline,
  run::{InterruptCause, RunLimit, review::AutoModeSettings},
  store::{MergeStore, ResourceStore, RunStore, TaskStore},
  task_group::{HoldActor, HoldScope, ReviewedPipeline, Scheduling, TaskGroup},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  app_mcp::ToolError,
  orchestrator::{proposal::Decision, tools::ChatScope},
  workspace::OrchestratorConsent,
};
use serde_json::json;

use super::{
  Served,
  orchestrator_chat::{actions, chat, chat_call, task_run},
};

/// What the seed's run that succeeded and waits to be merged belongs to.
const READY: &str = "Show the signed-in account";

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
  // The tools of a turn a run's end started by itself.
  let scope = ChatScope {
    on_its_own: true,
    ..scope
  };
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

/// Holds the project's new starts as `actor`, as Tasks or the chat holds them.
fn hold(db: &mut ProjectDb, actor: HoldActor) {
  db.set_queue_hold((HoldScope::Project, actor), true, Timestamp::now())
    .expect("held");
}

/// Takes the person's Apply on the newest action of `scope`'s conversation; the code and the reason it was refused
/// with (the tests' words are the keys).
fn apply_refused(served: &mut Served, db: &ProjectDb, scope: &ChatScope) -> (String, String) {
  let seq = db.chat_entries(scope.chat).expect("entries").last().expect("it").seq;
  super::orchestrator_continue::decide(served, scope, seq, Decision::Accept { widens: Vec::new() }).expect("decided");
  match actions(db, scope).pop().expect("kept").outcome {
    ActionOutcome::Refused { code, reason } => (code, reason),
    other => panic!("refused, not {other:?}"),
  }
}

#[test]
fn chat_release_is_proposed_only_for_what_automation_held() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = ChatScope {
    on_its_own: true,
    ..chat(&served, &mut db)
  };
  let release = || json!({ "project": "Kontur" });
  let nothing = chat_call(&mut served, &scope, "release_queue", release()).expect_err("nothing held");
  assert_eq!(refusal(&nothing), "nothing_held");
  hold(&mut db, HoldActor::Person);
  let person = chat_call(&mut served, &scope, "release_queue", release()).expect_err("a person's hold");
  assert_eq!(refusal(&person), "person_only");
  assert!(db.queue_holds().expect("holds").project, "the person's hold stays");
}

#[test]
fn chat_release_applied_once_nothing_or_a_person_holds_is_refused() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = ChatScope {
    on_its_own: true,
    ..chat(&served, &mut db)
  };
  let release = || json!({ "project": "Kontur" });
  hold(&mut db, HoldActor::Automation);
  chat_call(&mut served, &scope, "release_queue", release()).expect("proposed");
  // New starts went on in Tasks before the person applied it.
  db.set_queue_hold((HoldScope::Project, HoldActor::Person), false, Timestamp::now())
    .expect("released");
  assert_eq!(
    apply_refused(&mut served, &db, &scope),
    (
      "nothing_held".to_owned(),
      "orchestrator.proposal.releaseNothing".to_owned()
    ),
    "no release shown that did not happen"
  );
  hold(&mut db, HoldActor::Automation);
  chat_call(&mut served, &scope, "release_queue", release()).expect("proposed");
  // A person held them in Tasks since: theirs to release.
  hold(&mut db, HoldActor::Person);
  assert_eq!(
    apply_refused(&mut served, &db, &scope),
    (
      "person_only".to_owned(),
      "orchestrator.proposal.releasePersonHeld".to_owned()
    )
  );
  assert!(db.queue_holds().expect("holds").project, "the person's hold stays");
}

/// The newest action of `scope`'s conversation: its code and reason when refused.
fn newest_refusal(db: &ProjectDb, scope: &ChatScope) -> (String, String) {
  match actions(db, scope).pop().expect("kept").outcome {
    ActionOutcome::Refused { code, reason } => (code, reason),
    other => panic!("refused, not {other:?}"),
  }
}

#[test]
fn chat_hold_or_release_that_changes_nothing_is_refused() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  // A turn the person started.
  let scope = chat(&served, &mut db);
  let project = || json!({ "project": "Kontur" });
  let released = chat_call(&mut served, &scope, "release_queue", project()).expect_err("nothing held");
  assert_eq!(refusal(&released), "nothing_held");
  assert_eq!(
    newest_refusal(&db, &scope),
    (
      "nothing_held".to_owned(),
      "orchestrator.tools.releaseNothing".to_owned()
    ),
    "no release shown that did not happen"
  );
  chat_call(&mut served, &scope, "hold_queue", project()).expect("held");
  assert_eq!(actions(&db, &scope).pop().expect("kept").outcome, ActionOutcome::Done);
  let again = chat_call(&mut served, &scope, "hold_queue", project()).expect_err("held already");
  assert_eq!(refusal(&again), "already_held");
  assert_eq!(
    newest_refusal(&db, &scope),
    ("already_held".to_owned(), "orchestrator.tools.holdAlready".to_owned()),
    "no hold shown that did not happen"
  );
  chat_call(&mut served, &scope, "release_queue", project()).expect("released");
  assert_eq!(actions(&db, &scope).pop().expect("kept").outcome, ActionOutcome::Done);
  assert!(!db.queue_holds().expect("holds").project);
}

/// The seed's draft that is complete enough to run, queued alone in a reviewed group: its key and the group's.
fn grouped(db: &mut ProjectDb) -> (String, IdempotencyKey) {
  let draft = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == "Drop the legacy sync worker")
    .expect("the ready draft");
  let pipeline = draft.details().pipeline.expect("its pipeline");
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("read")
    .expect("published")
    .number();
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    vec![draft.id()],
    Scheduling::Sequential,
    TokenBudget::new(1_000_000).expect("cap"),
  )
  .expect("group");
  let reviewed = [ReviewedPipeline {
    task: draft.id(),
    pipeline,
    version,
  }];
  db.queue_task_group(&group, None, &reviewed).expect("queued");
  (draft.key().to_string(), group.key())
}

/// Holds new starts in `group` as `actor`.
fn hold_group(db: &mut ProjectDb, group: IdempotencyKey, actor: HoldActor) {
  db.set_queue_hold((HoldScope::Group(group), actor), true, Timestamp::now())
    .expect("held");
}

#[test]
fn chat_release_of_a_group_is_proposed_only_for_what_automation_held() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let own = chat(&served, &mut db);
  let scope = ChatScope {
    on_its_own: true,
    ..own.clone()
  };
  let (task, group) = grouped(&mut db);
  let release = || json!({ "project": "Kontur", "task": task });
  let nothing = chat_call(&mut served, &scope, "release_queue", release()).expect_err("nothing held");
  assert_eq!(refusal(&nothing), "nothing_held");
  hold_group(&mut db, group, HoldActor::Automation);
  let told = chat_call(&mut served, &scope, "release_queue", release()).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert!(
    db.queue_holds().expect("holds").groups.contains(&group),
    "nothing released yet"
  );
  let seq = db.chat_entries(scope.chat).expect("entries").last().expect("it").seq;
  super::orchestrator_continue::decide(&mut served, &scope, seq, Decision::Accept { widens: Vec::new() })
    .expect("decided");
  assert_eq!(
    actions(&db, &scope).pop().expect("kept").outcome,
    ActionOutcome::Applied
  );
  assert!(!db.queue_holds().expect("holds").groups.contains(&group), "released");
  // A person's hold on the group is theirs to release, whoever asks.
  hold_group(&mut db, group, HoldActor::Person);
  for scope in [&scope, &own] {
    let person = chat_call(&mut served, scope, "release_queue", release()).expect_err("a person's hold");
    assert_eq!(refusal(&person), "person_only");
  }
  assert!(
    db.queue_holds().expect("holds").groups.contains(&group),
    "the person's hold stays"
  );
}

#[test]
fn chat_release_of_a_task_in_no_group_names_that() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let own = chat(&served, &mut db);
  let scope = ChatScope {
    on_its_own: true,
    ..own.clone()
  };
  let (task, _) = task_run(&db, READY);
  let release = json!({ "project": "Kontur", "task": task.key().to_string() });
  for scope in [&scope, &own] {
    let refused = chat_call(&mut served, scope, "release_queue", release.clone()).expect_err("in no group");
    assert_eq!(refusal(&refused), "not_in_group", "on its own: {}", scope.on_its_own);
  }
}
