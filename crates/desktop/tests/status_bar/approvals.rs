//! Approvals in the status bar: the waiting requests with the oldest named, and, while a run
//! auto-approves, *Auto-approving* with *Stop*, which keeps its words when the bar is crowded.

use std::{num::NonZeroU32, sync::Arc};

use lurq::layout::quad::QuadContent;
use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId, TaskId},
  run::{PendingToolApproval, RunNumber, SimilarUnavailable},
  time::Timestamp,
};
use orchester_desktop::{
  status_bar::StatusBarProps,
  tasks::{
    approvals::{AlwaysBlocked, ApprovalChoices, ShownApproval},
    data::AutoApprovingRun,
  },
};

use super::{bounds, item_texts, laid_out, project};

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
    step_key: Some("design".into()),
    cwd: None,
    choices: ApprovalChoices {
      similar: Err(SimilarUnavailable::Unknown),
      always: Err(AlwaysBlocked::Missing),
      told: "The person reviewing this run denied pencil batch_design. Continue without it.".into(),
      codex: false,
      category: None,
      secrets: false,
      reason: None,
      auto_asks: false,
    },
    asking: None,
    auto_approving: false,
  }
}

fn with(approvals: Vec<ShownApproval>, auto: Vec<AutoApprovingRun>) -> StatusBarProps {
  let mut props = project(Some(approvals.len()));
  if let Some(bar) = props.project.as_mut() {
    bar.dialog = Arc::new(approvals.clone());
    bar.approvals = Arc::new(approvals);
    bar.auto_approving = Arc::new(auto);
  }
  props
}

#[test]
fn status_bar_approvals_name_the_oldest_and_count_the_others() {
  let (_app, mut tree) = laid_out(with((0..3).map(waiting).collect(), Vec::new()));
  // How many wait, then what the oldest asks, in parts that give way one after another.
  let texts = [
    item_texts(&mut tree, "status-approvals"),
    item_texts(&mut tree, "status-approvals-asks"),
  ]
  .concat()
  .concat();
  assert_eq!(
    texts,
    "3 approvals waiting · Designer asks to use pencil batch_design · ORC-146"
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
  // Which run it is shows whole or drops out; it is never cut.
  let (_app, mut tree) = laid_out(with((0..3).map(waiting).collect(), vec![auto.clone()]));
  assert_eq!(item_texts(&mut tree, "status-auto-approving"), ["Auto-approving"]);
  assert_eq!(item_texts(&mut tree, "status-auto-approving-stop"), ["Stop"]);
  let layout = tree.last_layout().expect("laid out").clone();
  assert!(
    !tree
      .resolve_quads(&layout)
      .into_iter()
      .any(|quad| matches!(quad.content, QuadContent::Text { ref text, .. } if text.starts_with(" · Run") && text.ends_with('…'))),
    "the run is not cut"
  );
  // A wide window names it whole.
  let bar = with((0..3).map(waiting).collect(), vec![auto]);
  let (_wide, wide) = super::plan_limits::with_bar(1920, Default::default(), bar);
  let layout = wide.last_layout().expect("laid out").clone();
  assert!(
    wide
      .resolve_quads(&layout)
      .into_iter()
      .any(|quad| matches!(quad.content, QuadContent::Text { ref text, .. } if text == " · Run 142 · ORC-142")),
    "the run is named whole"
  );
  let item = bounds(&mut tree, "status-auto-approving");
  let approvals = bounds(&mut tree, "status-approvals-time");
  assert!(
    approvals.x + approvals.width <= item.x + 0.5,
    "the approvals give way first"
  );
}

/// From 1700 down to 900, every 10px, with approvals waiting beside a run that auto-approves or not,
/// and every plan limits class: the bar never shows more at a narrower width. Each text it draws is
/// one it drew at the wider width, or that text cut shorter, and it draws no more texts than there.
#[test]
fn status_bar_never_shows_more_at_a_narrower_width() {
  use orchester_core::connection::AgentProvider;
  use orchester_desktop::plan_limits::Connections;
  let auto = AutoApprovingRun {
    run: RunId::generate(),
    number: RunNumber::new(NonZeroU32::new(3).expect("above zero")),
    task: TaskId::generate(),
    key: "KON-3".into(),
  };
  for (state, readings) in super::narrow::classes() {
    for runs in [vec![auto.clone()], Vec::new()] {
      let mut wider: Option<(u32, Vec<String>)> = None;
      for width in (900..=1700).rev().step_by(10) {
        let mut bar = with((0..2).map(waiting).collect(), runs.clone());
        bar.project.as_mut().expect("project").connections =
          Connections::Ready(AgentProvider::CLI.into_iter().map(super::plan_limits::cli).collect());
        let (_app, mut tree) = super::plan_limits::with_bar(width, readings.clone(), bar);
        let texts = super::narrow::drawn_texts(&mut tree);
        if let Some((at, before)) = &wider {
          assert!(
            texts.len() <= before.len(),
            "{state}, {} runs: {width} draws {texts:?}, more than {at}: {before:?}",
            runs.len()
          );
          for text in &texts {
            let shown = text.trim_end_matches('…');
            assert!(
              before
                .iter()
                .any(|earlier| earlier.trim_end_matches('…').starts_with(shown)),
              "{state}, {} runs: {width} draws {text:?}, which {at} did not: {before:?}",
              runs.len()
            );
          }
        }
        wider = Some((width, texts));
      }
    }
  }
}
