//! What the person chose on the dashboard: the period, the grouping and the filters, and how each
//! section shows its data (chart or table, which measure, tasks or runs).

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use orchester_core::{
  id::ProjectId,
  time::Timestamp,
  usage::{
    DayRange, FilterOptions, Grouping, Measure, Period, ProjectFacts, UsageFilter, UsageQuery, UsageReport,
    limits::{LimitHistory, WarningThreshold},
  },
};

use super::load::{Scope, UsageData};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Controls {
  pub period: Period,
  pub grouping: Grouping,
  pub filter: UsageFilter,
}

impl lurq::app::component::DevtoolsInspectable for Controls {}

impl Default for Controls {
  fn default() -> Self {
    Self {
      period: Period::Week,
      grouping: Grouping::Day,
      filter: UsageFilter::default(),
    }
  }
}

impl Controls {
  /// `period` chosen: *Today* reads best by hour, and a range longer than a week only by day.
  pub fn with_period(&self, period: Period, today: chrono::NaiveDate) -> Self {
    let grouping = match period {
      Period::Today => Grouping::Hour,
      _ if !period.range(today).offers_hours() => Grouping::Day,
      _ => self.grouping,
    };
    Self {
      period,
      grouping,
      filter: self.filter.clone(),
    }
  }

  /// The grouping in force for `range`: by hour only where it is offered.
  pub fn grouping_for(&self, range: DayRange) -> Grouping {
    if range.offers_hours() {
      self.grouping
    } else {
      Grouping::Day
    }
  }
}

/// How the sections show their data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Views {
  pub measure: Measure,
  pub tokens_table: bool,
  pub limits_table: bool,
  pub breakdown_table: bool,
  /// The three *Efficiency* cards switch on their own.
  pub cache_table: bool,
  pub edits_table: bool,
  pub failed_table: bool,
  /// *Most expensive* shows runs instead of tasks.
  pub runs: bool,
  /// *Most expensive* shows every row instead of the first five.
  pub all_rows: bool,
}

impl lurq::app::component::DevtoolsInspectable for Views {}

/// Everything the sections draw from, computed once per change.
#[derive(Clone, Debug, PartialEq)]
pub struct Dashboard {
  pub scope: Scope,
  pub range: DayRange,
  pub grouping: Grouping,
  pub filter: UsageFilter,
  pub report: Arc<UsageReport>,
  pub limits: Arc<LimitHistory>,
  pub options: Arc<FilterOptions>,
  pub projects: Arc<[ProjectFacts]>,
  /// Where each project is, to open its tasks.
  pub roots: Arc<HashMap<ProjectId, PathBuf>>,
  pub unread: Arc<[Arc<str>]>,
  pub threshold: WarningThreshold,
  /// The filters hide every run of the range.
  pub filtered_out: bool,
  /// The runs of the range, filters aside; counted only when the filters hide them all.
  pub unfiltered_runs: usize,
  pub now: Timestamp,
}

impl lurq::app::component::DevtoolsInspectable for Dashboard {}

impl Dashboard {
  pub fn of(scope: Scope, controls: &Controls, data: &UsageData, range: DayRange, now: Timestamp) -> Self {
    let grouping = controls.grouping_for(range);
    let query = UsageQuery {
      range,
      grouping,
      filter: &controls.filter,
      now,
    };
    let report = UsageReport::build(&data.projects, query);
    let limits = LimitHistory::of(&data.snapshots, (range.start(), range.end()), now, &controls.filter);
    let filtered_out = report.summary.runs == 0 && UsageReport::hidden_by(&data.projects, query);
    let unfiltered_runs = if filtered_out {
      let unfiltered = UsageQuery {
        filter: &UsageFilter::default(),
        ..query
      };
      UsageReport::build(&data.projects, unfiltered).summary.runs
    } else {
      report.summary.runs
    };
    Self {
      scope,
      range,
      grouping,
      filter: controls.filter.clone(),
      filtered_out,
      unfiltered_runs,
      report: Arc::new(report),
      limits: Arc::new(limits),
      options: Arc::new(FilterOptions::of(&data.projects)),
      projects: Arc::clone(&data.projects),
      roots: Arc::clone(&data.roots),
      unread: Arc::clone(&data.unread),
      threshold: data.threshold,
      now,
    }
  }

  /// Whether the range has no runs at all, filters aside.
  pub fn no_runs(&self) -> bool {
    self.report.summary.runs == 0 && !self.filtered_out
  }

  pub const fn all_projects(&self) -> bool {
    matches!(self.scope, Scope::All)
  }

  /// The range is today alone.
  pub fn is_today(&self) -> bool {
    self.range.days() == 1 && self.range.first() == orchester_core::usage::local_date(self.now)
  }

  /// Whether the range ends with today, which counts up to now.
  pub fn has_today(&self) -> bool {
    self.range.last() == orchester_core::usage::local_date(self.now)
  }

  /// The project shown, or every project's name when the scope is all of them.
  pub fn project_name(&self) -> Option<Arc<str>> {
    match &self.scope {
      Scope::Project(open) => Some(open.project.name().as_str().into()),
      Scope::All => None,
    }
  }
}
