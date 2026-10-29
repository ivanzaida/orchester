//! The order in which the bar gives way on a narrow window (`Narrow` in `Status bar · redesign`): words go first,
//! icons, actions and the health dot stay.

use super::*;

fn several_at_once() -> BarFacts {
  let tasks = ProjectTasks {
    auto_approving: Arc::new(vec![auto_approving()]),
    ..ProjectTasks::default()
  };
  BarFacts {
    mcp: McpStatus {
      kind: McpKind::Qa {
        folder: PathBuf::from(".tmp/qa/orc-242/mcp"),
        file: None,
      },
      ..listening()
    },
    ..facts(Some(kontur(runner(RunnerState::Online), tasks)))
  }
}

/// `25 · Narrow · several at once` (`vtjRN`): on a narrow bar the window control's words give way first; *Stop*,
/// MCP and the dot stay.
#[test]
fn status_gives_way_in_its_order_when_narrow() {
  let mut wide = Bar::of(several_at_once());
  assert!(wide.width("status-window-control") > 0.0);
  let mut narrow = Bar::at(520, several_at_once());
  for id in ["status-auto-approving-stop", "status-mcp", "status-health"] {
    assert!(narrow.width(id) > 0.0, "{id} never gives way");
  }
  assert!(
    narrow.width("status-window-control") < wide.width("status-window-control"),
    "the window control's words give way first"
  );
}

/// The health dot never gives way, at any width.
#[test]
fn status_health_dot_never_gives_way() {
  for width in (300..=1440).rev().step_by(20) {
    let mut bar = Bar::at(width, several_at_once());
    assert!(bar.width("status-health") > 0.0, "at {width}");
  }
}
