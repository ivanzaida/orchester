use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  run::{AcceptRefusal, GitMessage, PreflightProblem, RunLimit, WorktreeFailure},
};
use orchester_ipc::Rejection;
use orchester_workspace::{
  i18n::{FALLBACK_LOCALE, LOCALES},
  runner::{RequestError, RunRequestError},
  tasks::run::{StartOutcome, control_message, preflight_message, start_messages, worktree_failure_message},
};
use serde_json::Value;

#[test]
fn preflight_unpinned_queued_pipeline_has_a_specific_message() {
  let message = preflight_message(&PreflightProblem::PipelineVersionNotPinned);
  assert_eq!(message.key, "runs.preflight.pipelineVersionNotPinned");
}

#[test]
fn uncertain_start_stays_uncertain_after_later_definite_rejections() {
  let lost = RunRequestError::Request(RequestError::Lost);
  let preflight = RunRequestError::Rejected(Rejection::Preflight { problems: vec![] });
  let no_slot = RunRequestError::Rejected(Rejection::NoAgentSlot {
    slots: NonZeroU32::new(1).expect("one agent slot"),
  });
  let initial = StartOutcome::ConfirmedNotStarted;
  assert_eq!(initial.after(&preflight), initial);
  let uncertain = initial.after(&lost);
  assert_eq!(uncertain, StartOutcome::Unconfirmed);
  assert_eq!(uncertain.status_key(), "tasks.new.unconfirmed");
  assert_eq!(uncertain.description_key(), "tasks.new.unconfirmedDescription");
  assert_eq!(uncertain.after(&preflight), uncertain);
  assert_eq!(uncertain.after(&no_slot), uncertain);
  let failed = RunRequestError::Rejected(Rejection::Failed {
    detail: "runner failed after creating a run".to_owned(),
  });
  assert_eq!(initial.after(&failed), uncertain);
}

#[test]
fn uncertain_start_copy_does_not_claim_the_run_never_started() {
  let (_, contents) = LOCALES
    .iter()
    .find(|(locale, _)| *locale == FALLBACK_LOCALE)
    .expect("fallback locale");
  let locale: Value = serde_json::from_str(contents).expect("locale JSON");
  let status = locale
    .pointer("/tasks/new/unconfirmed")
    .and_then(Value::as_str)
    .expect("uncertain status");
  let description = locale
    .pointer("/tasks/new/unconfirmedDescription")
    .and_then(Value::as_str)
    .expect("uncertain description");
  let lost = locale
    .pointer("/runs/start/lost")
    .and_then(Value::as_str)
    .expect("lost reply");
  assert!(status.contains("unconfirmed"));
  assert!(description.contains("whether its run started"));
  assert!(lost.contains("may have started"));
  assert!(!status.contains("run not started"));
}

#[test]
fn preflight_a_refused_acceptance_says_why_in_the_persons_words() {
  let (_, contents) = LOCALES
    .iter()
    .find(|(locale, _)| *locale == FALLBACK_LOCALE)
    .expect("fallback locale");
  let locale: Value = serde_json::from_str(contents).expect("locale JSON");
  for (why, key) in [
    (AcceptRefusal::NotJudged, "notJudged"),
    (
      AcceptRefusal::NotBuiltOn {
        revision: "1c96380a2b4d".to_owned(),
      },
      "notBuiltOn",
    ),
    (AcceptRefusal::Uncommitted, "uncommitted"),
    (
      AcceptRefusal::WorktreeRemoved {
        branch: "orchester/kon-5-run-1--review-1".to_owned(),
      },
      "worktreeRemoved",
    ),
  ] {
    let message = control_message(&RunRequestError::Rejected(Rejection::NotAcceptable { why }));
    assert_eq!(message.key, format!("runs.control.notAcceptable.{key}"));
    assert!(
      locale
        .pointer(&format!("/runs/control/notAcceptable/{key}"))
        .is_some_and(Value::is_string),
      "{key} has words"
    );
  }
}

#[test]
fn a_run_that_could_not_start_says_why_wherever_it_was_started() {
  let why = WorktreeFailure::Git(GitMessage::new("fatal: disk full"));
  let error = RunRequestError::Rejected(Rejection::RunNotStarted {
    run: RunId::generate(),
    why: why.clone(),
  });
  // The new-task form, Run again and `run_task` read the start messages: what happened, then why.
  let said = start_messages(&error);
  assert_eq!(said[0].key, "runs.start.worktreeNotCreated");
  assert_eq!(said[1], worktree_failure_message(&why));
  assert_eq!(said[1].key, "runs.notStarted.git");
  assert_eq!(said[1].args, vec![("detail", "fatal: disk full".to_owned())]);
  // The task's Start run reads the control one; its Summary says why.
  assert_eq!(control_message(&error).key, "runs.start.worktreeNotCreated");
  assert_eq!(
    worktree_failure_message(&WorktreeFailure::NotIgnored).key,
    "runs.notStarted.notIgnored"
  );
  let silent = worktree_failure_message(&WorktreeFailure::Git(GitMessage::new(
    "Preparing worktree (new branch 'x')",
  )));
  assert_eq!(
    silent.key, "runs.notStarted.gitSilent",
    "no \"Git refused:\" with nothing after it"
  );
  assert!(silent.args.is_empty());
}

#[test]
fn a_limit_not_above_what_the_run_used_says_which() {
  let execution = orchester_core::id::StepExecutionId::generate();
  let said = |limit| control_message(&RunRequestError::Rejected(Rejection::LimitNotAboveUsed { limit })).key;
  assert_eq!(said(RunLimit::Tokens), "runs.limit.tokensNotAboveUsed");
  assert_eq!(said(RunLimit::Time), "runs.limit.timeNotAboveUsed");
  assert_eq!(said(RunLimit::StepBudget { execution }), "runs.limit.stepNotAboveUsed");
  for limit in [
    RunLimit::PlanLimit { execution },
    RunLimit::DailyBudget,
    RunLimit::GroupBudget,
  ] {
    assert_eq!(said(limit), "runs.control.failed");
  }
}
