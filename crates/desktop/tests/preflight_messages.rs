use std::{num::NonZeroU32, path::PathBuf};

use orchester_core::run::PreflightProblem;
use orchester_desktop::{
  i18n::{FALLBACK_LOCALE, LOCALES},
  runner::{RequestError, RunRequestError},
  tasks::run::{StartOutcome, preflight_message},
};
use orchester_ipc::Rejection;
use serde_json::Value;

const GIB: u64 = 1024 * 1024 * 1024;

#[test]
fn preflight_unpinned_queued_pipeline_has_a_specific_message() {
  let message = preflight_message(&PreflightProblem::PipelineVersionNotPinned);
  assert_eq!(message.key, "runs.preflight.pipelineVersionNotPinned");
}

#[test]
fn preflight_low_space_message_does_not_round_up_to_the_required_amount() {
  let target = PathBuf::from("/build/target");
  let message = preflight_message(&PreflightProblem::BuildSpaceLow {
    target: target.clone(),
    available: 10 * GIB - 1,
    required: 10 * GIB,
  });
  assert_eq!(message.key, "runs.preflight.buildSpaceLow");
  assert_eq!(
    message.args,
    vec![
      ("target", target.display().to_string()),
      ("available", "9.9".to_owned()),
      ("required", "10.0".to_owned()),
    ]
  );
}

#[test]
fn preflight_inaccessible_target_message_names_the_configured_folder() {
  let target = PathBuf::from("/build/target");
  let message = preflight_message(&PreflightProblem::BuildSpaceUnavailable { target: target.clone() });
  assert_eq!(message.key, "runs.preflight.buildSpaceUnavailable");
  assert_eq!(message.args, vec![("target", target.display().to_string())]);
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
