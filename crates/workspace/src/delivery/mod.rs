//! How delivery goes in a project (ORC-50), as the Delivery view beside Usage and `get_delivery` read it: throughput,
//! cycle and lead time, where agent time goes, cumulative flow ([`series`]) over a range of local days, and when the
//! open work of the project's queued tasks and of each open module is likely delivered ([`forecast`]). Everything is
//! built from what the project's store holds ([`facts`]); nothing is estimated, and nothing is money.

pub mod facts;
pub mod forecast;
pub mod series;

use std::sync::Arc;

use chrono::{Days, NaiveDate};
use orchester_core::{
  id::ModuleId,
  module::ModuleState,
  store::TaskStore,
  time::Timestamp,
  usage::{DayRange, local_date},
};
use orchester_store::{ProjectDb, StoreError};

pub use self::{
  facts::{DeliveryFacts, ExecutionFacts, TaskFacts},
  forecast::{Forecast, History, Outlook},
  series::{Bucketing, CycleRow, CycleTimes, FLOW_STATUSES, FlowDay, Spread, StepRow, ThroughputBar, WhereTimeGoes},
};
use crate::modules::data::{ModuleSummary, summaries};

/// The Delivery view's period: the last 30, 90 or 365 local days, today included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DeliveryPeriod {
  #[default]
  Month,
  Quarter,
  Year,
}

impl DeliveryPeriod {
  pub const ALL: [Self; 3] = [Self::Month, Self::Quarter, Self::Year];

  /// The period as a route's query names it.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Month => "30d",
      Self::Quarter => "90d",
      Self::Year => "365d",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|period| period.name() == name)
  }

  pub const fn days(self) -> u64 {
    match self {
      Self::Month => 30,
      Self::Quarter => 90,
      Self::Year => 365,
    }
  }

  pub fn range(self, today: NaiveDate) -> DayRange {
    last_days(self.days(), today)
  }
}

/// The last `days` local days through `today`.
pub fn last_days(days: u64, today: NaiveDate) -> DayRange {
  let first = today
    .checked_sub_days(Days::new(days.saturating_sub(1)))
    .unwrap_or(today);
  DayRange::new(first, today).expect("the first day is never after today")
}

/// Whose open work a forecast is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Work {
  /// The project's queued tasks that are not Done or Canceled.
  Queued,
  /// An open module's tasks in scope that are not Done.
  Module { id: ModuleId, name: Arc<str> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkForecast {
  pub work: Work,
  pub forecast: Forecast,
}

/// What the Delivery view and `get_delivery` show for a range.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
  pub range: DayRange,
  pub bucketing: Bucketing,
  pub throughput: Vec<ThroughputBar>,
  pub cycle: CycleTimes,
  pub time: WhereTimeGoes,
  pub flow: Vec<FlowDay>,
  /// The queued work first, then each open module, oldest first.
  pub forecasts: Vec<WorkForecast>,
  /// Runs whose record could not be read; what they did is left out.
  pub unread_runs: usize,
}

impl Delivery {
  /// The project's delivery in `db` over `range`, as it stands at `now`.
  pub fn read(db: &ProjectDb, range: DayRange, now: Timestamp) -> Result<Self, StoreError> {
    let tasks = db.tasks()?;
    let facts = DeliveryFacts::read(db, &tasks, now)?;
    let modules = summaries(&db.modules()?, &tasks);
    Ok(Self::of(&facts, &modules, range, local_date(now), forecast::SEED))
  }

  /// The delivery `facts` give over `range`, with forecasts after `today` of the queued work and of the open
  /// `modules`, drawn from `seed`.
  pub fn of(facts: &DeliveryFacts, modules: &[ModuleSummary], range: DayRange, today: NaiveDate, seed: u64) -> Self {
    let bucketing = Bucketing::of(range);
    let history = history(facts, today);
    let open = modules
      .iter()
      .filter(|summary| summary.module.details().state() == ModuleState::Open)
      .map(|summary| {
        let work = Work::Module {
          id: summary.module.id(),
          name: summary.module.name().as_str().into(),
        };
        (work, summary.progress.remaining())
      });
    let forecasts = std::iter::once((Work::Queued, queued_remaining(facts)))
      .chain(open)
      .map(|(work, remaining)| WorkForecast {
        work,
        forecast: Forecast::of(remaining, &history, today, seed),
      })
      .collect();
    Self {
      range,
      bucketing,
      throughput: series::throughput(facts, range, bucketing),
      cycle: series::cycle_times(facts, range),
      time: series::where_time_goes(facts, range),
      flow: series::cumulative_flow(facts, range),
      forecasts,
      unread_runs: facts.unread_runs,
    }
  }
}

/// The history a forecast after `today` rests on: tasks whose work was merged on each of the project's last whole
/// days, from its first task's day on.
pub fn history(facts: &DeliveryFacts, today: NaiveDate) -> History {
  let Some(first) = facts.tasks.iter().map(|task| local_date(task.created_at)).min() else {
    return History::new(Vec::new());
  };
  let merged: Vec<NaiveDate> = facts
    .tasks
    .iter()
    .filter_map(|task| task.merged_at.map(local_date))
    .collect();
  History::before(today, first, |date| {
    u32::try_from(merged.iter().filter(|day| **day == date).count()).unwrap_or(u32::MAX)
  })
}

/// The project's queued tasks still to be delivered.
pub fn queued_remaining(facts: &DeliveryFacts) -> u32 {
  let open = facts.tasks.iter().filter(|task| task.queued && task.open());
  u32::try_from(open.count()).unwrap_or(u32::MAX)
}
