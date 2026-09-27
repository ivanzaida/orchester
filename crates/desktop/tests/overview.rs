//! The project overview (02.08 `Twtqc`) as data: the pulse, the task tabs, the footer and the
//! recent run activity, counted from the project's tasks and their latest runs.

mod support;

use orchester_core::{
  run::{PlanVersion, TaskPhase, WaitReason},
  store::RunStore,
};
use orchester_desktop::tasks::{
  data::ProjectTasks,
  list::AwaitedPlan,
  overview::{OverviewTab, Pulse, activity_empty, figure, footer, recent_runs},
};

/// A project with a completed, a running and a draft task.
fn project() -> (support::Fixture, ProjectTasks) {
  let mut fixture = support::Fixture::new();
  let done = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(done, support::at(20));
  fixture.succeed(&mut run, support::at(30));
  let running = fixture.task("Fix zoom", support::at(40));
  fixture.started_run(running, support::at(50));
  fixture.task("Write docs", support::at(60));
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  (fixture, tasks)
}

#[test]
fn overview_pulse_counts_running_waiting_and_completed_today() {
  let (_fixture, tasks) = project();
  let pulse = Pulse::of(&tasks.phases, support::at(25));
  assert_eq!(pulse.running, 1);
  assert_eq!(pulse.needs_input, 0);
  assert_eq!(pulse.completed_today, 1);
  // Completed before the day began does not count as today.
  assert_eq!(Pulse::of(&tasks.phases, support::at(35)).completed_today, 0);
}

/// Counts are the plain number everywhere (decided 2026-09-27): never zero-padded.
#[test]
fn overview_figures_are_plain_numbers() {
  assert_eq!(figure(0), "0");
  assert_eq!(figure(2), "2");
  assert_eq!(figure(12), "12");
  assert_eq!(figure(128), "128");
}

/// The Overview metric *Need your input*, its *Needs you* heading and tab, and the status bar's
/// *Needs you* count the same runs.
#[test]
fn overview_needs_you_counts_runs_waiting_for_the_user() {
  let mut fixture = support::Fixture::new();
  let asking = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(asking, support::at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run
    .wait_step(execution, WaitReason::Message, support::at(30))
    .expect("the step waits for a message");
  fixture.db.append_events(&run, &events).expect("append");
  let working = fixture.task("Fix zoom", support::at(40));
  fixture.started_run(working, support::at(50));
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");

  let pulse = Pulse::of(&tasks.phases, support::at(0));
  assert_eq!(pulse.needs_input, 1);
  assert_eq!(pulse.running, 1);
  assert_eq!(
    OverviewTab::NeedsYou.count(&tasks.tasks, &tasks.phases),
    pulse.needs_input
  );
}

#[test]
fn overview_pulse_without_a_runner_counts_interrupted_runs() {
  let (_fixture, tasks) = project();
  let context = Pulse::running_context(true, Some(2), 0);
  assert_eq!(context.key, "overview.metrics.agentsWorking.other");
  assert_eq!(context.args, vec![("count", "2".to_owned())]);
  assert_eq!(
    Pulse::running_context(true, None, 0).key,
    "overview.metrics.agentsUnknown"
  );

  assert_eq!(tasks.lost_to_runner, 0);
  let away = tasks.without_runner();
  assert_eq!(away.lost_to_runner, 1);
  assert_eq!(Pulse::of(&away.phases, support::at(0)).running, 0);
  let context = Pulse::running_context(false, None, away.lost_to_runner);
  assert_eq!(context.key, "overview.metrics.interrupted.one");
  assert_eq!(context.args, vec![("count", "1".to_owned())]);
}

#[test]
fn overview_tabs_filter_newest_first_and_count() {
  let (_fixture, tasks) = project();
  let titles = |tab: OverviewTab| {
    tab
      .rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, 6)
      .iter()
      .map(|row| row.title.to_string())
      .collect::<Vec<_>>()
  };
  assert_eq!(titles(OverviewTab::All), ["Write docs", "Fix zoom", "Add login"]);
  assert_eq!(titles(OverviewTab::Running), ["Fix zoom"]);
  assert!(titles(OverviewTab::NeedsYou).is_empty());
  assert_eq!(OverviewTab::All.count(&tasks.tasks, &tasks.phases), 3);
  assert_eq!(OverviewTab::Queued.count(&tasks.tasks, &tasks.phases), 0);
  let footer = footer(&tasks.tasks, &tasks.phases);
  assert_eq!(footer.key, "overview.footer.one");
  assert_eq!(
    footer.args,
    vec![("active", "1".to_owned()), ("completed", "1".to_owned())]
  );
}

#[test]
fn overview_activity_lists_the_latest_runs_newest_first() {
  let (_fixture, tasks) = project();
  let recent = recent_runs(&tasks.tasks, &tasks.phases, 3);
  assert_eq!(
    recent.iter().map(|run| run.phase).collect::<Vec<_>>(),
    [TaskPhase::Running, TaskPhase::Succeeded]
  );
  assert_eq!(recent[0].title().key, "overview.activity.running");
  assert_eq!(recent[1].title().key, "overview.activity.succeeded");
  assert_eq!(recent[1].title().args, vec![("run", "1".to_owned())]);
  assert_eq!(recent_runs(&tasks.tasks, &tasks.phases, 1).len(), 1);
}

/// A run waiting on a plan decision counts in Needs you, and its row names the plan that waits.
#[test]
fn overview_needs_you_names_the_plan_that_waits() {
  let mut fixture = support::Fixture::gated();
  let task = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(task, support::at(20));
  fixture.propose(&mut run, "1. Add the provider.", support::at(30));
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  assert_eq!(Pulse::of(&tasks.phases, support::at(0)).needs_input, 1);
  let rows = OverviewTab::NeedsYou.rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, 3);
  assert_eq!(rows.len(), 1);
  assert_eq!(
    rows[0].plan,
    Some(AwaitedPlan {
      version: PlanVersion::FIRST,
      agent: "Builder".into(),
    })
  );
  assert_eq!(rows[0].detail.key, "tasks.progress.waiting");
  // No agent works for it, so it still waits while no runner is connected.
  let offline = tasks.without_runner();
  assert_eq!(offline.lost_to_runner, 0);
  assert_eq!(OverviewTab::NeedsYou.count(&offline.tasks, &offline.phases), 1);
}

/// A task whose run was merged into its branch reads as Merged and counts as completed, like one
/// that only succeeded.
#[test]
fn overview_counts_a_merged_task_as_completed() {
  use orchester_core::{
    integration::{MergeKind, RunMerge},
    revision::Revision,
    run::BranchName,
    store::MergeStore,
  };
  let (mut fixture, _) = project();
  let merged = fixture.task("Add sign-out", support::at(70));
  let mut run = fixture.started_run(merged, support::at(80));
  fixture.succeed(&mut run, support::at(90));
  let merge = RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("valid branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: support::at(95),
  };
  fixture.db.save_run_merge(&merge).expect("record the merge");
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  let latest = &tasks.phases[&merged];
  assert_eq!(latest.phase(), Some(TaskPhase::Merged));
  assert_eq!(latest.message().key, "tasks.progress.merged");
  assert_eq!(latest.message().args, [("branch", "main".to_owned())]);
  assert_eq!(Pulse::of(&tasks.phases, support::at(25)).completed_today, 2);
  let footer = footer(&tasks.tasks, &tasks.phases);
  assert!(footer.args.contains(&("completed", "2".to_owned())), "{footer:?}");
}

#[test]
fn overview_activity_without_runs_says_there_are_none_yet() {
  let mut fixture = support::Fixture::new();
  fixture.task("Write docs", support::at(10));
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  assert!(recent_runs(&tasks.tasks, &tasks.phases, 3).is_empty());
  assert_eq!(activity_empty(&tasks.phases).key, "overview.activity.empty");
}
