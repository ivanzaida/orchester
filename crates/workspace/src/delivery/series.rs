//! The delivery charts of a range of local days (ORC-50), from [`super::DeliveryFacts`]: tasks delivered (their work
//! merged) per day or week, with their story points; the cycle time of each delivered task from its first agent's
//! start to the merge and its lead time from its creation, with their median and spread; where agent time went by
//! pipeline step, with how often each step's work was returned, and the attempts delivered tasks took; and the tasks
//! in each workflow status at the end of each day.

use std::{collections::BTreeMap, sync::Arc};

use chrono::{Days, NaiveDate};
use orchester_core::{
  id::TaskId,
  task::organization::WorkflowStatus,
  time::Timestamp,
  usage::{DayRange, day_start, local_date},
};

use super::facts::{DeliveryFacts, ExecutionFacts, TaskFacts};

/// The widest range throughput shows by day; a wider one is shown by week.
pub const DAILY_DAYS: u64 = 31;

/// One bar of *Throughput* per day, or per week of seven days from the range's first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucketing {
  Day,
  Week,
}

impl Bucketing {
  pub fn of(range: DayRange) -> Self {
    if range.days() <= DAILY_DAYS {
      Self::Day
    } else {
      Self::Week
    }
  }

  const fn days(self) -> u64 {
    match self {
      Self::Day => 1,
      Self::Week => 7,
    }
  }
}

/// One bar of *Throughput*: its days, the tasks delivered in them and the points of those that had points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThroughputBar {
  pub first: NaiveDate,
  /// The last day, at most the range's.
  pub last: NaiveDate,
  pub tasks: u32,
  pub points: u32,
  /// Delivered tasks without points.
  pub unpointed: u32,
}

pub fn throughput(facts: &DeliveryFacts, range: DayRange, bucketing: Bucketing) -> Vec<ThroughputBar> {
  let span = bucketing.days();
  range
    .dates()
    .step_by(usize::try_from(span).expect("a bucket is a few days"))
    .map(|first| {
      let last = first
        .checked_add_days(Days::new(span - 1))
        .unwrap_or(NaiveDate::MAX)
        .min(range.last());
      let mut bar = ThroughputBar {
        first,
        last,
        tasks: 0,
        points: 0,
        unpointed: 0,
      };
      for task in delivered_between(facts, first, last) {
        bar.tasks += 1;
        match task.points {
          Some(points) => bar.points += points,
          None => bar.unpointed += 1,
        }
      }
      bar
    })
    .collect()
}

/// The tasks whose work was merged on a day from `first` through `last`.
fn delivered_between(facts: &DeliveryFacts, first: NaiveDate, last: NaiveDate) -> impl Iterator<Item = &TaskFacts> {
  facts.tasks.iter().filter(move |task| {
    task
      .merged_at
      .map(local_date)
      .is_some_and(|day| first <= day && day <= last)
  })
}

/// A delivered task's times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CycleRow {
  pub task: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  pub merged_at: Timestamp,
  /// From its first agent's start to the merge; `None` when no agent of its started (work merged outside a run).
  pub cycle_ms: Option<u64>,
  /// From its creation to the merge.
  pub lead_ms: u64,
  pub attempts: u32,
}

/// The median and spread of some durations: the middle one, the quartiles around it, and the 85th percentile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spread {
  pub count: usize,
  pub median: u64,
  pub lower: u64,
  pub upper: u64,
  pub p85: u64,
}

impl Spread {
  /// `None` without values.
  pub fn of(mut values: Vec<u64>) -> Option<Self> {
    if values.is_empty() {
      return None;
    }
    values.sort_unstable();
    let at = |share: usize| values[(values.len() * share).div_ceil(100).max(1) - 1];
    Some(Self {
      count: values.len(),
      median: at(50),
      lower: at(25),
      upper: at(75),
      p85: at(85),
    })
  }
}

/// *Cycle time*: the tasks delivered in the range, by when, and their spreads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CycleTimes {
  pub rows: Vec<CycleRow>,
  pub cycle: Option<Spread>,
  pub lead: Option<Spread>,
}

pub fn cycle_times(facts: &DeliveryFacts, range: DayRange) -> CycleTimes {
  let mut rows: Vec<CycleRow> = delivered_between(facts, range.first(), range.last())
    .filter_map(|task| {
      let merged_at = task.merged_at?;
      let since = |from: Timestamp| u64::try_from(merged_at.unix_millis() - from.unix_millis()).unwrap_or(0);
      Some(CycleRow {
        task: task.task,
        key: task.key.clone(),
        title: task.title.clone(),
        merged_at,
        cycle_ms: task.first_start.map(since),
        lead_ms: since(task.created_at),
        attempts: task.attempts,
      })
    })
    .collect();
  rows.sort_by_key(|row| row.merged_at);
  CycleTimes {
    cycle: Spread::of(rows.iter().filter_map(|row| row.cycle_ms).collect()),
    lead: Spread::of(rows.iter().map(|row| row.lead_ms).collect()),
    rows,
  }
}

/// One pipeline step (by its name) in *Where time goes*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRow {
  pub step: Arc<str>,
  pub active_ms: u64,
  pub executions: u32,
  /// Executions whose work a later attempt returned.
  pub returned: u32,
}

impl StepRow {
  /// The share of its executions that were returned, in whole percent.
  pub fn return_percent(&self) -> u32 {
    if self.executions == 0 {
      return 0;
    }
    self.returned * 100 / self.executions
  }
}

/// *Where time goes*: active agent time and returns by step of the executions that started in the range, most time
/// first; and the attempts of the tasks delivered in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhereTimeGoes {
  pub steps: Vec<StepRow>,
  pub delivered: u32,
  pub attempts: u32,
}

impl WhereTimeGoes {
  /// Attempts per delivered task, to a tenth; `None` without delivered tasks.
  pub fn attempts_per_task(&self) -> Option<f64> {
    (self.delivered > 0).then(|| (f64::from(self.attempts) * 10.0 / f64::from(self.delivered)).round() / 10.0)
  }
}

pub fn where_time_goes(facts: &DeliveryFacts, range: DayRange) -> WhereTimeGoes {
  let mut steps: BTreeMap<Arc<str>, StepRow> = BTreeMap::new();
  let ran = facts
    .executions
    .iter()
    .filter(|execution| range.contains(execution.started_at));
  for ExecutionFacts {
    step,
    active_ms,
    returned,
    ..
  } in ran
  {
    let row = steps.entry(step.clone()).or_insert_with(|| StepRow {
      step: step.clone(),
      active_ms: 0,
      executions: 0,
      returned: 0,
    });
    row.active_ms = row.active_ms.saturating_add(*active_ms);
    row.executions += 1;
    row.returned += u32::from(*returned);
  }
  let mut steps: Vec<StepRow> = steps.into_values().collect();
  steps.sort_by(|a, b| b.active_ms.cmp(&a.active_ms).then_with(|| a.step.cmp(&b.step)));
  let delivered: Vec<&TaskFacts> = delivered_between(facts, range.first(), range.last()).collect();
  WhereTimeGoes {
    steps,
    delivered: u32::try_from(delivered.len()).unwrap_or(u32::MAX),
    attempts: delivered.iter().map(|task| task.attempts).sum(),
  }
}

/// The workflow statuses *Cumulative flow* stacks, from the bottom; Canceled tasks are left out.
pub const FLOW_STATUSES: [WorkflowStatus; 5] = [
  WorkflowStatus::Done,
  WorkflowStatus::InReview,
  WorkflowStatus::InProgress,
  WorkflowStatus::Todo,
  WorkflowStatus::Backlog,
];

/// One day of *Cumulative flow*: the tasks made by its end in each of [`FLOW_STATUSES`] then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowDay {
  pub date: NaiveDate,
  pub counts: [u32; 5],
}

impl FlowDay {
  pub fn total(&self) -> u32 {
    self.counts.iter().sum()
  }
}

pub fn cumulative_flow(facts: &DeliveryFacts, range: DayRange) -> Vec<FlowDay> {
  range
    .dates()
    .map(|date| {
      let end = day_start(date.succ_opt().unwrap_or(date));
      let mut counts = [0; 5];
      for task in facts.tasks.iter().filter(|task| task.created_at < end) {
        let status = task.status_before(end);
        if let Some(index) = FLOW_STATUSES.iter().position(|shown| *shown == status) {
          counts[index] += 1;
        }
      }
      FlowDay { date, counts }
    })
    .collect()
}
