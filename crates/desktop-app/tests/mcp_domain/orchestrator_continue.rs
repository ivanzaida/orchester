//! The Orchestrator chat's `continue_run` (`docs/design/orchestrator-toolset.md`, decision 5), on the seeded Kontur's
//! run that waits for a decision after its retries: one more retry or a return to an earlier step is always a
//! proposal, whatever the person's switches, which the person dismisses or applies through `continue_run`; one the run
//! went on from since it was proposed is refused; applying is the person's own decision, which needs no consent for
//! automation; raising limits, resuming and accepting a step are the person's and
//! refused, as is a choice the run does not offer.

use std::{thread, time::Duration};

use orchester_core::{
  orchestrator::{
    ActionOutcome, EntryKind, StopReason,
    change::{Change, ContinueChoice},
  },
  run::{RunStatus, StuckDecision},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  app_mcp::{self, ToolError, Translate},
  messages::Message,
  orchestrator::{
    proposal::{self, Decision},
    tools::ChatScope,
  },
  workspace::OrchestratorConsent,
};
use serde_json::{Value, json};

use super::{
  Served,
  orchestrator_chat::{actions, chat, chat_call, task_run},
};

/// The seeded task whose run waits for a decision after Review used up its retries.
const STUCK: &str = "Cache GitHub API responses";

/// Takes the person's `decision` on the action `seq` of `scope`'s conversation, as the window takes it.
pub(super) fn decide(served: &mut Served, scope: &ChatScope, seq: i64, decision: Decision) -> Result<(), Message> {
  let s = served.services();
  let services = app_mcp::Services::new(
    s.workspace.clone(),
    s.runners.clone(),
    s.servers.clone(),
    s.programs.clone(),
    (s.changes.clone(), s.activity.clone(), s.jobs.clone()),
  );
  let runners = s.runners.clone();
  let scope = scope.clone();
  let decided = served._runtime.spawn(async move {
    let translate = Translate::new(|message| message.key.to_owned());
    proposal::decide((&services, &translate, &runners), &scope, seq, decision).await
  });
  while !decided.is_finished() {
    served.pass();
    thread::sleep(Duration::from_millis(5));
  }
  served._runtime.block_on(decided).expect("the decision ends")
}

/// The entry of `scope`'s newest action.
fn newest_action(db: &ProjectDb, scope: &ChatScope) -> i64 {
  db.chat_entries(scope.chat)
    .expect("entries")
    .into_iter()
    .rev()
    .find(|entry| matches!(entry.kind, EntryKind::Action(_)))
    .expect("an action")
    .seq
}

/// The code a refusal of the chat's tool carries.
fn code(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    ToolError::InvalidArguments(_) => "invalid_arguments",
    ToolError::NotFound { .. } => "not_found",
    ToolError::AmbiguousProject(_) => "ambiguous_project",
  }
}

fn retry(run: &str) -> Value {
  json!({ "project": "Kontur", "run": run, "choice": { "kind": "allow_retry" } })
}

#[test]
fn chat_continue_is_a_proposal_the_person_dismisses() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, STUCK);
  assert!(
    matches!(run.status(), RunStatus::WaitingForPerson(_)),
    "{:?}",
    run.status()
  );
  let scope = chat(&served, &mut db);
  // No switch lets the chat decide how a run that waits for a decision goes on, not even continuing interrupted runs.
  for consent in [
    OrchestratorConsent::AnswerAgents,
    OrchestratorConsent::MergeRuns,
    OrchestratorConsent::ContinueInterrupted,
  ] {
    served
      .services()
      .workspace
      .set_orchestrator_allows(consent, true)
      .expect("the person turned it on");
  }
  let number = run.number().get().to_string();
  let told = chat_call(&mut served, &scope, "continue_run", retry(&number)).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert_eq!(told["proposed"]["continue"]["choice"]["kind"], "allow_retry", "{told}");
  let proposed = actions(&db, &scope).pop().expect("the proposal");
  assert_eq!(proposed.outcome, ActionOutcome::Waits);
  assert_eq!(proposed.task.as_deref(), Some(task.key().to_string().as_str()));
  let Some(Change::Continue {
    attempt,
    step,
    why,
    choice,
    ..
  }) = proposed.change.as_ref().map(|change| change.change.clone())
  else {
    panic!("a continuation: {proposed:?}");
  };
  let now = run.current_attempt().expect("an attempt").number().get();
  assert_eq!((attempt, choice), (now, ContinueChoice::AllowRetry));
  assert_eq!(
    step.map(|step| step.name).as_deref(),
    Some("Review"),
    "the step it stopped at"
  );
  assert!(
    matches!(why, Some(StopReason::RetriesExhausted { retries }) if retries > 0),
    "{why:?}"
  );
  decide(&mut served, &scope, newest_action(&db, &scope), Decision::Dismiss).expect("dismissed");
  assert_eq!(
    actions(&db, &scope).pop().expect("it").outcome,
    ActionOutcome::Dismissed
  );
  let unchanged = db.run(run.id()).expect("read").expect("the run");
  assert_eq!(unchanged.status(), run.status(), "a dismissal changes nothing");
}

#[test]
fn chat_continue_return_is_refused_once_the_run_went_on() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, mut run) = task_run(&db, STUCK);
  let number = run.number().get().to_string();
  let scope = chat(&served, &mut db);
  let target = run
    .return_targets()
    .into_iter()
    .next()
    .expect("an earlier step to return to");
  let back = json!({
    "project": "Kontur", "run": number,
    "choice": { "kind": "return_to", "step": target.as_str(), "note": "Keep the ETag per URL." },
  });
  chat_call(&mut served, &scope, "continue_run", back).expect("proposed");
  let Some(Change::Continue {
    choice: ContinueChoice::ReturnTo { step, to, note },
    ..
  }) = actions(&db, &scope)
    .pop()
    .and_then(|action| action.change.map(|change| change.change))
  else {
    panic!("a return");
  };
  assert_eq!(
    (step.as_str(), note.as_str()),
    (target.as_str(), "Keep the ETag per URL.")
  );
  assert!(
    to.is_some_and(|to| to.place > 0),
    "the step it returns to, as people read it"
  );
  // The person retries on the task's page before applying the chat's proposal: the run went on since.
  let events = run
    .decide_stuck(&StuckDecision::Retry, Timestamp::now())
    .expect("retried");
  db.append_events(&run, &events).expect("recorded");
  decide(
    &mut served,
    &scope,
    newest_action(&db, &scope),
    Decision::Accept { widens: Vec::new() },
  )
  .expect("decided");
  let ActionOutcome::Refused { code, .. } = actions(&db, &scope).pop().expect("it").outcome else {
    panic!("refused");
  };
  assert_eq!(code, "changed_since_proposed");
}

#[test]
fn chat_continue_applied_goes_on_through_continue_run() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, STUCK);
  let scope = chat(&served, &mut db);
  chat_call(
    &mut served,
    &scope,
    "continue_run",
    retry(&run.number().get().to_string()),
  )
  .expect("proposed");
  decide(
    &mut served,
    &scope,
    newest_action(&db, &scope),
    Decision::Accept { widens: Vec::new() },
  )
  .expect("decided");
  // This app has no runner: the decision reached continue_run, which asked the runner, and its refusal is kept.
  let ActionOutcome::Refused { code, .. } = actions(&db, &scope).pop().expect("it").outcome else {
    panic!("refused without a runner");
  };
  assert!(code.starts_with("runner"), "{code}");
  assert_ne!(
    code, "automation_not_allowed",
    "the person's Apply is their own decision, not automation's"
  );
}

#[test]
fn chat_continue_refuses_what_is_the_persons_and_what_is_not_offered() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, STUCK);
  let number = run.number().get().to_string();
  let scope = chat(&served, &mut db);
  for choice in [
    json!({ "kind": "accept" }),
    json!({ "kind": "resume" }),
    json!({ "kind": "raise_limits", "budget_tokens": null, "time_limit_minutes": null }),
    json!({ "kind": "raise_step_budget", "budget_tokens": null }),
  ] {
    let call = json!({ "project": "Kontur", "run": number, "choice": choice });
    let refused = chat_call(&mut served, &scope, "continue_run", call).expect_err("the person's");
    assert_eq!(code(&refused), "person_only", "{choice}: {refused:?}");
  }
  let nowhere = json!({
    "project": "Kontur", "run": number, "choice": { "kind": "return_to", "step": "deploy", "note": "Again." },
  });
  let refused = chat_call(&mut served, &scope, "continue_run", nowhere).expect_err("no such step");
  assert_eq!(code(&refused), "step_not_offered", "{refused:?}");
  let (_, going) = task_run(&db, "Sync contacts with HubSpot");
  let refused = chat_call(
    &mut served,
    &scope,
    "continue_run",
    retry(&going.number().get().to_string()),
  )
  .expect_err("a run at work waits for no decision");
  assert_eq!(code(&refused), "not_stopped_at_limit", "{refused:?}");
  assert!(
    actions(&db, &scope)
      .iter()
      .all(|action| matches!(action.outcome, ActionOutcome::Refused { .. })),
    "each refusal is kept, nothing waits"
  );
}
