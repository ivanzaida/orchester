//! Narrow windows (state R, `VTnmr`): whatever the plan limits item shows, the right group never
//! covers the activity, which gives way with an ellipsis.

use lurq::layout::quad::QuadContent;
use orchester_core::{
  connection::{
    AgentProvider,
    plan_limits::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
  },
  time::Timestamp,
};
use orchester_desktop::plan_limits::Connections;

use super::{
  plan_limits::{cli, with_bar},
  *,
};

const WIDTHS: [u32; 5] = [1280, 1279, 1120, 1119, 960];
/// The room the right group leaves at 960 with percentages (`VTnmr`), less a little for the
/// varying width of reset times: no class leaves the activity less.
const ACTIVITY_ROOM: f32 = 330.0;
/// An activity item narrower than its padding, icon and an ellipsis has no label left.
const SQUEEZED: f32 = 40.0;

/// A CLI's weekly window at `percent`, reached at 100 % or refused for `blocked`.
fn reading(provider: AgentProvider, percent: f64, blocked: Option<LimitBlock>) -> LimitReading {
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + 3 * 86_400_000);
  LimitReading {
    provider,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(10080, Some(percent), Some(reset), LimitState::Normal, false).expect("window")],
      plan: None,
      credits: None,
      blocked: blocked.map(|reason| Blocked { reason, window: None }),
    },
  }
}

/// The item's width classes: percentages, one CLI reached or refused, both.
fn classes() -> Vec<(&'static str, PlanReadings)> {
  let (codex, claude) = (AgentProvider::CodexCli, AgentProvider::ClaudeCodeCli);
  let credits = Some(LimitBlock::Credits);
  [
    ("normal", [reading(codex, 65.0, None), reading(claude, 42.0, None)]),
    ("reached", [reading(codex, 100.0, None), reading(claude, 42.0, None)]),
    ("blocked", [reading(codex, 40.0, credits), reading(claude, 42.0, None)]),
    (
      "both reached",
      [reading(codex, 100.0, None), reading(claude, 100.0, None)],
    ),
    (
      "both blocked",
      [reading(codex, 40.0, credits), reading(claude, 100.0, None)],
    ),
  ]
  .into_iter()
  .map(|(name, list)| {
    let mut readings = PlanReadings::default();
    for reading in list {
      readings.observe(reading);
    }
    (name, readings)
  })
  .collect()
}

/// The runner of `root` after it could not start: the activity slot then says how many runs
/// were interrupted (state G), with its actions. A runner that is online cannot be reached
/// headless; every activity item gives way the same way.
fn failed_runner(root: &std::path::Path) -> orchester_desktop::runner::RunnerStatus {
  let runtime = Runtime::new().expect("tokio runtime");
  let runners = Runners::new(
    root.join("no-such-runner"),
    runtime.handle().clone(),
    Activity::new(),
    SessionEnd::Clean,
  );
  runners.ensure(root, "Kontur");
  let deadline = Instant::now() + std::time::Duration::from_secs(20);
  loop {
    let status = runners.status_of(root).expect("started");
    if status.state().is_problem() {
      return status;
    }
    assert!(
      Instant::now() < deadline,
      "the runner never failed: {:?}",
      status.state()
    );
    std::thread::sleep(std::time::Duration::from_millis(20));
  }
}

#[test]
fn status_bar_narrow_right_group_never_covers_the_activity() {
  let folder = tempfile::tempdir().expect("project folder");
  let root = folder.path().to_path_buf();
  let runner = failed_runner(&root);
  let mut cut = Vec::new();
  for (state, readings) in classes() {
    for width in WIDTHS {
      let connections = Connections::Ready(AgentProvider::CLI.into_iter().map(cli).collect());
      let mut bar = project(Some(2));
      let project = bar.project.as_mut().expect("project");
      project.connections = connections;
      project.repo_root = root.clone();
      project.runner = runner.clone();
      project.lost_to_runner = Some(1_000_000_000);
      let (_app, mut tree) = with_bar(width, readings.clone(), bar);
      let activity = bounds(&mut tree, "status-activity");
      let agents = bounds(&mut tree, "status-agents");
      assert!(
        activity.x + activity.width <= agents.x,
        "{state} at {width}: the activity ends at {} past the right group at {}",
        activity.x + activity.width,
        agents.x
      );
      let full = "1000000000 runs interrupted · nothing runs until the runner starts";
      let layout = tree.last_layout().expect("laid out").clone();
      let label = tree
        .resolve_quads(&layout)
        .into_iter()
        .find_map(|quad| match quad.content {
          QuadContent::Text { text, .. }
            if (text.starts_with('1') || text == "…") && full.starts_with(text.trim_end_matches('…')) =>
          {
            Some((quad.x, quad.width, text))
          }
          _ => None,
        });
      // With the runner's two actions beside it, the label is the first to give way: it may be cut
      // to nothing, but is never drawn under the right group or cut without an ellipsis.
      let Some((x, label_width, text)) = label else {
        // Its padding and icon (30px) are all that is left: no room for even an ellipsis.
        assert!(
          activity.width < SQUEEZED,
          "{state} at {width}: an activity with room draws its label"
        );
        continue;
      };
      assert!(
        x + label_width <= agents.x,
        "{state} at {width}: the label runs into the right group"
      );
      if text != full {
        cut.push(format!("{state} {width}"));
      }
      assert!(
        text == full || text.ends_with('…'),
        "{state} at {width}: a cut activity ends with an ellipsis, got {text:?}"
      );
    }
  }
  assert!(
    cut.len() >= 5,
    "the label is cut with an ellipsis in most cases, only in {cut:?}"
  );
}

#[test]
fn status_bar_narrow_every_class_leaves_the_activity_its_room() {
  let mut widths: Vec<u32> = (960..=1600).step_by(40).collect();
  widths.extend([1119, 1279, 1439, 1599]);
  for (state, readings) in classes() {
    for &width in &widths {
      let mut bar = project(Some(2));
      bar.project.as_mut().expect("project").connections =
        Connections::Ready(AgentProvider::CLI.into_iter().map(cli).collect());
      let (_app, mut tree) = with_bar(width, readings.clone(), bar);
      let agents = bounds(&mut tree, "status-agents");
      assert!(
        agents.x >= ACTIVITY_ROOM,
        "{state} at {width}: the right group starts at {}",
        agents.x
      );
    }
  }
}
