use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  run::{AcceptRefusal, PreflightProblem},
};
use orchester_ipc::Rejection;
use orchester_workspace::{
  i18n::{FALLBACK_LOCALE, LOCALES},
  runner::{RequestError, RunRequestError},
  tasks::run::{StartOutcome, control_message, preflight_message, start_messages},
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
  let why = "`git worktree add` failed (Some(128)): fatal: disk full".to_owned();
  let error = RunRequestError::Rejected(Rejection::RunNotStarted {
    run: RunId::generate(),
    why: why.clone(),
  });
  // The new-task form, Run again and `run_task` read the start messages; the task's Start run reads the control one.
  let mut said = start_messages(&error);
  said.push(control_message(&error));
  for message in said {
    assert_eq!(message.key, "runs.start.worktreeNotCreated");
    assert_eq!(message.args, vec![("reason", why.clone())]);
  }
}
