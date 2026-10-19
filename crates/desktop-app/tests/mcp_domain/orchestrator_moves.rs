//! What the Orchestrator chat's `send_message` does to a run (`docs/design/orchestrator-toolset.md`, decision 5), on
//! the seeded Kontur's task states: a message to a run none of whose agents waits or works moves it (follows it up,
//! continues, resumes, continues an interrupted run, waits for a raised limit, runs the task again), which is the
//! person's, so it is always proposed, even with *Let the Orchestrator answer agents* and automation's consent on; one
//! to an agent that waits for an answer is sent at once. And in a turn a wait or a run's end started by itself, a
//! pause, a resume, a release of new starts and a publication are only proposed.

use orchester_core::{
  orchestrator::{ActionOutcome, EntryKind},
  resource::{Draft, Pipeline, PromptText},
  run::InterruptCause,
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  orchestrator::{proposal::Decision, tools::ChatScope},
  workspace::OrchestratorConsent,
};
use serde_json::{Value, json};

use super::{
  Served,
  kontur::Seeded,
  orchestrator_chat::{actions, chat, chat_call, task_run},
};

/// The title of the seeded task in state `state` (the seed's report names it).
fn titled(seeded: &Seeded, state: &str) -> String {
  seeded
    .tasks
    .iter()
    .find(|task| task.state == state)
    .unwrap_or_else(|| panic!("a task {state}"))
    .title
    .clone()
}

/// The seeded `task-states` project served, the chat's switches and automation's consent on, and a conversation.
fn switched_on() -> (Served, Seeded, ProjectDb, ChatScope) {
  let (served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let workspace = &served.services().workspace;
  for consent in [
    OrchestratorConsent::AnswerAgents,
    OrchestratorConsent::MergeRuns,
    OrchestratorConsent::PublishDrafts,
  ] {
    workspace
      .set_orchestrator_allows(consent, true)
      .expect("the person turned it on");
  }
  workspace
    .set_automation_answers_approvals(true)
    .expect("decision consent");
  let scope = chat(&served, &mut db);
  (served, seeded, db, scope)
}

fn message(run: &str) -> Value {
  json!({ "project": "Kontur", "run": run, "text": "Keep the old export as it is." })
}

#[test]
fn chat_message_that_moves_a_run_is_always_proposed() {
  let (mut served, seeded, db, scope) = switched_on();
  for (state, then) in [
    ("ready for review", "follow_up"),
    ("waits after retries", "continue"),
    ("failed", "continue"),
    ("paused", "resume"),
    ("interrupted", "continue_interrupted"),
    ("stopped at its token limit", "raise_limit"),
    ("cancelled", "run_again"),
  ] {
    let (_, run) = task_run(&db, &titled(&seeded, state));
    let told = chat_call(&mut served, &scope, "send_message", message(&run.id().to_string()))
      .unwrap_or_else(|error| panic!("{state}: proposed, not {error:?}"));
    assert_eq!(told["waits_for_person"], true, "{state}: {told}");
    assert_eq!(told["proposed"]["message"]["then"], then, "{state}: {told}");
    assert_eq!(
      actions(&db, &scope).pop().expect("kept").outcome,
      ActionOutcome::Waits,
      "{state}"
    );
  }
}

#[test]
fn chat_message_to_an_agent_that_waits_is_sent_at_once() {
  let (mut served, seeded, db, scope) = switched_on();
  let (_, run) = task_run(&db, &titled(&seeded, "question waiting"));
  // Sent at once as the answer to its question; this session has no runner, so it queues for the agent.
  let told = chat_call(&mut served, &scope, "send_message", message(&run.id().to_string())).expect("sent");
  assert_eq!(told["action"], "answer", "{told}");
  assert_eq!(told.get("waits_for_person"), None, "not proposed: {told}");
  assert_eq!(actions(&db, &scope).pop().expect("kept").outcome, ActionOutcome::Done);
}

#[test]
fn chat_turn_on_its_own_proposes_pausing_resuming_releasing_and_publishing() {
  let (mut served, seeded, db, scope) = switched_on();
  let own = ChatScope {
    on_its_own: true,
    ..scope
  };
  let (_, running) = task_run(&db, &titled(&seeded, "running"));
  let (_, paused) = task_run(&db, &titled(&seeded, "paused"));
  let pipeline = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find_map(|task| task.details().pipeline)
    .expect("a task on a pipeline");
  let mut drafts = ProjectDb::open(&seeded.repo).expect("the project's data");
  drafts
    .update_draft::<Pipeline>(pipeline, |_, latest| {
      let latest = latest.expect("the pipeline is published");
      let mut steps = latest.body().steps().to_vec();
      steps[0].instructions = PromptText::new("Read the README first.").expect("prompt");
      let body = Pipeline::new(steps).expect("the same steps");
      Ok(Draft::new(body, Some(latest.number()), Timestamp::now()))
    })
    .expect("a draft to publish");
  for (tool, arguments) in [
    (
      "pause_run",
      json!({ "project": "Kontur", "run": running.id().to_string() }),
    ),
    (
      "resume_run",
      json!({ "project": "Kontur", "run": paused.id().to_string() }),
    ),
    ("release_queue", json!({ "project": "Kontur" })),
    (
      "publish_pipeline",
      json!({ "project": "Kontur", "pipeline": pipeline.to_string() }),
    ),
  ] {
    let told = chat_call(&mut served, &own, tool, arguments).unwrap_or_else(|error| panic!("{tool}: {error:?}"));
    assert_eq!(told["waits_for_person"], true, "{tool}: {told}");
    assert_eq!(
      actions(&db, &own).pop().expect("kept").outcome,
      ActionOutcome::Waits,
      "{tool}"
    );
  }
}

/// Takes the person's Apply on the newest action of `scope`'s conversation, as the window takes it.
fn apply_newest(served: &mut Served, db: &ProjectDb, scope: &ChatScope) {
  let seq = db
    .chat_entries(scope.chat)
    .expect("entries")
    .into_iter()
    .rev()
    .find(|entry| matches!(entry.kind, EntryKind::Action(_)))
    .expect("an action")
    .seq;
  super::orchestrator_continue::decide(served, scope, seq, Decision::Accept { widens: Vec::new() }).expect("decided");
}

/// The code of the newest action's refusal.
fn refused_with(db: &ProjectDb, scope: &ChatScope) -> String {
  match actions(db, scope).pop().expect("kept").outcome {
    ActionOutcome::Refused { code, .. } => code,
    other => panic!("refused, not {other:?}"),
  }
}

#[test]
fn chat_message_applied_after_its_run_moved_on_is_refused() {
  let (mut served, seeded, mut db, scope) = switched_on();
  let (_, mut run) = task_run(&db, &titled(&seeded, "interrupted"));
  let told = chat_call(&mut served, &scope, "send_message", message(&run.id().to_string())).expect("proposed");
  assert_eq!(told["proposed"]["message"]["then"], "continue_interrupted", "{told}");
  // The person cancels the run on its page: the message would now run the task again.
  let events = run.cancel(Timestamp::now()).expect("cancelled");
  db.append_events(&run, &events).expect("recorded");
  apply_newest(&mut served, &db, &scope);
  assert_eq!(refused_with(&db, &scope), "changed_since_proposed");
}

#[test]
fn chat_resume_applied_after_its_run_was_interrupted_is_refused() {
  let (mut served, seeded, mut db, scope) = switched_on();
  let own = ChatScope {
    on_its_own: true,
    ..scope
  };
  // A run the chat would resume once its pause settles; it is at work still.
  let (_, mut run) = task_run(&db, &titled(&seeded, "running"));
  let resume = json!({ "project": "Kontur", "run": run.id().to_string() });
  chat_call(&mut served, &own, "resume_run", resume).expect("proposed");
  // The app ends without closing: the run is interrupted before the person applies the resume.
  let events = run
    .interrupt(InterruptCause::Crashed, Timestamp::now())
    .expect("interrupted");
  db.append_events(&run, &events).expect("recorded");
  apply_newest(&mut served, &db, &own);
  assert_eq!(
    refused_with(&db, &own),
    "person_only",
    "an interrupted run is not resumed"
  );
}
