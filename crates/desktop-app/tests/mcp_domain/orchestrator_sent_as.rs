//! A message the Orchestrator chat proposed goes only as it was checked (`docs/design/orchestrator-toolset.md`,
//! decision 5): a card applied once its run waits on a plan is refused, never sent as the plan's changes; a card kept
//! before what it does was known is refused; and a card that runs the task again keeps the run it started, which its
//! Run fact then shows as it stands, with the real runner and the fake Codex.

use orchester_core::{
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{Change, ProposedChange},
  },
  store::RunStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::{
  facts::{self, Facts},
  proposal::Decision,
  tools::ChatScope,
};
use serde_json::json;

use super::{
  Served,
  kontur::Seeded,
  orchestrator_chat::{actions, chat, chat_call, task_run},
  orchestrator_continue::decide,
};

/// The title of the seeded task in state `state`.
fn titled(seeded: &Seeded, state: &str) -> String {
  seeded
    .tasks
    .iter()
    .find(|task| task.state == state)
    .unwrap_or_else(|| panic!("a task {state}"))
    .title
    .clone()
}

/// A message card the chat proposed to run `run` (`#number`) of task `key`, sending which did `sent_as` when it was
/// checked; the entry it is kept at.
fn proposed(
  db: &mut ProjectDb,
  scope: &ChatScope,
  (run, number, key): (orchester_core::id::RunId, u32, String),
  sent_as: Option<&str>,
) -> i64 {
  let action = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Message {
        run,
        number,
        text: "Keep the old export as it is.".to_owned(),
        now: false,
        then: None,
        sent_as: sent_as.map(str::to_owned),
      },
    })),
    ..ChatAction::new("send_message", Some(key), ActionOutcome::Waits)
  };
  db.add_chat_action(scope.chat, &action, Timestamp::now()).expect("kept");
  db.chat_entries(scope.chat).expect("entries").last().expect("it").seq
}

fn refused_with(db: &ProjectDb, scope: &ChatScope) -> String {
  match actions(db, scope).pop().expect("kept").outcome {
    ActionOutcome::Refused { code, .. } => code,
    other => panic!("refused, not {other:?}"),
  }
}

#[test]
fn chat_message_card_applied_once_its_run_waits_on_a_plan_is_refused() {
  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let (task, run) = task_run(&db, &titled(&seeded, "plan v1 waiting"));
  // Checked while an agent worked; the run waits on its plan now.
  let seq = proposed(
    &mut db,
    &scope,
    (run.id(), run.number().get(), task.key().to_string()),
    Some("send_after_turn"),
  );
  decide(&mut served, &scope, seq, Decision::Accept { widens: Vec::new() }).expect("decided");
  assert_eq!(refused_with(&db, &scope), "changed_since_proposed");
  let plans = db.plans(run.id()).expect("plans");
  assert!(
    plans.iter().all(|plan| plan.decision().is_none()),
    "nothing was sent as the plan's changes"
  );
}

#[test]
fn chat_message_card_kept_before_what_it_does_was_known_is_refused() {
  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let (task, run) = task_run(&db, &titled(&seeded, "question waiting"));
  let seq = proposed(
    &mut db,
    &scope,
    (run.id(), run.number().get(), task.key().to_string()),
    None,
  );
  decide(&mut served, &scope, seq, Decision::Accept { widens: Vec::new() }).expect("decided");
  assert_eq!(refused_with(&db, &scope), "changed_since_proposed");
}

#[test]
fn chat_run_again_card_keeps_and_shows_the_run_it_started() {
  let programs = super::runner_programs::programs();
  let (folder, seeded) = Served::seed("task-states", Some(programs.fake_codex.clone()));
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  // The seed used today's budget: lifted by the person, so the task runs again.
  db.update_project_limits(|limits| Some(limits.with_daily_budget_limit(None)))
    .expect("no daily budget");
  let mut served = Served::start_with(folder, &seeded.app_database, programs.runner.clone(), false);
  let scope = chat(&served, &mut db);
  let (task, old) = task_run(&db, &titled(&seeded, "cancelled"));
  let message = json!({ "project": "Kontur", "run": old.id().to_string(), "text": "Start over from main." });
  let told = chat_call(&mut served, &scope, "send_message", message).expect("proposed");
  assert_eq!(told["proposed"]["message"]["then"], "run_again", "{told}");
  let seq = db.chat_entries(scope.chat).expect("entries").last().expect("it").seq;
  decide(&mut served, &scope, seq, Decision::Accept { widens: Vec::new() }).expect("decided");
  let applied = actions(&db, &scope).pop().expect("kept");
  assert_eq!(applied.outcome, ActionOutcome::Applied, "{applied:?}");
  let latest = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("readable")
    .id();
  assert_ne!(latest, old.id(), "the task ran again: {applied:?}");
  assert_eq!(applied.started, Some(latest), "the card keeps the run it started");
  let programs = &served.services().programs;
  let Some(Facts::Run(shown)) = facts::load_in(&seeded.repo, programs, &applied).expect("facts") else {
    panic!("the card shows a run");
  };
  assert_eq!(
    shown.run,
    latest,
    "the Run fact is the started run, not #{}",
    old.number()
  );
}
