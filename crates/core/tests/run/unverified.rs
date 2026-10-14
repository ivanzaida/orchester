//! A step that could not check everything it is for reports `unverified`: the run waits for the person without
//! taking the step's failure route or using a retry, and the person accepts the step, returns the work with a note
//! or runs the step again. A step is not run again on the revision its latest failed or unverified result was for
//! unless a person asks for it.

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::{
    AttemptReason, Run, RunError, RunStatus, StepFailure, StepOutcome, StepOutput, StepResult, StepStatus, Stuck,
    StuckDecision, UnverifiedCheck,
  },
};

use crate::{
  fixture::{Logged, at, key, revision, stored},
  steps::implemented,
};

fn note(text: &str) -> orchester_core::run::ReviewNote {
  orchester_core::run::ReviewNote::new(text).expect("a note")
}

fn status(logged: &Logged, step: &str) -> StepStatus {
  logged.run.latest_execution(&key(step)).expect("an execution").status()
}

/// `checks` and `review` start in parallel on revision `b`; `checks` reports what it could not check.
fn unverified() -> Logged {
  let mut logged = implemented();
  let checks = logged.exec("checks");
  let review = logged.exec("review");
  logged.apply(|run| run.start_step(checks, at(40)));
  logged.apply(|run| run.start_step(review, at(40)));
  logged.apply(|run| run.fail_judged(checks, (StepFailure::Unverified, Some(revision('b'))), at(41)));
  logged
}

#[test]
fn unverified_waits_for_the_person_without_a_retry() {
  let logged = unverified();
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::Unverified(key("checks")))
  );
  // No failure route was taken: still one attempt, and no retry counted.
  assert_eq!(logged.run.attempts().len(), 1);
  assert_eq!(logged.run.retries_taken(&key("checks")), 0);
  assert_eq!(status(&logged, "review"), StepStatus::Skipped);
  let failed = logged.run.latest_execution(&key("checks")).expect("checks");
  assert_eq!(failed.failure(), Some(StepFailure::Unverified));
  assert_eq!(failed.judged(), Some(&revision('b')));
  assert_eq!(logged.run.retry_from(), Some(&key("checks")));
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run));
}

#[test]
fn unverified_accept_passes_the_step_and_runs_the_steps_that_have_not_passed() {
  let mut logged = unverified();
  logged.apply(|run| run.decide_stuck(&StuckDecision::Accept, at(50)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(status(&logged, "checks"), StepStatus::Passed);
  let accepted = logged.run.current_attempt().expect("an attempt");
  assert_eq!(accepted.reason(), &AttemptReason::Accepted { step: key("checks") });
  let steps: Vec<_> = accepted.executions().iter().map(|e| e.step().as_str()).collect();
  assert_eq!(steps, ["review", "pr"]);
  logged.pass("review");
  logged.pass("pr");
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run));
}

#[test]
fn unverified_accept_of_the_last_step_succeeds_the_run() {
  let mut logged = implemented();
  logged.pass("checks");
  logged.pass("review");
  let pr = logged.exec("pr");
  logged.apply(|run| run.start_step(pr, at(60)));
  logged.apply(|run| run.fail_judged(pr, (StepFailure::Unverified, Some(revision('c'))), at(61)));
  logged.apply(|run| run.decide_stuck(&StuckDecision::Accept, at(62)));
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(logged.run.attempts().len(), 1);
}

#[test]
fn unverified_retry_runs_the_step_again_as_the_person_asked() {
  let mut logged = unverified();
  logged.apply(|run| run.decide_stuck(&StuckDecision::Retry, at(50)));
  let again = logged.run.current_attempt().expect("an attempt");
  assert_eq!(again.reason(), &AttemptReason::Restart { step: key("checks") });
  let steps: Vec<_> = again.executions().iter().map(|e| e.step().as_str()).collect();
  assert_eq!(steps, ["checks", "review", "pr"]);
  // The person asked for it: the same revision is no reason to refuse.
  assert!(!logged.run.reruns_unchanged(logged.exec("checks"), &revision('b')));
  // Waiting unverified is no breakage: the step breaking now restarts once, it does not wait.
  let checks = logged.exec("checks");
  logged.apply(|run| run.start_step(checks, at(51)));
  logged.apply(|run| run.fail_step(checks, StepFailure::AgentFailed, at(52)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
}

#[test]
fn unverified_return_to_implement_refuses_checks_on_the_unchanged_revision() {
  let mut logged = unverified();
  let decision = StuckDecision::ReturnTo {
    step: key("implement"),
    note: note("Add the missing fixture."),
  };
  logged.apply(|run| run.decide_stuck(&decision, at(50)));
  logged.pass("implement");
  let checks = logged.exec("checks");
  assert!(logged.run.reruns_unchanged(checks, &revision('b')));
  assert!(
    !logged.run.reruns_unchanged(checks, &revision('c')),
    "a new revision runs"
  );
  logged.apply(|run| run.refuse_unchanged(checks, revision('b'), at(60)));
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::Unchanged(key("checks")))
  );
  assert_eq!(
    logged.run.latest_execution(&key("checks")).and_then(|e| e.failure()),
    Some(StepFailure::Unchanged)
  );
  // The person may still accept it as it is.
  logged.apply(|run| run.decide_stuck(&StuckDecision::Accept, at(61)));
  assert_eq!(status(&logged, "checks"), StepStatus::Passed);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
}

#[test]
fn unverified_a_failed_verdict_on_an_unchanged_revision_is_not_run_again() {
  let mut logged = implemented();
  let checks = logged.exec("checks");
  logged.apply(|run| run.start_step(checks, at(40)));
  logged.apply(|run| run.fail_judged(checks, (StepFailure::ReportedFailed, Some(revision('b'))), at(41)));
  assert!(matches!(
    logged.run.current_attempt().map(|attempt| attempt.reason()),
    Some(AttemptReason::Retry { .. })
  ));
  logged.pass("implement");
  assert!(logged.run.reruns_unchanged(logged.exec("checks"), &revision('b')));
  // A breakage holds for no revision.
  let mut broke = implemented();
  let checks = broke.exec("checks");
  broke.apply(|run| run.start_step(checks, at(40)));
  broke.apply(|run| run.fail_judged(checks, (StepFailure::AgentFailed, Some(revision('b'))), at(41)));
  assert!(!broke.run.reruns_unchanged(broke.exec("checks"), &revision('b')));
}

#[test]
fn unverified_only_a_step_without_a_verdict_can_be_accepted() {
  let mut logged = implemented();
  for _ in 0..3 {
    let checks = logged.exec("checks");
    logged.apply(|run| run.start_step(checks, at(40)));
    logged.apply(|run| run.fail_step(checks, StepFailure::ReportedFailed, at(41)));
    if logged.run.status() == &RunStatus::Running {
      logged.pass("implement");
    }
  }
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::RetriesExhausted(key("checks")))
  );
  assert_eq!(
    logged.run.decide_stuck(&StuckDecision::Accept, at(50)),
    Err(RunError::NotAcceptable(key("checks")))
  );
}

#[test]
fn unverified_the_result_lists_what_could_not_be_checked() {
  let message = r#"{"outcome":"unverified","summary":"Two states unreachable.","unverified":[
    {"check":"Summary 02.B","reason":"no fixture reaches it"},{"check":"  "},"stray",{"check":"Dialog","reason":3}],
    "completed":null,"decisions":null,"open_questions":null,"next_agent_should_know":null}"#;
  let result = StepResult::parse(message).expect("a result");
  assert_eq!(result.outcome, StepOutcome::Unverified);
  assert_eq!(
    result.unverified,
    [
      UnverifiedCheck {
        check: "Summary 02.B".into(),
        reason: "no fixture reaches it".into(),
      },
      UnverifiedCheck {
        check: "Dialog".into(),
        reason: String::new(),
      },
    ]
  );
  let output = StepOutput::from_final_message(
    RunId::generate(),
    StepExecutionId::generate(),
    key("verify"),
    Some(message),
    at(1),
  );
  assert_eq!(output.unverified.len(), 2);
  assert_eq!(output.failure(true), Some(StepFailure::Unverified));
  // A null list, as Codex fills every field, is empty; another outcome keeps no list.
  let done = r#"{"outcome":"done","summary":"Ok.","unverified":[{"check":"x","reason":"y"}]}"#;
  let output = StepOutput::from_final_message(
    RunId::generate(),
    StepExecutionId::generate(),
    key("verify"),
    Some(done),
    at(1),
  );
  assert!(output.unverified.is_empty());
  assert!(StepFailure::Unverified.waits_for_person() && !StepFailure::Unverified.is_breakage());
}
