//! Delivery (ORC-50, `orchester_workspace::delivery`): throughput, cycle and lead time, where agent time goes and
//! cumulative flow over a range, built from fixture histories; the Monte Carlo forecast with fixed seeds, its history
//! and the plain *not enough history* state; and the facts read from a project with a run merged outside Orchester.

mod support;

use std::sync::Arc;

use chrono::{Days, NaiveDate};
use orchester_core::{
  connection::stream::InvocationOutcome,
  id::TaskId,
  integration::{MergeKind, RunMerge},
  resource::ModelId,
  revision::Revision,
  run::BranchName,
  store::{ActivityStore, MergeStore, TaskStore},
  task::organization::WorkflowStatus,
  time::Timestamp,
  usage::{DayRange, day_start},
};
use orchester_workspace::delivery::{
  Bucketing, Delivery, DeliveryFacts, DeliveryPeriod, ExecutionFacts, FLOW_STATUSES, Forecast, History, Outlook,
  TaskFacts, Work,
  forecast::{MIN_DELIVERED, MIN_HISTORY_DAYS, SEED},
  history, queued_remaining,
};
use support::Fixture;

fn day(day: u32) -> NaiveDate {
  NaiveDate::from_ymd_opt(2026, 10, day).expect("an October day")
}

/// `hour` o'clock local time on October `on`.
fn at(on: u32, hour: i64) -> Timestamp {
  Timestamp::from_unix_millis(day_start(day(on)).unix_millis() + hour * 3_600_000)
}

fn range(first: u32, last: u32) -> DayRange {
  DayRange::new(day(first), day(last)).expect("a range")
}

/// A task made on October `made` at 9, Todo until its statuses change.
fn task(key: u32, made: u32) -> TaskFacts {
  TaskFacts {
    task: TaskId::generate(),
    key: format!("KON-{key}").into(),
    title: format!("Task {key}").into(),
    module: None,
    created_at: at(made, 9),
    queued: true,
    first_status: WorkflowStatus::Todo,
    statuses: Vec::new(),
    first_start: None,
    merged_at: None,
    points: None,
    attempts: 0,
  }
}

/// `task` started on October `started` at 10 and merged on `merged` at 15, which set it Done.
fn delivered(mut task: TaskFacts, started: u32, merged: u32) -> TaskFacts {
  task.first_start = Some(at(started, 10));
  task.merged_at = Some(at(merged, 15));
  task.statuses = vec![
    (at(started, 10), WorkflowStatus::InProgress),
    (at(merged, 15), WorkflowStatus::Done),
  ];
  task.attempts = 1;
  task
}

fn ran(step: &str, on: u32, active_ms: u64, returned: bool) -> ExecutionFacts {
  ExecutionFacts {
    step: Arc::from(step),
    started_at: at(on, 11),
    active_ms,
    returned,
  }
}

#[test]
fn delivery_throughput_counts_merged_tasks_and_points_per_day_or_week() {
  let mut pointed = delivered(task(1, 1), 1, 2);
  pointed.points = Some(5);
  let facts = DeliveryFacts {
    tasks: vec![
      pointed,
      delivered(task(2, 1), 1, 2),
      delivered(task(3, 1), 2, 4),
      task(4, 1),
    ],
    ..DeliveryFacts::default()
  };
  let by_day = Delivery::of(&facts, &[], range(1, 4), day(4), SEED);
  assert_eq!(by_day.bucketing, Bucketing::Day);
  let bars: Vec<(u32, u32, u32)> = by_day
    .throughput
    .iter()
    .map(|bar| (bar.tasks, bar.points, bar.unpointed))
    .collect();
  assert_eq!(bars, [(0, 0, 0), (2, 5, 1), (0, 0, 0), (1, 0, 1)]);
  let wide = DayRange::new(day(1), day(1) + Days::new(35)).expect("36 days");
  let by_week = Delivery::of(&facts, &[], wide, day(4), SEED);
  assert_eq!(by_week.bucketing, Bucketing::Week);
  assert_eq!(by_week.throughput.len(), 6, "36 days are five weeks and a day");
  assert_eq!(
    (by_week.throughput[0].first, by_week.throughput[0].last),
    (day(1), day(7))
  );
  assert_eq!(by_week.throughput[0].tasks, 3);
  assert_eq!(
    by_week.throughput[5].first, by_week.throughput[5].last,
    "the last bar ends with the range"
  );
}

#[test]
fn delivery_cycle_and_lead_time_have_a_median_and_spread() {
  let mut outside = task(4, 1);
  outside.merged_at = Some(at(5, 9));
  let facts = DeliveryFacts {
    tasks: vec![
      delivered(task(1, 1), 1, 2),
      delivered(task(2, 1), 2, 4),
      delivered(task(3, 2), 3, 8),
      outside,
    ],
    ..DeliveryFacts::default()
  };
  let delivery = Delivery::of(&facts, &[], range(1, 8), day(8), SEED);
  let cycle = &delivery.cycle;
  assert_eq!(cycle.rows.len(), 4, "every task merged in the range");
  let keys: Vec<&str> = cycle.rows.iter().map(|row| &*row.key).collect();
  assert_eq!(keys, ["KON-1", "KON-2", "KON-4", "KON-3"], "by when they were merged");
  assert_eq!(
    cycle.rows[2].cycle_ms, None,
    "work merged outside a run has no cycle time"
  );
  let hours = |hours: u64| hours * 3_600_000;
  let spread = cycle.cycle.expect("three cycle times");
  assert_eq!(spread.count, 3);
  assert_eq!(
    (spread.lower, spread.median, spread.p85),
    (hours(29), hours(53), hours(125))
  );
  let lead = cycle.lead.expect("four lead times");
  assert_eq!(lead.count, 4);
  assert_eq!(
    lead.median,
    hours(78),
    "Oct 2 9:00 to Oct 5 15:00 is the second of four"
  );
}

#[test]
fn delivery_shows_where_agent_time_goes_and_how_often_steps_return_work() {
  let mut retried = delivered(task(1, 1), 1, 3);
  retried.attempts = 3;
  let facts = DeliveryFacts {
    tasks: vec![retried, delivered(task(2, 1), 1, 3)],
    executions: vec![
      ran("Implement", 1, 3_600_000, false),
      ran("Review", 1, 600_000, true),
      ran("Implement", 2, 1_800_000, false),
      ran("Review", 2, 600_000, false),
      ran("Implement", 9, 1_000, false),
    ],
    unread_runs: 0,
  };
  let delivery = Delivery::of(&facts, &[], range(1, 3), day(3), SEED);
  let steps: Vec<(&str, u64, u32, u32, u32)> = delivery
    .time
    .steps
    .iter()
    .map(|step| {
      (
        &*step.step,
        step.active_ms,
        step.executions,
        step.returned,
        step.return_percent(),
      )
    })
    .collect();
  assert_eq!(
    steps,
    [("Implement", 5_400_000, 2, 0, 0), ("Review", 1_200_000, 2, 1, 50)],
    "most time first; an execution after the range is left out"
  );
  assert_eq!(delivery.time.attempts_per_task(), Some(2.0));
}

#[test]
fn delivery_cumulative_flow_counts_tasks_per_workflow_status_at_each_days_end() {
  let mut canceled = task(3, 1);
  canceled.statuses = vec![(at(2, 12), WorkflowStatus::Canceled)];
  let facts = DeliveryFacts {
    tasks: vec![delivered(task(1, 1), 2, 3), task(2, 2), canceled],
    ..DeliveryFacts::default()
  };
  let delivery = Delivery::of(&facts, &[], range(1, 3), day(3), SEED);
  let index = |status| FLOW_STATUSES.iter().position(|shown| *shown == status).expect("shown");
  let count = |day: usize, status| delivery.flow[day].counts[index(status)];
  assert_eq!((count(0, WorkflowStatus::Todo), delivery.flow[0].total()), (2, 2));
  assert_eq!(count(1, WorkflowStatus::InProgress), 1);
  assert_eq!(delivery.flow[1].total(), 2, "a Canceled task leaves the flow");
  assert_eq!((count(2, WorkflowStatus::Done), count(2, WorkflowStatus::Todo)), (1, 1));
}

#[test]
fn delivery_forecast_is_exact_for_a_steady_history() {
  let steady = History::new(vec![1; 12]);
  let forecast = Forecast::of(5, &steady, day(20), SEED);
  assert_eq!(
    (forecast.remaining, forecast.history_days, forecast.history_delivered),
    (5, 12, 12)
  );
  assert_eq!(
    forecast.outlook,
    Outlook::Dates {
      likely: day(25),
      safe: day(25),
      past_horizon: false,
    },
    "one task a day, every day, delivers five tasks in five days"
  );
}

#[test]
fn delivery_forecast_with_a_fixed_seed_is_repeatable_and_ordered() {
  let uneven = History::new(vec![0, 2, 1, 0, 3, 1, 0, 2, 0, 1, 0, 2]);
  let forecast = Forecast::of(10, &uneven, day(1), 7);
  assert_eq!(
    forecast,
    Forecast::of(10, &uneven, day(1), 7),
    "the same seed, the same dates"
  );
  // Twelve days delivered twelve tasks, one a day with a variance of one: ten take about ten days, and the 85% date
  // about one standard deviation (some three days) later.
  assert_eq!(
    forecast.outlook,
    Outlook::Dates {
      likely: day(11),
      safe: day(15),
      past_horizon: false,
    }
  );
  let other = Forecast::of(10, &uneven, day(1), SEED);
  assert!(matches!(other.outlook, Outlook::Dates { .. }));
}

#[test]
fn delivery_forecast_says_when_history_is_too_short_or_nothing_is_left() {
  let short = History::new(vec![2; usize::try_from(MIN_HISTORY_DAYS).expect("days") - 1]);
  let forecast = Forecast::of(4, &short, day(10), SEED);
  assert_eq!(forecast.outlook, Outlook::NotEnoughHistory);
  assert_eq!(
    forecast.history_days,
    MIN_HISTORY_DAYS - 1,
    "it says how much history there is"
  );
  let mut quiet = vec![0; 20];
  quiet[3] = MIN_DELIVERED - 1;
  assert_eq!(
    Forecast::of(4, &History::new(quiet), day(30), SEED).outlook,
    Outlook::NotEnoughHistory
  );
  assert_eq!(Forecast::of(0, &short, day(10), SEED).outlook, Outlook::Done);
}

#[test]
fn delivery_history_takes_the_last_whole_days_not_before_the_first_task() {
  let facts = DeliveryFacts {
    tasks: vec![delivered(task(1, 3), 3, 4), delivered(task(2, 3), 4, 9), task(3, 3)],
    ..DeliveryFacts::default()
  };
  let young = history(&facts, day(9));
  assert_eq!(
    (young.days(), young.delivered()),
    (6, 1),
    "Oct 3 to 8; today's merge waits for tomorrow"
  );
  let old = history(&facts, day(3) + Days::new(60));
  assert_eq!(old.days(), 30, "a month at most");
  assert_eq!(old.delivered(), 0);
  assert_eq!(queued_remaining(&facts), 1);
}

#[test]
fn delivery_reads_a_run_merged_outside_orchester_and_forecasts_the_queue() {
  let mut fixture = Fixture::new();
  let merged = fixture.queued_task("Sign in", at(1, 9));
  fixture.queued_task("Docs", at(1, 9));
  fixture.task("Idea", at(1, 9));
  let mut run = fixture.started_run(merged, at(2, 9));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let mut invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(2, 9))
    .expect("invocation");
  invocation.ended_at = Some(at(2, 10));
  invocation.outcome = Some(InvocationOutcome::Completed);
  fixture.db.save_invocation(&invocation).expect("ended");
  fixture.succeed(&mut run, at(3, 9));
  let merge = RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::AlreadyMerged,
    merged_at: at(4, 9),
  };
  fixture.db.save_run_merge(&merge).expect("merged outside Orchester");
  let tasks = fixture.db.tasks().expect("tasks");
  let facts = DeliveryFacts::read(&fixture.db, &tasks, at(5, 12)).expect("facts");
  let read = facts
    .tasks
    .iter()
    .find(|task| task.task == merged)
    .expect("the merged task");
  assert_eq!(
    (read.first_start, read.merged_at, read.attempts),
    (Some(at(2, 9)), Some(at(4, 9)), 1)
  );
  assert_eq!(read.status(), WorkflowStatus::Done, "the merge set it Done");
  assert_eq!(facts.executions.len(), 1);
  assert_eq!(facts.executions[0].active_ms, 3_600_000);
  let delivery = Delivery::read(&fixture.db, DeliveryPeriod::Month.range(day(5)), at(5, 12)).expect("delivery");
  assert_eq!(delivery.cycle.rows.len(), 1);
  assert_eq!(delivery.cycle.rows[0].cycle_ms, Some(48 * 3_600_000));
  let queued = &delivery.forecasts[0];
  assert_eq!(queued.work, Work::Queued);
  assert_eq!(
    queued.forecast.remaining, 1,
    "the queued task still open; the draft is not queued"
  );
  assert_eq!(
    queued.forecast.outlook,
    Outlook::NotEnoughHistory,
    "four days and one merged task"
  );
}
