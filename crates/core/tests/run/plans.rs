use orchester_core::{
  id::{InvocationId, RunId},
  resource::InjectionEvent,
  run::{
    CONTINUE_PROMPT, Invocation, Plan, PlanDecision, PlanRevision, PlanVersion, PromptInput, ReviewNote,
    ReviewNoteError, Run, RunError, RunStatus, StepStatus, TokenUsage, WaitReason, active_time,
  },
};

use crate::{
  fixture::{Logged, at, key, model, task},
  prompt::{Prompt, builder, step_with},
};

fn status(logged: &Logged, step: &str) -> StepStatus {
  logged.run.latest_execution(&key(step)).expect("execution").status()
}

fn note(text: &str) -> ReviewNote {
  ReviewNote::new(text).expect("a note")
}

fn changes(text: &str) -> PlanDecision {
  PlanDecision::ChangesRequested { note: note(text) }
}

/// Research has passed and Plan proposed plan v1.
fn proposed() -> (Logged, Plan) {
  let mut logged = Logged::started();
  logged.pass("research");
  let plan = logged.propose("plan", "1. Add the provider.", &[]);
  (logged, plan)
}

#[test]
fn plans_gated_step_waits_with_plan_v1_and_holds_the_steps_after_it() {
  let (logged, plan) = proposed();
  let execution = logged.exec("plan");
  assert_eq!(plan.version, PlanVersion::FIRST);
  assert_eq!((plan.run, plan.execution), (logged.run.id(), execution));
  assert_eq!(plan.step, key("plan"));
  assert_eq!(plan.text, "1. Add the provider.");
  assert_eq!(plan.review, None);
  assert_eq!(status(&logged, "plan"), StepStatus::Waiting(WaitReason::Plan));
  assert_eq!(logged.run.awaiting_plan(), [execution]);
  assert!(logged.run.ready_executions().is_empty());
  assert!(logged.run.waits_for_a_person());
  assert_eq!(logged.run.status(), &RunStatus::Running);
}

#[test]
fn plans_approval_passes_the_step_and_the_graph_continues() {
  let (mut logged, mut plan) = proposed();
  logged.apply(|run| run.decide_plan(&mut plan, PlanDecision::Approved, at(30)));
  assert_eq!(status(&logged, "plan"), StepStatus::Passed);
  assert_eq!(plan.decision(), Some(&PlanDecision::Approved));
  assert_eq!(plan.review.as_ref().map(|review| review.at), Some(at(30)));
  assert_eq!(logged.run.ready_executions(), [logged.exec("implement")]);
  assert!(!logged.run.waits_for_a_person());
  assert!(logged.run.awaiting_plan().is_empty());
  // A plan is decided once.
  assert_eq!(
    logged.run.decide_plan(&mut plan, PlanDecision::Approved, at(31)),
    Err(RunError::PlanAlreadyDecided(PlanVersion::FIRST))
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn plans_requested_changes_send_the_step_back_and_its_revision_is_the_next_version() {
  let (mut logged, mut first) = proposed();
  logged.apply(|run| run.decide_plan(&mut first, changes("  Keep email sign-in.  "), at(30)));
  assert_eq!(status(&logged, "plan"), StepStatus::Running);
  assert_eq!(first.decision(), Some(&changes("Keep email sign-in.")));
  assert!(!logged.run.waits_for_a_person());
  // The step works again: nothing downstream may start, and there is nothing to decide.
  assert!(logged.run.ready_executions().is_empty());
  let mut stale = first.clone();
  stale.review = None;
  assert!(matches!(
    logged.run.decide_plan(&mut stale, PlanDecision::Approved, at(31)),
    Err(RunError::InvalidStepTransition {
      status: StepStatus::Running,
      ..
    })
  ));
  let mut second = logged.propose("plan", "1. Add the provider.\n2. Keep email sign-in.", &[first.clone()]);
  assert_eq!(second.version.get(), 2);
  assert_eq!(second.execution, first.execution);
  logged.apply(|run| run.decide_plan(&mut second, PlanDecision::Approved, at(40)));
  assert_eq!(status(&logged, "plan"), StepStatus::Passed);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn plans_the_first_invocation_after_requested_changes_revises_the_plan() {
  let (mut logged, mut first) = proposed();
  let execution = first.execution;
  logged.apply(|run| run.decide_plan(&mut first, changes("Keep email sign-in."), at(30)));
  let plans = [first.clone()];
  // The invocation that proposed v1 started before the decision: the next one revises.
  let (plan, revision) = Plan::awaiting_revision(&plans, execution, Some(at(22))).expect("a revision");
  assert_eq!(
    (plan.version, revision.as_str()),
    (PlanVersion::FIRST, "Keep email sign-in.")
  );
  // One started after it already got the note; a later one continues like any other.
  assert!(Plan::awaiting_revision(&plans, execution, Some(at(31))).is_none());
  let undecided = Plan {
    review: None,
    ..first.clone()
  };
  assert!(Plan::awaiting_revision(&[undecided], execution, Some(at(22))).is_none());
  assert_eq!(Plan::latest_of(&plans, execution), Some(&first));
}

#[test]
fn plans_review_notes_say_what_should_change() {
  assert_eq!(ReviewNote::new(" \n "), Err(ReviewNoteError::Blank));
  assert_eq!(note("  Split the migration. ").as_str(), "Split the migration.");
  let json = serde_json::to_string(&changes("Split it.")).expect("serialize");
  assert_eq!(json, r#"{"kind":"changes_requested","note":"Split it."}"#);
  let blank = serde_json::from_str::<PlanDecision>(r#"{"kind":"changes_requested","note":" "}"#);
  assert!(blank.is_err());
}

#[test]
fn plans_a_waiting_run_pauses_resumes_and_cancels() {
  let (mut logged, mut plan) = proposed();
  logged.apply(|run| run.request_pause(at(30)));
  logged.apply(|run| run.pause(at(30)));
  assert_eq!(status(&logged, "plan"), StepStatus::Waiting(WaitReason::Plan));
  assert!(!logged.run.waits_for_a_person());
  // A paused run takes no decision until it runs again.
  assert!(matches!(
    logged.run.decide_plan(&mut plan, PlanDecision::Approved, at(31)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  logged.apply(|run| run.resume(at(32)));
  assert!(logged.run.waits_for_a_person());
  logged.apply(|run| run.cancel(at(33)));
  assert_eq!(logged.run.status(), &RunStatus::Cancelled);
  assert_eq!(status(&logged, "plan"), StepStatus::Skipped);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn plans_waiting_for_a_decision_is_not_active_agent_time() {
  let (logged, _) = proposed();
  let invocation = Invocation {
    id: InvocationId::generate(),
    run: RunId::generate(),
    execution: logged.exec("plan"),
    number: 1,
    model: model("gpt-5.5"),
    session: Some("thread-1".into()),
    started_at: at(1_000),
    ended_at: Some(at(61_000)),
    usage: TokenUsage::default(),
    outcome: None,
  };
  // An hour of waiting for the reviewer adds nothing to the minute the agent worked.
  let active = active_time(std::slice::from_ref(&invocation), at(3_661_000));
  assert_eq!(active, std::time::Duration::from_secs(60));
}

#[test]
fn plans_a_resumed_session_is_told_what_should_change() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let note = note("Keep email sign-in.");
  let revision = PlanRevision {
    version: PlanVersion::FIRST,
    plan: "1. Replace sign-in.",
    note: &note,
  };
  let resumed = PromptInput {
    revision: Some(revision),
    ..prompt.input(&[], &[], true)
  }
  .assemble(InjectionEvent::Resume)
  .expect("no variables");
  assert!(!resumed.text.contains(CONTINUE_PROMPT));
  assert!(
    resumed
      .text
      .starts_with("A person reviewed your result for this step and requested changes.")
  );
  assert!(
    resumed
      .text
      .ends_with("What should change in plan v1:\n\nKeep email sign-in.")
  );
  assert!(resumed.sources.sources.is_empty());
}

#[test]
fn plans_a_new_session_gets_its_earlier_plan_and_the_note() {
  let mut step = step_with("Step.");
  step.approval_gate = true;
  let prompt = Prompt::new(builder("Role."), step, task("Brief.", &[]));
  let started = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("no variables");
  // A gated step's summary is what the reviewer reads.
  assert!(
    started
      .text
      .contains("A person reviews your summary before the steps after this one start")
  );
  assert!(!started.text.contains("# Changes requested"));
  let note = note("Keep email sign-in.");
  let revised = PromptInput {
    revision: Some(PlanRevision {
      version: PlanVersion::FIRST,
      plan: "1. Replace sign-in.",
      note: &note,
    }),
    ..prompt.input(&[], &[], true)
  }
  .assemble(InjectionEvent::Start)
  .expect("no variables");
  assert!(revised.text.contains(
    "# Changes requested to your plan v1\n\nYour earlier result for this step:\n\n1. Replace sign-in.\n\nThe reviewer \
     asks:\n\nKeep email sign-in.\n\n# When you finish"
  ));
}
