//! The order in which the bar gives way on a narrow window.

use super::*;

/// The bar at 960 with everything shown: the version and Tokens today give way, then the counts' words;
/// the hand, the counts, *Review*, *Stop* and MCP stay.
#[test]
fn status_gives_way_in_its_order_at_960() {
  let tasks = ProjectTasks {
    pending_tool_approvals: Arc::new(vec![approval()]),
    auto_approving: Arc::new(vec![auto_approving()]),
    ..ProjectTasks::default()
  };
  let project = || {
    Some(kontur(
      RunnerFacts {
        work: Some(WorkSummary {
          agent_slots: NonZeroU32::new(4).expect("slots"),
          runs: Vec::new(),
        }),
        ..runner(RunnerState::Online)
      },
      tasks.clone(),
    ))
  };
  let mut wide = Bar::of(facts(project()));
  for id in ["status-version", "status-tokens"] {
    assert!(wide.width(id) > 0.0, "{id} shows at 1440");
  }
  let mut narrow = Bar::at(960, facts(project()));
  for id in ["status-version", "status-tokens"] {
    assert_eq!(narrow.width(id), 0.0, "{id} gives way at 960");
  }
  for id in [
    "status-needs-you-value",
    "status-agents-value",
    "status-queued-value",
    "status-approvals-review",
    "status-auto-approving-stop",
    "status-mcp",
  ] {
    assert!(narrow.width(id) > 0.0, "{id} never gives way");
  }
}

/// The counts' labels give way together: at any width all show or none does.
#[test]
fn status_count_labels_give_way_as_one_group() {
  let project = || Some(kontur(runner(RunnerState::Online), ProjectTasks::default()));
  let labels = ["status-agents-label", "status-queued-label", "status-needs-you-label"];
  let mut dropped_somewhere = false;
  for width in (300..=1440).rev().step_by(10) {
    let mut bar = Bar::at(width, facts(project()));
    let shown: Vec<bool> = labels.iter().map(|id| bar.width(id) > 0.0).collect();
    assert!(shown.iter().all(|shows| *shows == shown[0]), "at {width}: {shown:?}");
    dropped_somewhere |= !shown[0];
  }
  assert!(dropped_somewhere, "the labels give way somewhere above 300");
}
