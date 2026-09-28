//! Tabs over live projects, on the real runner and `orchester-fake-codex`: a project stays open
//! while a tab shows it or its runner has work, so closing the last tab of an idle project stops
//! its runner and closing it during a run lets the run finish first; every tab of a project that
//! needs the user shows the attention dot; and inactive tabs cost nothing per frame.

use std::time::{Duration, Instant};

use orchester_core::{resource::Step, run::RunStatus};
use orchester_desktop::{activity::Target, messages::Message, runner::state::RunnerState};

use crate::{
  github::shows,
  harness::Desktop,
  tabs::{command, has},
  task_run::{agent_step, create_and_run, init_repository, latest_run, programs, run_finished, seed, seed_steps},
};

fn online(desktop: &Desktop) -> bool {
  desktop
    .runners
    .status_of(desktop.repo())
    .is_some_and(|status| status.state() == &RunnerState::Online)
}

fn stopped(desktop: &Desktop) -> bool {
  desktop.runners.status_of(desktop.repo()).is_none()
}

/// Kontur with its runner online, one step on the fake Codex CLI.
fn live() -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  desktop.wait_until("the runner online", |desktop| online(desktop));
  desktop
}

#[test]
fn tabs_runner_of_an_idle_project_stops_when_its_last_tab_closes() {
  let mut desktop = live();
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  assert!(online(&desktop), "another tab does not stop it");
  desktop.press("window-tab-close-1");
  desktop.wait_until("the runner stopped", |desktop| stopped(desktop));
  // Opening the project in a tab starts its runner again.
  desktop.wait_for("project-row-0");
  desktop.press("project-row-0");
  desktop.wait_until("the runner online again", |desktop| online(desktop));
}

#[test]
fn tabs_runner_with_work_keeps_running_until_idle() {
  let mut desktop = live();
  create_and_run(&mut desktop, "Work slowly.", "1400000");
  desktop.wait_until("the run at work", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Running)
  });
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  desktop.press("window-tab-close-1");
  assert!(
    !stopped(&desktop) || run_finished(desktop.repo()),
    "the runner keeps working with no tab"
  );
  desktop.wait_until("the runner stopped once idle", |desktop| stopped(desktop));
  assert!(
    run_finished(desktop.repo()),
    "the run finished instead of being interrupted: {:?}",
    latest_run(desktop.repo()).map(|run| run.status().clone())
  );
}

#[test]
fn tabs_show_the_attention_dot_of_a_project_that_needs_you() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        Step {
          approval_gate: true,
          ..agent_step(builder, "plan", "Plan", "Propose a plan.", &[])
        },
        agent_step(builder, "implement", "Implement", "Follow the plan.", &["plan"]),
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the plan waits for review", |desktop| has(desktop, "plan-review-title"));
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  desktop.wait_until("the dot on Kontur's tab", |desktop| {
    has(desktop, "window-tab-attention-1")
  });
  assert!(
    !has(&mut desktop, "window-tab-attention-2"),
    "All projects tabs never show it"
  );
  desktop.hover("window-tab-1");
  desktop.wait_until("the tooltip", |desktop| {
    desktop.tree.tick_timers();
    desktop.pass();
    has(desktop, "window-tab-tooltip")
  });
  assert!(shows(
    &mut desktop,
    "window-tab-tooltip",
    "Kontur · KON-1 · 1 needs you"
  ));
}

/// How long `passes` passes take while the session's activity changes before each, as a stream of
/// status updates does: every tab's status bar follows it.
fn frame_cost(desktop: &mut Desktop, passes: u32) -> Duration {
  let title = Message::new("statusBar.operations.title");
  let started = Instant::now();
  for _ in 0..passes {
    desktop.activity.record(None, title.clone(), Target::Nothing);
    desktop.tree.tick_futures();
    desktop.pass();
  }
  started.elapsed() / passes
}

/// Frame cost of a window whose active tab is All projects: alone; beside seven inactive tabs on
/// heavy pages, one of them a run at work; and beside seven inactive tabs on All projects. The
/// heavy and the light inactive tabs cost the same: an inactive page is not rendered, laid out or
/// painted. What eight tabs cost over one is the title bar's own seven more tabs. Numbers for the
/// report; run with `--ignored --nocapture`.
#[test]
#[ignore = "measures frame cost for the report"]
fn tabs_inactive_tabs_cost_nothing_per_frame() {
  // Where the numbers go when run with --nocapture; a second test in the process has one already.
  let _ = tracing_subscriber::fmt().with_test_writer().try_init();
  const PASSES: u32 = 1000;
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  desktop.wait_until("the runner online", |desktop| online(desktop));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("the run at work", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Running)
  });
  let run_page = frame_cost(&mut desktop, PASSES);
  for (page, ready) in [
    ("nav-tasks", "tasks-new-task"),
    ("nav-pipelines", "pipelines-new"),
    ("nav-settings", "settings-repository"),
    ("nav-agents", "agent-role"),
    ("nav-connections", "add-connection"),
    ("nav-tools", "tools-add"),
  ] {
    command(&mut desktop, "t", "KeyT", false);
    desktop.wait_for("project-row-0");
    desktop.press("project-row-0");
    desktop.wait_for(page);
    desktop.click(page);
    desktop.wait_until(ready, |desktop| has(desktop, ready) || has(desktop, "tools-empty-add"));
  }
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  assert!(!has(&mut desktop, "task-page"), "the run's tab is offstage");
  let heavy = frame_cost(&mut desktop, PASSES);
  for _ in 0..7 {
    command(&mut desktop, "1", "Digit1", false);
    command(&mut desktop, "w", "KeyW", false);
  }
  desktop.wait_for("all-projects");
  let alone = frame_cost(&mut desktop, PASSES);
  for _ in 0..7 {
    command(&mut desktop, "t", "KeyT", false);
  }
  desktop.wait_for("all-projects");
  let light = frame_cost(&mut desktop, PASSES);
  tracing::info!(
    ?run_page,
    ?heavy,
    ?light,
    ?alone,
    "frame cost with All projects active: the run's page itself; beside 7 inactive heavy tabs; beside 7 inactive All projects tabs; alone"
  );
  assert!(
    heavy.as_secs_f64() < light.as_secs_f64() * 1.5,
    "inactive pages cost per frame: {heavy:?} beside heavy tabs, {light:?} beside light ones"
  );
}
