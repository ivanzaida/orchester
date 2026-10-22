//! A module's burndown per local day (ORC-48): the work still to do at the end of each day, the ideal line from the
//! scope at the start to nothing at the target date, and the work added to and removed from the module that day.
//!
//! What counts is read from each task's history ([`TaskTrack`]): a task is in scope while it is in the module and not
//! Canceled, and done while its workflow status is Done. Merging its run sets Done (in Orchester, or found merged
//! outside it by git: `orchester_store`'s `save_run_merge`), and so does a person; a person who moves it away from
//! Done reopens it, and it counts as remaining again.

use chrono::{Days, NaiveDate};
use orchester_core::{
  id::TaskId,
  task::organization::WorkflowStatus,
  time::Timestamp,
  usage::{DayRange, day_start, local_date},
};

use super::measure::WorkUnit;
use crate::delivery::{Forecast, Outlook};

/// What one task did over time, as far as one module is concerned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskTrack {
  pub task: TaskId,
  /// How much work the task is, in the module's unit.
  pub weight: u32,
  /// Each move into (`true`) or out of (`false`) the module, oldest first.
  pub membership: Vec<(Timestamp, bool)>,
  /// The workflow status before its first recorded change.
  pub first_status: WorkflowStatus,
  /// Each change of its workflow status, oldest first.
  pub statuses: Vec<(Timestamp, WorkflowStatus)>,
}

/// Where a task stands in the module at one moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Standing {
  Out,
  Remaining,
  Done,
}

impl TaskTrack {
  /// How the task stood just before `at`.
  fn standing_before(&self, at: Timestamp) -> Standing {
    let member = self
      .membership
      .iter()
      .rev()
      .find(|(when, _)| *when < at)
      .is_some_and(|(_, member)| *member);
    let status = self
      .statuses
      .iter()
      .rev()
      .find(|(when, _)| *when < at)
      .map_or(self.first_status, |(_, status)| *status);
    match (member, status) {
      (false, _) | (true, WorkflowStatus::Canceled) => Standing::Out,
      (true, WorkflowStatus::Done) => Standing::Done,
      (true, _) => Standing::Remaining,
    }
  }
}

/// One day of the chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurndownDay {
  pub date: NaiveDate,
  /// The work in scope at the end of the day; `None` for a day still to come, and for a day before the module had
  /// any work.
  pub scope: Option<u32>,
  /// The work still to do at the end of the day; `None` where `scope` is.
  pub remaining: Option<u32>,
  /// Where the ideal line is that day; `None` without a target date, and before the module had any work.
  pub ideal: Option<f64>,
  /// Work that came into scope that day, and work that left it.
  pub added: u32,
  pub removed: u32,
}

/// A module's chart.
#[derive(Clone, Debug, PartialEq)]
pub struct Burndown {
  pub unit: WorkUnit,
  pub days: Vec<BurndownDay>,
  /// The scope the ideal line starts from: what was in scope at the end of the first day the module had work, or of
  /// its first day while it never had any.
  pub start_scope: u32,
}

/// The most days a chart shows: a year.
pub const MAX_CHART_DAYS: u64 = 366;

/// When the chart's days are: from the module's start (or the day it got its first task, or was made), through
/// today or its target date, whichever is later, and at most [`MAX_CHART_DAYS`] of them. A longer module shows its
/// first year while today is in it, and the year up to today after that; a far-off target date is cut off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChartDays {
  pub first: NaiveDate,
  pub target: Option<NaiveDate>,
  pub today: NaiveDate,
}

impl ChartDays {
  fn range(self) -> DayRange {
    let span = Days::new(MAX_CHART_DAYS - 1);
    let wanted = self.today.max(self.target.unwrap_or(self.today)).max(self.first);
    let first_year = self.first.checked_add_days(span).unwrap_or(NaiveDate::MAX);
    let last = wanted.min(first_year.max(self.today));
    let first = last.checked_sub_days(span).unwrap_or(NaiveDate::MIN).max(self.first);
    DayRange::new(first, last).expect("the last day is never before the first")
  }
}

/// The tracks of a module, read at the ends of its days.
struct Tracks<'a>(&'a [TaskTrack]);

impl Tracks<'_> {
  /// The work standing as one of `standing` just before `at`.
  fn sum(&self, at: Timestamp, standing: &[Standing]) -> u32 {
    self
      .0
      .iter()
      .filter(|track| standing.contains(&track.standing_before(at)))
      .map(|track| track.weight)
      .sum()
  }

  fn scope(&self, at: Timestamp) -> u32 {
    self.sum(at, &[Standing::Remaining, Standing::Done])
  }

  /// The first day from `days.first` through today that ends with work in scope, also when it is before the shown
  /// days. Scope only changes when a task moves or changes status, so only the days of those changes are read.
  fn begins(&self, days: ChartDays) -> Option<NaiveDate> {
    let mut dates: Vec<NaiveDate> = self
      .0
      .iter()
      .flat_map(|track| {
        let moves = track.membership.iter().map(|(at, _)| *at);
        moves.chain(track.statuses.iter().map(|(at, _)| *at))
      })
      .map(|at| local_date(at).max(days.first))
      .chain([days.first])
      .filter(|date| *date <= days.today)
      .collect();
    dates.sort_unstable();
    dates.dedup();
    dates.into_iter().find(|date| self.scope(end_of(*date)) > 0)
  }

  /// The work that was in scope (`from`) or not at the start of `date`, and is (`to`) or not at its end.
  fn changed(&self, date: NaiveDate, (from, to): (bool, bool)) -> u32 {
    let (start, end) = (day_start(date), end_of(date));
    self
      .0
      .iter()
      .filter(|track| {
        let (before, after) = (track.standing_before(start), track.standing_before(end));
        (before != Standing::Out) == from && (after != Standing::Out) == to
      })
      .map(|track| track.weight)
      .sum()
  }
}

impl Burndown {
  pub fn of(tracks: &[TaskTrack], unit: WorkUnit, days: ChartDays) -> Self {
    let range = days.range();
    let tracks = Tracks(tracks);
    // The line starts on the first day the module had work: a module whose tasks came after its start date
    // burns down from them, not from nothing.
    let begins = tracks.begins(days).unwrap_or(range.first());
    let start_scope = tracks.scope(end_of(begins));
    let shown = range
      .dates()
      .map(|date| {
        let shown = date >= begins && date <= days.today;
        let end = end_of(date);
        BurndownDay {
          date,
          scope: shown.then(|| tracks.scope(end)),
          remaining: shown.then(|| tracks.sum(end, &[Standing::Remaining])),
          ideal: (date >= begins)
            .then(|| ideal(start_scope, begins, days.target, date))
            .flatten(),
          added: if shown { tracks.changed(date, (false, true)) } else { 0 },
          removed: if shown { tracks.changed(date, (true, false)) } else { 0 },
        }
      })
      .collect();
    Self {
      unit,
      days: shown,
      start_scope,
    }
  }

  /// The day the chart reads as now: today's, or the last one before it.
  pub fn latest(&self) -> Option<&BurndownDay> {
    self.days.iter().rev().find(|day| day.remaining.is_some())
  }

  /// The forecast's lines over the chart's days, then over the days after them through the 85% date, at most
  /// [`MAX_CHART_DAYS`] more: from the work left on [`Self::latest`] each line falls evenly to nothing on its date.
  /// Without dates there are no lines and no days are added.
  pub fn forecast_days(&self, forecast: &Forecast) -> Vec<ForecastDay> {
    let (Outlook::Dates { likely, safe, .. }, Some(latest)) = (&forecast.outlook, self.latest()) else {
      return self.days.iter().map(|day| ForecastDay::none(day.date)).collect();
    };
    let (likely, safe) = (*likely, *safe);
    let (from, left) = (latest.date, f64::from(latest.remaining.unwrap_or(0)));
    let last_shown = self.days.last().map_or(from, |day| day.date);
    let last = safe.max(last_shown).min(
      last_shown
        .checked_add_days(Days::new(MAX_CHART_DAYS))
        .unwrap_or(NaiveDate::MAX),
    );
    let line = |to: NaiveDate, date: NaiveDate| {
      let span = (to - from).num_days();
      let gone = (date - from).num_days();
      (gone >= 0 && date <= to).then(|| {
        if span <= 0 {
          0.0
        } else {
          left * (span - gone) as f64 / span as f64
        }
      })
    };
    self
      .days
      .first()
      .map_or(from, |day| day.date)
      .iter_days()
      .take_while(|date| *date <= last)
      .map(|date| ForecastDay {
        date,
        likely: line(likely, date),
        safe: line(safe, date),
      })
      .collect()
  }
}

/// A day of the burndown with the forecast's 50% and 85% lines; `None` before the latest day and after a line's date.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForecastDay {
  pub date: NaiveDate,
  pub likely: Option<f64>,
  pub safe: Option<f64>,
}

impl ForecastDay {
  const fn none(date: NaiveDate) -> Self {
    Self {
      date,
      likely: None,
      safe: None,
    }
  }
}

/// Local midnight after `date`, where the day ends.
fn end_of(date: NaiveDate) -> Timestamp {
  day_start(date.checked_add_days(Days::new(1)).unwrap_or(date))
}

/// The ideal line on `date`: `scope` on the first day, falling evenly to nothing on the target day, and nothing
/// after it. A target on the first day is nothing on that day.
fn ideal(scope: u32, first: NaiveDate, target: Option<NaiveDate>, date: NaiveDate) -> Option<f64> {
  let target = target?;
  let span = (target - first).num_days();
  let left = (target - date).num_days();
  if left <= 0 || span <= 0 {
    return Some(0.0);
  }
  Some(f64::from(scope) * left as f64 / span as f64)
}
