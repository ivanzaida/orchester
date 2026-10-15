//! Everything the Usage page draws from, worked out once per change of the query or the data: the report
//! of the range and the period before it, the plan limits history, what the filter menus offer and which
//! state the page is in (`et35n`: the charts, no runs in the period, or filters that match nothing).

use std::sync::Arc;

use chrono::NaiveDate;
use orchester_core::{
  id::{ProjectId, RunId, TaskId},
  resource::VersionNumber,
  time::Timestamp,
  usage::{
    AccountOrder, AccountSplit, AccountUsage, Change, DayRange, FilterOptions, Grouping, UsageFilter,
    UsageQuery as ReportQuery, UsageReport,
    limits::{LimitHistory, WarningThreshold},
    local_date,
  },
};

use super::{UsageData, UsageQuery, names::Names};

/// What the page shows under its summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashboardState {
  /// The charts and tables.
  Charts,
  /// No run had activity in the period, filters aside (`AY6FM`): what the last run was and the plan limits
  /// last reported.
  NoRuns,
  /// The filters hide every run of the period (`YxAed`), which has this many runs without them.
  FilteredOut { runs: usize },
}

/// The summary's changes against the period before, never good or bad: spent tokens and agent time in
/// percent, runs in runs, the shares in points (`None` while either period has no share).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Changes {
  pub spent: Change,
  pub runs: Change,
  pub agent_time: Change,
  pub cache_share: Option<Change>,
  pub failed_share: Option<Change>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dashboard {
  /// The project's name; `None` on every project's page.
  pub project: Option<Arc<str>>,
  /// How many projects' usage was read.
  pub projects: usize,
  /// The listed projects that could not be read, by name.
  pub unread: Arc<[Arc<str>]>,
  pub query: UsageQuery,
  pub range: DayRange,
  pub grouping: Grouping,
  pub report: Arc<UsageReport>,
  pub limits: Arc<LimitHistory>,
  pub options: Arc<FilterOptions>,
  pub(super) names: Arc<Names>,
  /// The connected accounts in series order.
  pub(super) order: Arc<AccountOrder>,
  /// The range's usage by series: which series the legend and table show, and what Other holds.
  pub(super) range_split: Arc<AccountSplit>,
  /// Some CLI reported its plan limits, whatever the filters choose.
  pub limits_reported: bool,
  pub threshold: WarningThreshold,
  pub state: DashboardState,
  pub now: Timestamp,
}

impl Dashboard {
  /// The time the page reads from for `query` at `now`: the start of the period before its range, which
  /// the changes are measured against.
  pub fn since(query: &UsageQuery, now: Timestamp) -> Timestamp {
    query.range(local_date(now)).previous().start()
  }

  /// The dashboard of the project named `project`, or of every project, for `query` over `data`.
  pub fn of(project: Option<Arc<str>>, query: &UsageQuery, data: &UsageData, now: Timestamp) -> Self {
    let range = query.range(local_date(now));
    let grouping = query.grouping(range);
    let report_query = ReportQuery {
      range,
      grouping,
      filter: &query.filter,
      now,
    };
    let report = UsageReport::build(&data.projects, report_query);
    let order = AccountOrder::of(&data.accounts);
    let mut range_usage = AccountUsage::default();
    for bar in &report.series {
      for (account, usage) in bar.by_account.entries() {
        range_usage.add(*account, *usage);
      }
    }
    let range_split = order.split(&range_usage);
    let limits = LimitHistory::of(&data.snapshots, (range.start(), range.end()), now, &query.filter);
    let state = if report.summary.runs > 0 {
      DashboardState::Charts
    } else if UsageReport::hidden_by(&data.projects, report_query) {
      let unfiltered = ReportQuery {
        filter: &UsageFilter::default(),
        ..report_query
      };
      DashboardState::FilteredOut {
        runs: UsageReport::build(&data.projects, unfiltered).summary.runs,
      }
    } else {
      DashboardState::NoRuns
    };
    Self {
      project,
      projects: data.projects.len(),
      unread: data.unread.clone(),
      query: query.clone(),
      range,
      grouping,
      report: Arc::new(report),
      limits: Arc::new(limits),
      options: Arc::new(FilterOptions::of(&data.projects)),
      names: Arc::new(Names::of(data)),
      order: Arc::new(order),
      range_split: Arc::new(range_split),
      limits_reported: !data.snapshots.is_empty(),
      threshold: data.threshold,
      state,
      now,
    }
  }

  pub fn today(&self) -> NaiveDate {
    local_date(self.now)
  }

  /// The range is today alone.
  pub fn is_today(&self) -> bool {
    self.range.days() == 1 && self.range.first() == self.today()
  }

  /// The range ends with today, which counts up to now.
  pub fn has_today(&self) -> bool {
    self.range.last() == self.today()
  }

  /// The period the changes are measured against.
  pub fn previous(&self) -> DayRange {
    self.range.previous()
  }

  /// The one project whose Overview lists the period's failed runs (*Show failed tasks →*): the page's, or on
  /// every project's page the project all of them belong to; `None` without failed runs, or when they
  /// belong to several projects, whose failed tasks no one page lists.
  pub fn failed_project(&self) -> Option<ProjectId> {
    let mut projects = self
      .report
      .runs
      .iter()
      .filter(|run| run.outcome.failed())
      .map(|run| run.project);
    let project = projects.next()?;
    projects.all(|other| other == project).then_some(project)
  }

  /// The newest version of the pipeline called `pipeline` the period's runs pinned: a pipeline reads as
  /// *Feature delivery v3*.
  pub fn pipeline_version(&self, pipeline: &str) -> Option<VersionNumber> {
    self.names.pipeline_version(pipeline)
  }

  /// The pipeline version `run` pinned.
  pub fn run_pipeline_version(&self, run: RunId) -> Option<VersionNumber> {
    self.names.run_pipeline_version(run)
  }

  /// The pipeline version the latest run of `task` pinned.
  pub fn task_pipeline_version(&self, task: TaskId) -> Option<VersionNumber> {
    self.names.task_pipeline_version(task)
  }

  pub fn changes(&self) -> Changes {
    let (now, before) = (&self.report.summary, &self.report.previous);
    let count = |count: usize| u64::try_from(count).unwrap_or(u64::MAX);
    Changes {
      spent: Change::between(now.spent(), before.spent()),
      runs: Change::count(count(now.runs), count(before.runs)),
      agent_time: Change::between(now.agent_ms, before.agent_ms),
      cache_share: Change::points(now.cache_share(), before.cache_share()),
      failed_share: Change::points(now.failed_share(), before.failed_share()),
    }
  }
}
