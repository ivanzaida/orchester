//! A module's progress and burndown rules in core (`orchester_core::module::{progress, burndown}`), which the desktop
//! and a team server's module summaries share: Canceled tasks are out of scope, Done counts as done, and every move of
//! a task into or out of the module changes its scope on that day; and the delivery forecast they rest on.

use chrono::NaiveDate;
use orchester_core::{
  forecast::{Forecast, History, Outlook, SEED},
  id::TaskId,
  module::{
    burndown::{Burndown, ChartDays, TaskTrack},
    measure::{Measure, WorkUnit},
    progress::Progress,
  },
  name::Name,
  project::{ProjectKey, TaskKey},
  task::{
    Task, TaskDetails, TaskLimits,
    organization::{TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
  usage::day_start,
};

fn day(day: u32) -> NaiveDate {
  NaiveDate::from_ymd_opt(2026, 11, day).expect("a November day")
}

/// `hour` o'clock local time on November `on`.
fn at(on: u32, hour: i64) -> Timestamp {
  Timestamp::from_unix_millis(day_start(day(on)).unix_millis() + hour * 3_600_000)
}

fn task(number: u32, status: WorkflowStatus) -> Task {
  let details = TaskDetails {
    title: Name::new("Task").expect("title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let key = TaskKey::new(
    ProjectKey::new("KON").expect("key"),
    std::num::NonZeroU32::new(number).expect("number"),
  );
  Task::draft(TaskId::generate(), key, details, at(1, 9))
    .with_organization(TaskOrganization::default().with_status(status))
}

fn track(membership: Vec<(Timestamp, bool)>, statuses: Vec<(Timestamp, WorkflowStatus)>) -> TaskTrack {
  TaskTrack {
    task: TaskId::generate(),
    weight: 1,
    membership,
    first_status: WorkflowStatus::Todo,
    statuses,
  }
}

#[test]
fn module_rules_progress_leaves_canceled_out_and_counts_done() {
  let tasks = [
    task(1, WorkflowStatus::Done),
    task(2, WorkflowStatus::InProgress),
    task(3, WorkflowStatus::Canceled),
    task(4, WorkflowStatus::Backlog),
  ];
  let measure = Measure::of(&tasks);
  let progress = Progress::of(&tasks, &measure);
  assert_eq!(
    progress,
    Progress {
      unit: WorkUnit::Tasks,
      scope: 3,
      done: 1
    }
  );
  assert_eq!((progress.remaining(), progress.percent()), (2, Some(33)));
  let canceled = [task(5, WorkflowStatus::Canceled)];
  let none = Progress::of(&canceled, &Measure::of(&canceled));
  assert_eq!(
    (none.scope, none.percent()),
    (0, None),
    "a module of canceled tasks has no work in scope"
  );
}

#[test]
fn module_rules_burndown_tracks_moves_and_cancels() {
  let tracks = [
    track(vec![(at(1, 9), true)], vec![(at(2, 12), WorkflowStatus::Done)]),
    // Moved out on the 2nd, back in on the 3rd.
    track(vec![(at(1, 9), true), (at(2, 10), false), (at(3, 10), true)], vec![]),
    // Canceled on the 3rd: out of scope, not done.
    track(vec![(at(1, 9), true)], vec![(at(3, 15), WorkflowStatus::Canceled)]),
  ];
  let days = ChartDays {
    first: day(1),
    target: Some(day(4)),
    today: day(4),
  };
  let burndown = Burndown::of(&tracks, WorkUnit::Tasks, days);
  let shown: Vec<(Option<u32>, Option<u32>, u32, u32)> = burndown
    .days
    .iter()
    .map(|day| (day.scope, day.remaining, day.added, day.removed))
    .collect();
  assert_eq!(
    shown,
    [
      (Some(3), Some(3), 3, 0),
      (Some(2), Some(1), 0, 1),
      (Some(2), Some(1), 1, 1),
      (Some(2), Some(1), 0, 0),
    ]
  );
  assert_eq!(burndown.start_scope, 3);
  assert_eq!(burndown.days[3].ideal, Some(0.0));
}

#[test]
fn module_rules_forecast_lines_follow_the_outlook() {
  let tracks = [track(vec![(at(1, 9), true)], vec![])];
  let days = ChartDays {
    first: day(1),
    target: None,
    today: day(2),
  };
  let burndown = Burndown::of(&tracks, WorkUnit::Tasks, days);
  let steady = History::new(vec![1; 20]);
  let forecast = Forecast::of(1, &steady, day(2), SEED);
  let Outlook::Dates { likely, safe, .. } = forecast.outlook else {
    panic!("a steady history forecasts dates");
  };
  assert_eq!((likely, safe), (day(3), day(3)));
  let lines = burndown.forecast_days(&forecast);
  assert_eq!(lines.last().map(|line| line.date), Some(day(3)));
  assert_eq!(
    lines[1].likely,
    Some(1.0),
    "the lines start from the latest day's remaining work"
  );
  assert_eq!(lines[0].likely, None, "and not before it");
  let unknown = Forecast::of(1, &History::new(vec![1; 3]), day(2), SEED);
  assert_eq!(unknown.outlook, Outlook::NotEnoughHistory);
  assert!(
    burndown
      .forecast_days(&unknown)
      .iter()
      .all(|line| line.likely.is_none())
  );
}
