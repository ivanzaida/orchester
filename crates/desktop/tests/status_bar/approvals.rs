//! Approvals in the status bar: the waiting requests with the oldest named, and, while a run
//! auto-approves, *Auto-approving* with *Stop*, which keeps its words when the bar is crowded.

use std::{num::NonZeroU32, sync::Arc, time::SystemTime};

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId, TaskId},
  run::{PendingToolApproval, RunNumber, SimilarUnavailable},
  time::Timestamp,
};
use orchester_desktop::{
  activity::{ActivityLog, OperationSpec, Target},
  messages::Message,
  status_bar::StatusBarProps,
  tasks::{
    approvals::{AlwaysBlocked, ApprovalChoices, ShownApproval},
    data::AutoApprovingRun,
  },
};

use super::{bounds, item_texts, laid_out, laid_out_with_log, project};

fn waiting(number: usize) -> ShownApproval {
  let invocation = InvocationId::generate();
  ShownApproval {
    approval: PendingToolApproval {
      id: format!("{invocation}:call_{number}"),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation,
      agent: "Designer".to_owned(),
      command: false,
      scope: None,
      server: "pencil".to_owned(),
      action: "batch_design".to_owned(),
      target: None,
      requested_at: Timestamp::from_unix_millis(0),
      native: None,
    },
    provider: None,
    vendor: None,
    model: None,
    read_only: false,
    run: Some(RunNumber::new(NonZeroU32::new(146).expect("above zero"))),
    task: Some("ORC-146".into()),
    task_id: Some(TaskId::generate()),
    step: Some("Design".into()),
    cwd: None,
    choices: ApprovalChoices {
      similar: Err(SimilarUnavailable::Unknown),
      always: Err(AlwaysBlocked::Missing),
      told: "The person reviewing this run denied pencil batch_design. Continue without it.".into(),
      codex: false,
      category: None,
    },
  }
}

fn with(approvals: Vec<ShownApproval>, auto: Vec<AutoApprovingRun>) -> StatusBarProps {
  let mut props = project(Some(approvals.len()));
  if let Some(bar) = props.project.as_mut() {
    bar.approvals = Arc::new(approvals);
    bar.auto_approving = Arc::new(auto);
  }
  props
}

#[test]
fn status_bar_approvals_name_the_oldest_and_count_the_others() {
  let (_app, mut tree) = laid_out(with((0..3).map(waiting).collect(), Vec::new()));
  let texts = item_texts(&mut tree, "status-approvals").join(" ");
  assert!(
    texts.starts_with("3 approvals waiting · Designer asks to use pencil batch_design · ORC-146"),
    "{texts}"
  );
  assert_eq!(item_texts(&mut tree, "status-needs-you"), ["Needs you", "3"]);
}

#[test]
fn status_bar_auto_approving_keeps_its_words_and_has_stop() {
  let auto = AutoApprovingRun {
    run: RunId::generate(),
    number: RunNumber::new(NonZeroU32::new(142).expect("above zero")),
    task: TaskId::generate(),
    key: "ORC-142".into(),
  };
  let (_app, mut tree) = laid_out(with((0..3).map(waiting).collect(), vec![auto]));
  assert_eq!(
    item_texts(&mut tree, "status-auto-approving"),
    ["Auto-approving · Run 142 · ORC-142"]
  );
  assert_eq!(item_texts(&mut tree, "status-auto-approving-stop"), ["Stop"]);
  let item = bounds(&mut tree, "status-auto-approving");
  let approvals = bounds(&mut tree, "status-approvals");
  assert!(
    approvals.x + approvals.width <= item.x + 0.5,
    "the approvals give way first"
  );
}

#[test]
fn status_bar_approvals_and_a_run_waiting_for_a_decision_show_together() {
  let mut log = ActivityLog::default();
  log.record_needs_you(
    Some("Kontur".into()),
    Message::new("statusBar.operations.projectCreate.running"),
    Target::Nothing,
    SystemTime::now(),
  );
  let (_app, mut tree) = laid_out_with_log(with((0..2).map(waiting).collect(), Vec::new()), log);
  assert!(
    tree.get_element_by_id("status-approvals").is_some(),
    "the approvals stay"
  );
  assert!(
    tree.get_element_by_id("status-activity").is_some(),
    "the waiting run stays"
  );
}

#[test]
fn status_bar_approvals_come_before_other_activity() {
  let mut log = ActivityLog::default();
  let spec = OperationSpec::new(
    Message::new("statusBar.operations.projectCreate.running"),
    Message::new("statusBar.operations.projectCreate.done"),
    Message::new("statusBar.operations.projectCreate.failed"),
  );
  log.start(spec, std::time::Instant::now());
  let (_app, mut tree) = laid_out_with_log(with(vec![waiting(0)], Vec::new()), log);
  assert!(tree.get_element_by_id("status-approvals").is_some());
  assert!(tree.get_element_by_id("status-activity").is_none());
}
