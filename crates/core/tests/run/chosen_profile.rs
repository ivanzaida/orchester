//! The agent profile a person chooses when they return a run's work to a step (ORC-808): the returned step's execution
//! runs with it and with its model, for that return only; every other execution, and the run's snapshot, keep their
//! agents. Attempts and decisions recorded before the choice existed read as having none.

use orchester_core::{
  id::{ResourceId, VersionId},
  resource::VersionNumber,
  run::{
    Answerer, AttemptReason, ChosenProfile, PinnedVersion, ReviewNote, Run, RunEventKind, RunStatus, StepExecutor,
    StuckDecision,
  },
};

use crate::{
  fixture::{Logged, at, key, model, stored},
  steps::{fail_checks, implemented},
};

fn note() -> ReviewNote {
  ReviewNote::new("Rename it.").expect("a note")
}

fn chosen() -> ChosenProfile {
  ChosenProfile {
    profile: PinnedVersion {
      resource: ResourceId::generate(),
      version: VersionId::generate(),
      number: VersionNumber::new(std::num::NonZeroU32::new(4).expect("positive")),
    },
    model: model("model-x"),
  }
}

fn succeeded() -> Logged {
  let mut logged = Logged::started();
  logged.through_plan();
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  logged
}

/// What the snapshot says `step` runs with.
fn snapshot_agent(run: &Run, step: &str) -> (PinnedVersion, String) {
  match run.snapshot().step(&key(step)).expect("a step").executor() {
    StepExecutor::Agent { profile, model, .. } => (*profile, model.as_str().to_owned()),
    StepExecutor::Tool { .. } | StepExecutor::Test => panic!("an agent step"),
  }
}

#[test]
fn a_return_after_the_end_runs_the_returned_step_with_the_chosen_profile_only() {
  let mut logged = succeeded();
  let before = snapshot_agent(&logged.run, "implement");
  let chosen = chosen();
  logged.apply(|run| {
    run.return_ended(
      &key("implement"),
      (&note(), None),
      Some(chosen.clone()),
      Answerer::Person,
      at(50),
    )
  });
  let implement = logged.exec("implement");
  assert_eq!(
    logged.run.executing_agent(implement),
    Some((&chosen.profile, &chosen.model)),
    "the returned step runs with the chosen profile and its model"
  );
  let checks = logged.exec("checks");
  let (profile, model) = snapshot_agent(&logged.run, "checks");
  assert_eq!(
    logged
      .run
      .executing_agent(checks)
      .map(|(profile, model)| (*profile, model.as_str())),
    Some((profile, model.as_str())),
    "the steps after it keep their agents"
  );
  assert_eq!(
    snapshot_agent(&logged.run, "implement"),
    before,
    "the run's snapshot is not changed"
  );
  let earlier = logged.run.attempts()[0].executions()[0].id();
  assert_eq!(
    logged.run.executing_agent(earlier).map(|(profile, _)| profile.resource),
    Some(snapshot_agent(&logged.run, "research").0.resource),
    "an execution of an earlier attempt keeps its own"
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run));
}

#[test]
fn a_return_without_a_choice_keeps_the_snapshots_agent() {
  let mut logged = succeeded();
  logged.apply(|run| run.return_ended(&key("implement"), (&note(), None), None, Answerer::Person, at(50)));
  let implement = logged.exec("implement");
  let (profile, model) = snapshot_agent(&logged.run, "implement");
  assert_eq!(
    logged
      .run
      .executing_agent(implement)
      .map(|(profile, model)| (*profile, model.as_str())),
    Some((profile, model.as_str()))
  );
}

#[test]
fn a_later_return_does_not_keep_the_earlier_choice() {
  let mut logged = succeeded();
  let chosen = chosen();
  logged.apply(|run| {
    run.return_ended(
      &key("implement"),
      (&note(), None),
      Some(chosen.clone()),
      Answerer::Person,
      at(50),
    )
  });
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  logged.apply(|run| run.return_ended(&key("implement"), (&note(), None), None, Answerer::Person, at(60)));
  let implement = logged.exec("implement");
  let (profile, _) = snapshot_agent(&logged.run, "implement");
  assert_eq!(
    logged.run.executing_agent(implement).map(|(profile, _)| *profile),
    Some(profile)
  );
}

#[test]
fn a_stuck_run_returned_with_a_choice_records_it_in_the_decision_and_the_attempt() {
  let mut logged = implemented();
  fail_checks(&mut logged);
  logged.pass("implement");
  fail_checks(&mut logged);
  logged.pass("implement");
  fail_checks(&mut logged);
  let chosen = chosen();
  let decision = StuckDecision::ReturnTo {
    step: key("implement"),
    note: note(),
    profile: Some(chosen.clone()),
  };
  logged.apply(|run| run.decide_stuck(&decision, at(80)));
  assert!(logged.log.iter().any(|event| matches!(event.kind(),
    RunEventKind::StuckDecided { decision: kept, .. } if *kept == decision)));
  let reason = logged.run.current_attempt().expect("the attempt").reason().clone();
  assert_eq!(
    reason,
    AttemptReason::Returned {
      failed_step: key("checks"),
      return_to: key("implement"),
      note: note(),
      profile: Some(chosen.clone()),
    }
  );
  let implement = logged.exec("implement");
  assert_eq!(
    logged.run.executing_agent(implement),
    Some((&chosen.profile, &chosen.model))
  );
  let checks = logged.exec("checks");
  let (profile, _) = snapshot_agent(&logged.run, "checks");
  assert_eq!(
    logged.run.executing_agent(checks).map(|(profile, _)| *profile),
    Some(profile)
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn data_recorded_before_the_choice_existed_reads_as_having_none() {
  let attempt = r#"{"returned_after_end":{"return_to":"implement","note":"Rename it.","conflicts":null}}"#;
  let reason: AttemptReason = serde_json::from_str(attempt).expect("an old attempt reason");
  assert!(
    matches!(reason, AttemptReason::ReturnedAfterEnd { profile: None, .. }),
    "{reason:?}"
  );
  let returned = r#"{"returned":{"failed_step":"checks","return_to":"plan","note":"Again."}}"#;
  let reason: AttemptReason = serde_json::from_str(returned).expect("an old attempt reason");
  assert!(
    matches!(reason, AttemptReason::Returned { profile: None, .. }),
    "{reason:?}"
  );
  let decision = r#"{"kind":"return_to","step":"plan","note":"Again."}"#;
  let decision: StuckDecision = serde_json::from_str(decision).expect("an old decision");
  assert!(
    matches!(decision, StuckDecision::ReturnTo { profile: None, .. }),
    "{decision:?}"
  );
  let written = serde_json::to_string(&decision).expect("serializes");
  assert!(
    !written.contains("profile"),
    "a return without a choice writes none: {written}"
  );
}
