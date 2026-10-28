//! A module's burndown rules (ORC-48, `orchester_workspace::modules`): remaining work at the end of each local day,
//! the ideal line from the scope at the start to nothing at the target date, scope added and removed, Done from the
//! workflow status (which merges set) and a reopened task counting again; a module's page read from a project with a
//! merged run; and the module filter of the Tasks list.

mod support;

use std::sync::Arc;

use chrono::NaiveDate;
use orchester_core::{
  id::{ModuleId, TaskId},
  integration::{MergeKind, RunMerge},
  module::{Module, ModuleDetails, ModuleState},
  name::Name,
  project::{ProjectKey, TaskKey},
  revision::Revision,
  run::BranchName,
  store::{MergeStore, TaskStore},
  task::{
    Task, TaskDetails, TaskLimits,
    organization::{ChangeActor, WorkflowStatus},
  },
  time::Timestamp,
  usage::day_start,
};
use orchester_workspace::modules::{
  ModuleFilter,
  burndown::{Burndown, ChartDays, MAX_CHART_DAYS, TaskTrack},
  data::{ModulePage, summaries},
  form::ModuleForm,
  last_write::LastWrite,
  measure::WorkUnit,
};
use support::Fixture;

fn day(day: u32) -> NaiveDate {
  NaiveDate::from_ymd_opt(2026, 10, day).expect("an October day")
}

/// `hour` o'clock local time on October `on`.
fn at(on: u32, hour: i64) -> Timestamp {
  Timestamp::from_unix_millis(day_start(day(on)).unix_millis() + hour * 3_600_000)
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

fn chart(tracks: &[TaskTrack], (first, target, today): (u32, Option<u32>, u32)) -> Burndown {
  let days = ChartDays {
    first: day(first),
    target: target.map(day),
    today: day(today),
  };
  Burndown::of(tracks, WorkUnit::Tasks, days)
}

#[test]
fn modules_burndown_counts_remaining_work_and_the_ideal_line_per_day() {
  let tracks = [
    track(vec![(at(1, 9), true)], vec![(at(2, 15), WorkflowStatus::Done)]),
    track(vec![(at(1, 9), true)], vec![(at(3, 11), WorkflowStatus::Done)]),
    track(vec![(at(1, 10), true)], vec![]),
  ];
  let burndown = chart(&tracks, (1, Some(4), 4));
  assert_eq!(burndown.unit, WorkUnit::Tasks);
  assert_eq!(burndown.start_scope, 3);
  let remaining: Vec<Option<u32>> = burndown.days.iter().map(|day| day.remaining).collect();
  assert_eq!(remaining, [Some(3), Some(2), Some(1), Some(1)]);
  let ideal: Vec<Option<f64>> = burndown.days.iter().map(|day| day.ideal).collect();
  assert_eq!(ideal, [Some(3.0), Some(2.0), Some(1.0), Some(0.0)]);
  assert_eq!(
    burndown.days[0].added, 3,
    "what came in on the first day is added that day"
  );
  assert!(burndown.days[1..].iter().all(|day| day.added == 0 && day.removed == 0));
  assert_eq!(burndown.latest().map(|day| day.date), Some(day(4)));
}

#[test]
fn modules_burndown_shows_scope_added_removed_and_canceled() {
  let tracks = [
    track(vec![(at(1, 9), true)], vec![]),
    track(vec![(at(1, 9), true), (at(3, 12), false)], vec![]),
    track(vec![(at(1, 9), true)], vec![(at(2, 16), WorkflowStatus::Canceled)]),
    track(vec![(at(3, 8), true)], vec![]),
    // In and out on the same day: no change of scope that day.
    track(vec![(at(2, 8), true), (at(2, 18), false)], vec![]),
  ];
  let burndown = chart(&tracks, (1, None, 3));
  let scope: Vec<(Option<u32>, u32, u32)> = burndown
    .days
    .iter()
    .map(|day| (day.scope, day.added, day.removed))
    .collect();
  assert_eq!(scope, [(Some(3), 3, 0), (Some(2), 0, 1), (Some(2), 1, 1)]);
  assert!(
    burndown.days.iter().all(|day| day.ideal.is_none()),
    "no target, no ideal line"
  );
}

#[test]
fn modules_burndown_counts_a_reopened_task_again_and_leaves_days_to_come_open() {
  let tracks = [track(
    vec![(at(1, 9), true)],
    vec![
      (at(2, 10), WorkflowStatus::Done),
      (at(3, 10), WorkflowStatus::InProgress),
    ],
  )];
  let burndown = chart(&tracks, (1, Some(6), 3));
  assert_eq!(burndown.days.len(), 6, "the chart runs to the target date");
  let remaining: Vec<Option<u32>> = burndown.days.iter().map(|day| day.remaining).collect();
  assert_eq!(remaining, [Some(1), Some(0), Some(1), None, None, None]);
  assert_eq!(burndown.days[5].ideal, Some(0.0));
  assert_eq!(burndown.latest().map(|day| day.date), Some(day(3)));
}

#[test]
fn modules_burndown_takes_a_task_done_before_it_joined_as_done() {
  let mut done = track(vec![(at(2, 9), true)], vec![]);
  done.first_status = WorkflowStatus::Done;
  let burndown = chart(&[done], (1, Some(2), 2));
  assert_eq!(
    burndown.start_scope, 1,
    "the line starts on the day the module had work"
  );
  assert_eq!((burndown.days[1].scope, burndown.days[1].remaining), (Some(1), Some(0)));
  assert_eq!(burndown.days[1].added, 1);
}

#[test]
fn modules_burndown_starts_on_the_first_day_the_module_had_work() {
  let tracks = [
    track(vec![(at(3, 9), true)], vec![]),
    track(vec![(at(3, 9), true)], vec![(at(4, 9), WorkflowStatus::Done)]),
  ];
  let burndown = chart(&tracks, (1, Some(5), 5));
  assert_eq!(burndown.start_scope, 2);
  let shown: Vec<(Option<u32>, Option<f64>)> = burndown.days.iter().map(|day| (day.remaining, day.ideal)).collect();
  assert_eq!(
    shown,
    [
      (None, None),
      (None, None),
      (Some(2), Some(2.0)),
      (Some(1), Some(1.0)),
      (Some(1), Some(0.0)),
    ],
    "nothing to burn down before the tasks came"
  );
}

#[test]
fn modules_burndown_shows_at_most_a_year_of_days() {
  let date = |year, month, day| NaiveDate::from_ymd_opt(year, month, day).expect("a date");
  let tracks = [
    track(vec![(day_start(date(2020, 1, 1)), true)], vec![]),
    track(
      vec![(day_start(date(2020, 1, 1)), true)],
      vec![(day_start(date(2026, 10, 3)), WorkflowStatus::Done)],
    ),
  ];
  let far = ChartDays {
    first: day(1),
    target: Some(date(2206, 10, 1)),
    today: day(5),
  };
  let burndown = Burndown::of(&tracks, WorkUnit::Tasks, far);
  assert_eq!(
    burndown.days.len(),
    MAX_CHART_DAYS as usize,
    "a far-off target is cut off"
  );
  assert_eq!(burndown.days[0].date, day(1));
  let old = ChartDays {
    first: date(2020, 1, 1),
    target: Some(date(2021, 1, 1)),
    today: day(5),
  };
  let burndown = Burndown::of(&tracks, WorkUnit::Tasks, old);
  assert_eq!(
    burndown.days.len(),
    MAX_CHART_DAYS as usize,
    "an old module shows the year up to today"
  );
  assert_eq!(burndown.days.last().map(|day| day.date), Some(day(5)));
  assert_eq!(
    burndown.start_scope, 2,
    "the line still starts where the module got its work"
  );
  assert_eq!(burndown.latest().map(|day| day.remaining), Some(Some(1)));
  assert!(
    burndown.days.iter().all(|day| day.ideal == Some(0.0)),
    "the target is long past"
  );
}

#[test]
fn modules_form_reads_dates_and_refuses_a_target_before_the_start() {
  let form = |start, target| ModuleForm {
    name: "Checkout",
    description: "  Payments  ",
    start,
    target,
    state: ModuleState::Open,
  };
  let details = form("2026-10-01", "2026-10-15").details().expect("valid");
  assert_eq!((details.start(), details.target()), (Some(day(1)), Some(day(15))));
  assert_eq!(details.description(), "Payments");
  assert!(form("", "").details().expect("no dates").start().is_none());
  let errors = form("2026-10-15", "2026-10-01").details().expect_err("reversed");
  assert_eq!(
    errors.target.map(|message| message.key),
    Some("modules.errors.targetBeforeStart")
  );
  let errors = form("1 Oct", "").details().expect_err("unreadable");
  assert_eq!(
    errors.start.map(|message| message.key),
    Some("modules.errors.invalidDate")
  );
  assert!(
    ModuleDetails::new(
      Name::new("x").expect("name"),
      "",
      (Some(day(2)), Some(day(1))),
      ModuleState::Open
    )
    .is_err()
  );
}

#[test]
fn modules_filter_keeps_the_tasks_of_the_chosen_module() {
  let module = ModuleId::generate();
  let details = TaskDetails {
    title: Name::new("Task").expect("title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let key = |number| {
    TaskKey::new(
      ProjectKey::new("KON").expect("key"),
      std::num::NonZeroU32::new(number).expect("number"),
    )
  };
  let inside = Task::draft(TaskId::generate(), key(1), details.clone(), at(1, 9)).with_module(Some(module));
  let outside = Task::draft(TaskId::generate(), key(2), details, at(1, 9));
  assert!(ModuleFilter::Any.keeps(&inside) && ModuleFilter::Any.keeps(&outside));
  assert!(ModuleFilter::In(module).keeps(&inside) && !ModuleFilter::In(module).keeps(&outside));
  assert!(ModuleFilter::None.keeps(&outside) && !ModuleFilter::None.keeps(&inside));
}

#[test]
fn modules_page_reads_a_merged_run_as_done() {
  let mut fixture = Fixture::new();
  let details = ModuleDetails::new(
    Name::new("Sign-in").expect("name"),
    "",
    (Some(day(1)), Some(day(5))),
    ModuleState::Open,
  )
  .expect("details");
  let module = fixture.db.create_module(details, at(1, 8)).expect("module");
  let merged = fixture.queued_task("Sign in", at(1, 9));
  let open = fixture.task("Docs", at(1, 9));
  for task in [merged, open] {
    fixture
      .db
      .set_task_module(task, Some(module.id()), ChangeActor::Person, at(1, 10))
      .expect("in the module");
  }
  let mut run = fixture.started_run(merged, at(2, 9));
  fixture.succeed(&mut run, at(3, 9));
  let merge = RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::AlreadyMerged,
    merged_at: at(4, 9),
  };
  fixture.db.save_run_merge(&merge).expect("merged outside Orchester");
  let page = ModulePage::read(&fixture.db, module.id(), at(5, 12))
    .expect("read")
    .expect("the module");
  let remaining: Vec<Option<u32>> = page.burndown.days.iter().map(|day| day.remaining).collect();
  assert_eq!(remaining, [Some(2), Some(2), Some(2), Some(1), Some(1)]);
  assert_eq!((page.progress.scope, page.progress.done), (2, 1));
  assert_eq!(page.tasks.len(), 2);
  let listed = summaries(
    &fixture.db.modules().expect("modules"),
    &fixture.db.tasks().expect("tasks"),
  );
  assert_eq!(listed[0].progress, page.progress);
  assert_eq!(listed[0].progress.percent(), Some(50));
}

#[test]
fn modules_page_changes_again_over_its_own_write_before_reading_again() {
  let mut fixture = Fixture::new();
  let with = |state: ModuleState, target: Option<NaiveDate>| {
    ModuleDetails::new(Name::new("Checkout").expect("name"), "", (None, target), state).expect("details")
  };
  let shown = Arc::new(
    fixture
      .db
      .create_module(with(ModuleState::Open, None), at(1, 8))
      .expect("module"),
  );
  let change =
    |db: &mut orchester_store::ProjectDb, base: &Module, details| db.update_module(base.id(), base.details(), details);

  let base = LastWrite::base(None, &shown);
  let closed = Arc::new(change(&mut fixture.db, &base, with(ModuleState::Closed, None)).expect("closed"));
  let last = LastWrite::after(None, base, closed.clone());
  // The page still shows the module as first read: a second change goes over the first one's write.
  let base = LastWrite::base(Some(&last), &shown);
  assert_eq!(base, closed);
  let dated = Arc::new(
    change(&mut fixture.db, &base, with(ModuleState::Closed, Some(day(9)))).expect("the second change is saved"),
  );
  let last = LastWrite::after(Some(last), base, dated.clone());
  assert_eq!(
    LastWrite::base(Some(&last), &shown),
    dated,
    "and a third over the second's"
  );
  assert!(
    matches!(
      change(&mut fixture.db, &shown, with(ModuleState::Open, None)),
      Err(orchester_store::StoreError::Module(
        orchester_core::module::ModuleError::ChangedMeanwhile
      ))
    ),
    "over the first read itself, the change is refused as made meanwhile"
  );

  // Once the page reads again, a change goes over what it read, which may hold a change made elsewhere.
  let elsewhere =
    Arc::new(change(&mut fixture.db, &dated, with(ModuleState::Open, Some(day(9)))).expect("changed elsewhere"));
  assert_eq!(LastWrite::base(Some(&last), &elsewhere), elsewhere);
}
